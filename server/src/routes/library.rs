use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::{auth::Claims, error::AppError, state::AppState};

// Used for the two complex library SELECT queries (query_as skips compile-time checks)
#[derive(sqlx::FromRow)]
struct LibBookRow {
    id: i64,
    title: String,
    author: String,
    narrator: Option<String>,
    cover_path: Option<String>,
    file_path: String,
    uploaded_by: Option<String>,
    added_at: String,
    finished_at: Option<String>,
    created_at: String,
    chapters_count: i64,
    total_sec: Option<f64>,
}

const LIB_SELECT: &str =
    "SELECT b.id, b.title, b.author, b.narrator, b.cover_path, b.file_path,
            u.name AS uploaded_by, ul.added_at, ul.finished_at, b.created_at,
            (SELECT COUNT(*) FROM chapters WHERE book_id = b.id) AS chapters_count,
            (SELECT SUM(duration_sec) FROM chapters WHERE book_id = b.id) AS total_sec
     FROM user_library ul
     JOIN books b ON ul.book_id = b.id
     LEFT JOIN users u ON b.uploaded_by_id = u.id";

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChapterRow {
    id: i64,
    file_path: String,
    sort_order: i64,
    duration_sec: Option<f64>,
}

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressRow {
    book_id: i64,
    chapter_path: String,
    position_sec: f64,
}

async fn book_chapters(
    pool: &sqlx::SqlitePool,
    book_id: i64,
) -> Result<Vec<ChapterRow>, sqlx::Error> {
    sqlx::query_as::<_, ChapterRow>(
        "SELECT id, file_path, sort_order, duration_sec FROM chapters
         WHERE book_id = ? ORDER BY sort_order ASC",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await
}

async fn book_progress(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    book_id: i64,
) -> Result<Option<ProgressRow>, sqlx::Error> {
    sqlx::query_as::<_, ProgressRow>(
        "SELECT book_id, chapter_path, position_sec FROM progress
         WHERE user_id = ? AND book_id = ?
         ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(book_id)
    .fetch_optional(pool)
    .await
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Value>, AppError> {
    let books = sqlx::query_as::<_, LibBookRow>(
        &format!("{} WHERE ul.user_id = ? ORDER BY ul.added_at DESC", LIB_SELECT),
    )
    .bind(claims.id)
    .fetch_all(&state.pool)
    .await?;

    let mut result = Vec::with_capacity(books.len());
    for b in books {
        let chapters = book_chapters(&state.pool, b.id).await?;
        let progress = book_progress(&state.pool, claims.id, b.id).await?;
        result.push(json!({
            "id": b.id,
            "title": b.title,
            "author": b.author,
            "narrator": b.narrator,
            "coverPath": b.cover_path,
            "filePath": b.file_path,
            "uploadedBy": b.uploaded_by,
            "createdAt": b.created_at,
            "addedAt": b.added_at,
            "finishedAt": b.finished_at,
            "chaptersCount": b.chapters_count,
            "totalSec": b.total_sec,
            "chapters": chapters,
            "progress": progress,
        }));
    }
    Ok(Json(json!(result)))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let b = sqlx::query_as::<_, LibBookRow>(
        &format!(
            "{} WHERE ul.user_id = ? AND b.id = ?",
            LIB_SELECT
        ),
    )
    .bind(claims.id)
    .bind(book_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let chapters = book_chapters(&state.pool, b.id).await?;
    let progress = book_progress(&state.pool, claims.id, b.id).await?;

    Ok(Json(json!({
        "id": b.id,
        "title": b.title,
        "author": b.author,
        "narrator": b.narrator,
        "coverPath": b.cover_path,
        "filePath": b.file_path,
        "uploadedBy": b.uploaded_by,
        "createdAt": b.created_at,
        "addedAt": b.added_at,
        "finishedAt": b.finished_at,
        "chaptersCount": b.chapters_count,
        "totalSec": b.total_sec,
        "chapters": chapters,
        "progress": progress,
    })))
}

pub async fn add(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let exists = sqlx::query!("SELECT id FROM books WHERE id = ?", book_id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query!(
        "INSERT OR IGNORE INTO user_library (user_id, book_id) VALUES (?, ?)",
        claims.id,
        book_id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(json!({ "ok": true })))
}

pub async fn finish(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    // Toggle: if already finished → clear, else set now
    let current = sqlx::query_scalar!(
        "SELECT finished_at FROM user_library WHERE user_id = ? AND book_id = ?",
        claims.id, book_id
    )
    .fetch_optional(&state.pool)
    .await?;

    match current {
        None => return Err(AppError::NotFound),
        Some(Some(_)) => {
            sqlx::query!(
                "UPDATE user_library SET finished_at = NULL WHERE user_id = ? AND book_id = ?",
                claims.id, book_id
            )
            .execute(&state.pool)
            .await?;
        }
        Some(None) => {
            sqlx::query!(
                "UPDATE user_library SET finished_at = datetime('now') WHERE user_id = ? AND book_id = ?",
                claims.id, book_id
            )
            .execute(&state.pool)
            .await?;
        }
    }

    Ok(Json(json!({ "ok": true })))
}

pub async fn remove(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    sqlx::query!(
        "DELETE FROM user_library WHERE user_id = ? AND book_id = ?",
        claims.id,
        book_id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(json!({ "ok": true })))
}
