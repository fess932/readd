//! Files under the uploads directory: naming, ordering and per-book directories.

use axum::extract::multipart::{Field, Multipart};
use futures_util::TryStreamExt;
use std::{
    cmp::Ordering,
    path::{Path as FsPath, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::io::StreamReader;

use crate::error::AppError;

const AUDIO_EXT: &[&str] = &["mp3", "m4a", "m4b", "ogg", "flac", "wav", "aac", "opus"];
pub const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "avif"];
const COVER_NAMES: &[&str] = &["cover", "folder", "front", "artwork", "thumb"];

fn ext_lower(name: &str) -> Option<String> {
    FsPath::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
}

pub fn is_audio(name: &str) -> bool {
    ext_lower(name).is_some_and(|e| AUDIO_EXT.contains(&e.as_str()))
}

pub fn is_image(name: &str) -> bool {
    ext_lower(name).is_some_and(|e| IMAGE_EXT.contains(&e.as_str()))
}

pub fn is_cover_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    COVER_NAMES.iter().any(|prefix| lower.starts_with(prefix))
}

// Transliteration table for Cyrillic → Latin
const CYR: &[(char, &str)] = &[
    ('а', "a"),
    ('б', "b"),
    ('в', "v"),
    ('г', "g"),
    ('д', "d"),
    ('е', "e"),
    ('ё', "yo"),
    ('ж', "zh"),
    ('з', "z"),
    ('и', "i"),
    ('й', "y"),
    ('к', "k"),
    ('л', "l"),
    ('м', "m"),
    ('н', "n"),
    ('о', "o"),
    ('п', "p"),
    ('р', "r"),
    ('с', "s"),
    ('т', "t"),
    ('у', "u"),
    ('ф', "f"),
    ('х', "kh"),
    ('ц', "ts"),
    ('ч', "ch"),
    ('ш', "sh"),
    ('щ', "shch"),
    ('ъ', ""),
    ('ы', "y"),
    ('ь', ""),
    ('э', "e"),
    ('ю', "yu"),
    ('я', "ya"),
];

// Longest stem we keep, so that transliterated names stay under the 255-byte file name limit
const MAX_STEM_LEN: usize = 120;

pub fn sanitize_filename(raw: &str) -> String {
    let name = FsPath::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let path = FsPath::new(name);

    let ext: String = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            e.to_lowercase()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect()
        })
        .unwrap_or_default();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);

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
    result.truncate(MAX_STEM_LEN); // ASCII only at this point
    let result = result.trim_end_matches('-');
    let result = if result.is_empty() { "file" } else { result };
    if ext.is_empty() {
        result.to_string()
    } else {
        format!("{}.{}", result, ext)
    }
}

/// Orders strings so that embedded numbers compare by value: "2" < "10".
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn take_number(it: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut n = String::new();
        while let Some(c) = it.peek().copied().filter(|c| c.is_ascii_digit()) {
            n.push(c);
            it.next();
        }
        n
    }

    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = take_number(&mut ai);
                let nb = take_number(&mut bi);
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                ai.next();
                bi.next();
            }
        }
    }
}

/// Removes a freshly created upload directory unless the upload reached the database.
/// Covers every early return, and a client that drops the connection mid-upload.
pub struct DirGuard {
    path: PathBuf,
    keep: bool,
}

impl DirGuard {
    pub(super) fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// Creates `<uploads>/<prefix>-<millis>`, bumping the number if the name is taken.
pub async fn create_upload_dir(
    uploads_dir: &FsPath,
    prefix: &str,
) -> std::io::Result<(String, DirGuard)> {
    let mut ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    loop {
        let name = format!("{prefix}-{ts}");
        let path = uploads_dir.join(&name);
        match tokio::fs::create_dir(&path).await {
            Ok(()) => return Ok((name, DirGuard { path, keep: false })),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => ts += 1,
            Err(e) => return Err(e),
        }
    }
}

/// Streams a multipart file field to `path` and returns its size.
/// `tokio::io::copy` pulls only when the write side is ready, so memory use stays flat.
pub async fn save_field(field: Field<'_>, path: &FsPath) -> Result<u64, AppError> {
    let mut out = tokio::fs::File::create(path).await?;
    let stream =
        field.map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string()));
    let mut reader = StreamReader::new(stream);
    tokio::io::copy(&mut reader, &mut out)
        .await
        .map_err(|e| AppError::BadRequest(format!("загрузка прервана: {e}")))
}

/// Reads the `cover` multipart field and stores it in `book_dir` under a new unique name,
/// so that the URL changes and browsers do not show the cached old image.
pub async fn save_cover(
    uploads_dir: &FsPath,
    book_dir: &str,
    mut multipart: Multipart,
) -> Result<String, AppError> {
    if book_dir.is_empty() {
        return Err(AppError::BadRequest(
            "у книги нет каталога с файлами".into(),
        ));
    }

    while let Some(field) = multipart.next_field().await? {
        if field.name() != Some("cover") {
            continue;
        }

        let ext =
            ext_lower(field.file_name().unwrap_or("cover.jpg")).unwrap_or_else(|| "jpg".into());
        if !IMAGE_EXT.contains(&ext.as_str()) {
            return Err(AppError::BadRequest("не изображение".into()));
        }

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let cover_filename = format!("cover-{ts}.{ext}");
        let dir = uploads_dir.join(book_dir);
        tokio::fs::create_dir_all(&dir).await?;

        let data = field.bytes().await?;
        tokio::fs::write(dir.join(&cover_filename), &data).await?;

        return Ok(format!("{book_dir}/{cover_filename}"));
    }

    Err(AppError::BadRequest("поле cover не найдено".into()))
}

/// Deletes the cover file that `save_cover` has just replaced.
pub async fn remove_old_cover(uploads_dir: &FsPath, old: Option<&str>, new: &str) {
    let Some(old) = old.filter(|o| *o != new) else {
        return;
    };
    let is_ours = FsPath::new(old)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("cover"));
    if is_ours {
        tokio::fs::remove_file(uploads_dir.join(old)).await.ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_transliterates_and_keeps_extension() {
        assert_eq!(sanitize_filename("Глава 1.MP3"), "glava-1.mp3");
        assert_eq!(sanitize_filename("CD1/01 - Intro.mp3"), "01-intro.mp3");
        assert_eq!(sanitize_filename("???.mp3"), "file.mp3");
        assert_eq!(sanitize_filename("noext"), "noext");
    }

    #[test]
    fn sanitize_survives_extensions_that_change_length_when_lowercased() {
        assert_eq!(sanitize_filename("x.\u{212A}"), "x.k"); // Kelvin sign
        assert_eq!(sanitize_filename("a\u{e9}.\u{130}"), "a.i"); // İ lowercases to two chars
    }

    #[test]
    fn sanitize_is_idempotent() {
        let once = sanitize_filename("Книга — Том 2.m4b");
        assert_eq!(sanitize_filename(&once), once);
    }

    #[test]
    fn sanitize_caps_the_length() {
        let long = format!("{}.mp3", "щ".repeat(200));
        assert!(sanitize_filename(&long).len() <= MAX_STEM_LEN + 4);
    }

    #[test]
    fn natural_order_sorts_numbers_by_value() {
        let mut v = vec![
            "10.mp3",
            "2.mp3",
            "1.mp3",
            "cd2/01.mp3",
            "cd1/02.mp3",
            "cd1/01.mp3",
        ];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            v,
            [
                "1.mp3",
                "2.mp3",
                "10.mp3",
                "cd1/01.mp3",
                "cd1/02.mp3",
                "cd2/01.mp3"
            ]
        );
    }

    #[test]
    fn image_and_audio_detection_ignores_case() {
        assert!(is_audio("TRACK.MP3"));
        assert!(is_image("Cover.JPG"));
        assert!(is_cover_name("Folder.jpg"));
        assert!(!is_audio("notes.txt"));
    }

    #[tokio::test]
    async fn upload_dir_is_removed_unless_kept() {
        let root = tempfile::tempdir().unwrap();

        let (dropped, guard) = create_upload_dir(root.path(), "book").await.unwrap();
        drop(guard);
        assert!(!root.path().join(&dropped).exists());

        let (kept, guard) = create_upload_dir(root.path(), "book").await.unwrap();
        guard.keep();
        assert!(root.path().join(&kept).exists());
    }
}
