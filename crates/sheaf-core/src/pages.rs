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
    split_ranges(input, &spec, out_dir)
}

/// The blank pages of `input`, 1-based (Acrobat's Remove Blank Pages): no text, and nothing but
/// near-white once drawn (a scan's specks and a page's frame-thin marks under 0.3 % count as blank).
pub fn blank_pages(input: &Input) -> Result<Vec<u32>> {
    let _g = pdfium_lock();
    let doc = input.open()?;
    let mut blank = Vec::new();
    for (i, page) in doc.pages().iter().enumerate() {
        if page.text().map(|t| t.all().trim().is_empty()).unwrap_or(true) {
            let img = page.render_with_config(&PdfRenderConfig::new().set_target_width(120))?.as_image()?.to_luma8();
            let dark = img.pixels().filter(|p| p[0] < 235).count();
            if dark * 1000 <= img.len() * 3 {
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
    let size = if letter { PdfPagePaperSize::Portrait(PdfPagePaperStandardSize::USLetterAnsiA) } else { PdfPagePaperSize::a4() };
    dst.pages_mut().create_page_at_end(size)?;
    save(&dst, out)
}
