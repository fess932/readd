use std::{path::PathBuf, sync::Arc};
use sqlx::SqlitePool;
use tokio::sync::Notify;

pub struct AppState {
    pub pool: SqlitePool,
    pub jwt_secret: String,
    pub uploads_dir: PathBuf,
    pub tts_notify: Arc<Notify>,
    pub tts_server_url: String,
    pub http_client: reqwest::Client,
}
