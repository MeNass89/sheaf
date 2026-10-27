//! Review and correct the hidden text over a scan (Acrobat's Correct Recognized Text).
//! Existing PDFs rarely preserve recognition confidence, so every hidden run is reviewable;
//! decoding artifacts are flagged as possible errors, never presented as confidence scores.

use crate::pdfcraft::{engine::Edit, Doc};
use crate::{pages::Input, OpdfError, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// A reviewable OCR run. Page and line numbers are 1-based for the user; `rect` is PDF user space.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReviewLine {
    pub page: usize,
    pub line: usize,
    pub rect: [f64; 4],
    pub text: String,
    /// A decoding artifact (`?`, U+FFFD, control character), not a model-confidence estimate.
    pub suspect: bool,
}

pub fn review(input: &Input) -> Result<Vec<ReviewLine>> {
    let doc = Doc::open(input)?;
    Ok(review_document(doc.document()))
}

/// Review the live document, including unsaved changes in the reader.
pub fn review_document(doc: &crate::pdfcraft::engine::Document) -> Vec<ReviewLine> {
    let mut out = Vec::new();
    for page in 0..doc.info.pages.len() {
        for (line, run) in doc.text_lines(page).into_iter().enumerate().filter(|(_, run)| run.invisible()) {
            let suspect = run.text.chars().any(|c| c == '?' || c == '\u{fffd}' || (c.is_control() && !c.is_whitespace()));
            out.push(ReviewLine { page: page + 1, line: line + 1, rect: run.rect, text: run.text, suspect });
        }
    }
    out
}

/// Replace a hidden OCR run, keeping the scan intact. An empty replacement is Acrobat's
/// "This is not text". Coordinates are 0-based engine indexes; `expected` guards against a
/// stale review row. A visible run is always refused, even if its index and text match.
pub fn correct(input: &Input, page: usize, line: usize, expected: &str, replacement: &str, out: &Path) -> Result<PathBuf> {
    let mut doc = Doc::open(input)?;
    let edit = correction_edit(doc.document(), page, line, expected, replacement)?;
    doc.apply(edit)?;
    Ok(doc.save_unprotected(out)?.0)
}

/// Validate against the live text before returning an undoable correction.
pub fn correction_edit(doc: &crate::pdfcraft::engine::Document, page: usize, line: usize, expected: &str, replacement: &str) -> Result<Edit> {
    if page >= doc.info.pages.len() { return Err(OpdfError::Unsupported("The OCR page no longer exists; review again".into())); }
    if replacement.contains(['\n', '\r']) { return Err(OpdfError::Unsupported("Correct one OCR run at a time, without line breaks".into())); }
    let lines = doc.text_lines(page);
    let run = lines.get(line).ok_or_else(|| OpdfError::Unsupported("The OCR run no longer exists; review the page again".into()))?;
    if !run.invisible() { return Err(OpdfError::Unsupported("The selected text is visible artwork, not an OCR layer".into())); }
    if run.text != expected { return Err(OpdfError::Unsupported("The OCR text changed; review the page again".into())); }
    Ok(Edit::EditTextLine { page, line, text: replacement.into() })
}
