use crate::{pages::Input, paths::atomic_write, ranges::parse_ranges, OpdfError, Result};
use std::path::{Path, PathBuf};

/// Adds `degrees` (90, 180 or 270) to each selected page's effective `/Rotate` (its own, else
/// the nearest one inherited through `/Parent`), mod 360, and writes the result on the page.
pub fn rotate(input: &Input, pages: Option<&str>, degrees: i64, out: &Path) -> Result<PathBuf> {
    if ![90, 180, 270].contains(&degrees) { return Err(OpdfError::Unsupported(format!("rotation {degrees}°"))); }
    let mut doc = input.load_lopdf()?;
    let all = doc.get_pages();
    let total = all.len() as u32;
    let ranges = match pages { Some(s) => parse_ranges(s, total)?, None => std::iter::once(1..=total).collect() };
    for (n, id) in all {
        if !ranges.iter().any(|r| r.contains(&n)) { continue; }
        let cur = effective_rotate(&doc, id).rem_euclid(360);
        doc.get_dictionary_mut(id)?.set("Rotate", (cur + degrees).rem_euclid(360));
    }
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
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
