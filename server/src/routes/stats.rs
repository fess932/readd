use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::{auth::Claims, error::AppError, state::AppState};

pub async fn get(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Value>, AppError> {
    let pool = &state.pool;

    // Global
    let total_books: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM books")
        .fetch_one(pool)
        .await?;

    let total_users: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    let total_sec: f64 = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(duration_sec), 0.0) FROM chapters"
    )
    .fetch_one(pool)
    .await?;

    let top_books = sqlx::query!(
        "SELECT b.id, b.title, b.author, b.cover_path,
                COUNT(ul.user_id) AS library_count
         FROM books b
         LEFT JOIN user_library ul ON b.id = ul.book_id
         GROUP BY b.id
         ORDER BY library_count DESC
         LIMIT 5"
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| json!({
        "id": r.id,
        "title": r.title,
        "author": r.author,
        "coverPath": r.cover_path,
        "libraryCount": r.library_count,
    }))
    .collect::<Vec<_>>();

    let uploaders = sqlx::query!(
        "SELECT u.name, COUNT(b.id) AS books_count
         FROM users u
         INNER JOIN books b ON b.uploaded_by_id = u.id
         GROUP BY u.id
         ORDER BY books_count DESC"
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| json!({
        "name": r.name,
        "booksCount": r.books_count,
    }))
    .collect::<Vec<_>>();

    // Personal
    let my_books_count: i64 =
        sqlx::query_scalar!("SELECT COUNT(*) FROM user_library WHERE user_id = ?", claims.id)
            .fetch_one(pool)
            .await?;

    let my_listened_sec: f64 = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(position_sec), 0.0) FROM progress WHERE user_id = ?",
        claims.id
    )
    .fetch_one(pool)
    .await?;

    let favorite_author: Option<String> = sqlx::query_scalar!(
        "SELECT b.author FROM user_library ul
         JOIN books b ON ul.book_id = b.id
         WHERE ul.user_id = ?
         GROUP BY b.author
         ORDER BY COUNT(*) DESC
         LIMIT 1",
        claims.id
    )
    .fetch_optional(pool)
    .await?;

    Ok(Json(json!({
        "personal": {
            "booksInLibrary": my_books_count,
            "listenedSec": my_listened_sec,
            "favoriteAuthor": favorite_author,
        },
        "global": {
            "totalBooks": total_books,
            "totalUsers": total_users,
            "totalSec": total_sec,
            "topBooks": top_books,
            "uploaders": uploaders,
        },
    })))
}
