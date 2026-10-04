//! Turns a fully voiced job into an audiobook.

use std::sync::Arc;

use super::m4b::build_m4b;
use crate::state::AppState;

pub(super) struct DoneChunk {
    pub file_path: String,
    pub epub_chapter_idx: i64,
    pub duration_sec: Option<f64>,
}

async fn fail_job(state: &Arc<AppState>, job_id: i64, msg: &str) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE tts_jobs SET status = 'failed', error_msg = ?, updated_at = datetime('now') WHERE id = ?",
        msg, job_id
    )
    .execute(&state.pool)
    .await?;
    tracing::warn!("TTS job {} failed: {}", job_id, msg);
    Ok(())
}

pub(super) async fn finalize_job(state: &Arc<AppState>, job_id: i64) -> anyhow::Result<()> {
    let job = sqlx::query!(
        "SELECT j.audio_book_id, tb.title, tb.author, tb.uploaded_by_id
         FROM tts_jobs j JOIN text_books tb ON j.text_book_id = tb.id
         WHERE j.id = ?",
        job_id
    )
    .fetch_one(&state.pool)
    .await?;

    // The audiobook already exists: a previous run stopped right before marking the job
    if job.audio_book_id.is_some() {
        sqlx::query!(
            "UPDATE tts_jobs SET status = 'done', updated_at = datetime('now') WHERE id = ?",
            job_id
        )
        .execute(&state.pool)
        .await?;
        return Ok(());
    }

    let not_done = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM tts_chunks WHERE job_id = ? AND status != 'done'",
        job_id
    )
    .fetch_one(&state.pool)
    .await?;
    if not_done > 0 {
        // Nothing pending but not everything is voiced either
        return fail_job(
            state,
            job_id,
            &format!("Не озвучено фрагментов: {not_done}"),
        )
        .await;
    }

    let chunks: Vec<DoneChunk> = sqlx::query!(
        "SELECT file_path, epub_chapter_idx, duration_sec
         FROM tts_chunks WHERE job_id = ? AND status = 'done'
         ORDER BY seq",
        job_id
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .filter_map(|r| {
        Some(DoneChunk {
            file_path: r.file_path.filter(|p| !p.is_empty())?,
            epub_chapter_idx: r.epub_chapter_idx,
            duration_sec: r.duration_sec,
        })
    })
    .collect();

    if chunks.is_empty() {
        return fail_job(state, job_id, "Нет озвученных фрагментов").await;
    }

    // One M4B with chapter marks; if ffmpeg is unavailable, every chunk becomes a chapter
    let m4b = match build_m4b(state, job_id, &chunks, &job.title, &job.author).await {
        Ok(m4b) => Some(m4b),
        Err(e) => {
            tracing::warn!(
                "TTS job {}: ffmpeg failed ({}), falling back to individual files",
                job_id,
                e
            );
            None
        }
    };
    let chapter_files: Vec<(String, Option<f64>)> = match &m4b {
        Some((path, dur)) => vec![(path.clone(), *dur)],
        None => chunks
            .iter()
            .map(|c| (c.file_path.clone(), c.duration_sec))
            .collect(),
    };
    let book_file_path = chapter_files[0].0.clone();

    // Audiobook, chapters and job status change together
    let mut tx = state.pool.begin().await?;
    let book_id = sqlx::query!(
        "INSERT INTO books (title, author, file_path, uploaded_by_id) VALUES (?, ?, ?, ?) RETURNING id AS \"id!\"",
        job.title, job.author, book_file_path, job.uploaded_by_id
    )
    .fetch_one(&mut *tx)
    .await?
    .id;

    for (sort, (fp, dur)) in chapter_files.iter().enumerate() {
        let sort = sort as i64;
        sqlx::query!(
            "INSERT INTO chapters (book_id, file_path, sort_order, duration_sec) VALUES (?, ?, ?, ?)",
            book_id, fp, sort, dur
        )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
        "UPDATE tts_jobs SET status = 'done', audio_book_id = ?, error_msg = NULL,
                             updated_at = datetime('now')
         WHERE id = ?",
        book_id,
        job_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    // Only now are the chunk files redundant
    if m4b.is_some() {
        for c in &chunks {
            tokio::fs::remove_file(state.uploads_dir.join(&c.file_path))
                .await
                .ok();
        }
    }

    tracing::info!(
        "TTS job {} done: audiobook {} ({} chapter files)",
        job_id,
        book_id,
        chapter_files.len()
    );
    Ok(())
}
