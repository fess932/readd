use axum::{
    Router,
    routing::{any, delete, get, patch, post, put},
};

use std::sync::Arc;

use crate::state::AppState;

pub mod auth;
pub mod book_upload;
pub mod books;
pub mod library;
pub mod progress;
pub mod stats;
pub mod text_books;
pub mod tts;

pub fn api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        // auth
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        // books — literal routes before :id
        .route("/api/books/check", post(books::check))
        .route("/api/books/scan-durations", post(books::scan_durations))
        .route("/api/books", get(books::list))
        // a book is uploaded file by file: open, send each file, finish
        .route("/api/books/uploads", post(book_upload::start))
        .route("/api/books/uploads/{id}", delete(book_upload::cancel))
        .route(
            "/api/books/uploads/{id}/files/{index}",
            put(book_upload::put_file),
        )
        .route("/api/books/uploads/{id}/finish", post(book_upload::finish))
        .route("/api/books/{id}", delete(books::delete).patch(books::patch))
        .route("/api/books/{id}/cover", patch(books::upload_cover))
        // library
        .route("/api/library", get(library::list))
        .route(
            "/api/library/{book_id}",
            get(library::get).post(library::add).delete(library::remove),
        )
        .route("/api/library/{book_id}/finish", post(library::finish))
        // progress
        .route("/api/progress/last", get(progress::last))
        .route(
            "/api/progress/{book_id}",
            get(progress::get_book).post(progress::save),
        )
        // stats
        .route("/api/stats", get(stats::get))
        // text books
        .route(
            "/api/text-books",
            get(text_books::list).post(text_books::upload),
        )
        .route(
            "/api/text-books/{id}",
            delete(text_books::delete).patch(text_books::patch),
        )
        .route(
            "/api/text-books/{id}/cover",
            patch(text_books::upload_cover),
        )
        .route(
            "/api/text-books/{id}/tts",
            post(tts::create).get(tts::get_for_book),
        )
        // tts jobs
        .route("/api/tts-jobs", get(tts::list))
        .route("/api/tts-jobs/{id}/pause", post(tts::pause))
        .route("/api/tts-jobs/{id}/resume", post(tts::resume))
        .route("/api/tts-jobs/{id}", delete(tts::cancel))
        // unknown API paths must not fall through to the SPA index.html
        .route("/api/{*rest}", any(not_found))
        .with_state(state)
}

async fn not_found() -> crate::error::AppError {
    crate::error::AppError::NotFound
}

async fn health() -> impl axum::response::IntoResponse {
    axum::Json(serde_json::json!({ "status": "ok" }))
}
