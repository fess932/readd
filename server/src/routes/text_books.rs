use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use futures_util::TryStreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::{path::Path as FsPath, sync::Arc};
use tokio_util::io::StreamReader;

use crate::{auth::Claims, error::AppError, state::AppState};

use super::books::sanitize_filename;

const EPUB_EXT: &[&str] = &["epub"];
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "avif"];


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
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let dir_name = format!("textbook-{}", ts);
    let book_dir = state.uploads_dir.join(&dir_name);
    tokio::fs::create_dir_all(&book_dir).await?;

    let mut title: Option<String> = None;
    let mut author: Option<String> = None;
    let mut epub_rel_path: Option<String> = None;
    let mut epub_size: Option<i64> = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                tokio::fs::remove_dir_all(&book_dir).await.ok();
                return Err(AppError::BadRequest(e.to_string()));
            }
        };

        let field_name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().map(|s| s.to_string());

        match field_name.as_str() {
            "title" | "author" => {
                let val = field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?;
                match field_name.as_str() {
                    "title" => title = Some(val),
                    "author" => author = Some(val),
                    _ => {}
                }
            }
            _ => {
                let raw_name = file_name.unwrap_or_else(|| "file".to_string());
                if !is_epub(&raw_name) {
                    // skip non-epub files
                    let _ = field.bytes().await;
                    continue;
                }
                let safe = sanitize_filename(&raw_name);
                let file_path = book_dir.join(&safe);
                let mut out = tokio::fs::File::create(&file_path)
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?;

                let mapped = field.map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string())
                });
                let mut reader = StreamReader::new(mapped);
                let size = tokio::io::copy(&mut reader, &mut out)
                    .await
                    .map_err(|e| AppError::Internal(e.into()))?;

                epub_rel_path = Some(format!("{}/{}", dir_name, safe));
                epub_size = Some(size as i64);
            }
        }
    }

    let title = match title.filter(|s| !s.is_empty()) {
        Some(t) => t,
        None => {
            tokio::fs::remove_dir_all(&book_dir).await.ok();
            return Err(AppError::BadRequest("title и author обязательны".into()));
        }
    };
    let author = match author.filter(|s| !s.is_empty()) {
        Some(a) => a,
        None => {
            tokio::fs::remove_dir_all(&book_dir).await.ok();
            return Err(AppError::BadRequest("title и author обязательны".into()));
        }
    };
    let file_path = match epub_rel_path {
        Some(p) => p,
        None => {
            tokio::fs::remove_dir_all(&book_dir).await.ok();
            return Err(AppError::BadRequest("epub файл не найден".into()));
        }
    };

    let book_id = sqlx::query!(
        "INSERT INTO text_books (title, author, file_path, file_size, uploaded_by_id)
         VALUES (?, ?, ?, ?, ?) RETURNING id",
        title,
        author,
        file_path,
        epub_size,
        claims.id
    )
    .fetch_one(&state.pool)
    .await?
    .id;

    tracing::info!("text_book created: id={:?} title={}", book_id, title);

    Ok(Json(json!({
        "id": book_id,
        "title": title,
        "author": author,
        "filePath": file_path,
        "fileSize": epub_size,
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
        return Err(AppError::BadRequest("author and title cannot be empty".into()));
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
    mut multipart: Multipart,
) -> Result<Json<Value>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden);
    }

    let book = sqlx::query!("SELECT file_path FROM text_books WHERE id = ?", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let book_dir = book.file_path
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();

    while let Some(field) = multipart.next_field().await? {
        if field.name() != Some("cover") {
            continue;
        }

        let filename = field.file_name().unwrap_or("cover.jpg").to_string();
        let ext = FsPath::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_lowercase();

        if !IMAGE_EXT.contains(&ext.as_str()) {
            return Err(AppError::BadRequest("не изображение".into()));
        }

        let cover_filename = format!("cover.{ext}");
        let cover_path = format!("{book_dir}/{cover_filename}");
        let full_path = state.uploads_dir.join(&book_dir).join(&cover_filename);

        let data = field.bytes().await?;
        tokio::fs::write(&full_path, &data).await?;

        sqlx::query!("UPDATE text_books SET cover_path = ? WHERE id = ?", cover_path, id)
            .execute(&state.pool)
            .await?;

        return Ok(Json(json!({ "ok": true, "coverPath": cover_path })));
    }

    Err(AppError::BadRequest("поле cover не найдено".into()))
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

    sqlx::query!("DELETE FROM text_books WHERE id = ?", id)
        .execute(&state.pool)
        .await?;

    if let Some(dir) = book.file_path.split('/').next() {
        let dir_path = state.uploads_dir.join(dir);
        tokio::fs::remove_dir_all(&dir_path).await.ok();
        tracing::info!("text_book files removed: {:?}", dir_path);
    }

    tracing::info!("text_book deleted: id={}", id);
    Ok(Json(json!({ "ok": true })))
}
