use axum::{
    routing::{delete, get, post, patch},
    Router,
};
use std::sync::Arc;

use crate::state::AppState;

pub mod auth;
pub mod books;
pub mod library;
pub mod progress;
pub mod stats;
pub mod text_books;

pub fn api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        // auth
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        // books — literal routes before :id
        .route("/api/books/check", post(books::check))
        .route("/api/books/scan-durations", post(books::scan_durations))
        .route("/api/books", get(books::list).post(books::upload))
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
        .route("/api/text-books", get(text_books::list).post(text_books::upload))
        .route("/api/text-books/{id}", delete(text_books::delete).patch(text_books::patch))
        .route("/api/text-books/{id}/cover", patch(text_books::upload_cover))
        .with_state(state)
    // .layer(TraceLayer::new_for_http())
}

async fn health() -> impl axum::response::IntoResponse {
    axum::Json(serde_json::json!({ "status": "ok" }))
}
