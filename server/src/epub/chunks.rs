//! Groups paragraphs into pieces small enough for one TTS request.

const MAX_CHUNK_BYTES: usize = 400;

pub struct TextChunk {
    /// 0-based index of the epub spine item (chapter) this chunk came from.
    pub epub_chapter_idx: usize,
    pub text: String,
}

/// Merge paragraphs into chunks of at most MAX_CHUNK_BYTES.
/// Chunks never cross epub chapter boundaries.
pub(super) fn make_chunks(chapters: Vec<(usize, Vec<String>)>) -> Vec<TextChunk> {
    let mut chunks: Vec<TextChunk> = Vec::new();

    for (chapter_idx, paragraphs) in chapters {
        let mut push = |text: &str| {
            let text = text.trim();
            if !text.is_empty() {
                chunks.push(TextChunk {
                    epub_chapter_idx: chapter_idx,
                    text: text.to_string(),
                });
            }
        };
        let mut current = String::new();

        for p in paragraphs {
            if p.len() > MAX_CHUNK_BYTES {
                push(&current);
                current.clear();
                split_long(&p).iter().for_each(|sub| push(sub));
            } else if current.is_empty() {
                current = p;
            } else if current.len() + 1 + p.len() > MAX_CHUNK_BYTES {
                push(&current);
                current = p;
            } else {
                current.push('\n');
                current.push_str(&p);
            }
        }
        push(&current);
    }

    chunks
}

/// Split a paragraph that exceeds MAX_CHUNK_BYTES: at a sentence end once the piece
/// is at least half full, otherwise at the last space, otherwise mid-word.
fn split_long(text: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        current.push(c);
        let sentence_end = matches!(c, '.' | '?' | '!' | '…');

        if sentence_end && current.len() >= MAX_CHUNK_BYTES / 2 {
            pieces.push(std::mem::take(&mut current));
        } else if current.len() >= MAX_CHUNK_BYTES {
            let tail = match current.rfind(' ') {
                Some(space) => current.split_off(space),
                None => String::new(),
            };
            pieces.push(std::mem::replace(
                &mut current,
                tail.trim_start().to_string(),
            ));
        }
    }
    pieces.push(current);

    pieces.retain(|p| !p.trim().is_empty());
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(chapters: Vec<(usize, Vec<&str>)>) -> Vec<(usize, String)> {
        let chapters = chapters
            .into_iter()
            .map(|(i, ps)| (i, ps.into_iter().map(String::from).collect()))
            .collect();
        make_chunks(chapters)
            .into_iter()
            .map(|c| (c.epub_chapter_idx, c.text))
            .collect()
    }

    #[test]
    fn short_paragraphs_are_merged_within_a_chapter() {
        assert_eq!(
            texts(vec![(0, vec!["one", "two"]), (3, vec!["three"])]),
            [(0, "one\ntwo".to_string()), (3, "three".to_string())]
        );
    }

    #[test]
    fn chunks_respect_the_size_limit() {
        let sentence = "Это довольно длинное предложение для проверки. ";
        let chunks = texts(vec![(0, vec![&sentence.repeat(40)])]);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|(_, t)| t.len() <= MAX_CHUNK_BYTES));
    }

    #[test]
    fn no_text_is_lost_and_no_chunk_is_empty() {
        let text = format!("{} {}", "a".repeat(150), "b".repeat(900));
        let chunks = texts(vec![(0, vec![&text])]);
        assert!(chunks.iter().all(|(_, t)| !t.is_empty()));
        let joined: String = chunks.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(joined.len(), text.len() - 1); // only the split space is dropped
    }
}
