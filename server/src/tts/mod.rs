//! Background worker that turns queued text chunks into audio, one at a time.

use std::sync::Arc;
use tokio::time::{Duration, sleep};

use crate::state::AppState;

mod finalize;
mod m4b;
mod synth;

use finalize::finalize_job;
use synth::{TtsError, call_tts};

/// How many times the TTS server may reject one chunk before the job is failed.
const MAX_CHUNK_ATTEMPTS: i64 = 3;

enum PollResult {
    /// Made progress; poll again right away.
    Done,
    /// No pending work right now.
    NoWork,
    /// TTS server unreachable — chunk was put back to pending.
    ServerDown,
}

pub async fn run(state: Arc<AppState>) {
    tracing::info!("TTS worker started");

    // Reset any chunks left in 'processing' state from a previous crash
    if let Err(e) =
        sqlx::query!("UPDATE tts_chunks SET status = 'pending' WHERE status = 'processing'")
            .execute(&state.pool)
            .await
    {
        tracing::error!("TTS worker: cannot reset in-flight chunks: {e:#}");
    }

    // A failed chunk now fails its job, so an active job must not have any;
    // such rows are leftovers that would silently become holes in the audiobook.
    if let Err(e) = sqlx::query!(
        "UPDATE tts_chunks SET status = 'pending', attempts = 0
         WHERE status = 'failed'
           AND job_id IN (SELECT id FROM tts_jobs WHERE status IN ('running', 'paused'))"
    )
    .execute(&state.pool)
    .await
    {
        tracing::error!("TTS worker: cannot requeue failed chunks: {e:#}");
    }

    let mut backoff_secs: u64 = 1;

    loop {
        match process_next(&state).await {
            Ok(PollResult::Done) => {
                backoff_secs = 1;
                sleep(Duration::from_millis(50)).await;
            }
            Ok(PollResult::NoWork) => {
                backoff_secs = 1;
                tokio::select! {
                    _ = state.tts_notify.notified() => {
                        tracing::debug!("TTS worker woken up");
                    }
                    _ = sleep(Duration::from_secs(10)) => {}
                }
            }
            Ok(PollResult::ServerDown) => {
                tracing::warn!("TTS server unreachable, retrying in {}s", backoff_secs);
                // A pause, resume or cancel should not have to wait out the backoff
                tokio::select! {
                    _ = state.tts_notify.notified() => {}
                    _ = sleep(Duration::from_secs(backoff_secs)) => {}
                }
                backoff_secs = (backoff_secs * 2).min(60);
            }
            Err(e) => {
                tracing::error!("TTS worker error: {e:#}");
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn process_next(state: &Arc<AppState>) -> anyhow::Result<PollResult> {
    // Find a running job
    let job = sqlx::query!(
        "SELECT id AS \"id!\", audio_ext FROM tts_jobs WHERE status = 'running' ORDER BY id LIMIT 1"
    )
    .fetch_optional(&state.pool)
    .await?;

    let job = match job {
        None => return Ok(PollResult::NoWork),
        Some(j) => j,
    };

    // Next pending chunk (there is a single worker, so no one else can take it)
    let chunk = sqlx::query!(
        "SELECT id AS \"id!\", seq, text, attempts FROM tts_chunks
         WHERE job_id = ? AND status = 'pending'
         ORDER BY seq LIMIT 1",
        job.id
    )
    .fetch_optional(&state.pool)
    .await?;

    let chunk = match chunk {
        None => {
            finalize_job(state, job.id).await?;
            return Ok(PollResult::Done);
        }
        Some(c) => c,
    };

    // Mark as processing
    sqlx::query!(
        "UPDATE tts_chunks SET status = 'processing' WHERE id = ?",
        chunk.id
    )
    .execute(&state.pool)
    .await?;

    let result = process_chunk(
        state,
        job.id,
        &job.audio_ext,
        chunk.id,
        chunk.seq,
        &chunk.text,
        chunk.attempts,
    )
    .await;
    if result.is_err() {
        // Disk or database trouble: do not leave the chunk stranded in 'processing'
        sqlx::query!(
            "UPDATE tts_chunks SET status = 'pending' WHERE id = ? AND status = 'processing'",
            chunk.id
        )
        .execute(&state.pool)
        .await
        .ok();
    }
    result
}

async fn process_chunk(
    state: &Arc<AppState>,
    job_id: i64,
    job_audio_ext: &str,
    chunk_id: i64,
    seq: i64,
    text: &str,
    attempts: i64,
) -> anyhow::Result<PollResult> {
    // Re-check job status (might have been paused between the SELECT above and now)
    let status = sqlx::query_scalar!("SELECT status FROM tts_jobs WHERE id = ?", job_id)
        .fetch_optional(&state.pool)
        .await?;

    if status.as_deref() != Some("running") {
        // Put the chunk back
        sqlx::query!(
            "UPDATE tts_chunks SET status = 'pending' WHERE id = ?",
            chunk_id
        )
        .execute(&state.pool)
        .await?;
        return Ok(PollResult::Done);
    }

    // Call TTS server
    let url = format!("{}/synthesize", state.tts_server_url);
    match call_tts(state, &url, text).await {
        Ok((audio_bytes, ext)) => {
            let audio_ext = if job_audio_ext == "mp3" {
                ext
            } else {
                job_audio_ext.to_string()
            };
            let (file_path, duration_sec) =
                save_audio(state, job_id, seq, &audio_bytes, &audio_ext).await?;

            let updated = sqlx::query!(
                "UPDATE tts_chunks SET status = 'done', file_path = ?, duration_sec = ? WHERE id = ?",
                file_path, duration_sec, chunk_id
            )
            .execute(&state.pool)
            .await?
            .rows_affected();

            if updated == 0 {
                // The job was cancelled while the request was in flight
                let dir = state.uploads_dir.join(format!("tts-{}", job_id));
                tokio::fs::remove_dir_all(&dir).await.ok();
                return Ok(PollResult::Done);
            }

            sqlx::query!(
                "UPDATE tts_jobs SET done_chunks = done_chunks + 1,
                                     audio_ext = ?,
                                     error_msg = NULL,
                                     updated_at = datetime('now')
                 WHERE id = ?",
                audio_ext,
                job_id
            )
            .execute(&state.pool)
            .await?;

            Ok(PollResult::Done)
        }
        Err(TtsError::Unavailable(msg)) => {
            tracing::warn!("TTS chunk seq={} job={}: {}", seq, job_id, msg);

            sqlx::query!(
                "UPDATE tts_chunks SET status = 'pending' WHERE id = ?",
                chunk_id
            )
            .execute(&state.pool)
            .await?;
            // Shown in the UI while the job waits for the server to come back
            sqlx::query!(
                "UPDATE tts_jobs SET error_msg = ?, updated_at = datetime('now') WHERE id = ?",
                msg,
                job_id
            )
            .execute(&state.pool)
            .await?;

            Ok(PollResult::ServerDown)
        }
        Err(TtsError::Rejected(msg)) => {
            let attempts = attempts + 1;
            tracing::warn!(
                "TTS chunk seq={} job={} rejected (attempt {}/{}): {}",
                seq,
                job_id,
                attempts,
                MAX_CHUNK_ATTEMPTS,
                msg
            );

            if attempts < MAX_CHUNK_ATTEMPTS {
                sqlx::query!(
                    "UPDATE tts_chunks SET status = 'pending', attempts = ?, error_msg = ? WHERE id = ?",
                    attempts, msg, chunk_id
                )
                .execute(&state.pool)
                .await?;
                return Ok(PollResult::Done);
            }

            // Out of attempts: stop the job instead of producing a book with a hole in it.
            // "Retry" in the UI resumes it from this chunk.
            let mut tx = state.pool.begin().await?;
            sqlx::query!(
                "UPDATE tts_chunks SET status = 'failed', attempts = ?, error_msg = ? WHERE id = ?",
                attempts,
                msg,
                chunk_id
            )
            .execute(&mut *tx)
            .await?;
            let job_msg = format!("Фрагмент {}: {}", seq + 1, msg);
            sqlx::query!(
                "UPDATE tts_jobs SET status = 'failed', failed_chunks = 1, error_msg = ?,
                                     updated_at = datetime('now')
                 WHERE id = ?",
                job_msg,
                job_id
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;

            Ok(PollResult::Done)
        }
    }
}

async fn save_audio(
    state: &Arc<AppState>,
    job_id: i64,
    seq: i64,
    bytes: &[u8],
    ext: &str,
) -> anyhow::Result<(String, Option<f64>)> {
    let dir_name = format!("tts-{}", job_id);
    let dir = state.uploads_dir.join(&dir_name);
    tokio::fs::create_dir_all(&dir).await?;

    let filename = format!("{:05}.{}", seq, ext);
    let rel_path = format!("{}/{}", dir_name, filename);
    let full_path = state.uploads_dir.join(&rel_path);
    tokio::fs::write(&full_path, bytes).await?;

    // Read duration via symphonia in a blocking thread
    let dur = tokio::task::spawn_blocking(move || crate::media::audio_duration(&full_path))
        .await
        .ok()
        .flatten();

    Ok((rel_path, dur))
}
