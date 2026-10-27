//! Local HTML printing with iLovePDF's paper, orientation and margin controls.

use crate::{office, OpdfError, Result};
use std::path::{Path, PathBuf};

#[path = "html_source.rs"]
mod source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    A4,
    Letter,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub paper: Paper,
    pub landscape: bool,
    /// White space inside the paper, in points.
    pub margin: f64,
}

/// Print a local HTML file using an explicit paper layout. The caller must bypass any cached
/// default-layout conversion: it represents different page breaks.
pub fn to_pdf(input: &Path, options: Options, out: &Path) -> Result<PathBuf> {
    match crate::textdocs::kind(input) {
        Some("postscript") => return Err(OpdfError::Unsupported("PostScript has its own page layout. Choose Original layout to preserve it".into())),
        Some(_) => return crate::textdocs::to_pdf(input, out, |html, out| to_pdf(html, options, out)),
        None => {}
    }
    let source = source::decode(&std::fs::read(input)?)?;
    // The original document URL also preserves fragment and query references, not only images.
    let absolute = if cfg!(windows) { std::path::absolute(input)? } else { std::fs::canonicalize(input)? };
    let html = print_html(&source, &office::file_url(&absolute), options)?;
    let dir = tempfile::tempdir()?;
    let prepared = dir.path().join("print.html");
    std::fs::write(&prepared, html)?;
    office::office_to_pdf(&prepared, out)
}

fn print_html(source: &str, base: &str, options: Options) -> Result<String> {
    if !options.margin.is_finite() || !(0.0..=144.0).contains(&options.margin) {
        return Err(OpdfError::Unsupported("HTML margin must be between 0 and 144 points".into()));
    }
    let (w, h) = match options.paper {
        Paper::A4 => (595.2756, 841.8898),
        Paper::Letter => (612.0, 792.0),
    };
    let (w, h) = if options.landscape { (h, w) } else { (w, h) };
    let prepared = source::prepare(source, base)?;
    let source = prepared.as_str();
    let lower = source.to_ascii_lowercase();
    let base = base.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
    let base = if source::has_base(source) { String::new() } else { format!("<base href=\"{base}\">") };
    let style = format!("<style>@page {{ size: {w:.4}pt {h:.4}pt; margin: {:.4}pt; }}</style>", options.margin);
    // Put the base before existing relative resources; the page rule after existing styles.
    if let Some(head) = opening_tag(&lower, "head") {
        let end = tag_span(&lower[head..], "/head").map(|span| head + span.start).unwrap_or(head);
        return Ok(format!("{}<meta charset=\"utf-8\">{}{}{}{}", &source[..head], base, &source[head..end], style, &source[end..]));
    }
    let head = format!("<head><meta charset=\"utf-8\">{base}{style}</head>");
    // A fragment gets a document; an existing doctype stays at the start of the document.
    let at = opening_tag(&lower, "html")
        .or_else(|| lower.find("<!doctype").and_then(|p| lower[p..].find('>').map(|end| p + end + 1)))
        .unwrap_or(0);
    Ok(format!("{}{head}{}", &source[..at], &source[at..]))
}

/// The end of a real opening tag, skipping comments and raw script/style text. A `<header>` or
/// a string in JavaScript isn't the document's `<head>`.
fn opening_tag(lower: &str, name: &str) -> Option<usize> {
    tag_span(lower, name).map(|span| span.end)
}

fn tag_span(lower: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let bytes = lower.as_bytes();
    let mut from = 0;
    while let Some(start) = lower[from..].find('<').map(|p| p + from) {
        if lower[start..].starts_with("<!--") {
            from = lower[start + 4..].find("-->").map(|p| start + 4 + p + 3)?;
            continue;
        }
        let mut end = start + 1;
        let mut quote = None;
        while let Some(&b) = bytes.get(end) {
            match (quote, b) {
                (Some(q), b) if b == q => quote = None,
                (None, b'\'' | b'"') => quote = Some(b),
                (None, b'>') => break,
                _ => {}
            }
            end += 1;
        }
        if end == bytes.len() { return None; }
        let tag = &lower[start + 1..end];
        let is = |key: &str| tag.strip_prefix(key).is_some_and(|rest| rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_whitespace() || c == '/'));
        if is(name) { return Some(start..end + 1); }
        if is("script") || is("style") {
            let close = if is("script") { "</script" } else { "</style" };
            from = lower[end + 1..].find(close).map(|p| end + 1 + p)?;
        } else {
            from = end + 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_print_settings_keep_resources_and_override_the_page_rule() {
        let source = "<!DOCTYPE html><html><HEAD><link href=\"style.css\"><style>@page { margin: 99pt }</style></HEAD><body><img src=\"plot.png\">Text</body></html>";
        let out = print_html(source, "file:///a%20b/", Options { paper: Paper::Letter, landscape: true, margin: 18.0 }).unwrap();
        assert!(out.starts_with("<!DOCTYPE html>"));
        assert!(out.find("<base").unwrap() < out.find("<link").unwrap());
        assert!(out.contains("size: 792.0000pt 612.0000pt; margin: 18.0000pt"));
        assert!(out.find("99pt").unwrap() < out.find("18.0000pt").unwrap());
        assert!(out.contains("<img src=\"plot.png\">Text"));
    }

    #[test]
    fn html_print_settings_reject_invalid_margins() {
        for margin in [f64::NAN, f64::INFINITY, -1.0, 145.0] {
            assert!(print_html("body", "file:///", Options { paper: Paper::A4, landscape: false, margin }).is_err());
        }
    }

    #[test]
    fn html_layout_finds_tags_instead_of_comments_scripts_or_headers() {
        let source = "<!-- <head> --><html><script>let example = '<head><base>';</script><header>Title</header><p>Text</p></html>";
        let out = print_html(source, "file:///local/", Options { paper: Paper::A4, landscape: false, margin: 0.0 }).unwrap();
        assert!(out.contains("<html><head><meta charset=\"utf-8\"><base"));
        assert!(out.ends_with("<header>Title</header><p>Text</p></html>"));
        assert_eq!(opening_tag("<head title='a > b'>", "head"), Some(20));
    }

    #[test]
    fn custom_html_layout_explains_unsupported_encodings_and_postscript() {
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("legacy.html");
        std::fs::write(&input, [0xff, 0xfe, 0x00]).unwrap();
        let options = Options { paper: Paper::Letter, landscape: true, margin: 18.0 };
        let out = t.path().join("out.pdf");
        for source in [input, t.path().join("page.ps")] {
            let error = to_pdf(&source, options, &out).unwrap_err();
            assert!(matches!(error, OpdfError::Unsupported(message) if message.contains("Original layout")));
            assert!(!out.exists());
        }
    }
}
