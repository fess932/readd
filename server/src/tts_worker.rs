use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::state::AppState;

struct DoneChunk {
    file_path: Option<String>,
    epub_chapter_idx: i64,
    duration_sec: Option<f64>,
}

enum PollResult {
    /// Chunk processed successfully.
    Done,
    /// No pending work right now.
    NoWork,
    /// TTS server unreachable — chunk was put back to pending.
    ServerDown,
}

pub async fn run(state: Arc<AppState>) {
    tracing::info!("TTS worker started");

    // Reset any chunks left in 'processing' state from a previous crash
    sqlx::query!("UPDATE tts_chunks SET status = 'pending' WHERE status = 'processing'")
        .execute(&state.pool)
        .await
        .ok();

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
                sleep(Duration::from_secs(backoff_secs)).await;
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
        "SELECT id, audio_ext FROM tts_jobs WHERE status = 'running' ORDER BY id LIMIT 1"
    )
    .fetch_optional(&state.pool)
    .await?;

    let job = match job {
        None => return Ok(PollResult::NoWork),
        Some(j) => j,
    };

    // Grab the next pending chunk atomically
    let chunk = sqlx::query!(
        "SELECT id, seq, text FROM tts_chunks
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

    // Re-check job status (might have been paused between the SELECT above and now)
    let status = sqlx::query_scalar!("SELECT status FROM tts_jobs WHERE id = ?", job.id)
        .fetch_one(&state.pool)
        .await?;

    if status != "running" {
        // Put the chunk back
        sqlx::query!(
            "UPDATE tts_chunks SET status = 'pending' WHERE id = ?",
            chunk.id
        )
        .execute(&state.pool)
        .await?;
        return Ok(PollResult::NoWork);
    }

    // Call TTS server
    let url = format!("{}/synthesize", state.tts_server_url);
    let result: anyhow::Result<(Vec<u8>, String)> = call_tts(state, &url, &chunk.text).await;

    match result {
        Ok((audio_bytes, ext)) => {
            let audio_ext = if job.audio_ext == "mp3" { ext } else { job.audio_ext.clone() };
            let (file_path, duration_sec) =
                save_audio(state, job.id, chunk.seq, &audio_bytes, &audio_ext).await?;

            sqlx::query!(
                "UPDATE tts_chunks SET status = 'done', file_path = ?, duration_sec = ? WHERE id = ?",
                file_path, duration_sec, chunk.id
            )
            .execute(&state.pool)
            .await?;

            sqlx::query!(
                "UPDATE tts_jobs SET done_chunks = done_chunks + 1,
                                     audio_ext = ?,
                                     updated_at = datetime('now')
                 WHERE id = ?",
                audio_ext, job.id
            )
            .execute(&state.pool)
            .await?;
        }
        Err(e) => {
            let msg = e.to_string();
            tracing::warn!("TTS chunk seq={} job={} error (will retry): {}", chunk.seq, job.id, msg);

            // Keep error_msg for visibility but reset to pending — backoff will retry
            sqlx::query!(
                "UPDATE tts_chunks SET status = 'pending', error_msg = ? WHERE id = ?",
                msg, chunk.id
            )
            .execute(&state.pool)
            .await?;

            sqlx::query!(
                "UPDATE tts_jobs SET failed_chunks = failed_chunks + 1,
                                     updated_at = datetime('now')
                 WHERE id = ?",
                job.id
            )
            .execute(&state.pool)
            .await?;

            return Ok(PollResult::ServerDown);
        }
    }

    Ok(PollResult::Done)
}

async fn call_tts(
    state: &Arc<AppState>,
    url: &str,
    text: &str,
) -> anyhow::Result<(Vec<u8>, String)> {
    let resp = state
        .http_client
        .post(url)
        .json(&serde_json::json!({ "text": text }))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("TTS server unreachable: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("TTS server {}: {}", status, body);
    }

    let ext = content_type_to_ext(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("audio/mpeg"),
    );

    let bytes = resp.bytes().await?.to_vec();
    Ok((bytes, ext.to_string()))
}

fn content_type_to_ext(ct: &str) -> &'static str {
    if ct.contains("ogg") || ct.contains("vorbis") {
        "ogg"
    } else if ct.contains("wav") {
        "wav"
    } else if ct.contains("aac") {
        "aac"
    } else if ct.contains("opus") {
        "opus"
    } else {
        "mp3"
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
    let dur = tokio::task::spawn_blocking(move || crate::routes::books::get_audio_duration(&full_path))
        .await
        .ok()
        .flatten();

    Ok((rel_path, dur))
}

async fn finalize_job(state: &Arc<AppState>, job_id: i64) -> anyhow::Result<()> {
    // Make sure no chunks are still pending/processing
    let pending = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM tts_chunks WHERE job_id = ? AND status IN ('pending', 'processing')",
        job_id
    )
    .fetch_one(&state.pool)
    .await?;

    if pending > 0 {
        return Ok(());
    }

    let job = sqlx::query!(
        "SELECT j.done_chunks, j.failed_chunks, tb.title, tb.author, tb.uploaded_by_id
         FROM tts_jobs j JOIN text_books tb ON j.text_book_id = tb.id
         WHERE j.id = ?",
        job_id
    )
    .fetch_one(&state.pool)
    .await?;

    if job.done_chunks == 0 {
        sqlx::query!(
            "UPDATE tts_jobs SET status = 'failed',
                                  error_msg = 'Все фрагменты завершились с ошибкой',
                                  updated_at = datetime('now')
             WHERE id = ?",
            job_id
        )
        .execute(&state.pool)
        .await?;
        tracing::warn!("TTS job {} failed: no successful chunks", job_id);
        return Ok(());
    }

    // Collect done chunks grouped by epub chapter
    let chunks: Vec<DoneChunk> = sqlx::query!(
        "SELECT file_path, epub_chapter_idx, duration_sec
         FROM tts_chunks WHERE job_id = ? AND status = 'done'
         ORDER BY seq",
        job_id
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| DoneChunk {
        file_path: r.file_path,
        epub_chapter_idx: r.epub_chapter_idx,
        duration_sec: r.duration_sec,
    })
    .collect();

    // Group chunk paths by epub chapter index
    let mut chapters: Vec<(i64, Vec<String>)> = Vec::new(); // (epub_chapter_idx, [file_paths])
    for c in &chunks {
        let fp = match c.file_path.as_deref() {
            Some(fp) if !fp.is_empty() => fp.to_string(),
            _ => continue,
        };
        match chapters.last_mut() {
            Some((idx, paths)) if *idx == c.epub_chapter_idx => paths.push(fp),
            _ => chapters.push((c.epub_chapter_idx, vec![fp])),
        }
    }

    // Try to build M4B chapters via ffmpeg
    let (book_file_path, chapter_paths) =
        match build_m4b_chapters(state, job_id, &chapters, &chunks, &job.title, &job.author).await {
            Ok(result) => {
                tracing::info!("TTS job {}: M4B built ({} chapters)", job_id, result.len());
                // Delete individual chunk files to save space
                for c in &chunks {
                    if let Some(fp) = &c.file_path {
                        tokio::fs::remove_file(state.uploads_dir.join(fp)).await.ok();
                    }
                }
                let first = result.first().cloned().unwrap_or_default();
                (first, result)
            }
            Err(e) => {
                tracing::warn!("TTS job {}: ffmpeg failed ({}), falling back to individual files", job_id, e);
                let all: Vec<String> = chunks.iter().filter_map(|c| c.file_path.clone()).collect();
                let first = all.first().cloned().unwrap_or_default();
                (first, all)
            }
        };

    // Create audiobook record
    let book_id = sqlx::query!(
        "INSERT INTO books (title, author, file_path, uploaded_by_id) VALUES (?, ?, ?, ?) RETURNING id",
        job.title, job.author, book_file_path, job.uploaded_by_id
    )
    .fetch_one(&state.pool)
    .await?
    .id;

    for (sort, fp) in chapter_paths.iter().enumerate() {
        let sort = sort as i64;
        sqlx::query!(
            "INSERT INTO chapters (book_id, file_path, sort_order) VALUES (?, ?, ?)",
            book_id, fp, sort
        )
        .execute(&state.pool)
        .await?;
    }

    sqlx::query!(
        "UPDATE tts_jobs SET status = 'done', audio_book_id = ?, updated_at = datetime('now') WHERE id = ?",
        book_id, job_id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "TTS job {} done: audiobook {} ({} chapters, {} failed chunks)",
        job_id, book_id, chapter_paths.len(), job.failed_chunks
    );
    Ok(())
}

/// Concatenates all chunks into a single M4B with embedded epub-chapter markers.
/// Returns a single-element vec with the M4B relative path (or errors for ffmpeg fallback).
async fn build_m4b_chapters(
    state: &Arc<AppState>,
    job_id: i64,
    chapters: &[(i64, Vec<String>)],
    all_chunks: &[DoneChunk],
    title: &str,
    author: &str,
) -> anyhow::Result<Vec<String>> {
    let dir_name = format!("tts-{}", job_id);
    let work_dir = state.uploads_dir.join(&dir_name);

    // Build per-chapter cumulative timestamps for ffmetadata
    // We compute the running total of chunk durations across all done chunks
    let chunk_start_ms: Vec<u64> = {
        let mut ms = 0u64;
        let mut starts = Vec::with_capacity(all_chunks.len());
        for c in all_chunks {
            starts.push(ms);
            let dur_ms = c.duration_sec.unwrap_or(0.0) * 1000.0;
            ms += dur_ms as u64;
        }
        starts
    };
    let total_ms = chunk_start_ms.last().copied().unwrap_or(0)
        + all_chunks.last().and_then(|c| c.duration_sec).unwrap_or(0.0) as u64 * 1000;

    // Chapter boundaries: start of first chunk per epub chapter
    let mut chapter_marks: Vec<(u64, String)> = Vec::new(); // (start_ms, title)
    let mut chunk_cursor = 0usize;
    for (epub_idx, chunk_paths) in chapters {
        // Find the first matching chunk in all_chunks to get its start time
        while chunk_cursor < all_chunks.len() {
            if all_chunks[chunk_cursor].epub_chapter_idx == *epub_idx {
                break;
            }
            chunk_cursor += 1;
        }
        let start_ms = chunk_start_ms.get(chunk_cursor).copied().unwrap_or(0);
        chapter_marks.push((start_ms, format!("Глава {}", epub_idx + 1)));
        chunk_cursor += chunk_paths.len();
    }

    // Write concat file list (all chunks together for the whole book)
    let all_paths: Vec<&str> = all_chunks.iter()
        .filter_map(|c| c.file_path.as_deref())
        .filter(|p| !p.is_empty())
        .collect();

    let list_content: String = all_paths.iter()
        .map(|rel| format!("file '{}'\n", state.uploads_dir.join(rel).display()))
        .collect();
    let list_path = work_dir.join("_filelist.txt");
    tokio::fs::write(&list_path, &list_content).await?;

    // Write ffmetadata with chapter marks
    let mut meta = String::from(";FFMETADATA1\n");
    meta.push_str(&format!("title={}\n", title));
    meta.push_str(&format!("artist={}\n", author));
    meta.push_str("genre=Audiobook\n\n");

    for (i, (start_ms, ch_title)) in chapter_marks.iter().enumerate() {
        let end_ms = chapter_marks.get(i + 1).map(|(s, _)| *s).unwrap_or(total_ms);
        meta.push_str("[CHAPTER]\nTIMEBASE=1/1000\n");
        meta.push_str(&format!("START={}\n", start_ms));
        meta.push_str(&format!("END={}\n", end_ms));
        meta.push_str(&format!("title={}\n\n", ch_title));
    }
    let meta_path = work_dir.join("_metadata.txt");
    tokio::fs::write(&meta_path, &meta).await?;

    // Single M4B for the whole book
    let m4b_rel = format!("{}/book.m4b", dir_name);
    let m4b_full = state.uploads_dir.join(&m4b_rel);

    let output = tokio::process::Command::new("ffmpeg")
        .args([
            "-f", "concat", "-safe", "0",
            "-i", list_path.to_str().unwrap_or(""),
            "-i", meta_path.to_str().unwrap_or(""),
            "-map_metadata", "1",
            "-map", "0:a",
            "-c:a", "aac",
            "-profile:a", "aac_low",
            "-b:a", "128k",
            "-movflags", "+faststart",
            "-y",
            m4b_full.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .await?;

    // Cleanup temp files
    tokio::fs::remove_file(&list_path).await.ok();
    tokio::fs::remove_file(&meta_path).await.ok();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffmpeg: {}", &stderr[..stderr.len().min(300)]);
    }

    // Scan duration of the resulting M4B for the player
    let m4b_path_clone = m4b_full.clone();
    let dur = tokio::task::spawn_blocking(move || {
        crate::routes::books::get_audio_duration(&m4b_path_clone)
    })
    .await
    .ok()
    .flatten();

    tracing::info!("M4B created: {} ({:.0?} min)", m4b_rel, dur.map(|d| d / 60.0));
    Ok(vec![m4b_rel])
}
