//! Local HTML printing with iLovePDF's paper, orientation and margin controls.

use crate::{office, OpdfError, Result};
use std::path::{Path, PathBuf};

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
    let source = std::fs::read_to_string(input)?;
    let base = office::file_url(std::fs::canonicalize(input)?.parent().ok_or_else(|| OpdfError::Unsupported("The HTML file has no parent folder".into()))?);
    let html = print_html(&source, &format!("{base}/"), options)?;
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
    let lower = source.to_ascii_lowercase();
    // An explicit base may be relative to the original document; copying it elsewhere would
    // silently change what its pictures and links refer to. Keep that case explicit.
    if lower.contains("<base") {
        return Err(OpdfError::Unsupported("This HTML file defines its own base address. Choose Original layout to preserve it".into()));
    }
    let base = base.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
    let base = format!("<base href=\"{base}\">");
    let style = format!("<style>@page {{ size: {w:.4}pt {h:.4}pt; margin: {:.4}pt; }}</style>", options.margin);
    // Put the base before existing relative resources; the page rule after existing styles.
    if let Some(head) = lower.find("<head").and_then(|p| lower[p..].find('>').map(|end| p + end + 1)) {
        let end = lower[head..].find("</head>").map(|p| p + head).unwrap_or(head);
        return Ok(format!("{}{}{}{}{}", &source[..head], base, &source[head..end], style, &source[end..]));
    }
    let head = format!("<head><meta charset=\"utf-8\">{base}{style}</head>");
    // A fragment gets a document; an existing doctype stays at the start of the document.
    let at = lower.find("<html").and_then(|p| lower[p..].find('>').map(|end| p + end + 1))
        .or_else(|| lower.find("<!doctype").and_then(|p| lower[p..].find('>').map(|end| p + end + 1)))
        .unwrap_or(0);
    Ok(format!("{}{head}{}", &source[..at], &source[at..]))
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
    fn html_print_settings_reject_invalid_margins_and_existing_bases() {
        for margin in [f64::NAN, f64::INFINITY, -1.0, 145.0] {
            assert!(print_html("body", "file:///", Options { paper: Paper::A4, landscape: false, margin }).is_err());
        }
        assert!(print_html("<head><BASE href='../assets/'></head>", "file:///", Options { paper: Paper::A4, landscape: false, margin: 0.0 }).is_err());
    }
}
