//! Job runner: one `Job` per tool run, executed off the UI thread.
//!
//! `run` is pure (no UI) so it can be tested; `spawn` runs it on a worker thread and posts
//! progress and the result back to the Slint event loop.

use sheaf_core::compress::{self, Level};
use sheaf_core::images::{self, PageLayout};
use sheaf_core::pages::{self, Input};
use sheaf_core::crop::{self, CropSpec};
use sheaf_core::numbers::{self, NumberOptions};
use sheaf_core::compare::{self, CompareMode};
use sheaf_core::excel::{self, SheetLayout};
use sheaf_core::export::{self, ExportOptions, Office};
use sheaf_core::ocr::{self, OcrOptions};
use sheaf_core::pdfa::{self, PdfALevel};
use sheaf_core::redact::{self, Mark};
use sheaf_core::edit::{self, EditSpec, Item};
use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
use sheaf_core::watermark::{self, WatermarkOptions};
use sheaf_core::{repair, rotate, security, OpdfError, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Remove pages' spec for "the blank pages", worked out when the job runs; ":N" after it, the
/// strictness (`pages::blank_pages`).
pub const BLANK_PAGES: &str = "blank";

/// How Split cuts the document.
#[derive(Debug, Clone)]
pub enum SplitMode {
    /// Page ranges such as "1-3,5": one output per range.
    Ranges(String),
    /// One output every N pages.
    Every(u32),
    /// Every range, in order, into one output (`<stem>-split.pdf`).
    OneFile(String),
    /// One output per top-level bookmark, named after it.
    Bookmarks,
    /// Outputs of at most this many bytes each.
    Size(u64),
}

/// What PDF to Word / PowerPoint / Excel writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportKind {
    Word,
    PowerPoint,
    Excel(SheetLayout),
    /// A web page, Markdown, rich text or plain text.
    Text(Office),
}

/// One run of a tool, with owned parameters. Batch tools carry `(input, output)` pairs:
/// the output path is the wanted name; the engine suffixes it if the name is taken.
#[derive(Debug, Clone)]
pub enum Job {
    /// `pages`: the range spec taken from each input ("" = all). `seq`, when not empty, is the
    /// pages one by one instead, (input, 1-based page), files mixed (Merge's Pages view).
    Merge { inputs: Vec<Input>, pages: Vec<String>, seq: Vec<(usize, u32)>, out: PathBuf },
    Split { input: Input, mode: SplitMode, out_dir: PathBuf },
    RemovePages { input: Input, pages: String, out: PathBuf },
    ExtractPages { input: Input, pages: String, out: PathBuf },
    /// 1-based page numbers in output order; 0 inserts a blank page.
    Organize { input: Input, order: Vec<u32>, out: PathBuf },
    Rotate { items: Vec<(Input, PathBuf)>, degrees: i64, pages: Option<String>, filter: rotate::PageFilter },
    Compress { items: Vec<(Input, PathBuf)>, level: Level },
    /// `sanitize`: the hidden information removed first (Acrobat's Protect ▸ Sanitize Document).
    Protect { items: Vec<(Input, PathBuf)>, password: String, owner: Option<String>, permissions: security::ProtectPermissions, sanitize: bool },
    Unlock { items: Vec<(Input, PathBuf)> },
    Repair { items: Vec<(Input, PathBuf)> },
    JpgToPdf { images: Vec<PathBuf>, layout: PageLayout, out: PathBuf },
    /// `quality`: JPEG quality, or `None` for PNG.
    /// `extract`: the pictures in the pages instead of the pages (iLovePDF's Extract images).
    PdfToJpg { inputs: Vec<Input>, dpi: u32, quality: Option<u8>, extract: bool, out_dir: PathBuf },
    /// Word / PowerPoint / Excel / HTML → PDF through LibreOffice.
    Office { items: Vec<(PathBuf, PathBuf)> },
    /// JPG to PDF with "Merge all images" off: a PDF for each image.
    ImagesEach { items: Vec<(PathBuf, PathBuf)>, layout: PageLayout },
    PageNumbers { items: Vec<(Input, PathBuf)>, options: NumberOptions },
    Watermark { items: Vec<(Input, PathBuf)>, options: WatermarkOptions },
    /// `pages`: "1-3,5", or `None` for every page.
    Crop { input: Input, pages: Option<String>, spec: CropSpec, out: PathBuf },
    PdfA { items: Vec<(Input, PathBuf)>, level: PdfALevel },
    Ocr { items: Vec<(Input, PathBuf)>, options: OcrOptions },
    Export { items: Vec<(Input, PathBuf)>, kind: ExportKind, options: ExportOptions },
    /// `pages`: the pages of the original and of the changed document compared (none: all).
    Compare { old: Input, new: Input, mode: CompareMode, out: PathBuf, pages: [Option<String>; 2] },
    /// Areas to black out for good (search matches, rectangles, whole pages).
    Redact { input: Input, marks: Vec<Mark>, out: PathBuf },
    /// A signature in `view_box` (points, y down, on page `page` as displayed), optionally
    /// signed with a certificate.
    Sign { input: Input, page: usize, view_box: [f64; 4], graphic: Graphic, digital: Option<DigitalSpec>, out: PathBuf },
    /// Text and images placed on the pages (view space: points, y down, as displayed).
    Edit { input: Input, items: Vec<EditPlaced>, out: PathBuf },
}

/// Edit PDF: one item placed in the page viewer, in view space (points from the top-left of
/// page `page` as displayed, y down).
#[derive(Debug, Clone, PartialEq)]
pub enum EditPlaced {
    /// `at`: the text's top-left corner; `size` in points.
    Text { page: usize, at: [f64; 2], text: String, size: f64, color: [u8; 3] },
    /// `rect`: `[x0, y0, x1, y1]`.
    Image { page: usize, rect: [f64; 4], path: PathBuf },
}

impl Job {
    /// The files the job reads, in order.
    pub fn inputs(&self) -> Vec<&Path> {
        match self {
            Job::Merge { inputs, .. } | Job::PdfToJpg { inputs, .. } => inputs.iter().map(|i| i.path.as_path()).collect(),
            Job::Split { input, .. }
            | Job::RemovePages { input, .. }
            | Job::ExtractPages { input, .. }
            | Job::Organize { input, .. }
            | Job::Crop { input, .. }
            | Job::Redact { input, .. }
            | Job::Sign { input, .. }
            | Job::Edit { input, .. } => vec![input.path.as_path()],
            Job::Compare { old, new, .. } => vec![old.path.as_path(), new.path.as_path()],
            Job::Rotate { items, .. }
            | Job::PageNumbers { items, .. }
            | Job::Watermark { items, .. }
            | Job::PdfA { items, .. }
            | Job::Ocr { items, .. }
            | Job::Export { items, .. }
            | Job::Compress { items, .. }
            | Job::Protect { items, .. }
            | Job::Unlock { items }
            | Job::Repair { items } => items.iter().map(|(i, _)| i.path.as_path()).collect(),
            Job::JpgToPdf { images, .. } => images.iter().map(|p| p.as_path()).collect(),
            Job::Office { items } | Job::ImagesEach { items, .. } => items.iter().map(|(i, _)| i.as_path()).collect(),
        }
    }
}

/// What a finished job produced.
#[derive(Debug, Clone, Default)]
pub struct JobOutcome {
    pub outputs: Vec<PathBuf>,
    /// Total size of the inputs, in bytes.
    pub before: u64,
    /// Total size of the outputs, in bytes.
    pub after: u64,
    /// Some input was password-protected and the output is not (every tool but Protect and Unlock).
    pub was_encrypted: bool,
    /// Compress only: an unprotected copy came out larger than its protected original.
    pub grew: bool,
    /// Batch tools: inputs that failed, with the error, while the others were written.
    pub failures: Vec<(PathBuf, String)>,
    /// One line about the result, shown under the title ("Conforms to PDF/A-2b").
    pub summary: String,
    /// Things the user should know about the outputs, one per line (PDF/A rules still broken).
    pub notes: Vec<String>,
    /// A page of the output as it came out, 0-based (Redact: the first page redacted).
    pub preview: Option<(usize, crate::files::Thumb)>,
}

fn size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) { Err(OpdfError::Cancelled) } else { Ok(()) }
}

/// Outputs written, and the inputs that failed with their error message.
type BatchResult = Result<(Vec<PathBuf>, Vec<(PathBuf, String)>)>;

/// Runs `f` over each item, checking `cancel` before each one and reporting progress after.
/// A cancel deletes the outputs already written: "Cancelled" always means nothing was written.
/// Any other error is recorded against that item (`path` names it) and the batch goes on;
/// only when every item failed is the first error returned.
fn each<T>(
    items: &[T],
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
    path: fn(&T) -> &Path,
    mut f: impl FnMut(&T) -> Result<Vec<PathBuf>>,
) -> BatchResult {
    let mut outs = Vec::new();
    let mut failures = Vec::new();
    let mut first_err = None;
    for (i, it) in items.iter().enumerate() {
        if let Err(e) = check(cancel) {
            for p in &outs {
                let _ = std::fs::remove_file(p);
            }
            return Err(e);
        }
        match f(it) {
            Ok(o) => outs.extend(o),
            Err(OpdfError::Cancelled) => {
                for p in &outs {
                    let _ = std::fs::remove_file(p);
                }
                return Err(OpdfError::Cancelled);
            }
            Err(e) => {
                failures.push((path(it).to_path_buf(), e.to_string()));
                first_err.get_or_insert(e);
            }
        }
        progress((i + 1) as f32 / items.len() as f32);
    }
    match first_err {
        Some(e) if outs.is_empty() => Err(e),
        _ => Ok((outs, failures)),
    }
}

/// Whether any PDF input of `job` is password-protected while its output will not be.
/// Protect (output encrypted by design), Unlock (the point of the tool) and the
/// image/Office converters (no PDF input) never warn.
fn pair(it: &(Input, PathBuf)) -> &Path {
    &it.0.path
}

fn loses_protection(job: &Job) -> bool {
    let pdfs: Vec<&Input> = match job {
        Job::Merge { inputs, .. } | Job::PdfToJpg { inputs, .. } => inputs.iter().collect(),
        Job::Split { input, .. }
        | Job::RemovePages { input, .. }
        | Job::ExtractPages { input, .. }
        | Job::Organize { input, .. } => vec![input],
        Job::Rotate { items, .. } | Job::Compress { items, .. } | Job::Repair { items } => items.iter().map(|(i, _)| i).collect(),
        // The PdfCraft tools report it from the engine.
        Job::Protect { .. }
        | Job::Unlock { .. }
        | Job::JpgToPdf { .. }
        | Job::ImagesEach { .. }
        | Job::Office { .. }
        | Job::PageNumbers { .. }
        | Job::Watermark { .. }
        | Job::PdfA { .. }
        | Job::Ocr { .. }
        | Job::Export { .. }
        | Job::Compare { .. }
        | Job::Redact { .. }
        | Job::Sign { .. }
        | Job::Edit { .. }
        | Job::Crop { .. } => vec![],
    };
    pdfs.into_iter().any(Input::is_encrypted)
}

/// Executes `job`. `cancel` is checked between files; `progress` receives 0..=1.
pub fn run(job: &Job, cancel: &AtomicBool, progress: &dyn Fn(f32)) -> Result<JobOutcome> {
    check(cancel)?;
    let mut was_encrypted = loses_protection(job);
    let mut grew = false;
    let mut failures = Vec::new();
    let mut summary = String::new();
    let mut notes = Vec::new();
    let mut preview = None;
    let one = |r: Result<PathBuf>| r.map(|p| vec![p]);
    let mut batch = |r: BatchResult| -> Result<Vec<PathBuf>> {
        let (o, f) = r?;
        failures = f;
        Ok(o)
    };
    let outputs = match job {
        Job::Merge { inputs, pages: specs, seq, out } => {
            let tmp = tempfile::tempdir()?;
            let parts = as_pdfs(inputs, tmp.path(), cancel, progress)?;
            check(cancel)?;
            let r = one(if seq.is_empty() { pages::merge_pages(&parts, specs, out) } else { pages::assemble(&parts, seq, out) })?;
            progress(1.0);
            r
        }
        Job::Split { input, mode, out_dir } => {
            let r = match mode {
                SplitMode::Ranges(s) => pages::split_ranges(input, s, out_dir)?,
                SplitMode::OneFile(s) => vec![pages::extract(input, s, &out_dir.join(format!("{}-split.pdf", input.stem())))?],
                SplitMode::Every(n) => pages::split_every(input, *n, out_dir)?,
                SplitMode::Bookmarks => pages::split_bookmarks(input, out_dir)?,
                SplitMode::Size(max) => {
                    let (files, over) = pages::split_size_report(input, *max, out_dir)?;
                    summary = pages::oversize_note(&over);
                    files
                }
            };
            progress(1.0);
            r
        }
        Job::RemovePages { input, pages: spec, out } => {
            // "blank": the blank pages, found now (Acrobat's Remove Blank Pages).
            let spec = if let Some(level) = spec.strip_prefix(BLANK_PAGES) {
                let blank = pages::blank_pages(input, level.trim_start_matches(':').parse().unwrap_or(1))?;
                if blank.is_empty() {
                    return Err(OpdfError::Unsupported("this PDF has no blank pages".into()));
                }
                blank.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
            } else {
                spec.clone()
            };
            let r = one(pages::remove(input, &spec, out))?;
            progress(1.0);
            r
        }
        Job::ExtractPages { input, pages: spec, out } => {
            let r = one(pages::extract(input, spec, out))?;
            progress(1.0);
            r
        }
        Job::Organize { input, order, out } => {
            let r = one(pages::organize(input, order, out))?;
            progress(1.0);
            r
        }
        Job::Rotate { items, degrees, pages: spec, filter } => {
            batch(each(items, cancel, progress, pair, |(i, o)| one(rotate::rotate_filtered(i, spec.as_deref(), *degrees, *filter, o))))?
        }
        Job::Compress { items, level } => {
            let n = items.len() as f32;
            let started = std::cell::Cell::new(0usize);
            batch(each(items, cancel, progress, pair, |(i, o)| {
                let k = started.get() as f32;
                started.set(started.get() + 1);
                // Progress within the file, and a cancel that stops between its pictures.
                let rep = compress::compress_with(i, *level, o, &mut |p| {
                    progress((k + p) / n);
                    !cancel.load(Ordering::Relaxed)
                })?;
                was_encrypted |= rep.was_encrypted;
                grew |= rep.grew;
                Ok(vec![rep.path])
            }))?
        }
        Job::Protect { items, password, owner, permissions, sanitize } => {
            let tmp = tempfile::tempdir()?;
            batch(each(items, cancel, progress, pair, |(i, o)| {
                let clean;
                let i = if *sanitize {
                    clean = Input::new(security::sanitize(i, &tmp.path().join(i.path.file_name().unwrap_or_default()))?);
                    &clean
                } else {
                    i
                };
                one(security::protect_with_permissions(i, password, owner.as_deref(), *permissions, o))
            }))?
        }
        Job::Unlock { items } => {
            batch(each(items, cancel, progress, pair, |(i, o)| one(security::unlock(i, o))))?
        }
        Job::Repair { items } => {
            batch(each(items, cancel, progress, pair, |(i, o)| one(repair::repair(i, o))))?
        }
        Job::JpgToPdf { images, layout, out } => {
            jpg_to_pdf(images, *layout, out, cancel, progress)?
        }
        Job::PdfToJpg { inputs, dpi, quality, extract, out_dir } => batch(each(inputs, cancel, progress, |i| i.path.as_path(), |i| {
            if *extract { images::extract_images(i, *quality, out_dir) } else { images::pdf_to_jpg(i, *dpi, *quality, out_dir) }
        }))?,
        Job::ImagesEach { items, layout } => {
            batch(each(items, cancel, progress, |(i, _)| i.as_path(), |(i, o)| one(images::images_to_pdf(std::slice::from_ref(i), *layout, o))))?
        }
        Job::Office { items } => {
            batch(each(items, cancel, progress, |(i, _)| i.as_path(), |(i, o)| one(crate::files::office_to_pdf(i, o))))?
        }
        Job::PageNumbers { items, options } => batch(each(items, cancel, progress, pair, |(i, o)| {
            let (p, w) = numbers::number_pages(i, options, o)?;
            was_encrypted |= w;
            Ok(vec![p])
        }))?,
        Job::Watermark { items, options } => batch(each(items, cancel, progress, pair, |(i, o)| {
            let (p, w) = watermark::watermark(i, options, o)?;
            was_encrypted |= w;
            Ok(vec![p])
        }))?,
        Job::Crop { input, pages: spec_pages, spec, out } => {
            let (p, w) = crop::crop(input, spec_pages.as_deref(), spec, out)?;
            was_encrypted |= w;
            progress(1.0);
            vec![p]
        }
        Job::Ocr { items, options } => {
            let n = items.len() as f32;
            let started = std::cell::Cell::new(0usize);
            let (mut read, mut skipped, mut words) = (0, 0, 0);
            let outs = batch(each(items, cancel, progress, pair, |(i, o)| {
                let k = started.get() as f32;
                started.set(started.get() + 1);
                // Progress within the file, and a cancel that stops between its pages.
                let r = ocr::ocr(i, options, o, &mut |p| {
                    progress((k + p) / n);
                    !cancel.load(Ordering::Relaxed)
                })?;
                was_encrypted |= r.was_encrypted;
                read += r.pages_read;
                skipped += r.pages_skipped;
                words += r.words;
                if let Some(w) = r.failure_warning() {
                    let name = r.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    notes.push(if n > 1.0 { format!("{name} — {w}") } else { w });
                }
                Ok(vec![r.path])
            }))?;
            summary = ocr::summary(read, words, skipped);
            outs
        }
        Job::Export { items, kind, options } => {
            let n = items.len() as f32;
            let started = std::cell::Cell::new(0usize);
            let mut tables = 0;
            let outs = batch(each(items, cancel, progress, pair, |(i, o)| {
                let k = started.get() as f32;
                started.set(started.get() + 1);
                let mut step = |p: f32| {
                    progress((k + p) / n);
                    !cancel.load(Ordering::Relaxed)
                };
                // Office files are not PDFs: an encrypted input needs no protection warning.
                let path = match kind {
                    ExportKind::Word => export::to_office(i, Office::Word, options, o, &mut step)?.path,
                    ExportKind::PowerPoint => export::to_office(i, Office::PowerPoint, options, o, &mut step)?.path,
                    ExportKind::Text(format) => export::to_office(i, *format, options, o, &mut step)?.path,
                    ExportKind::Excel(layout) => {
                        let r = excel::to_excel(i, *layout, options, o, &mut step)?;
                        tables += r.tables;
                        r.path
                    }
                };
                Ok(vec![path])
            }))?;
            if matches!(kind, ExportKind::Excel(_)) {
                summary = excel::summary(tables);
            }
            outs
        }
        Job::Compare { old, new, mode, out, pages: chosen } => {
            // Pages chosen in either document (Acrobat compares page ranges): those pages only, as
            // a document of their own.
            let tmp = tempfile::tempdir()?;
            let part = |i: &Input, spec: &Option<String>, name: &str| -> Result<Input> {
                match spec.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    Some(s) => Ok(Input { path: pages::extract(i, s, &tmp.path().join(name))?, password: None }),
                    None => Ok(i.clone()),
                }
            };
            let (old, new) = (part(old, &chosen[0], "original.pdf")?, part(new, &chosen[1], "changed.pdf")?);
            let r = compare::compare(&old, &new, *mode, out)?;
            was_encrypted |= r.was_encrypted;
            summary = r.summary();
            progress(1.0);
            vec![r.path]
        }
        Job::Redact { input, marks, out } => {
            // The marks carry their matched texts: unchecked matches are not removed anywhere.
            let r = redact::apply(input, marks, &[], out)?;
            // The first page redacted, as written: what is gone shows black.
            if let Some(page) = marks.iter().map(|m| m.page).min() {
                preview = crate::files::page_thumb(&Input::new(&r.path), page, 420).ok().map(|t| (page, t));
            }
            notes.extend(r.warnings.iter().cloned());
            was_encrypted |= r.was_encrypted;
            summary = redact::summary(r.counts);
            progress(1.0);
            vec![r.path]
        }
        Job::Sign { input, page, view_box, graphic, digital, out } => {
            let info = sign::page_info(input, *page)?;
            let spec = SignSpec { page: *page, rect: sign::view_rect_to_user(&info, *view_box), graphic: graphic.clone(), digital: digital.clone() };
            let r = sign::sign(input, &spec, out)?;
            summary = r.summary();
            progress(1.0);
            vec![r.path]
        }
        Job::Edit { input, items, out } => {
            let mut infos = std::collections::HashMap::new();
            let mut spec = EditSpec::default();
            for it in items {
                let page = match it {
                    EditPlaced::Text { page, .. } | EditPlaced::Image { page, .. } => *page,
                };
                let info = match infos.entry(page) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => e.insert(edit::page_info(input, page)?),
                };
                spec.items.push(match it {
                    EditPlaced::Text { at, text, size, color, .. } => {
                        Item::Text { page, at: edit::view_to_user(info, *at), text: text.clone(), font_size: *size, color: *color }
                    }
                    EditPlaced::Image { rect, path, .. } => Item::Image { page, rect: sign::view_rect_to_user(info, *rect), path: path.clone() },
                });
            }
            let r = edit::edit(input, &spec, out)?;
            was_encrypted |= r.was_encrypted;
            summary = r.summary();
            progress(1.0);
            vec![r.path]
        }
        Job::PdfA { items, level } => {
            let many = items.len() > 1;
            let mut conforming = 0;
            let outs = batch(each(items, cancel, progress, pair, |(i, o)| {
                let r = pdfa::to_pdfa(i, *level, o)?;
                was_encrypted |= r.was_encrypted;
                if r.issues.is_empty() {
                    conforming += 1;
                }
                let name = r.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                notes.extend(r.issues.iter().map(|x| {
                    let line = format!("{}: {}", x.clause, x.message);
                    if many { format!("{name} — {line}") } else { line }
                }));
                Ok(vec![r.path])
            }))?;
            summary = pdfa::summary(*level, outs.len() - conforming);
            outs
        }
    };
    let after = outputs.iter().map(|p| size(p)).sum();
    let before = job.inputs().iter().map(|p| size(p)).sum();
    Ok(JobOutcome { outputs, before, after, was_encrypted, grew, failures, summary, notes, preview })
}

/// `run`, with a panic anywhere in the engine turned into an error, so the tab never stays
/// stuck on "Processing…".
pub fn run_catching(job: &Job, cancel: &AtomicBool, progress: &dyn Fn(f32)) -> Result<JobOutcome> {
    catching(|| run(job, cancel, progress))
}

/// Runs `f`; a panic becomes `Conversion("internal error: <message>")`.
pub fn catching<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|p| Err(OpdfError::Conversion(format!("internal error: {}", panic_message(&*p)))))
}

pub fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown panic".into())
}

/// Applies a cancel that arrived while a job could not be interrupted (one engine call:
/// Merge, Split, a single-file batch…). If `cancelled`, the files the job just wrote are
/// deleted and the result is `Cancelled`; otherwise the outcome passes through.
pub fn finalize(outcome: JobOutcome, cancelled: bool) -> Result<JobOutcome> {
    if !cancelled {
        return Ok(outcome);
    }
    for p in &outcome.outputs {
        let _ = std::fs::remove_file(p);
    }
    Err(OpdfError::Cancelled)
}

/// Merge's inputs as PDFs: an image becomes a page of its size, an Office or HTML document is
/// converted by LibreOffice (iLovePDF merges mixed files), each into `dir`; PDFs pass through.
/// The conversions count as the first nine tenths of the work.
fn as_pdfs(inputs: &[Input], dir: &Path, cancel: &AtomicBool, progress: &dyn Fn(f32)) -> Result<Vec<Input>> {
    let images = sheaf_core::tools::find(sheaf_core::tools::ToolId::JpgToPdf).accepts;
    let n = inputs.len().max(1) as f32;
    let mut out = Vec::with_capacity(inputs.len());
    for (k, i) in inputs.iter().enumerate() {
        check(cancel)?;
        let ext = i.path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let part = dir.join(format!("{k:05}.pdf"));
        out.push(match ext.as_str() {
            "pdf" => i.clone(),
            e if images.contains(&e) => Input::new(images::images_to_pdf(std::slice::from_ref(&i.path), PageLayout::FitImage, &part)?),
            _ => Input::new(crate::files::office_to_pdf(&i.path, &part)?),
        });
        progress((k + 1) as f32 / n * 0.9);
    }
    Ok(out)
}

/// Images → one PDF, one image at a time so the job can report progress and be cancelled:
/// each image becomes a one-page PDF in a temp dir, then the pages are merged into `out`.
fn jpg_to_pdf(
    images: &[PathBuf],
    layout: PageLayout,
    out: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<Vec<PathBuf>> {
    if images.is_empty() {
        return Err(OpdfError::Unsupported("no images".into()));
    }
    let tmp = tempfile::tempdir()?;
    let n = images.len() as f32;
    let mut parts = Vec::new();
    for (k, img) in images.iter().enumerate() {
        check(cancel)?;
        let part = tmp.path().join(format!("{k:05}.pdf"));
        parts.push(Input::new(images::images_to_pdf(std::slice::from_ref(img), layout, &part)?));
        // The final merge counts as the last tenth of the work.
        progress((k + 1) as f32 / n * 0.9);
    }
    check(cancel)?;
    let r = pages::merge(&parts, out)?;
    progress(1.0);
    Ok(vec![r])
}

/// Default output file name for `stem` under `tool`.
pub fn default_name(tool: sheaf_core::tools::ToolId, stem: &str) -> String {
    use sheaf_core::tools::ToolId as T;
    let suffix = match tool {
        T::Merge => "-merged",
        T::Compress => "-compressed",
        T::Protect => "-protected",
        T::Unlock => "-unlocked",
        T::Rotate => "-rotated",
        T::RemovePages => "-edited",
        T::ExtractPages => "-extracted",
        T::Organize => "-organized",
        T::Repair => "-repaired",
        T::PageNumbers => "-numbered",
        T::Watermark => "-watermarked",
        T::Crop => "-cropped",
        T::PdfToPdfA => "-pdfa",
        T::Ocr => "-ocr",
        T::PdfToWord => return format!("{stem}.docx"),
        T::PdfToPowerPoint => return format!("{stem}.pptx"),
        T::PdfToExcel => return format!("{stem}.xlsx"),
        T::PdfToText => return format!("{stem}.html"),
        T::Compare => "-comparison",
        T::Redact => "-redacted",
        T::Sign => "-signed",
        T::Edit => "-edited",
        _ => "",
    };
    format!("{stem}{suffix}.pdf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_core::tools::ToolId;

    #[test]
    fn a_panicking_job_becomes_an_error() {
        let r: Result<()> = catching(|| panic!("boom {}", 7));
        match r {
            Err(OpdfError::Conversion(m)) => assert_eq!(m, "internal error: boom 7"),
            other => panic!("{other:?}"),
        }
        let r: Result<()> = catching(|| std::panic::panic_any(42u8));
        assert!(matches!(r, Err(OpdfError::Conversion(m)) if m == "internal error: unknown panic"));
        assert!(catching(|| Ok(JobOutcome::default())).is_ok());
    }

    #[test]
    fn default_names() {
        assert_eq!(default_name(ToolId::Merge, "a"), "a-merged.pdf");
        assert_eq!(default_name(ToolId::RemovePages, "a"), "a-edited.pdf");
        assert_eq!(default_name(ToolId::PageNumbers, "a"), "a-numbered.pdf");
        assert_eq!(default_name(ToolId::Watermark, "a"), "a-watermarked.pdf");
        assert_eq!(default_name(ToolId::Crop, "a"), "a-cropped.pdf");
        assert_eq!(default_name(ToolId::PdfToPdfA, "a"), "a-pdfa.pdf");
        assert_eq!(default_name(ToolId::Ocr, "a"), "a-ocr.pdf");
        assert_eq!(default_name(ToolId::PdfToWord, "a"), "a.docx");
        assert_eq!(default_name(ToolId::PdfToPowerPoint, "a"), "a.pptx");
        assert_eq!(default_name(ToolId::PdfToExcel, "a"), "a.xlsx");
        assert_eq!(default_name(ToolId::Compare, "a"), "a-comparison.pdf");
        assert_eq!(default_name(ToolId::JpgToPdf, "photo"), "photo.pdf");
        assert_eq!(default_name(ToolId::WordToPdf, "letter"), "letter.pdf");
    }
}

// ---------------------------------------------------------------------------------------------
// Background execution

type ProgressFn = Box<dyn Fn(&crate::AppWindow, i32, f32)>;
type DoneFn = Box<dyn Fn(&crate::AppWindow, i32, Result<JobOutcome>, Vec<Option<u32>>)>;

thread_local! {
    // Set once on the UI thread; called there from `upgrade_in_event_loop`.
    static HANDLERS: std::cell::RefCell<Option<(ProgressFn, DoneFn)>> = const { std::cell::RefCell::new(None) };
}

static CANCELS: std::sync::Mutex<Vec<(i32, std::sync::Arc<AtomicBool>)>> = std::sync::Mutex::new(Vec::new());

/// Registers, on the UI thread, what to do with progress (0..=1) and with a finished job
/// (its outcome and the page count of each output, `None` for non-PDF outputs).
pub fn set_handlers(
    progress: impl Fn(&crate::AppWindow, i32, f32) + 'static,
    done: impl Fn(&crate::AppWindow, i32, Result<JobOutcome>, Vec<Option<u32>>) + 'static,
) {
    HANDLERS.with(|h| *h.borrow_mut() = Some((Box::new(progress), Box::new(done))));
}

/// Asks the job of `tab_id` to stop at the next file boundary.
pub fn cancel(tab_id: i32) {
    if let Some((_, c)) = CANCELS.lock().unwrap().iter().find(|(t, _)| *t == tab_id) {
        c.store(true, Ordering::Relaxed);
    }
}

/// Runs `job` on a worker thread; progress and the result reach the handlers on the UI thread.
/// `author`: written as the Author of the PDFs the job makes from other files when they have
/// none (Acrobat's Create PDF uses the identity name).
pub fn spawn(ui: slint::Weak<crate::AppWindow>, tab_id: i32, job: Job, author: Option<String>) {
    let flag = std::sync::Arc::new(AtomicBool::new(false));
    {
        let mut c = CANCELS.lock().unwrap();
        c.retain(|(t, _)| *t != tab_id);
        c.push((tab_id, flag.clone()));
    }
    std::thread::spawn(move || {
        let post_progress = |p: f32| {
            let _ = ui.upgrade_in_event_loop(move |w| {
                HANDLERS.with(|h| if let Some((pf, _)) = &*h.borrow() { pf(&w, tab_id, p) });
            });
        };
        let r = run_catching(&job, &flag, &post_progress);
        // Debug builds: SHEAF_JOB_DELAY_MS holds the result back, so a cancel can be made to
        // arrive after an uninterruptible call returned Ok (end-to-end check of `finalize`).
        if cfg!(debug_assertions)
            && let Some(ms) = std::env::var("SHEAF_JOB_DELAY_MS").ok().and_then(|v| v.parse().ok())
        {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
        let r = r.and_then(|o| finalize(o, flag.load(Ordering::Relaxed)));
        if let (Ok(o), Some(name)) = (&r, &author) {
            for p in o.outputs.iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))) {
                let _ = sheaf_core::info::set_author_if_empty(p, name);
            }
        }
        let counts = match &r {
            Ok(o) => o.outputs.iter().map(|p| page_count(p)).collect(),
            Err(_) => Vec::new(),
        };
        CANCELS.lock().unwrap().retain(|(t, c)| !(*t == tab_id && std::sync::Arc::ptr_eq(c, &flag)));
        let _ = ui.upgrade_in_event_loop(move |w| {
            HANDLERS.with(|h| if let Some((_, df)) = &*h.borrow() { df(&w, tab_id, r, counts) });
        });
    });
}

fn page_count(p: &Path) -> Option<u32> {
    if crate::files::kind_of(p) != crate::files::Kind::Pdf {
        return None;
    }
    crate::files::page_count(&Input::new(p))
}
