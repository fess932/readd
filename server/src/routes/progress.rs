use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::{auth::Claims, error::AppError, state::AppState};

pub async fn last(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query!(
        "SELECT p.book_id, p.chapter_path, p.position_sec,
                b.title, b.author, b.cover_path, p.updated_at
         FROM progress p
         JOIN books b ON p.book_id = b.id
         WHERE p.user_id = ?
         ORDER BY p.updated_at DESC
         LIMIT 1",
        claims.id
    )
    .fetch_optional(&state.pool)
    .await?;

    match row {
        None => Ok(Json(Value::Null)),
        Some(r) => Ok(Json(json!({
            "bookId": r.book_id,
            "chapterPath": r.chapter_path,
            "positionSec": r.position_sec,
            "title": r.title,
            "author": r.author,
            "coverPath": r.cover_path,
            "updatedAt": r.updated_at,
        }))),
    }
}

pub async fn get_book(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query!(
        "SELECT chapter_path, position_sec FROM progress
         WHERE user_id = ? AND book_id = ?",
        claims.id, book_id
    )
    .fetch_all(&state.pool)
    .await?;

    let result: Vec<Value> = rows
        .into_iter()
        .map(|r| json!({ "chapterPath": r.chapter_path, "positionSec": r.position_sec }))
        .collect();

    Ok(Json(json!(result)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBody {
    pub chapter_path: String,
    pub position_sec: f64,
    pub chapter_duration: Option<f64>,
}

pub async fn save(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
    Json(body): Json<SaveBody>,
) -> Result<Json<Value>, AppError> {
    sqlx::query!(
        "INSERT INTO progress (user_id, book_id, chapter_path, position_sec, updated_at)
         VALUES (?, ?, ?, ?, datetime('now'))
         ON CONFLICT(user_id, book_id, chapter_path) DO UPDATE SET
           position_sec = excluded.position_sec,
           updated_at   = datetime('now')",
        claims.id, book_id, body.chapter_path, body.position_sec
    )
    .execute(&state.pool)
    .await?;

    // Update chapter duration if the player reported it
    if let Some(dur) = body.chapter_duration.filter(|&d| d > 0.0) {
        sqlx::query!(
            "UPDATE chapters SET duration_sec = ? WHERE file_path = ?",
            dur, body.chapter_path
        )
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(json!({ "ok": true })))
}
