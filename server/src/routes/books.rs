use axum::{
    Json,
    extract::{Multipart, Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::{
    auth::Claims,
    error::AppError,
    media::audio_duration,
    state::AppState,
    storage::{is_audio, remove_old_cover, sanitize_filename, save_cover},
};

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
struct BookRow {
    id: i64,
    title: String,
    author: String,
    narrator: Option<String>,
    cover_path: Option<String>,
    file_path: String,
    uploaded_by: Option<String>,
    created_at: String,
    chapters_count: i64,
    total_sec: Option<f64>,
}

/// Identifies a book by the names and sizes of its audio files, to reject re-uploads.
pub(super) fn fingerprint(files: &[(&str, u64)]) -> String {
    let mut entries: Vec<String> = files
        .iter()
        .filter(|(name, _)| is_audio(name))
        .map(|(name, size)| format!("{}:{}", sanitize_filename(name), size))
        .collect();
    entries.sort();
    let joined = entries.join("|");
    format!("{:x}", wyhash::wyhash(joined.as_bytes(), 0))
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CheckBody {
    pub files: Vec<FileInfo>,
}

#[derive(Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
}

pub async fn check(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CheckBody>,
) -> Result<Json<Value>, AppError> {
    let pairs: Vec<(&str, u64)> = body
        .files
        .iter()
        .map(|f| (f.name.as_str(), f.size))
        .collect();
    let fp = fingerprint(&pairs);

    let dup = sqlx::query!("SELECT id, title FROM books WHERE fingerprint = ?", fp)
        .fetch_optional(&state.pool)
        .await?;

    if let Some(d) = dup {
        return Err(AppError::Conflict(format!(
            "Книга уже загружена: «{}»",
            d.title
        )));
    }
    Ok(Json(json!({ "ok": true, "fingerprint": fp })))
}

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query_as::<_, BookRow>(
        "SELECT b.id, b.title, b.author, b.narrator, b.cover_path, b.file_path,
                u.name AS uploaded_by, b.created_at,
                (SELECT COUNT(*) FROM chapters WHERE book_id = b.id) AS chapters_count,
                (SELECT SUM(duration_sec) FROM chapters WHERE book_id = b.id) AS total_sec
         FROM books b
         LEFT JOIN users u ON b.uploaded_by_id = u.id
         ORDER BY b.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows)))
}

pub async fn patch(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<Value>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden);
    }

    let author = body["author"].as_str().map(|s| s.trim().to_string());
    let title = body["title"].as_str().map(|s| s.trim().to_string());
    // narrator is optional: an empty string or null clears it, a missing key keeps it
    let narrator_given = body.get("narrator").is_some();
    let narrator = body["narrator"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if author.as_deref().is_some_and(|s| s.is_empty())
        || title.as_deref().is_some_and(|s| s.is_empty())
    {
        return Err(AppError::BadRequest(
            "author and title cannot be empty".into(),
        ));
    }

    let result = sqlx::query(
        "UPDATE books SET author = COALESCE(?, author), title = COALESCE(?, title),
                          narrator = CASE WHEN ? THEN ? ELSE narrator END
         WHERE id = ?",
    )
    .bind(&author)
    .bind(&title)
    .bind(narrator_given)
    .bind(&narrator)
    .bind(id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(json!({ "ok": true })))
}

pub async fn upload_cover(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<Value>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden);
    }

    let book = sqlx::query!("SELECT file_path, cover_path FROM books WHERE id = ?", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let book_dir = book.file_path.split('/').next().unwrap_or("");
    let cover_path = save_cover(&state.uploads_dir, book_dir, multipart).await?;

    sqlx::query!(
        "UPDATE books SET cover_path = ? WHERE id = ?",
        cover_path,
        id
    )
    .execute(&state.pool)
    .await?;
    remove_old_cover(&state.uploads_dir, book.cover_path.as_deref(), &cover_path).await;

    Ok(Json(json!({ "ok": true, "coverPath": cover_path })))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden);
    }

    let book = sqlx::query!("SELECT file_path FROM books WHERE id = ?", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut tx = state.pool.begin().await?;
    // A book generated by TTS is referenced by its job; drop the job so the
    // text book can be voiced again.
    sqlx::query!(
        "DELETE FROM tts_chunks WHERE job_id IN (SELECT id FROM tts_jobs WHERE audio_book_id = ?)",
        id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM tts_jobs WHERE audio_book_id = ?", id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM progress WHERE book_id = ?", id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM user_library WHERE book_id = ?", id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM chapters WHERE book_id = ?", id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM books WHERE id = ?", id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // Delete files
    if let Some(dir) = book.file_path.split('/').next().filter(|d| !d.is_empty()) {
        let dir_path = state.uploads_dir.join(dir);
        tokio::fs::remove_dir_all(&dir_path).await.ok();
        tracing::info!("book files removed: {:?}", dir_path);
    }

    tracing::info!("book deleted: id={}", id);
    Ok(Json(json!({ "ok": true })))
}

pub async fn scan_durations(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Value>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden);
    }

    let missing = sqlx::query!("SELECT id, file_path FROM chapters WHERE duration_sec IS NULL")
        .fetch_all(&state.pool)
        .await?;

    tracing::info!(
        "scan-durations: {} chapters without duration",
        missing.len()
    );
    let mut updated = 0u32;

    for ch in &missing {
        let path = state.uploads_dir.join(&ch.file_path);
        let dur = tokio::task::spawn_blocking(move || audio_duration(&path))
            .await
            .ok()
            .flatten();

        if let Some(d) = dur {
            sqlx::query!(
                "UPDATE chapters SET duration_sec = ? WHERE id = ?",
                d,
                ch.id
            )
            .execute(&state.pool)
            .await?;
            updated += 1;
        }
    }

    let skipped = missing.len() as u32 - updated;
    tracing::info!(
        "scan-durations done: updated={} skipped={}",
        updated,
        skipped
    );
    Ok(Json(json!({ "updated": updated, "skipped": skipped })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;

    #[test]
    fn check_and_upload_agree_on_fingerprint() {
        // /books/check sees raw browser names, upload sees sanitized ones
        let raw = fingerprint(&[("TRACK 1.MP3", 10), ("cover.jpg", 5)]);
        let saved = fingerprint(&[("track-1.mp3", 10), ("cover.jpg", 5)]);
        assert_eq!(raw, saved);
    }

    #[test]
    fn fingerprint_ignores_order_and_non_audio() {
        let a = fingerprint(&[("1.mp3", 1), ("2.mp3", 2), ("cover.jpg", 9)]);
        let b = fingerprint(&[("2.mp3", 2), ("1.mp3", 1)]);
        assert_eq!(a, b);
        assert_ne!(a, fingerprint(&[("1.mp3", 1), ("2.mp3", 3)]));
    }

    #[tokio::test]
    async fn check_reports_an_already_uploaded_book() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let book_id = testing::book(&t.state, &admin, "Книга", &["a.mp3"]).await;
        let fp = fingerprint(&[("a.mp3", 100)]);
        sqlx::query("UPDATE books SET fingerprint = ? WHERE id = ?")
            .bind(&fp)
            .bind(book_id)
            .execute(&t.state.pool)
            .await
            .unwrap();

        let body = |size| CheckBody {
            files: vec![FileInfo {
                name: "A.MP3".into(),
                size,
            }],
        };
        let dup = check(State(t.state.clone()), Json(body(100))).await;
        assert!(matches!(dup, Err(AppError::Conflict(_))));
        assert!(check(State(t.state.clone()), Json(body(101))).await.is_ok());
    }

    #[tokio::test]
    async fn patch_can_clear_the_narrator() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let id = testing::book(&t.state, &admin, "Книга", &["a.mp3"]).await;
        let narrator = || async {
            sqlx::query_scalar::<_, Option<String>>("SELECT narrator FROM books WHERE id = ?")
                .bind(id)
                .fetch_one(&t.state.pool)
                .await
                .unwrap()
        };

        patch(
            State(t.state.clone()),
            admin.clone(),
            Path(id),
            Json(json!({ "narrator": "Чтец" })),
        )
        .await
        .unwrap();
        assert_eq!(narrator().await.as_deref(), Some("Чтец"));

        // a missing key keeps the value, an empty string clears it
        patch(
            State(t.state.clone()),
            admin.clone(),
            Path(id),
            Json(json!({ "title": "Новое" })),
        )
        .await
        .unwrap();
        assert_eq!(narrator().await.as_deref(), Some("Чтец"));
        patch(
            State(t.state.clone()),
            admin.clone(),
            Path(id),
            Json(json!({ "narrator": "" })),
        )
        .await
        .unwrap();
        assert_eq!(narrator().await, None);
    }

    #[tokio::test]
    async fn delete_requires_admin() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let reader = testing::user(&t.state, "reader", false).await;
        let id = testing::book(&t.state, &admin, "Книга", &["a.mp3"]).await;

        let denied = delete(State(t.state.clone()), reader, Path(id)).await;
        assert!(matches!(denied, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn delete_removes_a_tts_generated_book_with_everything_attached() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let id = testing::book(&t.state, &admin, "Озвученная", &["book.m4b"]).await;
        let text_book = testing::text_book(&t.state, &admin, "Исходник").await;
        let job = testing::tts_job(&t.state, text_book, "done", Some(id), &["done"]).await;
        testing::add_to_library(&t.state, &admin, id).await;
        let dir = t.state.uploads_dir.join(format!("book-{id}"));
        assert!(dir.exists());

        delete(State(t.state.clone()), admin, Path(id))
            .await
            .unwrap();

        for table in [
            "books",
            "chapters",
            "user_library",
            "tts_jobs",
            "tts_chunks",
        ] {
            assert_eq!(
                testing::count(&t.state, table).await,
                0,
                "{table} is not empty"
            );
        }
        assert_eq!(testing::count(&t.state, "text_books").await, 1);
        assert!(!dir.exists());
        let _ = job;
    }
}
