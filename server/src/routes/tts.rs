use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;
use serde_json::{Value, json};
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

/// The job SELECT with `$tail` appended; a macro so that the SQL stays a string literal.
macro_rules! job_select {
    ($tail:literal) => {
        concat!(
            "SELECT id, text_book_id, status, total_chunks, done_chunks, failed_chunks,
                    audio_book_id, error_msg, created_at, updated_at
             FROM tts_jobs ",
            $tail
        )
    };
}

/// GET /api/tts-jobs — list all jobs
pub async fn list(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query_as::<_, TtsJobRow>(job_select!("ORDER BY id DESC"))
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
    // One job per book: a failed one is retried via resume, a finished one
    // goes away together with its audiobook
    let existing = sqlx::query_scalar!(
        "SELECT id FROM tts_jobs WHERE text_book_id = ? LIMIT 1",
        text_book_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict(
            "Задача для этой книги уже существует".into(),
        ));
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

    // Insert the job together with its chunks: the worker must never see a job without them
    let mut tx = state.pool.begin().await?;
    let job_id = sqlx::query!(
        "INSERT INTO tts_jobs (text_book_id, status, total_chunks) VALUES (?, 'running', ?) RETURNING id AS \"id!\"",
        text_book_id, total
    )
    .fetch_one(&mut *tx)
    .await?
    .id;

    for (i, chunk) in chunks.iter().enumerate() {
        let seq = i as i64;
        let chapter_idx = chunk.epub_chapter_idx as i64;
        sqlx::query!(
            "INSERT INTO tts_chunks (job_id, seq, text, epub_chapter_idx) VALUES (?, ?, ?, ?)",
            job_id,
            seq,
            chunk.text,
            chapter_idx
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    // Wake up the worker
    state.tts_notify.notify_one();

    tracing::info!(
        "TTS job {} created: book={} chunks={} by user={}",
        job_id,
        text_book_id,
        total,
        uploader
    );

    let row = sqlx::query_as::<_, TtsJobRow>(job_select!("WHERE id = ?"))
        .bind(job_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!(row)))
}

/// GET /api/text-books/:id/tts — get the latest job for a book
pub async fn get_for_book(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
    Path(text_book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query_as::<_, TtsJobRow>(job_select!(
        "WHERE text_book_id = ? ORDER BY id DESC LIMIT 1"
    ))
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

/// POST /api/tts-jobs/:id/resume — continue a paused job or retry a failed one
pub async fn resume(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
    Path(job_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let mut tx = state.pool.begin().await?;
    let rows = sqlx::query!(
        "UPDATE tts_jobs SET status = 'running', failed_chunks = 0, error_msg = NULL,
                             updated_at = datetime('now')
         WHERE id = ? AND status IN ('paused', 'failed')",
        job_id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound);
    }

    // Give chunks that ran out of attempts another go
    sqlx::query!(
        "UPDATE tts_chunks SET status = 'pending', attempts = 0
         WHERE job_id = ? AND status IN ('failed', 'processing')",
        job_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

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
        return Err(AppError::BadRequest(
            "Задача завершена — удалите аудиокнигу, чтобы озвучить заново".into(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query!("DELETE FROM tts_chunks WHERE job_id = ?", job_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM tts_jobs WHERE id = ?", job_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // Remove audio files
    let dir = state.uploads_dir.join(format!("tts-{}", job_id));
    tokio::fs::remove_dir_all(&dir).await.ok();

    tracing::info!("TTS job {} cancelled", job_id);
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;

    async fn chunk_statuses(state: &AppState, job_id: i64) -> Vec<String> {
        sqlx::query_scalar("SELECT status FROM tts_chunks WHERE job_id = ? ORDER BY seq")
            .bind(job_id)
            .fetch_all(&state.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn resume_requeues_failed_chunks_of_a_failed_job() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let book = testing::text_book(&t.state, &admin, "Книга").await;
        let job = testing::tts_job(
            &t.state,
            book,
            "failed",
            None,
            &["done", "failed", "pending"],
        )
        .await;

        resume(State(t.state.clone()), admin, Path(job))
            .await
            .unwrap();

        assert_eq!(
            chunk_statuses(&t.state, job).await,
            ["done", "pending", "pending"]
        );
        let Json(jobs) = list(
            State(t.state.clone()),
            testing::user(&t.state, "u", false).await,
        )
        .await
        .unwrap();
        assert_eq!(jobs[0]["status"], "running");
        assert_eq!(jobs[0]["textBookId"], book);
        assert_eq!(jobs[0]["errorMsg"], Value::Null);
    }

    #[tokio::test]
    async fn pause_and_resume_only_apply_in_the_right_state() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let book = testing::text_book(&t.state, &admin, "Книга").await;
        let job = testing::tts_job(&t.state, book, "running", None, &["pending"]).await;

        assert!(matches!(
            resume(State(t.state.clone()), admin.clone(), Path(job)).await,
            Err(AppError::NotFound)
        ));
        pause(State(t.state.clone()), admin.clone(), Path(job))
            .await
            .unwrap();
        assert!(matches!(
            pause(State(t.state.clone()), admin.clone(), Path(job)).await,
            Err(AppError::NotFound)
        ));
        resume(State(t.state.clone()), admin, Path(job))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn cancel_removes_the_job_but_refuses_a_finished_one() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let book = testing::text_book(&t.state, &admin, "Книга").await;
        let audio = testing::book(&t.state, &admin, "Озвученная", &["book.m4b"]).await;
        let done = testing::tts_job(&t.state, book, "done", Some(audio), &["done"]).await;
        let paused = testing::tts_job(&t.state, book, "paused", None, &["done", "pending"]).await;

        let refused = cancel(State(t.state.clone()), admin.clone(), Path(done)).await;
        assert!(matches!(refused, Err(AppError::BadRequest(_))));

        cancel(State(t.state.clone()), admin, Path(paused))
            .await
            .unwrap();
        assert_eq!(testing::count(&t.state, "tts_jobs").await, 1);
        assert_eq!(chunk_statuses(&t.state, paused).await.len(), 0);
    }

    #[tokio::test]
    async fn create_refuses_a_second_job_for_the_same_book() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let book = testing::text_book(&t.state, &admin, "Книга").await;
        testing::tts_job(&t.state, book, "failed", None, &["failed"]).await;

        let res = create(State(t.state.clone()), admin, Path(book)).await;
        assert!(matches!(res, Err(AppError::Conflict(_))));
    }
}
