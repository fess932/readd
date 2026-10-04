//! Uploading an audiobook file by file.
//!
//! A book is a gigabyte of audio. Sent as one request, any hiccup on the way (Wi-Fi,
//! a proxy, a timeout) loses everything, and the browser cannot even say why. So the
//! client opens an upload, sends each file in its own request — which it can retry —
//! and then asks to turn the uploaded files into a book.
//!
//! An upload is the future book directory with a marker file in it; parts are stored
//! as `<index>.part` and get their real names when the upload is finished.

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
};
use futures_util::TryStreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio_util::io::StreamReader;

use super::books::fingerprint;
use crate::{
    auth::Claims,
    error::AppError,
    media::audio_duration,
    state::AppState,
    storage::{
        create_upload_dir, is_audio, is_cover_name, is_image, natural_cmp, sanitize_filename,
    },
};

/// Present in a book directory while its upload is in progress.
const MARKER: &str = ".uploading";
/// Uploads nobody finished or cancelled are removed after this long.
const ABANDONED_AFTER: Duration = Duration::from_secs(24 * 3600);

/// The directory of an upload in progress; `NotFound` for anything else.
/// The id comes from the URL, so it is checked before it gets near the file system.
fn upload_dir(uploads_dir: &FsPath, upload_id: &str) -> Result<PathBuf, AppError> {
    let well_formed = upload_id
        .strip_prefix("book-")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    let dir = uploads_dir.join(upload_id);
    if well_formed && dir.join(MARKER).exists() {
        Ok(dir)
    } else {
        Err(AppError::NotFound)
    }
}

fn part_path(dir: &FsPath, index: u32) -> PathBuf {
    dir.join(format!("{index}.part"))
}

/// POST /api/books/uploads — open an upload
pub async fn start(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
) -> Result<Json<Value>, AppError> {
    let (upload_id, guard) = create_upload_dir(&state.uploads_dir, "book").await?;
    tokio::fs::write(state.uploads_dir.join(&upload_id).join(MARKER), b"").await?;
    guard.keep();
    Ok(Json(json!({ "uploadId": upload_id })))
}

/// PUT /api/books/uploads/:id/files/:index — one file, as the raw request body.
/// Sending the same index again replaces the part, so the client can simply retry.
pub async fn put_file(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
    Path((upload_id, index)): Path<(String, u32)>,
    body: Body,
) -> Result<Json<Value>, AppError> {
    let dir = upload_dir(&state.uploads_dir, &upload_id)?;

    // Written under a temporary name: a part that exists is a part that arrived whole
    let incoming = dir.join(format!("{index}.incoming"));
    let mut out = tokio::fs::File::create(&incoming).await?;
    let stream = body
        .into_data_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string()));
    let copied = tokio::io::copy(&mut StreamReader::new(stream), &mut out).await;
    drop(out);

    match copied {
        Ok(size) => {
            tokio::fs::rename(&incoming, part_path(&dir, index)).await?;
            Ok(Json(json!({ "ok": true, "size": size })))
        }
        Err(e) => {
            tokio::fs::remove_file(&incoming).await.ok();
            Err(AppError::BadRequest(format!(
                "передача файла прервана: {e}"
            )))
        }
    }
}

/// DELETE /api/books/uploads/:id — give up on an upload
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    _claims: Claims,
    Path(upload_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let dir = upload_dir(&state.uploads_dir, &upload_id)?;
    tokio::fs::remove_dir_all(&dir).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct FinishBody {
    pub title: String,
    pub author: String,
    pub narrator: Option<String>,
    pub files: Vec<UploadedFile>,
}

#[derive(Deserialize)]
pub struct UploadedFile {
    /// The index the file was sent under.
    pub index: u32,
    /// Path inside the picked folder, e.g. `Book/CD1/01.mp3`.
    pub name: String,
    pub size: u64,
}

/// A part that has been given its final name in the book directory.
struct SavedFile {
    rel_path: String,
    /// Name on disk (sanitized, unique within the book directory).
    name: String,
    /// Sanitized name before the uniqueness suffix — what `/books/check` sees.
    fp_name: String,
    /// Lowercased path as sent by the browser; chapters are ordered by it.
    sort_key: String,
    size: u64,
}

/// `name`, or `stem-2.ext`, `stem-3.ext`… if it is already taken.
/// Happens when folders hold files with the same name (CD1/01.mp3, CD2/01.mp3).
fn unique_name(name: &str, used: &HashSet<String>) -> String {
    if !used.contains(name) {
        return name.to_string();
    }
    let path = FsPath::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    (2..)
        .map(|n| match ext {
            Some(ext) => format!("{stem}-{n}.{ext}"),
            None => format!("{stem}-{n}"),
        })
        .find(|candidate| !used.contains(candidate))
        .expect("an unbounded range always yields a free name")
}

/// Checks that every announced file arrived whole and gives the parts their real names.
async fn collect_files(
    dir: &FsPath,
    upload_id: &str,
    files: &[UploadedFile],
) -> Result<Vec<SavedFile>, AppError> {
    // Verify everything first, so that a missing file leaves the upload untouched and retryable
    for file in files {
        let received = tokio::fs::metadata(part_path(dir, file.index))
            .await
            .map(|m| m.len())
            .ok();
        if received != Some(file.size) {
            return Err(AppError::BadRequest(format!(
                "файл «{}» не загружен целиком: на сервере {} из {} байт",
                file.name,
                received.unwrap_or(0),
                file.size
            )));
        }
    }

    let mut saved = Vec::with_capacity(files.len());
    let mut used_names: HashSet<String> = HashSet::new();
    for file in files {
        let fp_name = sanitize_filename(&file.name);
        let name = unique_name(&fp_name, &used_names);
        used_names.insert(name.clone());
        tokio::fs::rename(part_path(dir, file.index), dir.join(&name)).await?;

        saved.push(SavedFile {
            rel_path: format!("{upload_id}/{name}"),
            name,
            fp_name,
            sort_key: file.name.to_lowercase(),
            size: file.size,
        });
    }
    Ok(saved)
}

/// Probes all files in parallel, giving up on each after 4 seconds.
async fn probe_durations(uploads_dir: &FsPath, files: &[&SavedFile]) -> Vec<Option<f64>> {
    let tasks: Vec<_> = files
        .iter()
        .map(|f| {
            let path = uploads_dir.join(&f.rel_path);
            tokio::spawn(tokio::time::timeout(
                Duration::from_secs(4),
                tokio::task::spawn_blocking(move || audio_duration(&path)),
            ))
        })
        .collect();

    let mut durations = Vec::with_capacity(tasks.len());
    for task in tasks {
        // JoinError, timeout and the blocking task's JoinError all mean "unknown"
        let duration = task
            .await
            .ok()
            .and_then(|r| r.ok())
            .and_then(|r| r.ok())
            .flatten();
        durations.push(duration);
    }
    durations
}

/// POST /api/books/uploads/:id/finish — turn the uploaded files into a book
pub async fn finish(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(upload_id): Path<String>,
    Json(body): Json<FinishBody>,
) -> Result<Json<Value>, AppError> {
    let dir = upload_dir(&state.uploads_dir, &upload_id)?;

    let title = body.title.trim().to_string();
    let author = body.author.trim().to_string();
    if title.is_empty() || author.is_empty() {
        return Err(AppError::BadRequest("title и author обязательны".into()));
    }
    let narrator = body
        .narrator
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let files = collect_files(&dir, &upload_id, &body.files).await?;

    // From here on the parts are renamed: a failure ends the upload for good
    let created = create_book(
        &state,
        &claims,
        &title,
        &author,
        narrator.as_deref(),
        &files,
    )
    .await;
    match created {
        Ok(book) => {
            tokio::fs::remove_file(dir.join(MARKER)).await.ok();
            Ok(Json(book))
        }
        Err(e) => {
            tokio::fs::remove_dir_all(&dir).await.ok();
            Err(e)
        }
    }
}

async fn create_book(
    state: &AppState,
    claims: &Claims,
    title: &str,
    author: &str,
    narrator: Option<&str>,
    files: &[SavedFile],
) -> Result<Value, AppError> {
    let mut audio: Vec<&SavedFile> = files.iter().filter(|f| is_audio(&f.name)).collect();
    if audio.is_empty() {
        return Err(AppError::BadRequest("Аудиофайлы не найдены".into()));
    }
    audio.sort_by(|a, b| natural_cmp(&a.sort_key, &b.sort_key));

    let cover = files
        .iter()
        .find(|f| is_image(&f.name) && is_cover_name(&f.name))
        .or_else(|| files.iter().find(|f| is_image(&f.name)));

    let pairs: Vec<(&str, u64)> = files.iter().map(|f| (f.fp_name.as_str(), f.size)).collect();
    let fp = fingerprint(&pairs);
    let duplicate = sqlx::query!("SELECT title FROM books WHERE fingerprint = ?", fp)
        .fetch_optional(&state.pool)
        .await?;
    if let Some(d) = duplicate {
        return Err(AppError::Conflict(format!(
            "Книга уже загружена: «{}»",
            d.title
        )));
    }

    let durations = probe_durations(&state.uploads_dir, &audio).await;
    let unknown = durations.iter().filter(|d| d.is_none()).count();
    if unknown > 0 {
        tracing::info!(
            "{} файлов без длительности — заполнятся при воспроизведении",
            unknown
        );
    }

    // Insert book and its chapters atomically
    let file_path = audio[0].rel_path.clone();
    let cover_path = cover.map(|c| c.rel_path.clone());

    let mut tx = state.pool.begin().await?;
    let book = sqlx::query!(
        "INSERT INTO books (title, author, narrator, file_path, cover_path, fingerprint, uploaded_by_id)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id AS \"id!\", created_at",
        title, author, narrator, file_path, cover_path, fp, claims.id
    )
    .fetch_one(&mut *tx)
    .await?;

    for (i, (file, duration)) in audio.iter().zip(&durations).enumerate() {
        let sort_order = i as i64;
        sqlx::query!(
            "INSERT INTO chapters (book_id, file_path, sort_order, duration_sec) VALUES (?, ?, ?, ?)",
            book.id, file.rel_path, sort_order, duration
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let known: Vec<f64> = durations.iter().flatten().copied().collect();
    let total_sec = (!known.is_empty()).then(|| known.iter().sum::<f64>());

    tracing::info!(
        "book created: id={} title={} chapters={} duration={:?}min",
        book.id,
        title,
        audio.len(),
        total_sec.map(|s| (s / 60.0) as i64)
    );

    Ok(json!({
        "id": book.id,
        "title": title,
        "author": author,
        "narrator": narrator,
        "coverPath": cover_path,
        "filePath": file_path,
        "uploadedBy": claims.name,
        "createdAt": book.created_at,
        "chaptersCount": audio.len(),
        "totalSec": total_sec,
    }))
}

/// Removes uploads that were started long ago and never finished. Run at startup.
pub async fn remove_abandoned(uploads_dir: &FsPath) {
    let Ok(mut entries) = tokio::fs::read_dir(uploads_dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let age = tokio::fs::metadata(entry.path().join(MARKER))
            .await
            .and_then(|m| m.modified())
            .ok()
            .and_then(|started| SystemTime::now().duration_since(started).ok());
        if age.is_some_and(|age| age > ABANDONED_AFTER) {
            tracing::info!("removing abandoned upload {:?}", entry.path());
            tokio::fs::remove_dir_all(entry.path()).await.ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;

    async fn start_upload(t: &testing::TestState, user: &Claims) -> String {
        let Json(res) = start(State(t.state.clone()), user.clone()).await.unwrap();
        res["uploadId"].as_str().unwrap().to_string()
    }

    async fn put(
        t: &testing::TestState,
        user: &Claims,
        id: &str,
        index: u32,
        bytes: &'static [u8],
    ) {
        put_file(
            State(t.state.clone()),
            user.clone(),
            Path((id.to_string(), index)),
            Body::from(bytes),
        )
        .await
        .unwrap();
    }

    fn file(index: u32, name: &str, size: u64) -> UploadedFile {
        UploadedFile {
            index,
            name: name.into(),
            size,
        }
    }

    fn finish_body(files: Vec<UploadedFile>) -> Json<FinishBody> {
        Json(FinishBody {
            title: "Книга".into(),
            author: "Автор".into(),
            narrator: None,
            files,
        })
    }

    #[test]
    fn unique_name_never_reuses_a_taken_name() {
        let mut used = HashSet::new();
        let mut names = Vec::new();
        for raw in ["a.mp3", "a-2.mp3", "a.mp3", "a.mp3"] {
            let name = unique_name(raw, &used);
            used.insert(name.clone());
            names.push(name);
        }
        assert_eq!(names, ["a.mp3", "a-2.mp3", "a-3.mp3", "a-4.mp3"]);
    }

    #[tokio::test]
    async fn files_sent_one_by_one_become_a_book_with_ordered_chapters() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = start_upload(&t, &user).await;

        // sent out of order, with a retry of one file and the same name in two folders
        put(&t, &user, &id, 2, b"ten").await;
        put(&t, &user, &id, 0, b"broken").await;
        put(&t, &user, &id, 0, b"one").await;
        put(&t, &user, &id, 1, b"two!").await;
        put(&t, &user, &id, 3, b"cover").await;

        let Json(book) = finish(
            State(t.state.clone()),
            user.clone(),
            Path(id.clone()),
            finish_body(vec![
                file(2, "Книга/CD1/10.mp3", 3),
                file(0, "Книга/CD1/1.mp3", 3),
                file(1, "Книга/CD2/1.mp3", 4),
                file(3, "Книга/cover.jpg", 5),
            ]),
        )
        .await
        .unwrap();

        assert_eq!(book["chaptersCount"], 3);
        assert_eq!(book["coverPath"], format!("{id}/cover.jpg"));
        let chapters: Vec<String> =
            sqlx::query_scalar("SELECT file_path FROM chapters ORDER BY sort_order")
                .fetch_all(&t.state.pool)
                .await
                .unwrap();
        assert_eq!(
            chapters,
            [
                format!("{id}/1.mp3"),
                format!("{id}/10.mp3"),
                format!("{id}/1-2.mp3")
            ]
        );
        let dir = t.state.uploads_dir.join(&id);
        assert_eq!(
            std::fs::read(dir.join("1.mp3")).unwrap(),
            b"one",
            "the retry replaced the part"
        );
        assert!(!dir.join(MARKER).exists());
        // finished: no longer an upload
        let again = cancel(State(t.state.clone()), user, Path(id)).await;
        assert!(matches!(again, Err(AppError::NotFound)));
    }

    #[tokio::test]
    async fn finish_names_the_file_that_did_not_arrive_and_keeps_the_upload() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = start_upload(&t, &user).await;
        put(&t, &user, &id, 0, b"whole").await;
        put(&t, &user, &id, 1, b"cut").await;

        let files = || vec![file(0, "a/1.mp3", 5), file(1, "a/2.mp3", 9)];
        let res = finish(
            State(t.state.clone()),
            user.clone(),
            Path(id.clone()),
            finish_body(files()),
        )
        .await;

        let Err(AppError::BadRequest(msg)) = res else {
            panic!("expected a bad request")
        };
        assert!(msg.contains("a/2.mp3") && msg.contains("3 из 9"), "{msg}");
        assert_eq!(testing::count(&t.state, "books").await, 0);

        // the client re-sends that one file and finishes
        put(&t, &user, &id, 1, b"all there").await;
        finish(State(t.state.clone()), user, Path(id), finish_body(files()))
            .await
            .unwrap();
        assert_eq!(testing::count(&t.state, "chapters").await, 2);
    }

    #[tokio::test]
    async fn a_duplicate_book_is_refused_and_its_files_removed() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let mut ids = Vec::new();
        let mut results = Vec::new();
        for _ in 0..2 {
            let id = start_upload(&t, &user).await;
            put(&t, &user, &id, 0, b"audio").await;
            let body = finish_body(vec![file(0, "b/1.mp3", 5)]);
            results
                .push(finish(State(t.state.clone()), user.clone(), Path(id.clone()), body).await);
            ids.push(id);
        }

        assert!(results[0].is_ok());
        assert!(matches!(results[1], Err(AppError::Conflict(_))));
        assert!(t.state.uploads_dir.join(&ids[0]).exists());
        assert!(!t.state.uploads_dir.join(&ids[1]).exists());
    }

    #[tokio::test]
    async fn upload_ids_cannot_point_outside_an_upload() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let book = testing::book(&t.state, &user, "Готовая", &["1.mp3"]).await;

        // a finished book's directory, a traversal, and nonsense
        for id in [
            format!("book-{book}"),
            "../book-1".to_string(),
            "book-".to_string(),
            "x".to_string(),
        ] {
            let res = cancel(State(t.state.clone()), user.clone(), Path(id.clone())).await;
            assert!(matches!(res, Err(AppError::NotFound)), "{id}");
        }
        assert!(t.state.uploads_dir.join(format!("book-{book}")).exists());
    }

    #[tokio::test]
    async fn cancel_removes_the_upload() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = start_upload(&t, &user).await;
        put(&t, &user, &id, 0, b"audio").await;

        cancel(State(t.state.clone()), user, Path(id.clone()))
            .await
            .unwrap();
        assert!(!t.state.uploads_dir.join(id).exists());
    }

    #[tokio::test]
    async fn only_old_unfinished_uploads_are_swept() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let fresh = start_upload(&t, &user).await;
        let old = start_upload(&t, &user).await;
        let book = testing::book(&t.state, &user, "Готовая", &["1.mp3"]).await;

        let two_days_ago = SystemTime::now() - Duration::from_secs(48 * 3600);
        std::fs::File::options()
            .write(true)
            .open(t.state.uploads_dir.join(&old).join(MARKER))
            .unwrap()
            .set_modified(two_days_ago)
            .unwrap();

        remove_abandoned(&t.state.uploads_dir).await;

        assert!(!t.state.uploads_dir.join(old).exists());
        assert!(t.state.uploads_dir.join(fresh).exists());
        assert!(t.state.uploads_dir.join(format!("book-{book}")).exists());
    }
}
