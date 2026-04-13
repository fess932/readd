use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::{auth::Claims, epub, error::AppError, state::AppState};

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
struct TtsJobRow {
    id: i64,
    text_book_id: i64,
    status: String,
    total_chunks: i64,
    done_chunks: i64,
    failed_chunks: i64,
    audio_book_id: Option<i64>,
    error_msg: Option<String>,
    created_at: String,
    updated_at: String,
}

/// GET /api/tts-jobs — list all jobs (admin) or current user's jobs
pub async fn list(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query_as::<_, TtsJobRow>(
        "SELECT id, text_book_id, status, total_chunks, done_chunks, failed_chunks,
                audio_book_id, error_msg, created_at, updated_at
         FROM tts_jobs ORDER BY id DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows)))
}

/// POST /api/text-books/:id/tts — create and immediately start a TTS job
pub async fn create(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(text_book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    // Check there isn't already an active job for this book
    let existing = sqlx::query_scalar!(
        "SELECT id FROM tts_jobs WHERE text_book_id = ? AND status IN ('running', 'paused') LIMIT 1",
        text_book_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("Задача для этой книги уже существует".into()));
    }

    // Load epub path
    let book = sqlx::query!(
        "SELECT file_path FROM text_books WHERE id = ?",
        text_book_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let epub_path = state.uploads_dir.join(&book.file_path);

    // Parse epub in a blocking thread
    let chunks = tokio::task::spawn_blocking(move || epub::extract_chunks(&epub_path))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?
        .map_err(|e| AppError::BadRequest(format!("Не удалось разобрать epub: {e}")))?;

    if chunks.is_empty() {
        return Err(AppError::BadRequest("Текст в epub не найден".into()));
    }

    let total = chunks.len() as i64;
    let uploader = claims.id;

    // Insert job
    let job_id = sqlx::query!(
        "INSERT INTO tts_jobs (text_book_id, status, total_chunks) VALUES (?, 'running', ?) RETURNING id",
        text_book_id, total
    )
    .fetch_one(&state.pool)
    .await?
    .id
    .ok_or_else(|| AppError::Internal(anyhow::anyhow!("RETURNING id was null")))?;

    // Insert chunks in a single transaction
    let mut tx = state.pool.begin().await?;
    for (i, chunk) in chunks.iter().enumerate() {
        let seq = i as i64;
        let chapter_idx = chunk.epub_chapter_idx as i64;
        sqlx::query!(
            "INSERT INTO tts_chunks (job_id, seq, text, epub_chapter_idx) VALUES (?, ?, ?, ?)",
            job_id, seq, chunk.text, chapter_idx
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    // Wake up the worker
    state.tts_notify.notify_one();

    tracing::info!(
        "TTS job {} created: book={} chunks={} by user={}",
        job_id, text_book_id, total, uploader
    );

    Ok(Json(json!({
        "id": job_id,
        "status": "running",
        "totalChunks": total,
        "doneChunks": 0,
    })))
}

/// GET /api/text-books/:id/tts — get the latest job for a book
pub async fn get_for_book(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
    Path(text_book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query_as::<_, TtsJobRow>(
        "SELECT id, text_book_id, status, total_chunks, done_chunks, failed_chunks,
                audio_book_id, error_msg, created_at, updated_at
         FROM tts_jobs WHERE text_book_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(text_book_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(json!(row)))
}

/// POST /api/tts-jobs/:id/pause
pub async fn pause(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
    Path(job_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query!(
        "UPDATE tts_jobs SET status = 'paused', updated_at = datetime('now')
         WHERE id = ? AND status = 'running'",
        job_id
    )
    .execute(&state.pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

/// POST /api/tts-jobs/:id/resume
pub async fn resume(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
    Path(job_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query!(
        "UPDATE tts_jobs SET status = 'running', updated_at = datetime('now')
         WHERE id = ? AND status = 'paused'",
        job_id
    )
    .execute(&state.pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound);
    }

    state.tts_notify.notify_one();
    Ok(Json(json!({ "ok": true })))
}

/// DELETE /api/tts-jobs/:id — cancel job
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(job_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden);
    }

    let job = sqlx::query!("SELECT status FROM tts_jobs WHERE id = ?", job_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    if job.status == "done" {
        return Err(AppError::BadRequest("Нельзя отменить завершённую задачу".into()));
    }

    // Reset any in-flight chunks before deleting
    sqlx::query!(
        "UPDATE tts_chunks SET status = 'pending' WHERE job_id = ? AND status = 'processing'",
        job_id
    )
    .execute(&state.pool)
    .await?;

    sqlx::query!("DELETE FROM tts_chunks WHERE job_id = ?", job_id)
        .execute(&state.pool)
        .await?;

    sqlx::query!("DELETE FROM tts_jobs WHERE id = ?", job_id)
        .execute(&state.pool)
        .await?;

    // Remove audio files
    let dir = state.uploads_dir.join(format!("tts-{}", job_id));
    tokio::fs::remove_dir_all(&dir).await.ok();

    tracing::info!("TTS job {} cancelled", job_id);
    Ok(Json(json!({ "ok": true })))
}
