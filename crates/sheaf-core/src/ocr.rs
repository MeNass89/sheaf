//! OCR PDF: PdfCraft's recogniser (ocrs models) adds an invisible, searchable text layer.
//!
//! The models (`text-detection.rten`, `text-recognition.rten`) are looked up in
//! `$PDFCRAFT_MODELS`, then beside the executable (`Sheaf.app/Contents/Resources/models`), then,
//! in debug builds, in the source tree's `vendor/models/` (scripts/fetch-models.sh).

use crate::pdfcraft::engine::ocr::{self as eng, Models, Ocr, OcrSettings};
use crate::pdfcraft::Doc;
use crate::{pages::Input, OpdfError, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// OCR settings.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrOptions {
    /// A code from [`languages`]. The models read Latin script, whatever the language.
    pub language: String,
    /// Also read pages that already contain text (only words not already there are added).
    pub read_text_pages: bool,
    /// Resolution pages are read at (a scan is read at its own resolution, up to this).
    pub dpi: f32,
}

impl Default for OcrOptions {
    fn default() -> Self {
        OcrOptions { language: "en".into(), read_text_pages: false, dpi: 300.0 }
    }
}

/// What [`ocr`] did.
#[derive(Debug, Clone)]
pub struct OcrReport {
    pub path: PathBuf,
    /// The input was password-protected (the output never is).
    pub was_encrypted: bool,
    /// Words added to the text layer.
    pub words: usize,
    /// Pages read, and pages left alone (they already had text, or could not be read).
    pub pages_read: usize,
    pub pages_skipped: usize,
}

/// The languages offered: (code, name).
pub fn languages() -> &'static [(&'static str, &'static str)] {
    eng::LANGUAGES
}

/// The source tree's models, for debug builds only: release builds carry no build path.
#[cfg(debug_assertions)]
fn debug_models() -> Option<Models> {
    Models::in_dir(&PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../vendor/models")))
}

#[cfg(not(debug_assertions))]
fn debug_models() -> Option<Models> {
    None
}

fn models() -> Option<Models> {
    Models::find().or_else(debug_models)
}

/// What to do when the models are missing: reinstall, or (in a debug build) fetch them.
const MISSING_MODELS: &str = if cfg!(debug_assertions) {
    "OCR models are missing: run scripts/fetch-models.sh"
} else {
    "OCR models are missing from this installation — reinstall Sheaf"
};

/// Whether the recognition models are installed.
pub fn available() -> bool {
    models().is_some()
}

/// The recogniser, loaded once (it takes a moment) and shared by every job.
fn engine() -> Result<Arc<Ocr>> {
    static OCR: Mutex<Option<Arc<Ocr>>> = Mutex::new(None);
    let mut slot = OCR.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(o) = slot.as_ref() {
        return Ok(o.clone());
    }
    let m = models().ok_or_else(|| OpdfError::Engine(MISSING_MODELS.into()))?;
    let o = Arc::new(Ocr::load(&m).map_err(|e| OpdfError::Engine(format!("OCR models: {e}")))?);
    *slot = Some(o.clone());
    Ok(o)
}

/// Recognises the text of `input` and writes `out` (never overwriting) with an invisible text
/// layer. `progress` gets 0..=1 before each page and at the end; returning `false` stops, and
/// the result is [`OpdfError::Cancelled`] with nothing written.
pub fn ocr(input: &Input, opts: &OcrOptions, out: &Path, progress: &mut dyn FnMut(f32) -> bool) -> Result<OcrReport> {
    let mut doc = Doc::open(input)?;
    recognize(&mut doc, opts, progress)?.save(doc, out)
}

/// What recognition found, before it is written.
pub(crate) struct Recognized {
    words: usize,
    pages_read: usize,
    pages_skipped: usize,
}

impl Recognized {
    fn save(self, mut doc: Doc, out: &Path) -> Result<OcrReport> {
        let (path, was_encrypted) = doc.save_unprotected(out)?;
        Ok(OcrReport { path, was_encrypted, words: self.words, pages_read: self.pages_read, pages_skipped: self.pages_skipped })
    }
}

/// Adds the text layer to `doc` in memory (for exports that read the text afterwards). Any
/// encryption stays: [`Recognized::save`] removes it last, and exports never need it removed.
pub(crate) fn recognize(doc: &mut Doc, opts: &OcrOptions, progress: &mut dyn FnMut(f32) -> bool) -> Result<Recognized> {
    if !languages().iter().any(|(code, _)| *code == opts.language) {
        return Err(OpdfError::Unsupported(format!("OCR language \"{}\"", opts.language)));
    }
    if let Some(why) = doc.session().get(doc.id()).and_then(|d| d.read_only_reason.clone()) {
        return Err(OpdfError::Unsupported(why));
    }
    let ocr = engine()?;
    let settings = OcrSettings { dpi: opts.dpi, language: opts.language.clone(), skip_text_pages: !opts.read_text_pages };
    let job = doc.session().ocr_job(doc.id(), &[], settings).ok_or_else(|| OpdfError::Engine("the document could not be read".into()))?;
    let mut stopped = false;
    let pages = job.run(&ocr, |done, total| {
        if stopped {
            return false;
        }
        let go = progress(if total == 0 { 1.0 } else { done as f32 / total as f32 });
        // The final call (done == total) only reports; a stop there changes nothing.
        stopped = !go && done < total;
        go
    });
    if stopped {
        return Err(OpdfError::Cancelled);
    }
    let id = doc.id();
    let words = doc.session_mut().apply_ocr(id, &pages).map_err(|e| OpdfError::Engine(e.to_string()))?;
    let skipped = pages.iter().filter(|p| p.skipped.is_some()).count();
    Ok(Recognized { words, pages_read: pages.len() - skipped, pages_skipped: skipped })
}
