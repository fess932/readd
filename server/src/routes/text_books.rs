use axum::{
    Json,
    extract::{Multipart, Path, State},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{path::Path as FsPath, sync::Arc};

use crate::{
    auth::Claims,
    error::AppError,
    state::AppState,
    storage::{create_upload_dir, remove_old_cover, sanitize_filename, save_cover, save_field},
};

const EPUB_EXT: &[&str] = &["epub"];

fn is_epub(name: &str) -> bool {
    FsPath::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| EPUB_EXT.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
struct TextBookRow {
    id: i64,
    title: String,
    author: String,
    cover_path: Option<String>,
    file_path: String,
    file_size: Option<i64>,
    uploaded_by: Option<String>,
    created_at: String,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query_as::<_, TextBookRow>(
        "SELECT tb.id, tb.title, tb.author, tb.cover_path, tb.file_path, tb.file_size,
                u.name AS uploaded_by, tb.created_at
         FROM text_books tb
         LEFT JOIN users u ON tb.uploaded_by_id = u.id
         ORDER BY tb.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows)))
}

pub async fn upload(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    mut multipart: Multipart,
) -> Result<Json<Value>, AppError> {
    let (dir_name, guard) = create_upload_dir(&state.uploads_dir, "textbook").await?;
    let mut title: Option<String> = None;
    let mut author: Option<String> = None;
    let mut epub: Option<(String, i64)> = None; // (relative path, size)

    while let Some(field) = multipart.next_field().await? {
        match field.name().unwrap_or("") {
            "title" => title = Some(field.text().await?.trim().to_string()),
            "author" => author = Some(field.text().await?.trim().to_string()),
            _ => {
                let raw_name = field.file_name().unwrap_or("file").to_string();
                if !is_epub(&raw_name) {
                    continue; // the next next_field() call drains the skipped field
                }
                let rel_path = format!("{}/{}", dir_name, sanitize_filename(&raw_name));
                let size = save_field(field, &state.uploads_dir.join(&rel_path)).await?;
                epub = Some((rel_path, size as i64));
            }
        }
    }

    let (Some(title), Some(author)) = (
        title.filter(|s| !s.is_empty()),
        author.filter(|s| !s.is_empty()),
    ) else {
        return Err(AppError::BadRequest("title и author обязательны".into()));
    };
    let Some((file_path, epub_size)) = epub else {
        return Err(AppError::BadRequest("epub файл не найден".into()));
    };

    let book = sqlx::query!(
        "INSERT INTO text_books (title, author, file_path, file_size, uploaded_by_id)
         VALUES (?, ?, ?, ?, ?) RETURNING id AS \"id!\", created_at",
        title,
        author,
        file_path,
        epub_size,
        claims.id
    )
    .fetch_one(&state.pool)
    .await?;
    guard.keep();

    tracing::info!("text_book created: id={} title={}", book.id, title);

    Ok(Json(json!({
        "id": book.id,
        "title": title,
        "author": author,
        "coverPath": null,
        "filePath": file_path,
        "fileSize": epub_size,
        "uploadedBy": claims.name,
        "createdAt": book.created_at,
    })))
}

pub async fn patch(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden);
    }

    let author = body["author"].as_str().map(|s| s.trim().to_string());
    let title = body["title"].as_str().map(|s| s.trim().to_string());

    if author.as_deref().is_some_and(|s| s.is_empty())
        || title.as_deref().is_some_and(|s| s.is_empty())
    {
        return Err(AppError::BadRequest(
            "author and title cannot be empty".into(),
        ));
    }

    let result = sqlx::query(
        "UPDATE text_books SET author = COALESCE(?, author), title = COALESCE(?, title) WHERE id = ?"
    )
    .bind(&author)
    .bind(&title)
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

    let book = sqlx::query!(
        "SELECT file_path, cover_path FROM text_books WHERE id = ?",
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let book_dir = book.file_path.split('/').next().unwrap_or("");
    let cover_path = save_cover(&state.uploads_dir, book_dir, multipart).await?;

    sqlx::query!(
        "UPDATE text_books SET cover_path = ? WHERE id = ?",
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

    let book = sqlx::query!("SELECT file_path FROM text_books WHERE id = ?", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    // Jobs that never produced an audiobook own a directory of chunk audio;
    // finished ones keep theirs — it holds the audiobook, which stays.
    let unfinished = sqlx::query!(
        "SELECT id AS \"id!\" FROM tts_jobs WHERE text_book_id = ? AND audio_book_id IS NULL",
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let mut tx = state.pool.begin().await?;
    sqlx::query!(
        "DELETE FROM tts_chunks WHERE job_id IN (SELECT id FROM tts_jobs WHERE text_book_id = ?)",
        id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM tts_jobs WHERE text_book_id = ?", id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM text_books WHERE id = ?", id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    for job in unfinished {
        let dir = state.uploads_dir.join(format!("tts-{}", job.id));
        tokio::fs::remove_dir_all(&dir).await.ok();
    }
    if let Some(dir) = book.file_path.split('/').next().filter(|d| !d.is_empty()) {
        let dir_path = state.uploads_dir.join(dir);
        tokio::fs::remove_dir_all(&dir_path).await.ok();
        tracing::info!("text_book files removed: {:?}", dir_path);
    }

    tracing::info!("text_book deleted: id={}", id);
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;

    #[tokio::test]
    async fn delete_takes_unfinished_jobs_and_their_audio_along() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let id = testing::text_book(&t.state, &admin, "Книга").await;
        let job = testing::tts_job(&t.state, id, "paused", None, &["done", "pending"]).await;
        let job_dir = t.state.uploads_dir.join(format!("tts-{job}"));
        std::fs::create_dir_all(&job_dir).unwrap();

        delete(State(t.state.clone()), admin, Path(id))
            .await
            .unwrap();

        for table in ["text_books", "tts_jobs", "tts_chunks"] {
            assert_eq!(
                testing::count(&t.state, table).await,
                0,
                "{table} is not empty"
            );
        }
        assert!(!job_dir.exists());
        assert!(!t.state.uploads_dir.join(format!("textbook-{id}")).exists());
    }

    #[tokio::test]
    async fn delete_keeps_the_audiobook_made_from_it() {
        let t = testing::state().await;
        let admin = testing::user(&t.state, "admin", true).await;
        let id = testing::text_book(&t.state, &admin, "Книга").await;
        let audio = testing::book(&t.state, &admin, "Озвученная", &["book.m4b"]).await;
        let job = testing::tts_job(&t.state, id, "done", Some(audio), &["done"]).await;
        let job_dir = t.state.uploads_dir.join(format!("tts-{job}"));
        std::fs::create_dir_all(&job_dir).unwrap();

        delete(State(t.state.clone()), admin, Path(id))
            .await
            .unwrap();

        assert_eq!(testing::count(&t.state, "text_books").await, 0);
        assert_eq!(testing::count(&t.state, "books").await, 1);
        assert!(
            job_dir.exists(),
            "the finished job's directory holds the audiobook"
        );
    }

    #[test]
    fn epub_detection_ignores_case() {
        assert!(is_epub("Book.EPUB"));
        assert!(!is_epub("book.pdf"));
    }
}
