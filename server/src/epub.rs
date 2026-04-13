/// Extracts text chunks from an epub file.
/// Each chunk is a group of consecutive paragraphs totalling ~MAX_CHUNK_CHARS characters.
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_CHUNK_CHARS: usize = 400;

pub struct TextChunk {
    /// 0-based index of the epub spine item (chapter) this chunk came from.
    pub epub_chapter_idx: usize,
    pub text: String,
}

/// Returns ordered text chunks with their epub chapter index.
pub fn extract_chunks(epub_path: &Path) -> anyhow::Result<Vec<TextChunk>> {
    let file = std::fs::File::open(epub_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let opf_path = find_opf_path(&mut archive)?;
    let opf_dir = PathBuf::from(&opf_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    let spine_hrefs = parse_opf_spine(&mut archive, &opf_path)?;

    // chapter_idx → paragraphs
    let mut chapter_paragraphs: Vec<(usize, Vec<String>)> = Vec::new();
    for (chapter_idx, href) in spine_hrefs.iter().enumerate() {
        let full_path = if opf_dir.is_empty() {
            href.clone()
        } else {
            format!("{}/{}", opf_dir, href)
        };
        let full_path = full_path.replace("//", "/");

        if let Ok(mut entry) = archive.by_name(&full_path) {
            let mut html = String::new();
            entry.read_to_string(&mut html).ok();
            let paragraphs = extract_paragraphs(&html);
            if !paragraphs.is_empty() {
                chapter_paragraphs.push((chapter_idx, paragraphs));
            }
        }
    }

    Ok(make_chunks(chapter_paragraphs))
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn find_opf_path(archive: &mut zip::ZipArchive<std::fs::File>) -> anyhow::Result<String> {
    let mut container = String::new();
    archive
        .by_name("META-INF/container.xml")?
        .read_to_string(&mut container)?;

    // Find: full-path="path/to/content.opf"
    let marker = "full-path=\"";
    let start = container
        .find(marker)
        .ok_or_else(|| anyhow::anyhow!("full-path not found in container.xml"))?;
    let rest = &container[start + marker.len()..];
    let end = rest
        .find('"')
        .ok_or_else(|| anyhow::anyhow!("malformed full-path in container.xml"))?;
    Ok(rest[..end].to_string())
}

fn parse_opf_spine(
    archive: &mut zip::ZipArchive<std::fs::File>,
    opf_path: &str,
) -> anyhow::Result<Vec<String>> {
    let mut opf = String::new();
    archive.by_name(opf_path)?.read_to_string(&mut opf)?;

    // Build id→href map from <manifest>
    let mut id_to_href: HashMap<String, String> = HashMap::new();
    let mut pos = 0;
    while let Some(rel) = opf[pos..].find("<item ").or_else(|| opf[pos..].find("<item\t")) {
        let start = pos + rel;
        let chunk = &opf[start..];
        let end = chunk.find('>').map(|e| start + e + 1).unwrap_or(opf.len());
        let item = &opf[start..end];

        if let (Some(id), Some(href)) = (attr_val(item, "id"), attr_val(item, "href")) {
            let media = attr_val(item, "media-type").unwrap_or_default();
            if media.contains("xhtml") || media.contains("html") || href.ends_with(".xhtml") || href.ends_with(".html") || href.ends_with(".htm") {
                id_to_href.insert(id, url_decode(&href));
            }
        }
        pos = end;
    }

    // Walk <spine> itemrefs in order
    let mut hrefs: Vec<String> = Vec::new();
    let mut pos = 0;
    while let Some(rel) = opf[pos..].find("<itemref ").or_else(|| opf[pos..].find("<itemref\t")) {
        let start = pos + rel;
        let chunk = &opf[start..];
        let end = chunk.find('>').map(|e| start + e + 1).unwrap_or(opf.len());
        let item = &opf[start..end];

        if let Some(idref) = attr_val(item, "idref") {
            if let Some(href) = id_to_href.get(&idref) {
                hrefs.push(href.clone());
            }
        }
        pos = end;
    }

    Ok(hrefs)
}

/// Extract `attr="..."` value from a tag string.
fn attr_val(s: &str, attr: &str) -> Option<String> {
    // Try attr="..." then attr='...'
    for quote in ['"', '\''] {
        let marker = format!("{}={}", attr, quote);
        if let Some(start) = s.find(&marker) {
            let rest = &s[start + marker.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                result.push(char::from(h * 16 + l));
                i += 3;
                continue;
            }
        }
        result.push(char::from(bytes[i]));
        i += 1;
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Extract non-empty text content from <p>…</p> blocks.
fn extract_paragraphs(html: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;

    while pos < lower.len() {
        // Find next <p or <p> or <p ...>
        let Some(rel) = lower[pos..].find("<p") else { break };
        let p_start = pos + rel;

        // Confirm it's <p> not <pre>, <progress>, etc.
        let after = lower.as_bytes().get(p_start + 2).copied().unwrap_or(0);
        if after != b'>' && after != b' ' && after != b'\t' && after != b'\n' && after != b'\r' && after != b'/' {
            pos = p_start + 2;
            continue;
        }

        // Find end of opening tag
        let Some(tag_end_rel) = lower[p_start..].find('>') else { break };
        let content_start = p_start + tag_end_rel + 1;

        // Self-closing <p/> has nothing
        if lower[p_start..p_start + tag_end_rel].ends_with('/') {
            pos = content_start;
            continue;
        }

        // Find </p>
        let Some(close_rel) = lower[content_start..].find("</p>") else {
            pos = content_start;
            continue;
        };
        let content_end = content_start + close_rel;

        let text = strip_html_and_entities(&html[content_start..content_end]);
        let text = text.trim().to_string();
        if !text.is_empty() {
            paragraphs.push(text);
        }

        pos = content_end + 4; // skip </p>
    }

    paragraphs
}

fn strip_html_and_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;

    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }

    // Decode common HTML entities
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ");

    // Collapse whitespace
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Merge paragraphs into chunks of at most MAX_CHUNK_CHARS.
/// Chunks never cross epub chapter boundaries.
fn make_chunks(chapters: Vec<(usize, Vec<String>)>) -> Vec<TextChunk> {
    let mut chunks: Vec<TextChunk> = Vec::new();

    for (chapter_idx, paragraphs) in chapters {
        let mut current = String::new();

        for p in paragraphs {
            if p.is_empty() {
                continue;
            }

            if p.len() > MAX_CHUNK_CHARS {
                // Flush before splitting
                if !current.trim().is_empty() {
                    chunks.push(TextChunk { epub_chapter_idx: chapter_idx, text: current.trim().to_string() });
                    current = String::new();
                }
                for sub in split_long(&p) {
                    chunks.push(TextChunk { epub_chapter_idx: chapter_idx, text: sub });
                }
                continue;
            }

            let sep_len = if current.is_empty() { 0 } else { 1 };
            if !current.is_empty() && current.len() + sep_len + p.len() > MAX_CHUNK_CHARS {
                chunks.push(TextChunk { epub_chapter_idx: chapter_idx, text: current.trim().to_string() });
                current = p;
            } else {
                if !current.is_empty() { current.push('\n'); }
                current.push_str(&p);
            }
        }

        if !current.trim().is_empty() {
            chunks.push(TextChunk { epub_chapter_idx: chapter_idx, text: current.trim().to_string() });
        }
    }

    chunks
}

/// Split a paragraph that exceeds MAX_CHUNK_CHARS at sentence boundaries.
fn split_long(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        current.push(c);
        if (c == '.' || c == '?' || c == '!' || c == '…') && current.len() >= MAX_CHUNK_CHARS / 2 {
            result.push(current.trim().to_string());
            current = String::new();
        } else if current.len() >= MAX_CHUNK_CHARS {
            // No sentence break found — hard cut at word boundary
            if let Some(space) = current.rfind(' ') {
                let tail = current[space + 1..].to_string();
                result.push(current[..space].trim().to_string());
                current = tail;
            } else {
                result.push(current.trim().to_string());
                current = String::new();
            }
        }
    }

    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }

    result
}
