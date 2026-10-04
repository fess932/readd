//! Fixtures for handler tests: an in-memory database and a temporary uploads directory.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{str::FromStr, sync::Arc};
use tokio::sync::Notify;

use crate::{auth::Claims, db, state::AppState};

pub struct TestState {
    pub state: Arc<AppState>,
    /// Removed from disk when dropped.
    _uploads: tempfile::TempDir,
}

pub async fn state() -> TestState {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    // A single connection that is never recycled: an in-memory database lives in it
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(opts)
        .await
        .unwrap();
    db::setup(&pool).await.unwrap();

    let uploads = tempfile::tempdir().unwrap();
    let state = Arc::new(AppState {
        pool,
        jwt_secret: "test".into(),
        uploads_dir: uploads.path().to_path_buf(),
        tts_notify: Arc::new(Notify::new()),
        tts_server_url: "http://127.0.0.1:1".into(),
        http_client: reqwest::Client::new(),
    });
    TestState {
        state,
        _uploads: uploads,
    }
}

pub async fn user(state: &AppState, name: &str, is_admin: bool) -> Claims {
    let id: i64 =
        sqlx::query_scalar("INSERT INTO users (name, is_admin) VALUES (?, ?) RETURNING id")
            .bind(name)
            .bind(is_admin)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    Claims {
        id,
        name: name.into(),
        is_admin,
        exp: None,
    }
}

/// A book in `book-<id>/` with one chapter (and an empty file on disk) per name.
pub async fn book(state: &AppState, owner: &Claims, title: &str, chapters: &[&str]) -> i64 {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO books (title, author, file_path, uploaded_by_id) VALUES (?, 'Автор', '', ?) RETURNING id",
    )
    .bind(title)
    .bind(owner.id)
    .fetch_one(&state.pool)
    .await
    .unwrap();

    let dir = format!("book-{id}");
    std::fs::create_dir_all(state.uploads_dir.join(&dir)).unwrap();
    for (i, name) in chapters.iter().enumerate() {
        let path = format!("{dir}/{name}");
        std::fs::write(state.uploads_dir.join(&path), b"").unwrap();
        sqlx::query("INSERT INTO chapters (book_id, file_path, sort_order) VALUES (?, ?, ?)")
            .bind(id)
            .bind(&path)
            .bind(i as i64)
            .execute(&state.pool)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE books SET file_path = ? WHERE id = ?")
        .bind(format!("{dir}/{}", chapters.first().copied().unwrap_or("")))
        .bind(id)
        .execute(&state.pool)
        .await
        .unwrap();
    id
}

pub async fn add_to_library(state: &AppState, user: &Claims, book_id: i64) {
    sqlx::query("INSERT INTO user_library (user_id, book_id) VALUES (?, ?)")
        .bind(user.id)
        .bind(book_id)
        .execute(&state.pool)
        .await
        .unwrap();
}

pub async fn text_book(state: &AppState, owner: &Claims, title: &str) -> i64 {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO text_books (title, author, file_path, uploaded_by_id) VALUES (?, 'Автор', '', ?) RETURNING id",
    )
    .bind(title)
    .bind(owner.id)
    .fetch_one(&state.pool)
    .await
    .unwrap();

    let dir = format!("textbook-{id}");
    std::fs::create_dir_all(state.uploads_dir.join(&dir)).unwrap();
    sqlx::query("UPDATE text_books SET file_path = ? WHERE id = ?")
        .bind(format!("{dir}/book.epub"))
        .bind(id)
        .execute(&state.pool)
        .await
        .unwrap();
    id
}

/// A job with one chunk per entry of `chunk_statuses`.
pub async fn tts_job(
    state: &AppState,
    text_book_id: i64,
    status: &str,
    audio_book_id: Option<i64>,
    chunk_statuses: &[&str],
) -> i64 {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO tts_jobs (text_book_id, status, total_chunks, audio_book_id) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(text_book_id)
    .bind(status)
    .bind(chunk_statuses.len() as i64)
    .bind(audio_book_id)
    .fetch_one(&state.pool)
    .await
    .unwrap();

    for (seq, chunk_status) in chunk_statuses.iter().enumerate() {
        sqlx::query("INSERT INTO tts_chunks (job_id, seq, text, status) VALUES (?, ?, 'текст', ?)")
            .bind(id)
            .bind(seq as i64)
            .bind(chunk_status)
            .execute(&state.pool)
            .await
            .unwrap();
    }
    id
}

pub async fn count(state: &AppState, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
        .fetch_one(&state.pool)
        .await
        .unwrap()
}
