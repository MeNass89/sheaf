//! Review and correct the hidden text over a scan (Acrobat's Correct Recognized Text).
//! Sheaf retains per-word confidence in its OCR layers. Older layers remain reviewable
//! with unknown confidence; decoding artifacts are also flagged as possible errors.

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
    /// Recognizer confidence (0..1), absent on older/external layers.
    pub confidence: Option<f32>,
    /// Low recognition confidence (<0.85) or a decoding artifact.
    pub suspect: bool,
}

pub fn review(input: &Input) -> Result<Vec<ReviewLine>> {
    let doc = Doc::open(input)?;
    Ok(review_document(doc.document()))
}

/// Review the live document, including unsaved changes in the reader.
pub fn review_document(doc: &crate::pdfcraft::engine::Document) -> Vec<ReviewLine> {
    (0..doc.info.pages.len()).flat_map(|page| review_page(doc, page)).collect()
}

fn review_page(doc: &crate::pdfcraft::engine::Document, page: usize) -> Vec<ReviewLine> {
    doc.text_lines(page).into_iter().enumerate().filter(|(_, run)| run.invisible()).map(|(line, run)| {
        let confidence = run.ocr_confidence;
        let suspect = confidence.is_some_and(|c| c < 0.85)
            || run.text.chars().any(|c| c == '?' || c == '\u{fffd}' || (c.is_control() && !c.is_whitespace()));
        ReviewLine { page: page + 1, line: line + 1, rect: run.rect, text: run.text, confidence, suspect }
    }).collect()
}

/// Cache review pages on the document thread. A targeted correction invalidates only its
/// page; arbitrary edits, undo/redo and page-tree changes invalidate the whole snapshot.
#[derive(Default)]
pub struct ReviewCache {
    generation: Option<u64>,
    pages: Vec<Option<Vec<ReviewLine>>>,
}

impl ReviewCache {
    pub fn review(&mut self, doc: &crate::pdfcraft::engine::Document) -> Vec<ReviewLine> {
        if self.generation != Some(doc.edit_generation()) || self.pages.len() != doc.info.pages.len() {
            self.pages = vec![None; doc.info.pages.len()];
            self.generation = Some(doc.edit_generation());
        }
        let mut rows = Vec::new();
        for (page, cached) in self.pages.iter_mut().enumerate() {
            rows.extend(cached.get_or_insert_with(|| review_page(doc, page)).iter().cloned());
        }
        rows
    }

    /// Call only after a successful single-page OCR correction, with its previous generation.
    pub fn corrected(&mut self, doc: &crate::pdfcraft::engine::Document, before: u64, page: usize) {
        if self.generation == Some(before) && self.pages.len() == doc.info.pages.len() {
            if let Some(cached) = self.pages.get_mut(page) { *cached = None; }
            self.generation = Some(doc.edit_generation());
        } else {
            self.generation = None;
        }
    }
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
