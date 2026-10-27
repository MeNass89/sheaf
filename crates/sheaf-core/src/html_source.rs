//! Decode the local source and keep its base address when printing a temporary HTML copy.

use crate::{OpdfError, Result};
use std::ops::Range;

struct Attribute<'a> {
    name: &'a str,
    value: Range<usize>,
}

/// Only attributes of an actual tag (never strings in scripts/comments) reach this parser.
fn attributes(tag: &str) -> Vec<Attribute<'_>> {
    let bytes = tag.as_bytes();
    let mut at = 1;
    while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'>' { at += 1; }
    let mut attrs = Vec::new();
    while at < bytes.len() {
        while at < bytes.len() && (bytes[at].is_ascii_whitespace() || bytes[at] == b'/') { at += 1; }
        let start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() && !b"=>/".contains(&bytes[at]) { at += 1; }
        let name = &tag[start..at];
        if name.is_empty() { break; }
        while at < bytes.len() && bytes[at].is_ascii_whitespace() { at += 1; }
        if bytes.get(at) != Some(&b'=') { continue; }
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() { at += 1; }
        let quote = bytes.get(at).copied().filter(|b| *b == b'\'' || *b == b'"');
        if quote.is_some() { at += 1; }
        let start = at;
        while at < bytes.len() && match quote {
            Some(q) => bytes[at] != q,
            None => !bytes[at].is_ascii_whitespace() && bytes[at] != b'>',
        } { at += 1; }
        attrs.push(Attribute { name, value: start..at });
        if quote.is_some() { at += 1; }
    }
    attrs
}

fn charset(tag: &str) -> Option<&str> {
    let attrs = attributes(tag);
    if let Some(a) = attrs.iter().find(|a| a.name.eq_ignore_ascii_case("charset")) {
        return Some(tag[a.value.clone()].trim());
    }
    let http = attrs.iter().find(|a| a.name.eq_ignore_ascii_case("http-equiv"))?;
    if !tag[http.value.clone()].eq_ignore_ascii_case("content-type") { return None; }
    let content = attrs.iter().find(|a| a.name.eq_ignore_ascii_case("content"))?;
    let content = &tag[content.value.clone()];
    let lower = content.to_ascii_lowercase();
    let start = lower.find("charset")? + 7;
    let rest = content[start..].trim_start().strip_prefix('=')?.trim_start().trim_start_matches(['\'', '"']);
    rest.split([';', ' ', '\'', '"']).next()
}

pub(super) fn decode(bytes: &[u8]) -> Result<String> {
    if let Some((encoding, skip)) = encoding_rs::Encoding::for_bom(bytes) {
        return decode_as(encoding, &bytes[skip..]);
    }
    let sniff = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    let lower = sniff.to_ascii_lowercase();
    let mut from = 0;
    let mut declared = None;
    while let Some(span) = super::tag_span(&lower[from..], "meta") {
        let tag = &sniff[from + span.start..from + span.end];
        if let Some(label) = charset(tag) {
            declared = Some(encoding_rs::Encoding::for_label_no_replacement(label.as_bytes())
                .ok_or_else(|| OpdfError::Unsupported(format!("Unsupported HTML encoding {label}. Choose Original layout")))?);
            break;
        }
        from += span.end;
    }
    let encoding = declared.unwrap_or(if std::str::from_utf8(bytes).is_ok() { encoding_rs::UTF_8 } else { encoding_rs::WINDOWS_1252 });
    decode_as(encoding, bytes)
}

fn decode_as(encoding: &'static encoding_rs::Encoding, bytes: &[u8]) -> Result<String> {
    encoding.decode_without_bom_handling_and_without_replacement(bytes).map(|s| s.into_owned())
        .ok_or_else(|| OpdfError::Unsupported("The HTML encoding contains malformed text. Choose Original layout".into()))
}

fn base_attribute(source: &str) -> Option<Range<usize>> {
    let lower = source.to_ascii_lowercase();
    let mut from = 0;
    while let Some(span) = super::tag_span(&lower[from..], "base") {
        let start = from + span.start;
        let end = from + span.end;
        if let Some(a) = attributes(&source[start..end]).into_iter().find(|a| a.name.eq_ignore_ascii_case("href")) {
            return Some(start + a.value.start..start + a.value.end);
        }
        from = end;
    }
    None
}

pub(super) fn has_base(source: &str) -> bool { base_attribute(source).is_some() }

/// Resolve a URI reference against the original file URL, preserving already escaped HTML.
fn resolve(base: &str, href: &str) -> String {
    let href = href.trim();
    let first = href.split(['/', '?', '#']).next().unwrap_or_default();
    if first.contains(':') { return href.into(); }
    if href.starts_with("//") { return format!("file:{href}"); }
    if href.is_empty() || href.starts_with('#') || href.starts_with('?') { return format!("{base}{href}"); }
    if let Some(rest) = href.strip_prefix('/') {
        // A Windows file URL's root-relative reference keeps its drive.
        let drive = base.strip_prefix("file:///").filter(|s| s.as_bytes().get(1) == Some(&b':')).and_then(|s| s.get(..2)).unwrap_or_default();
        return format!("file:///{drive}{}", if drive.is_empty() { rest } else { href });
    }
    let directory = base.rsplit_once('/').map_or(base, |(dir, _)| dir);
    format!("{directory}/{href}")
}

pub(super) fn prepare(source: &str, original: &str) -> Result<String> {
    let mut out = source.to_string();
    let mut replacements = Vec::new();
    let lower = source.to_ascii_lowercase();
    let mut from = 0;
    // The bytes written below are UTF-8: old encoding declarations must not decode them again.
    while let Some(span) = super::tag_span(&lower[from..], "meta") {
        let span = from + span.start..from + span.end;
        if charset(&source[span.clone()]).is_some() {
            replacements.push((span.clone(), "<meta charset=\"utf-8\">".to_string()));
        }
        from = span.end;
    }
    if let Some(span) = base_attribute(source) {
        let href = &source[span.clone()];
        if href.contains("&#") {
            return Err(OpdfError::Unsupported("This base address uses numeric HTML entities. Choose Original layout to preserve it".into()));
        }
        replacements.push((span, resolve(original, href)));
    }
    replacements.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    for (span, value) in replacements { out.replace_range(span, &value); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_encodings_keep_accents_and_normalize_the_declaration() {
        let latin = b"<html><head><META charset='windows-1252'></head><body>caf\xe9 \x80</body></html>";
        let text = decode(latin).unwrap();
        assert!(text.contains("café €"));
        let text = prepare(&text, "file:///tmp/page.html").unwrap();
        assert!(text.contains("<meta charset=\"utf-8\">"));
        assert!(!text.contains("windows-1252"));
        let utf16: Vec<u8> = [0xff, 0xfe].into_iter().chain("<p>日本語</p>".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        assert_eq!(decode(&utf16).unwrap(), "<p>日本語</p>");
        assert!(decode(&[0xff, 0xfe, 0x00]).is_err());
        assert!(decode(b"<meta name='description' content='charset=madeup'><p>caf\xc3\xa9</p>").unwrap().contains("café"));
        assert!(decode(b"\xef\xbb\xbf<meta charset=madeup><p>caf\xc3\xa9</p>").unwrap().contains("café"));
    }

    #[test]
    fn html_base_uses_the_original_file_and_keeps_target_and_escaped_queries() {
        let source = "<!-- <base href='bad'> --><head><base target='_blank' href='../assets/?a=1&amp;b=2'></head><img src='plot.png'>";
        let text = prepare(source, "file:///tmp/source/page.html").unwrap();
        assert!(text.contains("target='_blank' href='file:///tmp/source/../assets/?a=1&amp;b=2'"));
        assert!(text.contains("<!-- <base href='bad'> -->"));
        assert_eq!(resolve("file:///C:/docs/page.html", "/assets/"), "file:///C:/assets/");
        assert_eq!(resolve("file:///tmp/page.html", "/assets/"), "file:///assets/");
        assert_eq!(resolve("file:///tmp/page.html", "https://example.org/assets/"), "https://example.org/assets/");
    }
}
