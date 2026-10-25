use crate::{pages::Input, paths::atomic_write, ranges::parse_ranges, OpdfError, Result};
use std::path::{Path, PathBuf};

/// Adds `degrees` (90, 180 or 270) to each selected page's effective `/Rotate` (its own, else
/// the nearest one inherited through `/Parent`), mod 360, and writes the result on the page.
pub fn rotate(input: &Input, pages: Option<&str>, degrees: i64, out: &Path) -> Result<PathBuf> {
    rotate_filtered(input, pages, degrees, PageFilter::default(), out)
}

/// Which of the selected pages turn (Acrobat's Rotate Pages ▸ Even and Odd Pages, Portrait and
/// Landscape): `parity` 0 both, 1 odd, 2 even; `orientation` 0 both, 1 portrait, 2 landscape (as
/// shown, its rotation counted).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageFilter { pub parity: u8, pub orientation: u8 }

/// [`rotate`], the selected pages narrowed by `filter`.
pub fn rotate_filtered(input: &Input, pages: Option<&str>, degrees: i64, filter: PageFilter, out: &Path) -> Result<PathBuf> {
    if ![90, 180, 270].contains(&degrees) { return Err(OpdfError::Unsupported(format!("rotation {degrees}°"))); }
    let mut doc = input.load_lopdf()?;
    let all = doc.get_pages();
    let total = all.len() as u32;
    let ranges = match pages { Some(s) => parse_ranges(s, total)?, None => std::iter::once(1..=total).collect() };
    for (n, id) in all {
        if !ranges.iter().any(|r| r.contains(&n)) { continue; }
        if (filter.parity == 1 && n % 2 == 0) || (filter.parity == 2 && n % 2 == 1) { continue; }
        let cur = effective_rotate(&doc, id).rem_euclid(360);
        if filter.orientation != 0 {
            let (w, h) = page_size(&doc, id);
            let (w, h) = if cur % 180 == 90 { (h, w) } else { (w, h) };
            if (filter.orientation == 1) != (h >= w) { continue; }
        }
        doc.get_dictionary_mut(id)?.set("Rotate", (cur + degrees).rem_euclid(360));
    }
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}

/// The page's width and height in its /CropBox, else its /MediaBox (inherited through /Parent).
fn page_size(doc: &lopdf::Document, page: lopdf::ObjectId) -> (f64, f64) {
    let mut node = doc.get_dictionary(page).ok();
    for _ in 0..64 {
        let Some(d) = node else { break };
        for key in [&b"CropBox"[..], b"MediaBox"] {
            if let Ok(a) = d.get(key).and_then(|o| doc.dereference(o)).and_then(|(_, o)| o.as_array()) {
                let v: Vec<f64> = a.iter().filter_map(|x| x.as_float().ok().map(f64::from).or_else(|| x.as_i64().ok().map(|i| i as f64))).collect();
                if v.len() == 4 { return ((v[2] - v[0]).abs(), (v[3] - v[1]).abs()); }
            }
        }
        node = d.get(b"Parent").and_then(lopdf::Object::as_reference).and_then(|id| doc.get_dictionary(id)).ok();
    }
    (612.0, 792.0)
}

/// The page's own /Rotate, else the first one found walking up /Parent (at most 64 levels), else 0.
fn effective_rotate(doc: &lopdf::Document, page: lopdf::ObjectId) -> i64 {
    let mut node = doc.get_dictionary(page).ok();
    for _ in 0..64 {
        let Some(d) = node else { break };
        if let Ok(r) = d.get(b"Rotate").and_then(|o| doc.dereference(o)).and_then(|(_, o)| o.as_i64()) { return r; }
        node = d.get(b"Parent").and_then(lopdf::Object::as_reference).and_then(|id| doc.get_dictionary(id)).ok();
    }
    0
}
