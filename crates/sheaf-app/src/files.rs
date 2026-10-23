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
        "jpg" | "jpeg" | "png" | "webp" | "tiff" | "bmp" => Kind::Image,
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

pub fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// RGBA pixels of a thumbnail.
pub struct Thumb {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Page count and page-1 thumbnail of an Office or HTML file, converted to a PDF in a folder of
/// its own (removed after) by LibreOffice.
pub fn document_probe(path: &Path, width: i32) -> Result<(u32, Thumb)> {
    let dir = tempfile::Builder::new().prefix("sheaf-office-probe-").tempdir()?;
    let pdf = sheaf_core::office::office_to_pdf(path, &dir.path().join("probe.pdf"))?;
    probe_pdf(&Input::new(pdf), width)
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
    probe_pdf_full(input, width).map(|(n, t, _)| (n, t))
}

/// [`probe_pdf`], and whether the file is password-protected (user or owner password).
pub fn probe_pdf_full(input: &Input, width: i32) -> Result<(u32, Thumb, bool)> {
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open()?;
    let protected = sheaf_core::engine::is_protected(&doc);
    let pages = doc.pages().len() as u32;
    let page = doc.pages().first().map_err(|e| OpdfError::Unsupported(format!("no pages: {e}")))?;
    let bmp = page.render_with_config(
        &pdfium_render::prelude::PdfRenderConfig::new().set_target_width(width),
    )?;
    Ok((pages, Thumb { width: bmp.width() as u32, height: bmp.height() as u32, rgba: bmp.as_rgba_bytes() }, protected))
}

/// Why `name` cannot be opened, in one short line, the same in the reader and the tools: a file
/// the engines cannot read is "damaged or not a PDF" whatever their own wording.
pub fn open_error(name: &str, e: &OpdfError) -> String {
    match e {
        OpdfError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => format!("{name} is no longer there."),
        OpdfError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => format!("{name} can't be read: permission denied."),
        OpdfError::Pdfium(_) | OpdfError::Lopdf(_) | OpdfError::Engine(_) => format!("{name} can't be opened: it is damaged or not a PDF."),
        e => format!("{name} can't be opened: {e}."),
    }
}

/// Sign never asks for a password: a signature must not silently remove protection, so a
/// protected file is refused as soon as it is added.
pub const SIGN_PROTECTED: &str = "Unlock the PDF first, then sign it";

/// Whether `tool` refuses a password-protected file at once (no password dialog).
pub fn refuses_protected(tool: ToolId) -> bool {
    tool == ToolId::Sign
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
    /// Toast [`SIGN_PROTECTED`] and drop the file: the tool does not take protected files.
    RefuseProtected,
}

/// Routes a failed probe. `panicked`: the probe crashed rather than returning `err`.
pub fn probe_failure(tool: ToolId, err: &OpdfError, panicked: bool) -> ProbeFailure {
    match err {
        OpdfError::PasswordRequired(_) | OpdfError::WrongPassword(_) if refuses_protected(tool) => ProbeFailure::RefuseProtected,
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
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open()?;
    let page = doc.pages().first().map_err(|e| OpdfError::Unsupported(format!("no pages: {e}")))?;
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
        assert_eq!(
            route_drop(Some(ToolId::Merge), &docx),
            DropAction::Reject("Not supported by Merge PDF: letter.docx".into())
        );
        assert_eq!(route_drop(Some(ToolId::WordToPdf), &docx), DropAction::AddToTab);
        assert_eq!(route_drop(None, &pdf), DropAction::PickTool);
        assert!(matches!(route_drop(None, &docx), DropAction::Reject(_)));
        assert_eq!(route_drop(Some(ToolId::JpgToPdf), Path::new("/x/p.jpeg")), DropAction::AddToTab);
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
        // Sign refuses a protected file outright: no password dialog.
        assert_eq!(probe_failure(ToolId::Sign, &OpdfError::PasswordRequired(p()), false), ProbeFailure::RefuseProtected);
        assert_eq!(probe_failure(ToolId::Sign, &OpdfError::WrongPassword(p()), false), ProbeFailure::RefuseProtected);
        assert_eq!(probe_failure(ToolId::Sign, &bad(), false), ProbeFailure::Remove);
    }

    #[test]
    fn a_file_with_only_an_owner_password_is_seen_as_protected() {
        let t = tempfile::tempdir().unwrap();
        let png = t.path().join("x.png");
        image::RgbImage::from_pixel(8, 8, image::Rgb([0, 0, 0])).save(&png).unwrap();
        let a = sheaf_core::images::images_to_pdf(&[png], sheaf_core::images::PageLayout::FitImage, &t.path().join("a.pdf")).unwrap();
        assert!(!probe_pdf_full(&Input::new(&a), 40).unwrap().2);
        // Opens without a password (no dialog would be shown), yet is protected.
        let p = t.path().join("p.pdf");
        let st = std::process::Command::new("qpdf").args(["--encrypt", "", "owner", "256", "--"]).arg(&a).arg(&p).status().unwrap();
        assert!(st.success());
        assert!(probe_pdf_full(&Input::new(&p), 40).unwrap().2);
        assert!(refuses_protected(ToolId::Sign) && !refuses_protected(ToolId::Edit));
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
