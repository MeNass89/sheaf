//! Crop PDF: a crop box set by margins (as the reader sees the page), or removed.

use crate::pdfcraft::engine::{BoxSpec, Edit, PageBox};
use crate::pdfcraft::Doc;
use crate::{pages::Input, ranges::parse_ranges, Result};
use std::path::{Path, PathBuf};

/// What Crop does to the selected pages.
#[derive(Debug, Clone, PartialEq)]
pub enum CropSpec {
    /// Points cut from each edge of the page as displayed (after its rotation), measured from what
    /// shows: the crop box when the page has one (Sol, 26/10: they were measured on the visible page
    /// in the preview and applied from the media box).
    Margins { top: f64, bottom: f64, left: f64, right: f64 },
    /// Back to the full page (no crop box).
    Remove,
    /// Acrobat's Remove White Margins: each page cut to what is drawn on it, `pad` points around.
    WhiteMargins { pad: f64 },
    /// Acrobat's Change Page Size: the pages become `size` points, their content scaled to fit
    /// ([`crate::pages::resize`]).
    Resize { size: (f64, f64) },
}

/// The box of what is drawn in an RGBA render (any pixel visibly darker than white), in pixels:
/// [x0, y0, x1, y1] inclusive, `None` for a blank page.
fn drawn_box(rgba: &[u8], w: usize, h: usize) -> Option<[usize; 4]> {
    let mut b: Option<[usize; 4]> = None;
    for (i, px) in rgba.as_chunks::<4>().0.iter().enumerate().take(w * h) {
        let ink = px[3] > 16 && px[..3].iter().any(|c| *c < 235);
        if ink {
            let (x, y) = (i % w, i / w);
            b = Some(b.map_or([x, y, x, y], |[x0, y0, x1, y1]| [x0.min(x), y0.min(y), x1.max(x), y1.max(y)]));
        }
    }
    b
}

/// Millimetres to points.
pub fn mm(v: f64) -> f64 {
    v * 72.0 / 25.4
}

/// Displayed margins (top, right, bottom, left) → PdfCraft's user-space margins
/// (left, bottom, right, top) for a page turned `rotation` degrees clockwise.
fn user_margins(rotation: u16, [t, r, b, l]: [f64; 4]) -> [f64; 4] {
    match rotation % 360 {
        90 => [t, l, b, r],
        180 => [r, t, l, b],
        270 => [b, r, t, l],
        _ => [l, b, r, t],
    }
}

/// Crops `pages` ("1-3,5"; `None` = every page) of `input` and writes `out` (never
/// overwriting). Returns the path written and whether the input was password-protected (the
/// output never is).
pub fn crop(input: &Input, pages: Option<&str>, spec: &CropSpec, out: &Path) -> Result<(PathBuf, bool)> {
    if let CropSpec::Resize { size } = *spec {
        return Ok((crate::pages::resize(input, pages, size, out)?, input.password.is_some()));
    }
    let mut doc = Doc::open(input)?;
    let rotations: Vec<u16> = doc.session().get(doc.id()).map(|d| d.info.pages.iter().map(|p| p.rotation).collect()).unwrap_or_default();
    let total = rotations.len() as u32;
    let selected: Vec<usize> = match pages.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => {
            let ranges = parse_ranges(s, total)?;
            (1..=total).filter(|n| ranges.iter().any(|r| r.contains(n))).map(|n| n as usize - 1).collect()
        }
        None => (0..rotations.len()).collect(),
    };
    let edits = match *spec {
        CropSpec::Remove => vec![Edit::SetPageBox { pages: selected, which: PageBox::Crop, spec: BoxSpec::Remove }],
        CropSpec::WhiteMargins { pad } => {
            let d = doc.session().get(doc.id()).ok_or_else(|| crate::OpdfError::Engine("the document is not open".into()))?;
            let config = crate::pdfcraft::render::RenderConfig { password: d.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
            let mut r = crate::pdfcraft::render::PageRenderer::new(d.bytes.clone(), config);
            let infos = d.info.pages.clone();
            // A blank page keeps its size.
            selected
                .iter()
                .filter_map(|&p| {
                    let info = infos.get(p)?;
                    let px = r.render(crate::pdfcraft::render::RenderRequest { page: p, scale: 1.0, ..Default::default() });
                    let [x0, y0, x1, y1] = drawn_box(&px.rgba, px.width as usize, px.height as usize)?;
                    let s = px.width as f32 / info.width.max(1e-3);
                    let (a, b) = (info.view_to_user(x0 as f32 / s, y0 as f32 / s), info.view_to_user((x1 + 1) as f32 / s, (y1 + 1) as f32 / s));
                    let c = info.crop;
                    let rect = [
                        (a[0].min(b[0]) as f64 - pad).max(c[0] as f64),
                        (a[1].min(b[1]) as f64 - pad).max(c[1] as f64),
                        (a[0].max(b[0]) as f64 + pad).min(c[2] as f64),
                        (a[1].max(b[1]) as f64 + pad).min(c[3] as f64),
                    ];
                    Some(Edit::SetPageBox { pages: vec![p], which: PageBox::Crop, spec: BoxSpec::Rect(rect) })
                })
                .collect()
        }
        CropSpec::Resize { .. } => Vec::new(),
        CropSpec::Margins { top, bottom, left, right } => {
            // Each page from its own visible box; the same visible margins land on different
            // user-space edges as the page is turned.
            let crops: Vec<[f32; 4]> = doc.session().get(doc.id()).map(|d| d.info.pages.iter().map(|p| p.crop).collect()).unwrap_or_default();
            let mut edits = Vec::new();
            for &p in &selected {
                let (Some(c), Some(&rot)) = (crops.get(p), rotations.get(p)) else { continue };
                let [ml, mb, mr, mt] = user_margins(rot, [top, right, bottom, left]);
                let rect = [c[0] as f64 + ml, c[1] as f64 + mb, c[2] as f64 - mr, c[3] as f64 - mt];
                if rect[2] - rect[0] < 1.0 || rect[3] - rect[1] < 1.0 {
                    return Err(crate::OpdfError::Unsupported(format!("the margins leave nothing of page {}", p + 1)));
                }
                edits.push(Edit::SetPageBox { pages: vec![p], which: PageBox::Crop, spec: BoxSpec::Rect(rect) });
            }
            edits
        }
    };
    doc.apply(Edit::Batch { label: "Crop pages".into(), edits })?;
    doc.save_unprotected(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_margins() {
        let m = [1.0, 2.0, 3.0, 4.0]; // top, right, bottom, left
        assert_eq!(user_margins(0, m), [4.0, 3.0, 2.0, 1.0]);
        assert_eq!(user_margins(90, m), [1.0, 4.0, 3.0, 2.0]);
        assert_eq!(user_margins(180, m), [2.0, 1.0, 4.0, 3.0]);
        assert_eq!(user_margins(270, m), [3.0, 2.0, 1.0, 4.0]);
        assert!((mm(25.4) - 72.0).abs() < 1e-9);
    }
}
