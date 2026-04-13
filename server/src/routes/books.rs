use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::Path as FsPath,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_util::io::StreamReader;

use crate::{auth::Claims, error::AppError, state::AppState};

// ─── regex-like constants ────────────────────────────────────────────────────

const AUDIO_EXT: &[&str] = &["mp3", "m4a", "m4b", "ogg", "flac", "wav", "aac", "opus"];
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "avif"];
const COVER_NAMES: &[&str] = &["cover", "folder", "front", "artwork", "thumb"];

fn is_audio(name: &str) -> bool {
    ext_of(name).map_or(false, |e| AUDIO_EXT.contains(&e))
}

fn is_image(name: &str) -> bool {
    ext_of(name).map_or(false, |e| IMAGE_EXT.contains(&e))
}

fn is_cover_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    COVER_NAMES.iter().any(|prefix| lower.starts_with(prefix))
}

fn ext_of(name: &str) -> Option<&str> {
    FsPath::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            // We need a static-lifetime str — can't return a ref to local.
            // Instead compare via a known list. Return the ext str slice from name.
            e
        })
}

// Transliteration table for Cyrillic → Latin
const CYR: &[(char, &str)] = &[
    ('а', "a"), ('б', "b"), ('в', "v"), ('г', "g"), ('д', "d"),
    ('е', "e"), ('ё', "yo"), ('ж', "zh"), ('з', "z"), ('и', "i"),
    ('й', "y"), ('к', "k"), ('л', "l"), ('м', "m"), ('н', "n"),
    ('о', "o"), ('п', "p"), ('р', "r"), ('с', "s"), ('т', "t"),
    ('у', "u"), ('ф', "f"), ('х', "kh"), ('ц', "ts"), ('ч', "ch"),
    ('ш', "sh"), ('щ', "shch"), ('ъ', ""), ('ы', "y"), ('ь', ""),
    ('э', "e"), ('ю', "yu"), ('я', "ya"),
];

pub fn sanitize_filename(raw: &str) -> String {
    let name = FsPath::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    let ext = FsPath::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    let stem = &name[..name.len() - ext.len()];

    let mut latin = String::new();
    for c in stem.to_lowercase().chars() {
        if let Some(&(_, t)) = CYR.iter().find(|&&(k, _)| k == c) {
            latin.push_str(t);
        } else {
            latin.push(c);
        }
    }

    // replace non-alphanumeric runs with dash
    let mut result = String::new();
    let mut last_dash = true; // skip leading dashes
    for c in latin.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c);
            last_dash = false;
        } else if !last_dash {
            result.push('-');
            last_dash = true;
        }
    }
    // trim trailing dash
    let result = result.trim_end_matches('-').to_string();
    let result = if result.is_empty() { "file".to_string() } else { result };
    format!("{}{}", result, ext)
}

fn fingerprint(files: &[(&str, u64)]) -> String {
    let mut entries: Vec<String> = files
        .iter()
        .filter(|(name, _)| is_audio(name))
        .map(|(name, size)| format!("{}:{}", sanitize_filename(name), size))
        .collect();
    entries.sort();
    let joined = entries.join("|");
    format!("{:x}", wyhash::wyhash(joined.as_bytes(), 0))
}

// ─── DB row structs ───────────────────────────────────────────────────────────

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

// ─── Helpers ──────────────────────────────────────────────────────────────────

struct SavedFile {
    rel_path: String,
    name: String,
    size: u64,
}

pub fn get_audio_duration(path: &std::path::Path) -> Option<f64> {
    use symphonia::core::{
        formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
    };

    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .ok()?;

    let track = probed.format.default_track()?;
    let params = &track.codec_params;
    let tb = params.time_base?;
    let n_frames = params.n_frames?;
    let t = tb.calc_time(n_frames);
    Some(t.seconds as f64 + t.frac)
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
    let pairs: Vec<(&str, u64)> = body.files.iter().map(|f| (f.name.as_str(), f.size)).collect();
    let fp = fingerprint(&pairs);

    let dup = sqlx::query!("SELECT id, title FROM books WHERE fingerprint = ?", fp)
        .fetch_optional(&state.pool)
        .await?;

    if let Some(d) = dup {
        return Err(AppError::Conflict(format!("Книга уже загружена: «{}»", d.title)));
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

pub async fn upload(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    mut multipart: Multipart,
) -> Result<Json<Value>, AppError> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let dir_name = format!("book-{}", ts);
    let book_dir = state.uploads_dir.join(&dir_name);
    tokio::fs::create_dir_all(&book_dir).await?;

    let mut title: Option<String> = None;
    let mut author: Option<String> = None;
    let mut narrator: Option<String> = None;
    let mut saved: Vec<SavedFile> = Vec::new();
    let mut used_names: HashSet<String> = HashSet::new();

    // Stream each multipart field to disk or read as text
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
            "title" | "author" | "narrator" => {
                let val = field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?;
                match field_name.as_str() {
                    "title" => title = Some(val),
                    "author" => author = Some(val),
                    "narrator" => narrator = Some(val),
                    _ => {}
                }
            }
            _ => {
                let raw_name = file_name.unwrap_or_else(|| "file".to_string());
                let mut safe = sanitize_filename(&raw_name);

                if used_names.contains(&safe) {
                    let ext = FsPath::new(&safe)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| format!(".{}", e))
                        .unwrap_or_default();
                    let stem = &safe[..safe.len() - ext.len()];
                    safe = format!("{}-{}{}", stem, used_names.len(), ext);
                }
                used_names.insert(safe.clone());

                let file_path = book_dir.join(&safe);
                let mut out = tokio::fs::File::create(&file_path).await
                    .map_err(|e| AppError::Internal(e.into()))?;

                // Stream with proper backpressure — tokio::io::copy pulls only
                // when the write side is ready, so RAM usage stays near zero.
                let mapped = field.map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string())
                });
                let mut reader = StreamReader::new(mapped);
                let size = tokio::io::copy(&mut reader, &mut out).await.map_err(|e| {
                    AppError::Internal(e.into())
                })?;

                saved.push(SavedFile { rel_path: format!("{}/{}", dir_name, safe), name: safe, size });
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

    let mut audio: Vec<&SavedFile> = saved.iter().filter(|f| is_audio(&f.name)).collect();
    if audio.is_empty() {
        tokio::fs::remove_dir_all(&book_dir).await.ok();
        return Err(AppError::BadRequest("Аудиофайлы не найдены".into()));
    }
    audio.sort_by(|a, b| {
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    let cover = saved
        .iter()
        .find(|f| is_image(&f.name) && is_cover_name(&f.name))
        .or_else(|| saved.iter().find(|f| is_image(&f.name)));

    // Fingerprint for dedup
    let pairs: Vec<(&str, u64)> = saved.iter().map(|f| (f.name.as_str(), f.size)).collect();
    let fp = fingerprint(&pairs);

    let dup = sqlx::query!("SELECT title FROM books WHERE fingerprint = ?", fp)
        .fetch_optional(&state.pool)
        .await?;
    if let Some(d) = dup {
        tokio::fs::remove_dir_all(&book_dir).await.ok();
        return Err(AppError::Conflict(format!("Книга уже загружена: «{}»", d.title)));
    }

    // Parse durations in parallel with 4s timeout per file
    let durations: Vec<Option<f64>> = {
        let tasks: Vec<_> = audio
            .iter()
            .map(|f| {
                let path = state.uploads_dir.join(&f.rel_path);
                tokio::spawn(tokio::time::timeout(
                    Duration::from_secs(4),
                    tokio::task::spawn_blocking(move || get_audio_duration(&path)),
                ))
            })
            .collect();

        let mut results = Vec::with_capacity(tasks.len());
        for t in tasks {
            let dur = t.await.ok() // JoinError
                .and_then(|r| r.ok()) // timeout
                .and_then(|r| r.ok()) // spawn_blocking JoinError
                .flatten();
            results.push(dur);
        }
        results
    };

    let timed_out = durations.iter().filter(|d| d.is_none()).count();
    if timed_out > 0 {
        tracing::info!("{} файлов без длительности — заполнятся при воспроизведении", timed_out);
    }

    // Insert book
    let file_path = audio[0].rel_path.clone();
    let cover_path = cover.map(|c| c.rel_path.clone());
    let narrator = narrator.filter(|s| !s.is_empty());

    let book_id = sqlx::query!(
        "INSERT INTO books (title, author, narrator, file_path, cover_path, fingerprint, uploaded_by_id)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        title, author, narrator, file_path, cover_path, fp, claims.id
    )
    .fetch_one(&state.pool)
    .await?
    .id;

    for (i, f) in audio.iter().enumerate() {
        let dur = durations[i];
        let sort = i as i64;
        let rp = &f.rel_path;
        sqlx::query!(
            "INSERT INTO chapters (book_id, file_path, sort_order, duration_sec) VALUES (?, ?, ?, ?)",
            book_id, rp, sort, dur
        )
        .execute(&state.pool)
        .await?;
    }

    let total_sec: Option<f64> = {
        let vals: Vec<f64> = durations.iter().filter_map(|&d| d).collect();
        if vals.is_empty() { None } else { Some(vals.iter().sum()) }
    };

    tracing::info!(
        "book created: id={} title={} chapters={} duration={:?}min",
        book_id, title, audio.len(),
        total_sec.map(|s| (s / 60.0) as i64)
    );

    Ok(Json(json!({
        "id": book_id,
        "title": title,
        "author": author,
        "chaptersCount": audio.len(),
        "totalSec": total_sec,
    })))
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
    let narrator = body["narrator"].as_str().map(|s| s.trim().to_string());

    if author.as_deref().is_some_and(|s| s.is_empty())
        || title.as_deref().is_some_and(|s| s.is_empty())
    {
        return Err(AppError::BadRequest("author and title cannot be empty".into()));
    }

    let result = sqlx::query(
        "UPDATE books SET author = COALESCE(?, author), title = COALESCE(?, title), narrator = COALESCE(?, narrator) WHERE id = ?"
    )
    .bind(&author)
    .bind(&title)
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
    mut multipart: Multipart,
) -> Result<Json<Value>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden);
    }

    let book = sqlx::query!("SELECT file_path FROM books WHERE id = ?", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let book_dir = book.file_path
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();

    while let Some(field) = multipart.next_field().await? {
        if field.name() != Some("cover") { continue; }

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

        sqlx::query!("UPDATE books SET cover_path = ? WHERE id = ?", cover_path, id)
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

    let book = sqlx::query!("SELECT file_path FROM books WHERE id = ?", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    sqlx::query!("DELETE FROM progress WHERE book_id = ?", id).execute(&state.pool).await?;
    sqlx::query!("DELETE FROM user_library WHERE book_id = ?", id).execute(&state.pool).await?;
    sqlx::query!("DELETE FROM chapters WHERE book_id = ?", id).execute(&state.pool).await?;
    sqlx::query!("DELETE FROM books WHERE id = ?", id).execute(&state.pool).await?;

    // Delete files
    if let Some(dir) = book.file_path.split('/').next() {
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

    tracing::info!("scan-durations: {} chapters without duration", missing.len());
    let mut updated = 0u32;

    for ch in &missing {
        let path = state.uploads_dir.join(&ch.file_path);
        let dur = tokio::task::spawn_blocking(move || get_audio_duration(&path))
            .await
            .ok()
            .flatten();

        if let Some(d) = dur {
            sqlx::query!("UPDATE chapters SET duration_sec = ? WHERE id = ?", d, ch.id)
                .execute(&state.pool)
                .await?;
            updated += 1;
        }
    }

    let skipped = missing.len() as u32 - updated;
    tracing::info!("scan-durations done: updated={} skipped={}", updated, skipped);
    Ok(Json(json!({ "updated": updated, "skipped": skipped })))
}
