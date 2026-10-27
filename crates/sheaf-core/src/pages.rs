use crate::{engine::{is_protected, open_pdf, pdfium, pdfium_lock}, paths::{all_or_nothing, atomic_write}, ranges::parse_ranges, OpdfError, Result};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Input { pub path: PathBuf, pub password: Option<String> }
impl Input {
    pub fn new(p: impl Into<PathBuf>) -> Self { Self { path: p.into(), password: None } }
    /// Opens with pdfium. The caller must hold `pdfium_lock()` for the document's whole lifetime.
    pub fn open(&self) -> Result<PdfDocument<'static>> { open_pdf(&self.path, self.password.as_deref()) }
    /// Loads with lopdf, decrypted in memory. A decrypted document comes back without its
    /// /Encrypt dictionary, and its `max_id` is reset to the highest object number left, so the
    /// /Size written on save matches the objects (qpdf warns otherwise).
    pub fn load_lopdf(&self) -> Result<lopdf::Document> {
        let mut d = self.load_lopdf_raw()?;
        if d.was_encrypted() {
            d.max_id = d.objects.keys().map(|id| id.0).max().unwrap_or(0);
        }
        Ok(d)
    }
    fn load_lopdf_raw(&self) -> Result<lopdf::Document> {
        match &self.password {
            Some(pw) => lopdf::Document::load_with_password(&self.path, pw).map_err(|e| match e {
                lopdf::Error::InvalidPassword => OpdfError::WrongPassword(self.path.clone()),
                e => e.into(),
            }),
            None => {
                let d = lopdf::Document::load(&self.path)?;
                if d.is_encrypted() { return Err(OpdfError::PasswordRequired(self.path.clone())); }
                Ok(d)
            }
        }
    }
    /// Whether the file is password-protected (user or owner password). A password error counts
    /// as protected; any other open failure counts as protected only if a password was given.
    /// Takes the pdfium lock.
    pub fn is_encrypted(&self) -> bool {
        let _g = pdfium_lock();
        match self.open() {
            Ok(d) => is_protected(&d),
            Err(OpdfError::PasswordRequired(_) | OpdfError::WrongPassword(_)) => true,
            Err(_) => self.password.is_some(),
        }
    }
    pub fn stem(&self) -> String { self.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "document".into()) }
}

fn save(doc: &PdfDocument, out: &Path) -> Result<PathBuf> {
    atomic_write(out, |part| Ok(doc.save_to_file(part)?))
}

/// 1-based inclusive ranges → pdfium page string ("1-2,5").
fn page_string(ranges: &[std::ops::RangeInclusive<u32>]) -> String {
    ranges.iter().map(|r| if r.start() == r.end() { r.start().to_string() } else { format!("{}-{}", r.start(), r.end()) })
        .collect::<Vec<_>>().join(",")
}

pub fn merge(inputs: &[Input], out: &Path) -> Result<PathBuf> { merge_pages(inputs, &[], out) }

/// Merge taking, from each input, the pages of its range spec ("4, 1-2": in the order written);
/// an empty or missing spec takes every page (Acrobat's Combine picks pages per file).
pub fn merge_pages(inputs: &[Input], pages: &[String], out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let mut dst = pdfium().create_new_pdf()?;
    for (k, i) in inputs.iter().enumerate() {
        let src = i.open()?;
        match pages.get(k).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            None => dst.pages_mut().append(&src)?,
            Some(spec) => {
                let ranges = parse_ranges(spec, src.pages().len() as u32)?;
                let at = dst.pages().len();
                dst.pages_mut().copy_pages_from_document(&src, &page_string(&ranges), at)?;
            }
        }
    }
    save(&dst, out)
}

/// What the clipboard holds, for New PDF from clipboard.
pub enum Pasted {
    Image { width: u32, height: u32, rgba: Vec<u8> },
    Text(String),
}

/// A new PDF of the clipboard at `out` (Acrobat's Create PDF from Clipboard, Preview's New from
/// Clipboard): a picture on a page of its own size; text on US Letter pages in 11 pt Helvetica.
pub fn from_pasted(p: &Pasted, out: &Path) -> Result<PathBuf> {
    let session = crate::pdfcraft::engine::Session::new();
    let bytes = match p {
        Pasted::Image { width, height, rgba } => {
            let img = image::RgbaImage::from_raw(*width, *height, rgba.clone()).ok_or_else(|| OpdfError::Unsupported("the picture on the clipboard is damaged".into()))?;
            let mut png = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| OpdfError::Conversion(e.to_string()))?;
            session.create_from_images(&[("Clipboard.png".into(), png)])
        }
        Pasted::Text(t) if !t.trim().is_empty() => session.create_from_text("Clipboard", t),
        Pasted::Text(_) => return Err(OpdfError::Unsupported("the clipboard holds no picture and no text".into())),
    }
    .map_err(|e| OpdfError::Engine(e.to_string()))?;
    crate::paths::atomic_write(out, |part| Ok(std::fs::write(part, bytes.as_slice())?))
}

/// Paper sizes a page can be changed to (Acrobat's Set Page Boxes ▸ Change Page Size), in points:
/// A4, US Letter, A3, A5, US Legal.
pub const PAPER: [(&str, f64, f64); 5] = [("A4", 595.0, 842.0), ("Letter", 612.0, 792.0), ("A3", 842.0, 1191.0), ("A5", 420.0, 595.0), ("Legal", 612.0, 1008.0)];

/// Change Page Size: the pages chosen (`pages` a range, none: all) become `size` points (width,
/// height as the page is seen: a turned page takes it turned), their content scaled to fit and
/// centred (Acrobat's Change Page Size, Preview's Scale). Written to `out`; the output is not
/// password-protected. Comments are drawn into the pages, scaled with them.
pub fn resize(input: &Input, pages: Option<&str>, size: (f64, f64), out: &Path) -> Result<PathBuf> {
    if !(size.0 > 0.0 && size.1 > 0.0 && size.0.is_finite() && size.1.is_finite()) {
        return Err(OpdfError::Unsupported("the page size must be more than 0".into()));
    }
    let _g = pdfium_lock();
    let src = input.open()?;
    let total = src.pages().len() as u32;
    let chosen: Vec<i32> = match pages.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => {
            let ranges = parse_ranges(s, total)?;
            (1..=total).filter(|n| ranges.iter().any(|r| r.contains(n))).map(|n| n as i32 - 1).collect()
        }
        None => (0..total as i32).collect(),
    };
    let mut dst = pdfium().create_new_pdf()?;
    dst.pages_mut().append(&src)?;
    for i in chosen {
        let mut page = dst.pages().get(i)?;
        // Comments are drawn into the page first, so they shrink or grow with it (Sol, 26/10: a
        // highlight stayed where it was, off its words or off the page).
        page.flatten()?;
        let turned = matches!(page.rotation()?, PdfPageRenderRotation::Degrees90 | PdfPageRenderRotation::Degrees270);
        // In user space, before the page's own turn.
        let (w, h) = if turned { (size.1, size.0) } else { size };
        // What shows is scaled: the crop box when there is one, and what it hid stays hidden (Sol,
        // 26/10: from the media box, a cropped-away margin came back).
        let m = page.boundaries().crop().or_else(|_| page.boundaries().media())?.bounds;
        let (x0, y0, w0, h0) = (m.left().value as f64, m.bottom().value as f64, m.width().value as f64, m.height().value as f64);
        if w0 <= 0.0 || h0 <= 0.0 {
            continue;
        }
        let k = (w / w0).min(h / h0);
        let (dx, dy) = ((w - w0 * k) / 2.0 - x0 * k, (h - h0 * k) / 2.0 - y0 * k);
        let shown = PdfRect::new_from_values((y0 * k + dy) as f32, (x0 * k + dx) as f32, ((y0 + h0) * k + dy) as f32, ((x0 + w0) * k + dx) as f32);
        page.transform_with_clip(k as f32, 0.0, 0.0, k as f32, dx as f32, dy as f32, shown)?;
        let rect = PdfRect::new_from_values(0.0, 0.0, h as f32, w as f32);
        page.boundaries_mut().set_media(rect)?;
        page.boundaries_mut().set_crop(rect)?;
    }
    save(&dst, out)
}

/// Flip pages (a mirror image, for transfers and printing on film): the pages chosen (`pages` a
/// range, none: all) mirrored left to right (`horizontal`) or top to bottom, as seen. Written to
/// `out`, not password-protected. Comments are drawn into the pages flipped, so a highlight stays on
/// its words (a mirror is for printing).
pub fn flip(input: &Input, pages: Option<&str>, horizontal: bool, out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let src = input.open()?;
    let total = src.pages().len() as u32;
    let chosen: Vec<i32> = match pages.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => {
            let ranges = parse_ranges(s, total)?;
            (1..=total).filter(|n| ranges.iter().any(|r| r.contains(n))).map(|n| n as i32 - 1).collect()
        }
        None => (0..total as i32).collect(),
    };
    let mut dst = pdfium().create_new_pdf()?;
    dst.pages_mut().append(&src)?;
    for i in chosen {
        let mut page = dst.pages().get(i)?;
        page.flatten()?;
        // As seen: a page turned a quarter is mirrored about the other axis of its own space.
        let turned = matches!(page.rotation()?, PdfPageRenderRotation::Degrees90 | PdfPageRenderRotation::Degrees270);
        // About the middle of what shows: the crop box when there is one (Sol, 26/10: about the
        // media box, an off-centre crop left the mirrored content outside it).
        let m = page.boundaries().crop().or_else(|_| page.boundaries().media())?.bounds;
        let (x0, y0, x1, y1) = (m.left().value, m.bottom().value, m.right().value, m.top().value);
        let all = PdfRect::new_from_values(-1e6, -1e6, 1e6, 1e6);
        if horizontal != turned {
            page.transform_with_clip(-1.0, 0.0, 0.0, 1.0, x0 + x1, 0.0, all)?;
        } else {
            page.transform_with_clip(1.0, 0.0, 0.0, -1.0, 0.0, y0 + y1, all)?;
        }
    }
    save(&dst, out)
}

/// Cut each page in two (Stirling PDF's Split pages into sections; a scanned spread made pages
/// again): every page becomes its left and right halves (`across`), or its top and bottom, as
/// seen, in reading order. Written to `out`, not password-protected.
pub fn halves(input: &Input, across: bool, out: &Path) -> Result<PathBuf> {
    if across { cut(input, 2, 1, out) } else { cut(input, 1, 2, out) }
}

/// Cut each page into a grid of `cols` × `rows` parts as seen (Stirling PDF's sections; a poster
/// made sheets; Sol, 26/10), each a page, in reading order: left to right, then down. Written to
/// `out`, not password-protected.
pub fn cut(input: &Input, cols: u32, rows: u32, out: &Path) -> Result<PathBuf> {
    if !(1..=10).contains(&cols) || !(1..=10).contains(&rows) || cols * rows < 2 {
        return Err(OpdfError::Unsupported(format!("{cols} × {rows} parts (choose 1 to 10 columns and rows, two parts at least)")));
    }
    let _g = pdfium_lock();
    let src = input.open()?;
    let n = src.pages().len();
    let mut dst = pdfium().create_new_pdf()?;
    for i in 0..n {
        for k in 0..cols * rows {
            let at = dst.pages().len();
            dst.pages_mut().copy_page_range_from_document(&src, i..=i, at)?;
            let mut page = dst.pages().get(at)?;
            let c = page.boundaries().crop().map(|b| b.bounds).or_else(|_| page.boundaries().media().map(|b| b.bounds))?;
            let (l, b, r, t) = (c.left().value, c.bottom().value, c.right().value, c.top().value);
            let rot = page.rotation()?;
            // The page as seen: its width and height, and a point of it (from its top left) in user space.
            let (dw, dh) = match rot {
                PdfPageRenderRotation::Degrees90 | PdfPageRenderRotation::Degrees270 => (t - b, r - l),
                _ => (r - l, t - b),
            };
            let user = |x: f32, y: f32| match rot {
                PdfPageRenderRotation::None => (l + x, t - y),
                PdfPageRenderRotation::Degrees90 => (l + y, b + x),
                PdfPageRenderRotation::Degrees180 => (r - x, b + y),
                PdfPageRenderRotation::Degrees270 => (r - y, t - x),
            };
            let (col, row) = ((k % cols) as f32, (k / cols) as f32);
            let (cw, ch) = (dw / cols as f32, dh / rows as f32);
            let (p, q) = (user(col * cw, row * ch), user((col + 1.0) * cw, (row + 1.0) * ch));
            let rect = PdfRect::new_from_values(p.1.min(q.1), p.0.min(q.0), p.1.max(q.1), p.0.max(q.0));
            // Media too: a reader or printer that goes by the media box shows the part alone.
            page.boundaries_mut().set_media(rect)?;
            page.boundaries_mut().set_crop(rect)?;
        }
    }
    save(&dst, out)
}

/// The order a booklet's pages are printed in, two to a side, `n` pages padded with blanks (0) to
/// a multiple of four: folded, the sheets read 1, 2, 3… (Acrobat's Booklet printing).
pub fn booklet_order(n: u32) -> Vec<u32> {
    let total = n.div_ceil(4).max(1) * 4;
    let page = |k: u32| if k <= n { k } else { 0 };
    let mut out = Vec::with_capacity(total as usize);
    let (mut lo, mut hi) = (1, total);
    while lo < hi {
        // Front: last and first; back: second and second to last.
        out.extend([page(hi), page(lo), page(lo + 1), page(hi - 1)]);
        lo += 2;
        hi -= 2;
    }
    out
}

/// Several pages to a sheet (Acrobat's Multiple pages per sheet; iLovePDF's Pages per sheet):
/// `per_sheet` 2, 4, 6, 9 or 16, or a booklet (2 to a side, in [`booklet_order`]). The sheets are the
/// first page's size, turned for 2 and 6. Written to `out`, not password-protected.
pub fn impose(input: &Input, per_sheet: u8, booklet: bool, out: &Path) -> Result<PathBuf> {
    let (rows, cols) = match (booklet, per_sheet) {
        (true, _) | (_, 2) => (1u8, 2u8),
        (_, 4) => (2, 2),
        (_, 6) => (2, 3),
        (_, 9) => (3, 3),
        (_, 16) => (4, 4),
        (_, n) => return Err(OpdfError::Unsupported(format!("{n} pages to a sheet (choose 2, 4, 6, 9 or 16)"))),
    };
    let _g = pdfium_lock();
    let src = input.open()?;
    let n = src.pages().len() as u32;
    let first = src.pages().get(0)?;
    let (w, h) = (first.width(), first.height());
    let order: Vec<u32> = if booklet { booklet_order(n) } else { (1..=n).collect() };
    let mut seq = pdfium().create_new_pdf()?;
    for p in order {
        let at = seq.pages().len();
        if p == 0 {
            seq.pages_mut().create_page_at_index(PdfPagePaperSize::new_custom(w, h), at)?;
        } else {
            seq.pages_mut().copy_page_range_from_document(&src, (p - 1) as PdfPageIndex..=(p - 1) as PdfPageIndex, at)?;
        }
    }
    let sheet = if cols > rows { PdfPagePaperSize::new_custom(h, w) } else { PdfPagePaperSize::new_custom(w, h) };
    let tiled = seq.pages().tile_into_new_document(rows, cols, sheet)?;
    save(&tiled, out)
}

/// All the pages one under the other on a single long page (Stirling PDF's Single large page), as
/// wide as the widest, each in a cell as tall as the tallest. A PDF page is 14,400 pt (200 in) at
/// most: longer is refused. Written to `out`, not password-protected.
pub fn long_page(input: &Input, out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let src = input.open()?;
    let n = src.pages().len();
    let (mut w, mut h) = (0f32, 0f32);
    for p in src.pages().iter() {
        w = w.max(p.width().value);
        h = h.max(p.height().value);
    }
    let total = h * n as f32;
    if total > 14_400.0 {
        return Err(OpdfError::Unsupported(format!("{n} pages are too long for one page (a PDF page is 200 inches at most)")));
    }
    let rows = u8::try_from(n).map_err(|_| OpdfError::Unsupported("too many pages for one long page".into()))?;
    let sheet = PdfPagePaperSize::new_custom(PdfPoints::new(w), PdfPoints::new(total));
    let tiled = src.pages().tile_into_new_document(rows, 1, sheet)?;
    save(&tiled, out)
}

/// The pages of a PDF held in memory (0: it cannot be read).
pub fn count_in(bytes: &[u8]) -> usize {
    lopdf::Document::load_mem(bytes).map_or(0, |d| d.get_pages().len())
}

/// Merge in any order: `seq` lists (input index, 1-based page), each page wherever it is placed,
/// pages of different files mixed (Acrobat's Combine Files with its pages expanded).
pub fn assemble(inputs: &[Input], seq: &[(usize, u32)], out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let srcs = inputs.iter().map(|i| i.open()).collect::<std::result::Result<Vec<_>, _>>()?;
    if seq.is_empty() {
        return Err(OpdfError::InvalidRange { range: "no pages".into(), pages: 0 });
    }
    let mut dst = pdfium().create_new_pdf()?;
    for &(k, p) in seq {
        let src = srcs.get(k).ok_or_else(|| OpdfError::InvalidRange { range: format!("file {}", k + 1), pages: inputs.len() as u32 })?;
        let total = src.pages().len() as u32;
        if p == 0 || p > total {
            return Err(OpdfError::InvalidRange { range: p.to_string(), pages: total });
        }
        let at = dst.pages().len();
        dst.pages_mut().copy_pages_from_document(src, &p.to_string(), at)?;
    }
    save(&dst, out)
}

pub fn extract(input: &Input, spec: &str, out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let src = input.open()?;
    let ranges = parse_ranges(spec, src.pages().len() as u32)?;
    let mut dst = pdfium().create_new_pdf()?;
    dst.pages_mut().copy_pages_from_document(&src, &page_string(&ranges), 0)?;
    save(&dst, out)
}

pub fn split_ranges(input: &Input, spec: &str, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let _g = pdfium_lock();
    let src = input.open()?;
    let ranges = parse_ranges(spec, src.pages().len() as u32)?;
    all_or_nothing(&ranges, |r| {
        let mut dst = pdfium().create_new_pdf()?;
        dst.pages_mut().copy_pages_from_document(&src, &page_string(std::slice::from_ref(r)), 0)?;
        save(&dst, &out_dir.join(format!("{}-{}-{}.pdf", input.stem(), r.start(), r.end())))
    })
}

pub fn split_every(input: &Input, n: u32, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let _g = pdfium_lock();
    let total = input.open()?.pages().len() as u32;
    if n == 0 { return Err(OpdfError::InvalidRange { range: "0".into(), pages: total }); }
    let spec = (1..=total).step_by(n as usize).map(|a| format!("{a}-{}", a.saturating_add(n - 1).min(total))).collect::<Vec<_>>().join(",");
    split_ranges(input, &spec, out_dir)
}

/// Acrobat's Split by top-level bookmarks: a file from each top-level bookmark's page to the
/// next one's, named after it ("report-2 Methods.pdf"); pages before the first go with it.
pub fn split_bookmarks(input: &Input, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let _g = pdfium_lock();
    let src = input.open()?;
    let total = src.pages().len() as u32;
    let mut marks: Vec<(u32, String)> = Vec::new();
    let mut b = src.bookmarks().root();
    while let Some(m) = b {
        if let Some(p) = m.destination().and_then(|d| d.page_index().ok()).map(|p| p as u32).filter(|p| *p < total)
            && marks.last().is_none_or(|(q, _)| *q < p)
        {
            marks.push((p, m.title().unwrap_or_default()));
        }
        b = m.next_sibling();
    }
    if marks.len() < 2 {
        return Err(OpdfError::Unsupported("this PDF has fewer than two top-level bookmarks to split at".into()));
    }
    let width = marks.len().to_string().len();
    all_or_nothing(marks.iter().enumerate(), |(k, (p, title))| {
        let start = if k == 0 { 1 } else { p + 1 };
        let end = marks.get(k + 1).map_or(total, |(q, _)| *q);
        let mut dst = pdfium().create_new_pdf()?;
        dst.pages_mut().copy_pages_from_document(&src, &page_string(std::slice::from_ref(&(start..=end))), 0)?;
        let clean: String = title.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { ' ' } else { c }).collect();
        let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
        let clean: String = clean.chars().take(60).collect();
        save(&dst, &out_dir.join(format!("{}-{:0width$} {}.pdf", input.stem(), k + 1, if clean.is_empty() { "Untitled" } else { &clean })))
    })
}

/// Acrobat's Split by file size: consecutive pages grouped while their files stay under
/// `max_bytes` (each page measured alone; a page larger than that is a file on its own).
pub fn split_size(input: &Input, max_bytes: u64, out_dir: &Path) -> Result<Vec<PathBuf>> {
    split_size_report(input, max_bytes, out_dir).map(|(files, _)| files)
}

/// [`split_size`], and the pages (1-based) larger than `max_bytes` on their own: their files are
/// over the size asked (a page cannot be cut), which the result says (Sol, 25/10).
pub fn split_size_report(input: &Input, max_bytes: u64, out_dir: &Path) -> Result<(Vec<PathBuf>, Vec<u32>)> {
    let mut over = Vec::new();
    let spec = {
        let _g = pdfium_lock();
        let src = input.open()?;
        let total = src.pages().len() as u32;
        let mut groups: Vec<(u32, u32)> = Vec::new();
        let mut size = 0u64;
        for p in 1..=total {
            let mut one = pdfium().create_new_pdf()?;
            one.pages_mut().copy_pages_from_document(&src, &p.to_string(), 0)?;
            let n = one.save_to_bytes()?.len() as u64;
            if n > max_bytes {
                over.push(p);
            }
            match groups.last_mut() {
                Some(g) if size + n <= max_bytes => {
                    g.1 = p;
                    size += n;
                }
                _ => {
                    groups.push((p, p));
                    size = n;
                }
            }
        }
        groups.iter().map(|(a, b)| if a == b { a.to_string() } else { format!("{a}-{b}") }).collect::<Vec<_>>().join(",")
    };
    Ok((split_ranges(input, &spec, out_dir)?, over))
}

/// What the result says of the pages too large for the size asked ("" when none).
pub fn oversize_note(over: &[u32]) -> String {
    match over {
        [] => String::new(),
        [p] => format!("Page {p} alone is larger than the size asked: its file is too"),
        many => format!("{} pages alone are larger than the size asked (page {}…): their files are too", many.len(), many[0]),
    }
}

/// How blank a page must be, Stirling's threshold and white share in three steps: 0 strict (white
/// from 245, 0.1 % else), 1 as Acrobat (235, 0.3 %), 2 scans (grey paper: white from 200, 1 %).
const BLANKNESS: [(u8, usize); 3] = [(245, 1), (235, 3), (200, 10)];

/// The blank pages of `input`, 1-based (Acrobat's Remove Blank Pages): no text, and nothing but
/// near-white once drawn (a scan's specks and a page's frame-thin marks count as blank), at the
/// strictness `level` ([`BLANKNESS`]).
pub fn blank_pages(input: &Input, level: usize) -> Result<Vec<u32>> {
    let (white, per_mille) = BLANKNESS[level.min(2)];
    let _g = pdfium_lock();
    let doc = input.open()?;
    let mut blank = Vec::new();
    for (i, page) in doc.pages().iter().enumerate() {
        if page.text().map(|t| t.all().trim().is_empty()).unwrap_or(true) {
            let img = page.render_with_config(&PdfRenderConfig::new().set_target_width(120))?.as_image()?.to_luma8();
            let dark = img.pixels().filter(|p| p[0] < white).count();
            if dark * 1000 <= img.len() * per_mille {
                blank.push(i as u32 + 1);
            }
        }
    }
    Ok(blank)
}

pub fn remove(input: &Input, spec: &str, out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let total = input.open()?.pages().len() as u32;
    let drop = parse_ranges(spec, total)?;
    let keep: Vec<u32> = (1..=total).filter(|p| !drop.iter().any(|r| r.contains(p))).collect();
    if keep.is_empty() { return Err(OpdfError::WouldBeEmpty { pages: total }); }
    organize(input, &keep, out)
}

pub fn organize(input: &Input, order: &[u32], out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let src = input.open()?;
    let total = src.pages().len() as u32;
    if order.is_empty() || order.iter().any(|&p| p > total) {
        return Err(OpdfError::InvalidRange { range: format!("{order:?}"), pages: total });
    }
    let mut dst = pdfium().create_new_pdf()?;
    for &p in order {
        let at = dst.pages().len();
        if p == 0 { dst.pages_mut().create_page_at_index(PdfPagePaperSize::a4(), at)?; }
        else { dst.pages_mut().copy_pages_from_document(&src, &p.to_string(), at)?; }
    }
    save(&dst, out)
}

/// Whether the locale's region uses US Letter paper (CLDR's paper size data: the Americas that do,
/// and the Philippines); everywhere else A4. `locale` as the system gives it ("en-US", "fr_CA.UTF-8").
pub fn letter_region(locale: Option<&str>) -> bool {
    let region = locale.and_then(|l| l.split(['.', '@']).next()).and_then(|l| l.split(['-', '_']).nth(1)).unwrap_or("");
    ["US", "CA", "MX", "CL", "CO", "CR", "GT", "NI", "PA", "PH", "PR", "SV", "VE"].contains(&region.to_ascii_uppercase().as_str())
}

/// A new PDF of one blank page, A4 or US Letter (PDF Expert's New ▸ Blank Document), at `out`
/// (never overwriting). Pages, text and pictures are then added in the reader.
pub fn blank(letter: bool, out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let mut dst = pdfium().create_new_pdf()?;
    // Letter in points exactly (pdfium's own Letter is 612 × 791).
    let size = if letter { PdfPagePaperSize::new_custom(PdfPoints::new(612.0), PdfPoints::new(792.0)) } else { PdfPagePaperSize::a4() };
    dst.pages_mut().create_page_at_end(size)?;
    save(&dst, out)
}
