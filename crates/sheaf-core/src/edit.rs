//! Edit PDF: text and images added on top of pages, through PdfCraft's `Edit::AddText` and
//! `Edit::AddImage`.
//!
//! PdfCraft writes each added item as its own content stream on the page (plain page content,
//! not an annotation), so the result is already flat: every viewer shows it and nothing is left
//! to fill or flatten. Items are drawn upright as the page is displayed, also on a page with
//! `/Rotate` or a CropBox that does not start at the origin.
//!
//! A password-protected input gives an unprotected output, which the caller must say.

use crate::pdfcraft::engine::{AddedText, Edit, FontFamily};
use crate::pdfcraft::{render::PageInfo, Doc};
use crate::sign::{user_rect_to_view, view_rect_to_user};
use crate::{pages::Input, OpdfError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One thing to add.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A line (or lines, split at `\n`) of Helvetica. `at` is the top-left corner of the text as
    /// the page is displayed, in PDF user space; `font_size` is in points, 1–500.
    Text { page: usize, at: [f64; 2], text: String, font_size: f64, color: [u8; 3] },
    /// A PNG or JPEG stretched to `rect` (`[x0, y0, x1, y1]` in PDF user space), upright as the
    /// page is displayed.
    Image { page: usize, rect: [f64; 4], path: PathBuf },
}

impl Item {
    pub fn page(&self) -> usize {
        match self {
            Item::Text { page, .. } | Item::Image { page, .. } => *page,
        }
    }
}

/// Everything to add, drawn in order (a later item over an earlier one).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditSpec {
    pub items: Vec<Item>,
}

/// What [`edit`] wrote.
#[derive(Debug, Clone)]
pub struct EditReport {
    pub path: PathBuf,
    /// Items added.
    pub added: usize,
    /// The input was password-protected; the output is not.
    pub was_encrypted: bool,
}

impl EditReport {
    /// The result line, in Sheaf's CLI and app alike.
    pub fn summary(&self) -> String {
        format!("{} added", crate::tools::count(self.added, "item"))
    }
}

/// The default font size, in points.
pub const DEFAULT_FONT_SIZE: f64 = 12.0;

/// Height of one line of text, in em (PdfCraft's line spacing).
const LINE: f64 = 1.2;

/// The width and height in points of `text` at `font_size`, as it will be drawn (Helvetica,
/// one line per `\n`).
pub fn text_extent(text: &str, font_size: f64) -> (f64, f64) {
    let lines: Vec<&str> = text.split('\n').collect();
    let w = lines.iter().map(|l| FontFamily::Helvetica.width(l, font_size, false, false)).fold(0.0, f64::max);
    (w, lines.len() as f64 * font_size * LINE)
}

/// A view-space point (points from the displayed page's top-left, y down, as the page viewer
/// uses) → PDF user space.
pub fn view_to_user(page: &PageInfo, p: [f64; 2]) -> [f64; 2] {
    page.view_to_user(p[0] as f32, p[1] as f32).map(f64::from)
}

/// PDF user space → view space.
pub fn user_to_view(page: &PageInfo, p: [f64; 2]) -> [f64; 2] {
    page.user_to_view(p[0] as f32, p[1] as f32).map(f64::from)
}

/// The user-space box of an image `width` points wide and `width * aspect` tall as displayed,
/// its top-left corner at `top_left` (view space).
pub fn image_rect(page: &PageInfo, top_left: [f64; 2], width: f64, aspect: f64) -> [f64; 4] {
    let [x, y] = top_left;
    view_rect_to_user(page, [x, y, x + width, y + width * aspect])
}

/// The view-space box of an item already placed in user space.
pub fn item_view_rect(page: &PageInfo, item: &Item) -> [f64; 4] {
    match item {
        Item::Text { at, text, font_size, .. } => {
            let [x, y] = user_to_view(page, *at);
            let (w, h) = text_extent(text, *font_size);
            [x, y, x + w, y + h]
        }
        Item::Image { rect, .. } => user_rect_to_view(page, *rect),
    }
}

/// A checked PNG or JPEG: its bytes, its upright size in pixels and its upright pixels (EXIF
/// orientation applied, as PdfCraft embeds it), decoded once.
pub struct Picture {
    pub bytes: Arc<Vec<u8>>,
    pub size: (u32, u32),
    pub rgba: image::RgbaImage,
}

impl Picture {
    /// Reads `path`, refusing anything but a PNG or JPEG that decodes.
    pub fn read(path: &Path) -> Result<Picture> {
        use image::ImageDecoder;
        let bad = |why: &str| OpdfError::Unsupported(format!("{}: {why}", path.display()));
        let bytes = std::fs::read(path).map_err(|e| bad(&e.to_string()))?;
        let format = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            image::ImageFormat::Png
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            image::ImageFormat::Jpeg
        } else {
            return Err(bad("not a PNG or JPEG image"));
        };
        // Decoded whole: a file whose header is fine but whose pixels are not is refused here,
        // before anything is written.
        let mut img = {
            let mut dec = image::ImageReader::with_format(std::io::Cursor::new(&bytes), format).into_decoder().map_err(|_| bad("the image is damaged"))?;
            let orientation = dec.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
            let mut img = image::DynamicImage::from_decoder(dec).map_err(|_| bad("the image is damaged"))?;
            img.apply_orientation(orientation);
            img
        };
        if img.width() == 0 || img.height() == 0 {
            return Err(bad("the image is empty"));
        }
        let rgba = std::mem::take(&mut img).into_rgba8();
        Ok(Picture { bytes: Arc::new(bytes), size: rgba.dimensions(), rgba })
    }

    /// Height over width, as displayed.
    pub fn aspect(&self) -> f64 {
        self.size.1 as f64 / self.size.0 as f64
    }
}

/// The first character of `text` that Helvetica can't draw (outside WinAnsiEncoding), if any.
pub fn undrawable(text: &str) -> Option<char> {
    pdfcraft_fonts::first_non_win_ansi(&text.replace('\n', ""))
}

/// Why `text` can't be added, if it can't: a message the user can act on.
pub fn text_problem(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return Some("the text is empty: type some text first".into());
    }
    undrawable(text).map(|c| format!("\"{c}\" can't be drawn in Helvetica: use Latin letters, digits and common punctuation"))
}

/// Page `page` (0-based) of `input`, as displayed.
pub fn page_info(input: &Input, page: usize) -> Result<PageInfo> {
    let doc = Doc::open(input)?;
    let pages = &doc.document().info.pages;
    pages.get(page).cloned().ok_or_else(|| no_page(page, pages.len()))
}

/// Refuses an item whose view-space box `v` lies wholly outside the displayed page.
fn on_page(info: &PageInfo, page: usize, v: [f64; 4]) -> Result<()> {
    let (w, h) = (info.width as f64, info.height as f64);
    if v[2] <= 0.0 || v[3] <= 0.0 || v[0] >= w || v[1] >= h {
        return Err(OpdfError::Unsupported(format!(
            "an item on page {} is outside the page (the page is {} x {} points as displayed)",
            page + 1,
            w.round(),
            h.round()
        )));
    }
    Ok(())
}

fn no_page(page: usize, n: usize) -> OpdfError {
    OpdfError::Unsupported(format!("page {} does not exist (the document has {n} pages)", page + 1))
}

/// Adds `spec`'s items to `input` and writes the result to `out` (never overwriting). Nothing is
/// written when any item is refused (an empty text, a size out of range, a bad image).
pub fn edit(input: &Input, spec: &EditSpec, out: &Path) -> Result<EditReport> {
    if spec.items.is_empty() {
        return Err(OpdfError::Unsupported("nothing to add: place some text or an image first".into()));
    }
    let mut doc = Doc::open(input)?;
    let pages = doc.document().info.pages.clone();
    let mut edits = Vec::with_capacity(spec.items.len());
    for item in &spec.items {
        let info = pages.get(item.page()).ok_or_else(|| no_page(item.page(), pages.len()))?;
        // PdfCraft's boxes are in display space: points from the displayed page's bottom-left, y up.
        let display = |v: [f64; 4]| [v[0], info.height as f64 - v[3], v[2], info.height as f64 - v[1]];
        edits.push(match item {
            Item::Text { page, at, text, font_size, color } => {
                if let Some(why) = text_problem(text) {
                    return Err(OpdfError::Unsupported(format!("a text item on page {}: {why}", page + 1)));
                }
                if !(font_size.is_finite() && (1.0..=500.0).contains(font_size)) {
                    return Err(OpdfError::Unsupported(format!("font size {font_size} (use 1 to 500 points)")));
                }
                if !at.iter().all(|v| v.is_finite()) {
                    return Err(OpdfError::Unsupported("a text item has no position".into()));
                }
                let [x, y] = user_to_view(info, *at);
                let (w, h) = text_extent(text, *font_size);
                // A little wider than the widest line, so PdfCraft never wraps it.
                on_page(info, *page, [x, y, x + w, y + h])?;
                let rect = display([x, y, x + w + font_size, y + h]);
                Edit::AddText {
                    page: *page,
                    text: AddedText { rect, text: text.clone(), size: *font_size, color: color.map(|c| f64::from(c) / 255.0), ..Default::default() },
                }
            }
            Item::Image { page, rect, path } => {
                let r = *rect;
                if !(r.iter().all(|v| v.is_finite()) && r[2] - r[0] >= 1.0 && r[3] - r[1] >= 1.0) {
                    return Err(OpdfError::Unsupported("an image box is empty".into()));
                }
                on_page(info, *page, user_rect_to_view(info, r))?;
                let pic = Picture::read(path)?;
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "image".into());
                Edit::AddImage { page: *page, rect: Some(display(user_rect_to_view(info, r))), name, bytes: pic.bytes }
            }
        });
    }
    let added = edits.len();
    doc.apply(Edit::Batch { label: "Edit PDF".into(), edits }).map_err(|e| match e {
        OpdfError::Engine(m) => OpdfError::Unsupported(m),
        e => e,
    })?;
    let (path, was_encrypted) = doc.save_unprotected(out)?;
    Ok(EditReport { path, added, was_encrypted })
}
