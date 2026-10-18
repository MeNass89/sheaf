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
    /// Pages read, and pages left alone because they already had text.
    pub pages_read: usize,
    pub pages_skipped: usize,
    /// Pages that could not be rendered or recognised (1-based), with why. Written only when
    /// some other page was read; otherwise [`ocr`] fails.
    pub pages_failed: Vec<(usize, String)>,
}

impl OcrReport {
    /// The warning about [`Self::pages_failed`], if any.
    pub fn failure_warning(&self) -> Option<String> {
        failure_warning(&self.pages_failed)
    }
}

fn failure_warning(failed: &[(usize, String)]) -> Option<String> {
    let list = failed.iter().map(|(p, _)| p.to_string()).collect::<Vec<_>>().join(", ");
    match failed.len() {
        0 => None,
        1 => Some(format!("1 page could not be read (page {list}): it has no text layer")),
        n => Some(format!("{n} pages could not be read (pages {list}): they have no text layer")),
    }
}

/// The engine's reason for a page left alone because it already has text (any other reason is
/// a failure to render or recognise it).
const HAS_TEXT: &str = "the page already contains text";

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
    pages_failed: Vec<(usize, String)>,
}

impl Recognized {
    fn save(self, mut doc: Doc, out: &Path) -> Result<OcrReport> {
        let (path, was_encrypted) = doc.save_unprotected(out)?;
        Ok(OcrReport { path, was_encrypted, words: self.words, pages_read: self.pages_read, pages_skipped: self.pages_skipped, pages_failed: self.pages_failed })
    }
}

/// Adds the text layer to `doc` in memory (for exports that read the text afterwards). Any
/// encryption stays: [`Recognized::save`] removes it last, and exports never need it removed.
/// Pages that could not be read fail the whole job ("OCR failed on page N: …") unless some
/// other page was read.
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
    let (read, skipped, failed) = tally(&pages)?;
    let id = doc.id();
    let words = doc.session_mut().apply_ocr(id, &pages).map_err(|e| OpdfError::Engine(e.to_string()))?;
    Ok(Recognized { words, pages_read: read, pages_skipped: skipped, pages_failed: failed })
}

/// Pages that could not be read: 1-based page, and why.
type Failed = Vec<(usize, String)>;

/// Pages read, pages left alone because they had text, and pages that could not be read
/// (1-based, with why). An error when no page was read and one failed.
fn tally(pages: &[eng::OcrPage]) -> Result<(usize, usize, Failed)> {
    let skipped = pages.iter().filter(|p| p.skipped.as_deref() == Some(HAS_TEXT)).count();
    let failed: Vec<(usize, String)> = pages.iter().filter_map(|p| p.skipped.as_ref().filter(|w| *w != HAS_TEXT).map(|w| (p.page + 1, w.clone()))).collect();
    let read = pages.len() - skipped - failed.len();
    if read == 0
        && let Some((page, why)) = failed.first()
    {
        return Err(OpdfError::Engine(format!("OCR failed on page {page}: {why}")));
    }
    Ok((read, skipped, failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(page: usize, skipped: Option<&str>) -> eng::OcrPage {
        eng::OcrPage { page, words: Vec::new(), already_on_page: 0, skipped: skipped.map(str::to_string) }
    }

    #[test]
    fn a_page_that_could_not_be_read_is_a_failure_not_a_skip() {
        let crash = "renderer crashed on page 2: boom";
        // Nothing else read: the job fails, naming the page.
        let e = tally(&[page(0, Some(HAS_TEXT)), page(1, Some(crash))]).unwrap_err().to_string();
        assert!(e.contains("OCR failed on page 2: renderer crashed"), "{e}");
        // Another page read: written, the failure reported apart from the pages with text.
        let (read, skipped, failed) = tally(&[page(0, None), page(1, Some(crash)), page(2, Some(HAS_TEXT))]).unwrap();
        assert_eq!((read, skipped, &failed), (1, 1, &vec![(2, crash.to_string())]));
        assert_eq!(failure_warning(&failed).unwrap(), "1 page could not be read (page 2): it has no text layer");
        // Pages that already had text are not failures.
        assert_eq!(tally(&[page(0, Some(HAS_TEXT))]).unwrap(), (0, 1, vec![]));
    }
}
