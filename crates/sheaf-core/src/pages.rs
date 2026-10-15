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

pub fn merge(inputs: &[Input], out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let mut dst = pdfium().create_new_pdf()?;
    for i in inputs { let src = i.open()?; dst.pages_mut().append(&src)?; }
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
