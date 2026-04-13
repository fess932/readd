use axum::{extract::DefaultBodyLimit, middleware, Router};
use axum::body::Body;
use axum::http::{HeaderValue, Request, Response};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Notify;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

mod auth;
mod db;
mod epub;
mod error;
mod routes;
mod state;
mod tts_worker;

use state::AppState;

async fn fix_m4b_mime(request: Request<Body>, next: middleware::Next) -> Response<Body> {
    let is_m4b = request.uri().path().ends_with(".m4b");
    let mut response = next.run(request).await;
    if is_m4b {
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("audio/mp4"),
        );
    }
    response
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .compact()
        .init();

    let db_path = std::env::var("DATABASE_URL").unwrap_or_else(|_| "readd.db".to_string());
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "readd-secret-key".to_string());
    let uploads_dir =
        PathBuf::from(std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "uploads".to_string()));
    let dist_dir = PathBuf::from(std::env::var("DIST_DIR").unwrap_or_else(|_| "dist".to_string()));
    let tts_server_url = std::env::var("TTS_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    tokio::fs::create_dir_all(&uploads_dir).await?;

    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5))
        .pragma("cache_size", "-64000")
        .pragma("temp_store", "MEMORY");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    db::setup(&pool).await?;

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let tts_notify = Arc::new(Notify::new());

    let state = Arc::new(AppState {
        pool,
        jwt_secret,
        uploads_dir: uploads_dir.clone(),
        tts_notify: Arc::clone(&tts_notify),
        tts_server_url,
        http_client,
    });

    // Spawn TTS background worker
    tokio::spawn(tts_worker::run(Arc::clone(&state)));

    let dist_fallback = dist_dir.join("index.html");
    let app = Router::new()
        .layer(TraceLayer::new_for_http())
        .merge(routes::api_router(Arc::clone(&state)))
        .nest_service(
            "/uploads",
            tower::ServiceBuilder::new()
                .layer(middleware::from_fn(fix_m4b_mime))
                .service(ServeDir::new(&uploads_dir)),
        )
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::disable())
        .fallback_service(ServeDir::new(&dist_dir).fallback(ServeFile::new(dist_fallback)));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("Server started on port 3000");
    axum::serve(listener, app).await?;

    Ok(())
}
