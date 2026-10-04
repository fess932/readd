//! Builds one M4B with chapter marks out of the per-chunk audio files.

use std::sync::Arc;

use super::finalize::DoneChunk;
use crate::state::AppState;

/// Escapes a value for an ffmetadata file.
fn ffmeta_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '=' | ';' | '#' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// Concatenates all chunks into a single M4B with embedded epub-chapter markers.
/// Returns its path relative to the uploads dir and its duration.
pub(super) async fn build_m4b(
    state: &Arc<AppState>,
    job_id: i64,
    chunks: &[DoneChunk],
    title: &str,
    author: &str,
) -> anyhow::Result<(String, Option<f64>)> {
    let dir_name = format!("tts-{}", job_id);
    let work_dir = state.uploads_dir.join(&dir_name);

    // Chapter boundaries: start of the first chunk of every epub chapter
    let mut chapter_marks: Vec<(u64, i64)> = Vec::new(); // (start_ms, epub_chapter_idx)
    let mut elapsed_sec = 0.0f64;
    for c in chunks {
        if chapter_marks.last().map(|&(_, idx)| idx) != Some(c.epub_chapter_idx) {
            chapter_marks.push(((elapsed_sec * 1000.0) as u64, c.epub_chapter_idx));
        }
        elapsed_sec += c.duration_sec.unwrap_or(0.0);
    }
    let total_ms = (elapsed_sec * 1000.0) as u64;

    // The concat demuxer resolves entries relative to the list file, which lives
    // next to the chunks — so bare file names work for any UPLOADS_DIR.
    let list_content: String = chunks
        .iter()
        .filter_map(|c| std::path::Path::new(&c.file_path).file_name()?.to_str())
        .map(|name| format!("file '{}'\n", name))
        .collect();
    let list_path = work_dir.join("_filelist.txt");
    tokio::fs::write(&list_path, &list_content).await?;

    // Write ffmetadata with chapter marks
    let mut meta = String::from(";FFMETADATA1\n");
    meta.push_str(&format!("title={}\n", ffmeta_escape(title)));
    meta.push_str(&format!("artist={}\n", ffmeta_escape(author)));
    meta.push_str("genre=Audiobook\n\n");

    for (i, (start_ms, epub_idx)) in chapter_marks.iter().enumerate() {
        let end_ms = chapter_marks
            .get(i + 1)
            .map(|(s, _)| *s)
            .unwrap_or(total_ms);
        meta.push_str("[CHAPTER]\nTIMEBASE=1/1000\n");
        meta.push_str(&format!("START={}\n", start_ms));
        meta.push_str(&format!("END={}\n", end_ms));
        meta.push_str(&format!("title=Глава {}\n\n", epub_idx + 1));
    }
    let meta_path = work_dir.join("_metadata.txt");
    tokio::fs::write(&meta_path, &meta).await?;

    // Single M4B for the whole book
    let m4b_rel = format!("{}/book.m4b", dir_name);
    let m4b_full = state.uploads_dir.join(&m4b_rel);

    let output = tokio::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin"])
        .args(["-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path)
        .arg("-i")
        .arg(&meta_path)
        .args([
            "-map_metadata",
            "1",
            "-map",
            "0:a",
            "-c:a",
            "aac",
            "-profile:a",
            "aac_low",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
            "-y",
        ])
        .arg(&m4b_full)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await;

    // Cleanup temp files
    tokio::fs::remove_file(&list_path).await.ok();
    tokio::fs::remove_file(&meta_path).await.ok();

    let output = output?;
    if !output.status.success() {
        tokio::fs::remove_file(&m4b_full).await.ok();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr
            .trim()
            .chars()
            .rev()
            .take(500)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        anyhow::bail!("ffmpeg: {}", tail);
    }

    // Scan duration of the resulting M4B for the player
    let m4b_path_clone = m4b_full.clone();
    let dur = tokio::task::spawn_blocking(move || crate::media::audio_duration(&m4b_path_clone))
        .await
        .ok()
        .flatten()
        .or(Some(elapsed_sec).filter(|s| *s > 0.0));

    tracing::info!(
        "M4B created: {} ({:.0?} min)",
        m4b_rel,
        dur.map(|d| d / 60.0)
    );
    Ok((m4b_rel, dur))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffmeta_escape_neutralizes_special_chars() {
        assert_eq!(ffmeta_escape(r"a=b;c#d\e"), r"a\=b\;c\#d\\e");
        assert_eq!(ffmeta_escape("line\n[CHAPTER]"), "line [CHAPTER]");
    }
}
