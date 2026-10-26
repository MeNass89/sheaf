//! File helpers shared by the tool flow: what a tool accepts, where a dropped file goes,
//! sizes and captions, and page-1 thumbnails. No UI types here, so it is unit-testable.

use sheaf_core::pages::Input;
use sheaf_core::tools::{self, ToolId};
use sheaf_core::{OpdfError, Result};
use std::path::Path;

/// What kind of input a file is, for its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pdf,
    Image,
    /// Office or HTML document: page 1 and page count once converted on the side
    /// ([`document_probe`]), else size only.
    Document,
}

impl Kind {
    pub fn index(self) -> i32 {
        match self {
            Kind::Pdf => 0,
            Kind::Image => 1,
            Kind::Document => 2,
        }
    }
}

/// Lowercase extension without the dot ("" if none).
pub fn ext(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub fn kind_of(p: &Path) -> Kind {
    match ext(p).as_str() {
        "pdf" => Kind::Pdf,
        "jpg" | "jpeg" | "png" | "webp" | "tif" | "tiff" | "bmp" | "gif" => Kind::Image,
        _ => Kind::Document,
    }
}

/// Whether `tool` takes `p`, judged by extension against the catalogue.
pub fn accepts(tool: ToolId, p: &Path) -> bool {
    let e = ext(p);
    tools::find(tool).accepts.contains(&e.as_str())
}

/// The toast shown for a rejected file.
pub fn rejection(tool: ToolId, p: &Path) -> String {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    format!("Not supported by {}: {name}", tools::find(tool).name)
}

/// Where a file dropped on the window goes.
#[derive(Debug, PartialEq, Eq)]
pub enum DropAction {
    /// Add it to the active tool tab.
    AddToTab,
    /// On Home with a PDF: keep it and show Tools so the user picks one.
    PickTool,
    /// Show this toast.
    Reject(String),
}

/// Routes a dropped file. `active` is the active tool tab's tool, `None` on Home.
pub fn route_drop(active: Option<ToolId>, p: &Path) -> DropAction {
    if p.is_dir() {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return DropAction::Reject(format!("Folders cannot be added: {name}"));
    }
    match active {
        Some(t) if accepts(t, p) => DropAction::AddToTab,
        Some(t) => DropAction::Reject(rejection(t, p)),
        None if kind_of(p) == Kind::Pdf => DropAction::PickTool,
        None => {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            DropAction::Reject(format!("Open a tool first to add {name}"))
        }
    }
}

/// "3.17 MB", "845 KB", "512 B" (binary units, as Finder's "MB" label is read by most users).
pub fn human_size(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if b >= K * K * K {
        format!("{:.2} GB", b / (K * K * K))
    } else if b >= K * K {
        format!("{:.2} MB", b / (K * K))
    } else if b >= K {
        format!("{:.0} KB", b / K)
    } else {
        format!("{bytes} B")
    }
}

/// Card caption: "3.17 MB - 7 pages", or the size alone when the page count is unknown.
pub fn caption(bytes: u64, pages: Option<u32>) -> String {
    match pages {
        Some(1) => format!("{} - 1 page", human_size(bytes)),
        Some(n) => format!("{} - {n} pages", human_size(bytes)),
        None => human_size(bytes),
    }
}

/// Merge: what a file's page choice reads as on its card ("All 7 pages", "Pages 1-3, 5 · 4 of 7"),
/// and whether it is valid for its page count (unknown yet: taken as valid).
pub fn pages_label(spec: &str, pages: Option<u32>) -> (String, bool) {
    let spec = spec.trim();
    match (spec.is_empty(), pages) {
        (true, Some(1)) => ("1 page".into(), true),
        (true, Some(n)) => (format!("All {n} pages"), true),
        (true, None) => ("All pages".into(), true),
        (false, None) => (format!("Pages {spec}"), true),
        (false, Some(n)) => match sheaf_core::ranges::parse_ranges(spec, n) {
            Ok(r) => {
                let count: u32 = r.iter().map(|r| r.end() - r.start() + 1).sum();
                (format!("Pages {spec} · {count} of {n}"), true)
            }
            Err(_) => (format!("Pages {spec}: only 1–{n}"), false),
        },
    }
}

pub fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// RGBA pixels of a thumbnail.
#[derive(Debug, Clone, Default)]
pub struct Thumb {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Page count and page-1 thumbnail of an Office or HTML file, converted to a PDF by LibreOffice,
/// one file at a time (each LibreOffice is a few hundred MB). The PDF is kept for the conversion
/// itself ([`office_to_pdf`]), so a file is converted once.
pub fn document_probe(path: &Path, width: i32) -> Result<(u32, Thumb)> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let pdf = match office_cached(path) {
        Some(p) => p,
        None => {
            let dir = tempfile::Builder::new().prefix("sheaf-office-probe-").tempdir()?;
            let pdf = sheaf_core::office::office_to_pdf(path, &dir.path().join("probe.pdf"))?;
            office_cache_put(path, dir);
            pdf
        }
    };
    probe_pdf(&Input::new(pdf), width)
}

/// An Office file as it is on disk: its path, size and modification time.
type OfficeKey = (std::path::PathBuf, u64, std::time::SystemTime);

/// PDFs of Office files made for their cards, at most [`OFFICE_CACHE`] (the oldest go).
static OFFICE_PDFS: std::sync::Mutex<Vec<(OfficeKey, tempfile::TempDir)>> = std::sync::Mutex::new(Vec::new());
const OFFICE_CACHE: usize = 64;

fn office_key(path: &Path) -> Option<OfficeKey> {
    let m = std::fs::metadata(path).ok()?;
    Some((path.to_path_buf(), m.len(), m.modified().ok()?))
}

fn office_cached(path: &Path) -> Option<std::path::PathBuf> {
    let key = office_key(path)?;
    let cache = OFFICE_PDFS.lock().unwrap_or_else(|e| e.into_inner());
    cache.iter().find(|(k, _)| *k == key).map(|(_, d)| d.path().join("probe.pdf")).filter(|p| p.is_file())
}

fn office_cache_put(path: &Path, dir: tempfile::TempDir) {
    let Some(key) = office_key(path) else { return };
    let mut cache = OFFICE_PDFS.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(k, _)| k.0 != key.0);
    if cache.len() >= OFFICE_CACHE {
        cache.remove(0);
    }
    cache.push((key, dir));
}

/// Office or HTML file to PDF at `out` (or a free name beside it, written whole or not at all, as
/// every output): a copy of the PDF its card was made from when the file has not changed since,
/// else converted by LibreOffice (also when that copy has gone meanwhile).
pub fn office_to_pdf(path: &Path, out: &Path) -> Result<std::path::PathBuf> {
    if let Some(pdf) = office_cached(path)
        && let Ok(written) = sheaf_core::paths::atomic_write(out, |part| {
            std::fs::copy(&pdf, part)?;
            Ok(())
        })
    {
        return Ok(written);
    }
    sheaf_core::office::office_to_pdf(path, out)
}

/// The PDFs kept for cards are deleted (the app quits).
pub fn clear_office_cache() {
    OFFICE_PDFS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// Names compared as a person would: digits as numbers ("file 2" before "file 10"), letters
/// without regard to case.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let num = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (n, m) = (num(&mut a), num(&mut b));
                let (n2, m2) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let o = n2.len().cmp(&m2.len()).then_with(|| n2.cmp(m2));
                if o.is_ne() {
                    return o;
                }
            }
            (Some(x), Some(y)) => {
                let o = x.to_lowercase().cmp(y.to_lowercase());
                if o.is_ne() {
                    return o;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Page count and page-1 thumbnail (`width` px wide) of a PDF. Takes the pdfium lock;
/// call it from a worker thread. Errors include `PasswordRequired` / `WrongPassword`.
pub fn probe_pdf(input: &Input, width: i32) -> Result<(u32, Thumb)> {
    probe_pdf_full(input, width).map(|p| (p.pages, p.thumb))
}

/// Page `page` (0-based) of `input` rendered `width` pixels wide.
pub fn page_thumb(input: &Input, page: usize, width: i32) -> Result<Thumb> {
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open()?;
    let p = doc.pages().get(page as i32).map_err(|e| OpdfError::Unsupported(format!("page {}: {e}", page + 1)))?;
    let bmp = p.render_with_config(&pdfium_render::prelude::PdfRenderConfig::new().set_target_width(width))?;
    Ok(Thumb { width: bmp.width() as u32, height: bmp.height() as u32, rgba: bmp.as_rgba_bytes() })
}

/// What a PDF's probe finds: its page count, page 1's thumbnail, and whether it looks scanned
/// (its first pages carry next to no text: PDF to Office needs OCR).
pub struct Probe {
    pub pages: u32,
    pub thumb: Thumb,
    pub scanned: bool,
}

/// Fewer characters than this on each of the first pages: a scan without a text layer.
const SCANNED_CHARS: usize = 20;

pub fn probe_pdf_full(input: &Input, width: i32) -> Result<Probe> {
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open()?;
    let pages = doc.pages().len() as u32;
    let page = doc.pages().first().map_err(|e| OpdfError::Unsupported(format!("no pages: {e}")))?;
    let bmp = page.render_with_config(
        &pdfium_render::prelude::PdfRenderConfig::new().set_target_width(width),
    )?;
    let scanned = doc.pages().iter().take(3).all(|p| p.text().map(|t| t.all().chars().filter(|c| !c.is_whitespace()).count()).unwrap_or(0) < SCANNED_CHARS);
    Ok(Probe { pages, thumb: Thumb { width: bmp.width() as u32, height: bmp.height() as u32, rgba: bmp.as_rgba_bytes() }, scanned })
}

/// Why `name` cannot be opened, in one short line, the same in the reader and the tools: a file
/// the engines cannot read is "damaged or not a PDF" whatever their own wording.
/// Whether the file at `p` starts as a PDF does ("%PDF-" in its first kilobyte): a damaged one,
/// worth repairing, rather than another kind of file.
pub fn looks_like_pdf(p: &std::path::Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 1024];
    let n = std::fs::File::open(p).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
    head[..n].windows(5).any(|w| w == b"%PDF-")
}

pub fn open_error(name: &str, e: &OpdfError) -> String {
    match e {
        OpdfError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => format!("{name} is no longer there."),
        OpdfError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => format!("{name} can't be read: permission denied."),
        OpdfError::Pdfium(_) | OpdfError::Lopdf(_) | OpdfError::Engine(_) => format!("{name} can't be opened: it is damaged or not a PDF."),
        e => format!("{name} can't be opened: {e}."),
    }
}

/// What to do with a PDF whose probe (page count + thumbnail) failed.
#[derive(Debug, PartialEq, Eq)]
pub enum ProbeFailure {
    /// Show the password dialog (`wrong`: a password was tried).
    AskPassword { wrong: bool },
    /// Repair: keep the card ("Can't preview — will try to repair"); the job falls back to lopdf.
    KeepForRepair,
    /// The probe panicked: keep the card with "preview unavailable"; the job decides.
    KeepUnpreviewed,
    /// Toast the error and drop the file: the tool cannot use a PDF pdfium refuses.
    Remove,
}

/// Routes a failed probe. `panicked`: the probe crashed rather than returning `err`.
pub fn probe_failure(tool: ToolId, err: &OpdfError, panicked: bool) -> ProbeFailure {
    match err {
        OpdfError::PasswordRequired(_) => ProbeFailure::AskPassword { wrong: false },
        OpdfError::WrongPassword(_) => ProbeFailure::AskPassword { wrong: true },
        _ if tool == ToolId::Repair => ProbeFailure::KeepForRepair,
        _ if panicked => ProbeFailure::KeepUnpreviewed,
        _ => ProbeFailure::Remove,
    }
}

/// Thumbnails of every page of a PDF (Organize grid), `width` px wide, in page order.
pub fn page_thumbs(input: &Input, width: i32, mut each: impl FnMut(u32, Thumb)) -> Result<()> {
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open()?;
    let cfg = pdfium_render::prelude::PdfRenderConfig::new().set_target_width(width);
    for (i, page) in doc.pages().iter().enumerate() {
        let bmp = page.render_with_config(&cfg)?;
        each(i as u32 + 1, Thumb { width: bmp.width() as u32, height: bmp.height() as u32, rgba: bmp.as_rgba_bytes() });
    }
    Ok(())
}

/// Page 1 rendered `width` px wide, with its displayed size in points (Crop preview).
pub fn first_page_preview(input: &Input, width: i32) -> Result<(Thumb, f32, f32)> {
    page_preview(input, 0, width)
}

/// Page `index` (0-based) at `width` pixels, with its size in points (Crop's preview of any page).
pub fn page_preview(input: &Input, index: u16, width: i32) -> Result<(Thumb, f32, f32)> {
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open()?;
    let page = doc.pages().get(index).map_err(|e| OpdfError::Unsupported(format!("no page {}: {e}", index + 1)))?;
    let (w, h) = (page.width().value, page.height().value);
    let bmp = page.render_with_config(&pdfium_render::prelude::PdfRenderConfig::new().set_target_width(width))?;
    Ok((Thumb { width: bmp.width() as u32, height: bmp.height() as u32, rgba: bmp.as_rgba_bytes() }, w, h))
}

/// Page count of a PDF, if it opens. Takes the pdfium lock.
pub fn page_count(input: &Input) -> Option<u32> {
    let _g = sheaf_core::engine::pdfium_lock();
    input.open().ok().map(|d| d.pages().len() as u32)
}

/// Card thumbnail size for images, in pixels (long side).
pub const IMAGE_THUMB_PX: u32 = 120;

/// An image scaled to fit `max` px on its long side. The full-size decode is dropped as soon
/// as the thumbnail exists, so only one full image per probe thread is ever in memory.
pub fn image_thumb(p: &Path, max: u32) -> Result<Thumb> {
    let full = image::ImageReader::open(p)?.with_guessed_format()?.decode()?;
    let small = full.thumbnail(max, max);
    drop(full);
    let img = small.to_rgba8();
    Ok(Thumb { width: img.width(), height: img.height(), rgba: img.into_raw() })
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_merge_page_choice_reads_as_its_count_or_says_what_is_wrong() {
        assert_eq!(pages_label("", Some(7)), ("All 7 pages".into(), true));
        assert_eq!(pages_label(" ", Some(1)), ("1 page".into(), true));
        assert_eq!(pages_label("1-3, 5", Some(7)), ("Pages 1-3, 5 · 4 of 7".into(), true));
        assert_eq!(pages_label("9", Some(7)), ("Pages 9: only 1–7".into(), false));
        assert!(!pages_label("abc", Some(7)).1);
        assert_eq!(pages_label("2", None), ("Pages 2".into(), true));
    }
    use super::*;

    #[test]
    fn files_that_cannot_open_are_told_in_one_short_line() {
        let gone = OpdfError::Io(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert_eq!(open_error("a.pdf", &gone), "a.pdf is no longer there.");
        assert_eq!(open_error("a.pdf", &OpdfError::Engine("xref broken at 12".into())), "a.pdf can't be opened: it is damaged or not a PDF.");
        let denied = OpdfError::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert_eq!(open_error("a.pdf", &denied), "a.pdf can't be read: permission denied.");
    }
    use std::path::PathBuf;

    #[test]
    fn drops_are_routed_by_tool_and_extension() {
        let pdf = PathBuf::from("/x/a.PDF");
        let docx = PathBuf::from("/x/letter.docx");
        assert_eq!(route_drop(Some(ToolId::Merge), &pdf), DropAction::AddToTab);
        // Merge takes Word files too (iLovePDF merges mixed files), not a zip.
        assert_eq!(route_drop(Some(ToolId::Merge), &docx), DropAction::AddToTab);
        assert_eq!(
            route_drop(Some(ToolId::Merge), &PathBuf::from("/x/a.zip")),
            DropAction::Reject("Not supported by Merge PDF: a.zip".into())
        );
        assert_eq!(route_drop(Some(ToolId::WordToPdf), &docx), DropAction::AddToTab);
        assert_eq!(route_drop(None, &pdf), DropAction::PickTool);
        assert!(matches!(route_drop(None, &docx), DropAction::Reject(_)));
        assert_eq!(route_drop(Some(ToolId::JpgToPdf), Path::new("/x/p.jpeg")), DropAction::AddToTab);
        // Scans are often .tif; GIFs too (as Add image takes them).
        assert_eq!(route_drop(Some(ToolId::JpgToPdf), Path::new("/x/scan.tif")), DropAction::AddToTab);
        assert_eq!(route_drop(Some(ToolId::JpgToPdf), Path::new("/x/a.gif")), DropAction::AddToTab);
        assert_eq!(kind_of(Path::new("/x/scan.TIF")), Kind::Image);
    }

    #[test]
    fn an_office_file_converted_for_its_card_is_copied_until_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("a.docx");
        std::fs::write(&doc, b"first").unwrap();
        let made = tempfile::Builder::new().prefix("sheaf-office-probe-").tempdir().unwrap();
        std::fs::write(made.path().join("probe.pdf"), b"%PDF-cached").unwrap();
        office_cache_put(&doc, made);
        // A PDF of that name is already there: kept, the copy goes beside it.
        let out = dir.path().join("a.pdf");
        std::fs::write(&out, b"%PDF-mine").unwrap();
        let written = office_to_pdf(&doc, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"%PDF-mine");
        assert_eq!(written, dir.path().join("a (1).pdf"));
        assert_eq!(std::fs::read(&written).unwrap(), b"%PDF-cached");
        // Changed since (size differs): not the old PDF.
        std::fs::write(&doc, b"second version").unwrap();
        assert!(office_cached(&doc).is_none());
        clear_office_cache();
    }

    #[test]
    fn names_sort_as_a_person_would() {
        let mut v = vec!["file 10.docx", "File 2.docx", "file 1.docx", "abc.pptx", "file 02b.docx"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["abc.pptx", "file 1.docx", "File 2.docx", "file 02b.docx", "file 10.docx"]);
    }

    #[test]
    fn probe_failures_are_routed_per_tool() {
        let p = || PathBuf::from("/x/a.pdf");
        let bad = || OpdfError::Conversion("bad xref".into());
        assert_eq!(probe_failure(ToolId::Repair, &bad(), false), ProbeFailure::KeepForRepair);
        assert_eq!(probe_failure(ToolId::Repair, &bad(), true), ProbeFailure::KeepForRepair);
        assert_eq!(probe_failure(ToolId::Merge, &bad(), false), ProbeFailure::Remove);
        assert_eq!(probe_failure(ToolId::Merge, &bad(), true), ProbeFailure::KeepUnpreviewed);
        assert_eq!(probe_failure(ToolId::Compress, &OpdfError::Unsupported("no pages".into()), false), ProbeFailure::Remove);
        for t in [ToolId::Repair, ToolId::Merge] {
            assert_eq!(probe_failure(t, &OpdfError::PasswordRequired(p()), false), ProbeFailure::AskPassword { wrong: false });
            assert_eq!(probe_failure(t, &OpdfError::WrongPassword(p()), false), ProbeFailure::AskPassword { wrong: true });
        }
        // Sign asks for the password too: the signed file keeps its protection.
        assert_eq!(probe_failure(ToolId::Sign, &OpdfError::PasswordRequired(p()), false), ProbeFailure::AskPassword { wrong: false });
        assert_eq!(probe_failure(ToolId::Sign, &bad(), false), ProbeFailure::Remove);
    }

    #[test]
    fn a_page_that_is_only_a_picture_is_seen_as_scanned() {
        let t = tempfile::tempdir().unwrap();
        let png = t.path().join("x.png");
        image::RgbImage::from_pixel(8, 8, image::Rgb([0, 0, 0])).save(&png).unwrap();
        let a = sheaf_core::images::images_to_pdf(&[png], sheaf_core::images::PageLayout::FitImage, &t.path().join("a.pdf")).unwrap();
        // A page that is only a picture is a scan; the invoice has its text.
        assert!(probe_pdf_full(&Input::new(&a), 40).unwrap().scanned);
        let invoice = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sheaf-core/tests/fixtures/invoice.pdf");
        assert!(!probe_pdf_full(&Input::new(&invoice), 40).unwrap().scanned);
    }

    #[test]
    fn sizes_and_captions() {
        assert_eq!(human_size(3_322_740), "3.17 MB");
        assert_eq!(human_size(2048), "2 KB");
        assert_eq!(caption(3_322_740, Some(7)), "3.17 MB - 7 pages");
        assert_eq!(caption(100, Some(1)), "100 B - 1 page");
        assert_eq!(caption(100, None), "100 B");
    }
}
