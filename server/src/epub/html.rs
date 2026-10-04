//! Turns an XHTML chapter into plain paragraphs.

/// Tags that end the current paragraph. Anything else (span, em, a, …) is inline
/// and must not split a word: `<span>О</span>днажды` is one word.
const BLOCK_TAGS: &[&str] = &[
    "p",
    "div",
    "br",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "ul",
    "ol",
    "blockquote",
    "section",
    "article",
    "header",
    "footer",
    "aside",
    "table",
    "tr",
    "td",
    "th",
    "dt",
    "dd",
    "pre",
    "hr",
    "figure",
    "figcaption",
    "body",
    "title",
];
/// Elements whose content is never read aloud.
const SKIP_TAGS: &[&str] = &["head", "script", "style", "svg", "math"];

/// Extract the readable text of an XHTML document, one string per block element.
pub(super) fn extract_paragraphs(html: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut skip_until: Option<String> = None; // closing tag we are waiting for
    let mut pos = 0;

    let flush = |current: &mut String, paragraphs: &mut Vec<String>| {
        let text = decode_entities(current);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            paragraphs.push(text);
        }
        current.clear();
    };

    while pos < html.len() {
        let Some(rel) = html[pos..].find('<') else {
            if skip_until.is_none() {
                current.push_str(&html[pos..]);
            }
            break;
        };
        let lt = pos + rel;
        if skip_until.is_none() {
            current.push_str(&html[pos..lt]);
        }

        // Comments and CDATA / doctype / processing instructions carry no text
        let rest = &html[lt..];
        if rest.starts_with("<!--") {
            pos = rest.find("-->").map(|e| lt + e + 3).unwrap_or(html.len());
            continue;
        }
        let Some(gt_rel) = rest.find('>') else { break };
        let tag = &rest[1..gt_rel];
        pos = lt + gt_rel + 1;

        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let name = name.rsplit(':').next().unwrap_or("").to_string();

        if let Some(waiting) = &skip_until {
            if closing && *waiting == name {
                skip_until = None;
            }
            continue;
        }
        if SKIP_TAGS.contains(&name.as_str()) {
            if !closing && !tag.ends_with('/') {
                skip_until = Some(name);
            }
            continue;
        }
        if BLOCK_TAGS.contains(&name.as_str()) {
            flush(&mut current, &mut paragraphs);
        }
    }
    flush(&mut current, &mut paragraphs);

    paragraphs
}

const NAMED_ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", " "),
    ("ensp", " "),
    ("emsp", " "),
    ("thinsp", " "),
    ("shy", ""),
    ("mdash", "—"),
    ("ndash", "–"),
    ("hellip", "…"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("ldquo", "“"),
    ("rdquo", "”"),
    ("lsquo", "‘"),
    ("rsquo", "’"),
    ("bdquo", "„"),
    ("sbquo", "‚"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("deg", "°"),
    ("times", "×"),
    ("minus", "−"),
    ("bull", "•"),
    ("middot", "·"),
    ("sect", "§"),
    ("para", "¶"),
    ("numero", "№"),
    ("euro", "€"),
    ("pound", "£"),
];

/// Decodes named and numeric character references in one pass,
/// so `&amp;lt;` stays `&lt;` rather than becoming `<`.
pub(super) fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let decoded = after
            .find(';')
            .filter(|&semi| semi > 0 && semi <= 10)
            .and_then(|semi| {
                let name = &after[..semi];
                let text = if let Some(num) = name.strip_prefix('#') {
                    let code = match num.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => num.parse::<u32>().ok()?,
                    };
                    char::from_u32(code)?.to_string()
                } else {
                    NAMED_ENTITIES
                        .iter()
                        .find(|(n, _)| *n == name)?
                        .1
                        .to_string()
                };
                Some((text, semi))
            });
        match decoded {
            Some((text, semi)) => {
                out.push_str(&text);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_from_any_block_markup() {
        let html = "<html><head><title>x</title><style>p{}</style></head><body>\
                    <h1>Глава 1</h1><div>Первый абзац.</div>\
                    <p><span>О</span>днажды &mdash; сказал он&#8230;</p>\
                    <ul><li>раз</li><li>два</li></ul>строка<br/>другая</body></html>";
        assert_eq!(
            extract_paragraphs(html),
            [
                "Глава 1",
                "Первый абзац.",
                "Однажды — сказал он…",
                "раз",
                "два",
                "строка",
                "другая"
            ]
        );
    }

    #[test]
    fn comments_and_scripts_are_not_read() {
        let html =
            "<body><!-- <p>скрыто</p> --><script>var a = '<p>x</p>';</script><p>видно</p></body>";
        assert_eq!(extract_paragraphs(html), ["видно"]);
    }

    #[test]
    fn entities_decode_once() {
        assert_eq!(
            decode_entities("a &amp;lt; b &#x41; &unknown; &"),
            "a &lt; b A &unknown; &"
        );
    }
}
