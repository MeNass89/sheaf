//! Add watermark: text or a picture over (or under) the page content, at one of nine
//! positions or tiled as a mosaic, through PdfCraft's `Edit::AddWatermark`.

use crate::numbers::page_span;
use crate::pdfcraft::engine::{Edit, MarkFile, Watermark};
use crate::pdfcraft::Doc;
use crate::{pages::Input, OpdfError, Result};
use pdfcraft_edit::MarkSource;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What is stamped.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    /// `font_size` 1–500 points, or 0 for an automatic size (about half the page diagonal).
    Text { text: String, font_size: f64, color: [f64; 3] },
    /// A PNG or JPEG; `scale` is its size relative to the page, 0.05–1.0 (1.0 fits the page).
    Image { path: PathBuf, scale: f64 },
}

/// Where the mark goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Cell of the 3×3 grid, row by row from the top left (4 = centre).
    Grid(u8),
    /// Repeated on a 3×3 grid covering the page.
    Mosaic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WatermarkOptions {
    pub mark: Mark,
    pub placement: Placement,
    /// 1.0 = no transparency.
    pub opacity: f64,
    /// Degrees counter-clockwise.
    pub rotation: f64,
    /// Below the page content instead of over it.
    pub behind: bool,
    /// First and last page, 1-based (default: all).
    pub from: Option<u32>,
    pub to: Option<u32>,
}

impl Default for WatermarkOptions {
    fn default() -> Self {
        Self {
            mark: Mark::Text { text: String::new(), font_size: 48.0, color: [0.0; 3] },
            placement: Placement::Grid(4),
            opacity: 1.0,
            rotation: 0.0,
            behind: false,
            from: None,
            to: None,
        }
    }
}

/// Distance kept between a mark placed at an edge and the page edge, in points.
const EDGE: f64 = 36.0;

/// The font size text is drawn at on a `page` (w, h): `font_size`, or for 0 PdfCraft's
/// automatic size (its rule: the widest line spans half the page diagonal, 6 to 300 points).
fn text_size(lines: &[&str], font_size: f64, page: (f64, f64)) -> f64 {
    if font_size > 0.0 {
        return font_size;
    }
    let widest = lines.iter().map(|l| pdfcraft_fonts::helvetica_width(l, 1.0)).fold(0.0, f64::max).max(0.01);
    ((page.0 * page.0 + page.1 * page.1).sqrt() * 0.5 / widest).clamp(6.0, 300.0)
}

/// The mark's size in points on a `page` (w, h) before rotation.
fn mark_size(mark: &Mark, picture: Option<(f64, f64)>, page: (f64, f64)) -> (f64, f64) {
    match (mark, picture) {
        (Mark::Text { text, font_size, .. }, _) => {
            let lines: Vec<&str> = text.trim().lines().collect();
            let size = text_size(&lines, *font_size, page);
            let w = lines.iter().map(|l| pdfcraft_fonts::helvetica_width(l, size)).fold(0.0, f64::max);
            (w, size * (0.8 + 1.15 * (lines.len().max(1) - 1) as f64))
        }
        (Mark::Image { scale, .. }, Some((sw, sh))) => {
            let k = (page.0 / sw).min(page.1 / sh) * scale;
            (sw * k, sh * k)
        }
        (Mark::Image { .. }, None) => (0.0, 0.0),
    }
}

/// Offsets (right, up) from the page centre for each copy of the mark on a `page` (w, h).
fn offsets(placement: Placement, page: (f64, f64), size: (f64, f64), rotation: f64) -> Vec<[f64; 2]> {
    let (w, h) = page;
    match placement {
        Placement::Mosaic => {
            let mut v = Vec::new();
            for row in [1.0, 0.0, -1.0] {
                for col in [-1.0, 0.0, 1.0] {
                    v.push([col * w / 3.0, row * h / 3.0]);
                }
            }
            v
        }
        Placement::Grid(cell) => {
            // Half the extent of the turned mark, so a corner mark stays EDGE inside the page.
            let (s, c) = rotation.to_radians().sin_cos();
            let hx = (c.abs() * size.0 + s.abs() * size.1) / 2.0;
            let hy = (s.abs() * size.0 + c.abs() * size.1) / 2.0;
            let dx = (w / 2.0 - EDGE - hx).max(0.0);
            let dy = (h / 2.0 - EDGE - hy).max(0.0);
            let col = [-dx, 0.0, dx][(cell % 3) as usize];
            let row = [dy, 0.0, -dy][(cell / 3).min(2) as usize];
            vec![[col, row]]
        }
    }
}

/// Stamps the watermark on `input` and writes `out` (never overwriting). Returns the path
/// written and whether the input was password-protected (the output never is).
pub fn watermark(input: &Input, o: &WatermarkOptions, out: &Path) -> Result<(PathBuf, bool)> {
    if let Placement::Grid(c) = o.placement
        && c > 8
    {
        return Err(OpdfError::Unsupported(format!("watermark position {c}")));
    }
    match &o.mark {
        Mark::Text { font_size, .. } if !(*font_size == 0.0 || (1.0..=500.0).contains(font_size)) => {
            return Err(OpdfError::Unsupported(format!("watermark font size {font_size} (use 1 to 500, or 0 for automatic)")));
        }
        Mark::Image { scale, .. } if !(0.05..=1.0).contains(scale) => {
            return Err(OpdfError::Unsupported(format!("watermark image scale {scale} (use 0.05 to 1)")));
        }
        _ => {}
    }
    let mut doc = Doc::open(input)?;
    let sizes: Vec<(f64, f64)> =
        doc.session().get(doc.id()).map(|d| d.info.pages.iter().map(|p| (p.width as f64, p.height as f64)).collect()).unwrap_or_default();
    let pages: Vec<usize> = page_span(o.from, o.to, false, sizes.len())?.collect();
    let mut base = Watermark {
        scale: 1.0,
        font_size: 0.0,
        color: [0.0; 3],
        opacity: o.opacity.clamp(0.0, 1.0),
        rotation: o.rotation,
        behind: o.behind,
        ..Default::default()
    };
    let mut file = None;
    match &o.mark {
        Mark::Text { text, font_size, color } => {
            if text.trim().is_empty() {
                return Err(OpdfError::Unsupported("the watermark text is empty".into()));
            }
            base.text = text.clone();
            base.font_size = *font_size;
            base.color = *color;
        }
        Mark::Image { path, scale } => {
            base.scale = *scale;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "image".into());
            file = Some(MarkFile { name, bytes: Arc::new(std::fs::read(path)?), page: 0 });
        }
    }
    // The picture's natural proportions (PdfCraft fits it to the page, so only they matter).
    let natural = match &o.mark {
        Mark::Image { path, .. } => Some(picture_size(path)?),
        Mark::Text { .. } => None,
    };
    // Pages of one size share their offsets: one edit per (page size, copy).
    let mut groups: Vec<((f64, f64), Vec<usize>)> = Vec::new();
    for &p in &pages {
        match groups.iter_mut().find(|(s, _)| *s == sizes[p]) {
            Some((_, g)) => g.push(p),
            None => groups.push((sizes[p], vec![p])),
        }
    }
    let mut edits = Vec::new();
    for (size, group) in groups {
        for off in offsets(o.placement, size, mark_size(&o.mark, natural, size), o.rotation) {
            edits.push(Edit::AddWatermark { pages: group.clone(), settings: Watermark { offset: off, ..base.clone() }, replace: false, file: None });
        }
    }
    let Some(file) = file else {
        doc.apply(Edit::Batch { label: "Add watermark".into(), edits })?;
        return doc.save_unprotected(out);
    };
    // A picture is embedded once, by the first edit; the others reuse that XObject.
    let mut rest = edits.split_off(1);
    let Some(Edit::AddWatermark { pages: first_pages, settings, .. }) = edits.pop() else { unreachable!("one edit per copy") };
    let first = first_pages[0];
    let pictures = |doc: &Doc| -> Vec<pdfcraft_edit::PageImage> {
        doc.session().get(doc.id()).map(|d| d.page_images(first)).unwrap_or_default().into_iter().filter(|i| i.name.starts_with("PCPic")).collect()
    };
    let before: Vec<_> = pictures(&doc).iter().filter_map(|i| i.object).collect();
    doc.apply(Edit::AddWatermark { pages: first_pages, settings, replace: false, file: Some(file) })?;
    if !rest.is_empty() {
        let xobject = pictures(&doc)
            .into_iter()
            .filter_map(|i| i.object)
            .find(|r| !before.contains(r))
            .ok_or_else(|| OpdfError::Engine("the watermark picture was not placed".into()))?;
        let (w, h) = natural.unwrap_or((1.0, 1.0));
        let src = MarkSource { xobject, size: (w, h), image: true };
        for e in &mut rest {
            if let Edit::AddWatermark { settings, .. } = e {
                settings.source = Some(src);
            }
        }
        doc.apply(Edit::Batch { label: "Add watermark".into(), edits: rest })?;
    }
    doc.save_unprotected(out)
}

/// A PNG or JPEG's size in pixels, upright (EXIF orientation applied, as PdfCraft embeds it).
fn picture_size(path: &Path) -> Result<(f64, f64)> {
    use image::ImageDecoder;
    let mut dec = image::ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
    let (w, h) = dec.dimensions();
    let turned = matches!(
        dec.orientation()?,
        image::metadata::Orientation::Rotate90
            | image::metadata::Orientation::Rotate270
            | image::metadata::Orientation::Rotate90FlipH
            | image::metadata::Orientation::Rotate270FlipH
    );
    let (w, h) = if turned { (h, w) } else { (w, h) };
    Ok((w.max(1) as f64, h.max(1) as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_offsets_keep_the_mark_inside_the_margin() {
        let page = (600.0, 800.0);
        assert_eq!(offsets(Placement::Grid(4), page, (100.0, 20.0), 0.0), [[0.0, 0.0]]);
        assert_eq!(offsets(Placement::Grid(0), page, (100.0, 20.0), 0.0), [[-(300.0 - 36.0 - 50.0), 400.0 - 36.0 - 10.0]]);
        assert_eq!(offsets(Placement::Grid(8), page, (100.0, 20.0), 90.0)[0].map(f64::round), [300.0 - 36.0 - 10.0, -(400.0 - 36.0 - 50.0)]);
        assert_eq!(offsets(Placement::Mosaic, page, (1.0, 1.0), 0.0).len(), 9);
    }

    #[test]
    fn an_automatic_size_is_measured_at_the_size_drawn() {
        let a4 = (595.0, 842.0);
        let auto = Mark::Text { text: "DRAFT".into(), font_size: 0.0, color: [0.0; 3] };
        let (w, h) = mark_size(&auto, None, a4);
        // The widest line spans half the page diagonal (PdfCraft's rule).
        assert!((w - (595.0f64.hypot(842.0) * 0.5)).abs() < 0.01, "{w}");
        assert!(h > 50.0, "{h}");
        // A size given is used as it is.
        let fixed = Mark::Text { text: "DRAFT".into(), font_size: 40.0, color: [0.0; 3] };
        assert_eq!(mark_size(&fixed, None, a4).1, 40.0 * 0.8);
    }
}
