//! Extracts the readable text of an epub as chunks sized for the TTS server.

use std::{
    io::Read,
    path::{Path, PathBuf},
};

mod chunks;
mod html;
mod opf;

pub use chunks::TextChunk;

type Archive = zip::ZipArchive<std::fs::File>;

/// Returns ordered text chunks with their epub chapter index.
pub fn extract_chunks(epub_path: &Path) -> anyhow::Result<Vec<TextChunk>> {
    let file = std::fs::File::open(epub_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let opf_path = opf::find_opf_path(&mut archive)?;
    let opf_dir = PathBuf::from(&opf_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    let spine_hrefs = opf::parse_opf_spine(&mut archive, &opf_path)?;

    // chapter_idx → paragraphs
    let mut chapter_paragraphs: Vec<(usize, Vec<String>)> = Vec::new();
    for (chapter_idx, href) in spine_hrefs.iter().enumerate() {
        let full_path = opf::resolve_path(&opf_dir, href);

        match read_entry(&mut archive, &full_path) {
            Ok(html) => {
                let paragraphs = html::extract_paragraphs(&html);
                if !paragraphs.is_empty() {
                    chapter_paragraphs.push((chapter_idx, paragraphs));
                }
            }
            Err(e) => tracing::warn!("epub: spine item {:?} skipped: {}", full_path, e),
        }
    }

    Ok(chunks::make_chunks(chapter_paragraphs))
}

/// Reads a zip entry as text, honouring a BOM or the encoding declared in the XML prolog.
fn read_entry(archive: &mut Archive, name: &str) -> anyhow::Result<String> {
    let mut bytes = Vec::new();
    archive.by_name(name)?.read_to_end(&mut bytes)?;
    Ok(decode_text(&bytes))
}

fn decode_text(bytes: &[u8]) -> String {
    if let Some((encoding, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        return encoding
            .decode_without_bom_handling(&bytes[bom_len..])
            .0
            .into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // Not UTF-8: look for encoding="…" in the first bytes (the prolog is ASCII)
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
    let encoding = opf::attr_val(&head, "encoding")
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .unwrap_or(encoding_rs::WINDOWS_1251);
    encoding.decode(bytes).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_utf8_text_is_decoded() {
        let xml = "<?xml version=\"1.0\" encoding=\"windows-1251\"?><p>Привет</p>";
        let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode(xml);
        assert!(decode_text(&bytes).contains("Привет"));
    }

    #[test]
    fn utf16_with_bom_is_decoded() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend("Привет".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode_text(&bytes), "Привет");
    }
}
