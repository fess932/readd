//! Runtime settings, all optional, read from the environment.

use std::path::PathBuf;

pub struct Config {
    /// Path to the SQLite file; a `sqlite:` prefix is accepted.
    pub database_path: String,
    pub jwt_secret: String,
    /// Uploaded and generated media, served at `/uploads`.
    pub uploads_dir: PathBuf,
    /// The built web UI.
    pub dist_dir: PathBuf,
    pub tts_server_url: String,
    pub listen_addr: String,
}

fn var(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = var("DATABASE_URL", "readd.db");
        Self {
            database_path: database_url
                .strip_prefix("sqlite:")
                .unwrap_or(&database_url)
                .to_string(),
            jwt_secret: var("JWT_SECRET", "readd-secret-key"),
            uploads_dir: PathBuf::from(var("UPLOADS_DIR", "uploads")),
            dist_dir: PathBuf::from(var("DIST_DIR", "dist")),
            tts_server_url: var("TTS_SERVER_URL", "http://localhost:8080"),
            listen_addr: format!("{}:{}", var("HOST", "0.0.0.0"), var("PORT", "3000")),
        }
    }
}
