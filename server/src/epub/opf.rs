//! The package document: which files make up the book and in what order.

use std::collections::HashMap;

use super::{Archive, html::decode_entities, read_entry};

/// Joins an href from the OPF with the OPF directory, resolving `.` and `..`
/// and dropping a `#fragment`.
pub(super) fn resolve_path(base_dir: &str, href: &str) -> String {
    let href = href.split('#').next().unwrap_or(href);
    let mut parts: Vec<&str> = Vec::new();
    if !href.starts_with('/') {
        parts.extend(base_dir.split('/').filter(|p| !p.is_empty()));
    }
    for seg in href.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

pub(super) fn find_opf_path(archive: &mut Archive) -> anyhow::Result<String> {
    let container = read_entry(archive, "META-INF/container.xml")?;
    attr_val(&container, "full-path")
        .ok_or_else(|| anyhow::anyhow!("full-path not found in container.xml"))
}

/// Calls `f` with every start tag whose local name (namespace prefix dropped) is `name`.
fn for_each_tag(xml: &str, name: &str, mut f: impl FnMut(&str)) {
    let mut pos = 0;
    while let Some(rel) = xml[pos..].find('<') {
        let start = pos + rel;
        let end = xml[start..]
            .find('>')
            .map(|e| start + e + 1)
            .unwrap_or(xml.len());
        let tag = &xml[start..end];

        let tag_name: &str = tag[1..]
            .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .next()
            .unwrap_or("");
        let local = tag_name.rsplit(':').next().unwrap_or(tag_name);
        if local.eq_ignore_ascii_case(name) {
            f(tag);
        }
        pos = end;
    }
}

pub(super) fn parse_opf_spine(
    archive: &mut Archive,
    opf_path: &str,
) -> anyhow::Result<Vec<String>> {
    let opf = read_entry(archive, opf_path)?;

    // Build id→href map from <manifest>
    let mut id_to_href: HashMap<String, String> = HashMap::new();
    for_each_tag(&opf, "item", |item| {
        if let (Some(id), Some(href)) = (attr_val(item, "id"), attr_val(item, "href")) {
            let media = attr_val(item, "media-type").unwrap_or_default();
            let lower = href.to_ascii_lowercase();
            if media.contains("html")
                || lower.ends_with(".xhtml")
                || lower.ends_with(".html")
                || lower.ends_with(".htm")
            {
                id_to_href.insert(id, url_decode(&decode_entities(&href)));
            }
        }
    });

    // Walk <spine> itemrefs in order
    let mut hrefs: Vec<String> = Vec::new();
    for_each_tag(&opf, "itemref", |item| {
        if let Some(href) = attr_val(item, "idref").and_then(|idref| id_to_href.get(&idref)) {
            hrefs.push(href.clone());
        }
    });

    Ok(hrefs)
}

/// Extract `attr="..."` value from a tag string.
pub(super) fn attr_val(s: &str, attr: &str) -> Option<String> {
    // Try attr="..." then attr='...'
    for quote in ['"', '\''] {
        let marker = format!("{}={}", attr, quote);
        let mut from = 0;
        while let Some(rel) = s[from..].find(&marker) {
            let start = from + rel;
            // must be a whole attribute name: `id=` but not `xml:id=` or `data-id=`
            let standalone = s[..start]
                .chars()
                .next_back()
                .is_none_or(|c| c.is_whitespace());
            if standalone {
                let rest = &s[start + marker.len()..];
                if let Some(end) = rest.find(quote) {
                    return Some(rest[..end].to_string());
                }
            }
            from = start + marker.len();
        }
    }
    None
}

/// Percent-decodes a manifest href; the bytes are UTF-8, as zip entry names are.
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_decode_handles_utf8_names() {
        assert_eq!(
            url_decode("%D0%93%D0%BB%D0%B0%D0%B2%D0%B01.xhtml"),
            "Глава1.xhtml"
        );
        assert_eq!(url_decode("Глава 1.xhtml"), "Глава 1.xhtml");
        assert_eq!(url_decode("a%20b"), "a b");
        assert_eq!(url_decode("100%"), "100%");
    }

    #[test]
    fn resolve_path_normalizes() {
        assert_eq!(
            resolve_path("OEBPS", "Text/ch1.xhtml"),
            "OEBPS/Text/ch1.xhtml"
        );
        assert_eq!(
            resolve_path("OEBPS/content", "../Text/ch1.xhtml#p1"),
            "OEBPS/Text/ch1.xhtml"
        );
        assert_eq!(resolve_path("", "ch1.xhtml"), "ch1.xhtml");
    }

    #[test]
    fn tags_are_found_with_namespace_and_newlines() {
        let opf = "<opf:manifest><opf:item\n id=\"c1\" href=\"a.xhtml\"/>\
                   <item id='c2' href='b.html'/></opf:manifest>\
                   <spine><itemref idref=\"c2\"/><opf:itemref idref=\"c1\"/></spine>";
        let mut ids = Vec::new();
        for_each_tag(opf, "item", |t| ids.push(attr_val(t, "id").unwrap()));
        assert_eq!(ids, ["c1", "c2"]);
        let mut refs = Vec::new();
        for_each_tag(opf, "itemref", |t| refs.push(attr_val(t, "idref").unwrap()));
        assert_eq!(refs, ["c2", "c1"]);
    }

    #[test]
    fn attr_val_matches_whole_attribute_names_only() {
        let tag = "<item xml:id=\"x\" data-id=\"y\" id=\"real\"/>";
        assert_eq!(attr_val(tag, "id").as_deref(), Some("real"));
        assert_eq!(attr_val(tag, "href"), None);
    }
}
