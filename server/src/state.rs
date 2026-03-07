use std::path::PathBuf;
use sqlx::SqlitePool;

pub struct AppState {
    pub pool: SqlitePool,
    pub jwt_secret: String,
    pub uploads_dir: PathBuf,
}
