use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::{auth::Claims, error::AppError, state::AppState};

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
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

/// The library SELECT with `$tail` appended; a macro so that the SQL stays a string literal.
macro_rules! lib_select {
    ($tail:literal) => {
        concat!(
            "SELECT b.id, b.title, b.author, b.narrator, b.cover_path, b.file_path,
                    u.name AS uploaded_by, ul.added_at, ul.finished_at, b.created_at,
                    (SELECT COUNT(*) FROM chapters WHERE book_id = b.id) AS chapters_count,
                    (SELECT SUM(duration_sec) FROM chapters WHERE book_id = b.id) AS total_sec
             FROM user_library ul
             JOIN books b ON ul.book_id = b.id
             LEFT JOIN users u ON b.uploaded_by_id = u.id ",
            $tail
        )
    };
}

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

/// A library entry as the client sees it: the book, its chapters and where the user stopped.
#[derive(Serialize)]
struct LibraryBook {
    #[serde(flatten)]
    book: LibBookRow,
    chapters: Vec<ChapterRow>,
    progress: Option<ProgressRow>,
}

async fn with_details(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    book: LibBookRow,
) -> Result<LibraryBook, sqlx::Error> {
    let chapters = sqlx::query_as::<_, ChapterRow>(
        "SELECT id, file_path, sort_order, duration_sec FROM chapters
         WHERE book_id = ? ORDER BY sort_order ASC",
    )
    .bind(book.id)
    .fetch_all(pool)
    .await?;

    // The chapter listened to most recently
    let progress = sqlx::query_as::<_, ProgressRow>(
        "SELECT book_id, chapter_path, position_sec FROM progress
         WHERE user_id = ? AND book_id = ?
         ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(book.id)
    .fetch_optional(pool)
    .await?;

    Ok(LibraryBook {
        book,
        chapters,
        progress,
    })
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Value>, AppError> {
    let books = sqlx::query_as::<_, LibBookRow>(lib_select!(
        "WHERE ul.user_id = ? ORDER BY ul.added_at DESC"
    ))
    .bind(claims.id)
    .fetch_all(&state.pool)
    .await?;

    let mut result = Vec::with_capacity(books.len());
    for book in books {
        result.push(with_details(&state.pool, claims.id, book).await?);
    }
    Ok(Json(json!(result)))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let book = sqlx::query_as::<_, LibBookRow>(lib_select!("WHERE ul.user_id = ? AND b.id = ?"))
        .bind(claims.id)
        .bind(book_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(json!(
        with_details(&state.pool, claims.id, book).await?
    )))
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

#[derive(Deserialize)]
pub struct FinishBody {
    pub finished: bool,
}

pub async fn finish(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
    Json(body): Json<FinishBody>,
) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query!(
        "UPDATE user_library
         SET finished_at = CASE WHEN ? THEN COALESCE(finished_at, datetime('now')) ELSE NULL END
         WHERE user_id = ? AND book_id = ?",
        body.finished,
        claims.id,
        book_id
    )
    .execute(&state.pool)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;

    async fn finished_at(state: &AppState, book_id: i64) -> Option<String> {
        sqlx::query_scalar("SELECT finished_at FROM user_library WHERE book_id = ?")
            .bind(book_id)
            .fetch_one(&state.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn finish_is_idempotent_and_can_be_undone() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = testing::book(&t.state, &user, "Книга", &["1.mp3"]).await;
        testing::add_to_library(&t.state, &user, id).await;
        let set = |finished| {
            finish(
                State(t.state.clone()),
                user.clone(),
                Path(id),
                Json(FinishBody { finished }),
            )
        };

        set(true).await.unwrap();
        let first = finished_at(&t.state, id).await;
        assert!(first.is_some());

        // the player reports the end of the book again: still finished, same timestamp
        set(true).await.unwrap();
        assert_eq!(finished_at(&t.state, id).await, first);

        set(false).await.unwrap();
        assert_eq!(finished_at(&t.state, id).await, None);
    }

    #[tokio::test]
    async fn finish_needs_the_book_in_the_library() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = testing::book(&t.state, &user, "Книга", &["1.mp3"]).await;

        let res = finish(
            State(t.state.clone()),
            user,
            Path(id),
            Json(FinishBody { finished: true }),
        )
        .await;
        assert!(matches!(res, Err(AppError::NotFound)));
    }

    #[tokio::test]
    async fn list_returns_chapters_in_order_with_latest_progress() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = testing::book(&t.state, &user, "Книга", &["1.mp3", "2.mp3"]).await;
        add(State(t.state.clone()), user.clone(), Path(id))
            .await
            .unwrap();
        add(State(t.state.clone()), user.clone(), Path(id))
            .await
            .unwrap(); // adding twice is fine
        sqlx::query("INSERT INTO progress (user_id, book_id, chapter_path, position_sec) VALUES (?, ?, ?, 42)")
            .bind(user.id)
            .bind(id)
            .bind(format!("book-{id}/2.mp3"))
            .execute(&t.state.pool)
            .await
            .unwrap();

        let Json(books) = list(State(t.state.clone()), user).await.unwrap();

        assert_eq!(books.as_array().unwrap().len(), 1);
        let paths: Vec<&str> = books[0]["chapters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["filePath"].as_str().unwrap())
            .collect();
        assert_eq!(
            paths,
            [format!("book-{id}/1.mp3"), format!("book-{id}/2.mp3")]
        );
        assert_eq!(books[0]["progress"]["positionSec"], 42.0);
        assert_eq!(books[0]["finishedAt"], Value::Null);
    }
}
