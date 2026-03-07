use axum::{
    Router,
    extract::{Path, State},
    http::header,
    response::IntoResponse,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tokio_util::io::ReaderStream;

use crate::{error::AppError, state::AppState};

pub mod auth;
pub mod books;
pub mod library;
pub mod progress;

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
        .route("/api/books/:id", delete(books::delete))
        // library
        .route("/api/library", get(library::list))
        .route(
            "/api/library/:book_id",
            get(library::get).post(library::add).delete(library::remove),
        )
        // progress
        .route("/api/progress/last", get(progress::last))
        .route(
            "/api/progress/:book_id",
            get(progress::get_book).post(progress::save),
        )
        // uploads
        .route("/uploads/*path", get(serve_upload))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    axum::Json(serde_json::json!({ "status": "ok" }))
}

pub async fn serve_upload(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let path = path.trim_start_matches('/');
    if path.contains("..") {
        return Err(AppError::NotFound);
    }
    let file_path = state.uploads_dir.join(path);
    match tokio::fs::File::open(&file_path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = axum::body::Body::from_stream(stream);
            let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
            Ok(([(header::CONTENT_TYPE, mime.as_ref().to_string())], body).into_response())
        }
        Err(_) => Err(AppError::NotFound),
    }
}
