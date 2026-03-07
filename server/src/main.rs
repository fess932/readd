use std::path::PathBuf;
use std::sync::Arc;
use axum::{Router, extract::DefaultBodyLimit};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteJournalMode, SqliteSynchronous};
use std::str::FromStr;

mod error;
mod state;
mod db;
mod auth;
mod routes;

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .compact()
        .init();

    let db_path = std::env::var("DATABASE_URL").unwrap_or_else(|_| "readd.db".to_string());
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "readd-secret-key".to_string());
    let uploads_dir =
        PathBuf::from(std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "uploads".to_string()));
    let dist_dir =
        PathBuf::from(std::env::var("DIST_DIR").unwrap_or_else(|_| "dist".to_string()));

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

    let state = Arc::new(AppState { pool, jwt_secret, uploads_dir });

    let dist_fallback = dist_dir.join("index.html");
    let app = Router::new()
        .merge(routes::api_router(Arc::clone(&state)))
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::disable())
        .fallback_service(
            ServeDir::new(&dist_dir).fallback(ServeFile::new(dist_fallback)),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("Server started on port 3000");
    axum::serve(listener, app).await?;

    Ok(())
}
