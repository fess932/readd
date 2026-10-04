// Handlers return `Json`, which tests call only for the side effect
#![cfg_attr(test, allow(unused_must_use))]

use axum::body::Body;
use axum::http::{HeaderValue, Request, Response};
use axum::{Router, extract::DefaultBodyLimit, middleware};

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::io::IsTerminal;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;

mod auth;
mod config;
mod db;
mod epub;
mod error;
mod media;
mod routes;
mod state;
mod storage;
#[cfg(test)]
mod testing;
mod tts;

use config::Config;
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

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    tracing::info!("Shutting down");
}

async fn connect_db(path: &str) -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{path}"))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        .pragma("cache_size", "-64000")
        .pragma("temp_store", "MEMORY");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;
    db::setup(&pool).await?;
    Ok(pool)
}

fn init_logging() {
    // RUST_LOG overrides the default, e.g. RUST_LOG=debug or RUST_LOG=info,sqlx=debug
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .with_ansi(std::io::stdout().is_terminal())
        .compact()
        .init();
}

/// Runs the TTS worker for the lifetime of the process, restarting it if it panics.
fn spawn_tts_worker(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            let worker = tokio::spawn(tts::run(Arc::clone(&state)));
            if let Err(e) = worker.await {
                tracing::error!("TTS worker crashed: {e}; restarting in 5s");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

fn build_app(state: Arc<AppState>, dist_dir: &Path) -> Router {
    let index = dist_dir.join("index.html");
    if !index.exists() {
        tracing::warn!("{:?} not found — the web UI will not be served", index);
    }

    let uploads = tower::ServiceBuilder::new()
        .layer(middleware::from_fn(fix_m4b_mime))
        .service(ServeDir::new(&state.uploads_dir));

    Router::new()
        .merge(routes::api_router(state))
        .nest_service("/uploads", uploads)
        // anything else is a client-side route of the SPA
        .fallback_service(ServeDir::new(dist_dir).fallback(ServeFile::new(index)))
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::disable()) // audiobooks are uploaded whole
        // one INFO line per request: method, path, status, latency
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();
    let config = Config::from_env();

    tokio::fs::create_dir_all(&config.uploads_dir).await?;
    let pool = connect_db(&config.database_path).await?;

    let state = Arc::new(AppState {
        pool: pool.clone(),
        jwt_secret: config.jwt_secret,
        uploads_dir: config.uploads_dir,
        tts_notify: Arc::new(Notify::new()),
        tts_server_url: config.tts_server_url,
        http_client: reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?,
    });

    spawn_tts_worker(Arc::clone(&state));

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    tracing::info!("Server started on {}", config.listen_addr);
    axum::serve(listener, build_app(state, &config.dist_dir))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    pool.close().await;
    Ok(())
}
