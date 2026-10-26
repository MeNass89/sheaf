//! Detect fields on a scanned form (Acrobat's Prepare Form on a scan): the page holds only a
//! picture of its lines and boxes, so PdfCraft's detection, which reads drawn shapes and text,
//! finds nothing. The page is rendered, its lines and boxes are found in the pixels, its words
//! are read by OCR (labels, when the models are installed), and PdfCraft's detection runs on them.

use crate::pdfcraft::engine::detect::{self, Candidate, Shapes, Word};
use crate::pdfcraft::engine::ocr::OcrSettings;
use crate::pdfcraft::Doc;
use pdfcraft_render::{PageRenderer, RenderConfig, RenderRequest, RequestKind};
use std::sync::Arc;

/// Pixels per point the page is rendered at (144 dpi: a 1 pt line is 2 pixels).
const SCALE: f32 = 2.0;
/// Shortest line kept, in points (a check box's side), and the thickest, in pixels.
const MIN_LINE_PT: f32 = 5.0;
const MAX_THICK_PX: usize = 6;

/// A dark horizontal band in the image: columns `x0..x1`, rows `top..=bottom`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Band {
    x0: usize,
    x1: usize,
    top: usize,
    bottom: usize,
}

/// The rules and boxes in a grey image (a byte a pixel, `w` wide), in pixels `[x0, y0, x1, y1]`
/// (y down, end exclusive): horizontal lines at least `min` pixels long and at most `thick`
/// thick; a box is two such lines of the same span joined at both ends by dark columns. Lines
/// that are a box's edges are not rules.
pub fn lines_in(gray: &[u8], w: usize, min: usize, thick: usize) -> (Vec<[usize; 4]>, Vec<[usize; 4]>) {
    let h = gray.len().checked_div(w).unwrap_or(0);
    let dark = |x: usize, y: usize| gray[y * w + x] < 128;
    // Runs of dark pixels on each row, grown into bands over the rows below.
    let mut open: Vec<Band> = Vec::new();
    let mut bands: Vec<Band> = Vec::new();
    for y in 0..h {
        let mut runs = Vec::new();
        let mut x = 0;
        while x < w {
            if dark(x, y) {
                let s = x;
                while x < w && dark(x, y) {
                    x += 1;
                }
                if x - s >= min {
                    runs.push((s, x));
                }
            } else {
                x += 1;
            }
        }
        let mut next = Vec::new();
        for (s, e) in runs {
            let joins = open.iter().position(|b| {
                let overlap = e.min(b.x1).saturating_sub(s.max(b.x0));
                overlap * 10 >= 7 * (e - s).min(b.x1 - b.x0)
            });
            match joins {
                Some(i) => {
                    let b = open.remove(i);
                    next.push(Band { x0: b.x0.min(s), x1: b.x1.max(e), top: b.top, bottom: y });
                }
                None => next.push(Band { x0: s, x1: e, top: y, bottom: y }),
            }
        }
        bands.append(&mut open);
        open = next;
    }
    bands.append(&mut open);
    let mut lines: Vec<Band> = bands.into_iter().filter(|b| b.bottom - b.top < thick).collect();
    lines.sort_by_key(|b| (b.top, b.x0));
    // Boxes: the nearest line below with the same ends, both sides dark between them.
    let side = |x: usize, from: usize, to: usize| {
        let rows = to.saturating_sub(from);
        let hit = (from..to).filter(|&y| (x.saturating_sub(thick)..(x + thick).min(w)).any(|c| dark(c, y))).count();
        rows > 0 && hit * 10 >= rows * 9
    };
    let tol = thick + 2;
    let mut edge = vec![false; lines.len()];
    let mut boxes = Vec::new();
    for i in 0..lines.len() {
        let a = lines[i];
        let below = (i + 1..lines.len()).find(|&j| {
            let b = lines[j];
            b.top > a.bottom + 2 && a.x0.abs_diff(b.x0) <= tol && a.x1.abs_diff(b.x1) <= tol && side(a.x0.min(b.x0) + thick / 2, a.bottom + 1, b.top) && side(a.x1.max(b.x1) - 1 - thick / 2, a.bottom + 1, b.top)
        });
        if let Some(j) = below {
            let b = lines[j];
            edge[i] = true;
            edge[j] = true;
            boxes.push([a.x0.min(b.x0), a.top, a.x1.max(b.x1), b.bottom + 1]);
        }
    }
    let rules = lines.iter().zip(&edge).filter(|(_, e)| !**e).map(|(b, _)| [b.x0, b.top, b.x1, b.bottom + 1]).collect();
    (boxes, rules)
}

/// Fields proposed for the scanned pages among `pages` (a page with text of its own is left to
/// PdfCraft's detection): (page, candidate), names unique against `taken`, none over a field
/// already there (`existing`: page and rectangle; Sol, 26/10: detecting again stacked fields).
pub fn candidates(doc: &Doc, pages: &[usize], taken: &[String], existing: &[(usize, [f64; 4])]) -> Vec<(usize, Candidate)> {
    let Some(d) = doc.session().get(doc.id()) else { return Vec::new() };
    let config = RenderConfig { password: d.password.as_deref().map(Arc::from), ..Default::default() };
    let mut r = PageRenderer::new(d.bytes.clone(), config);
    let ocr = crate::ocr::engine().ok();
    let mut taken = taken.to_vec();
    let mut out = Vec::new();
    // A scan is read as a picture even with an OCR layer (Sol, 26/10: its boxes and lines, only in
    // the picture, were never found).
    let scans: Vec<bool> = {
        let _g = crate::engine::pdfium_lock();
        let doc = crate::engine::open_pdf_bytes(&d.bytes, d.password.as_deref()).ok();
        pages.iter().map(|&p| doc.as_ref().and_then(|doc| doc.pages().get(p as i32).ok()).is_some_and(|pg| crate::enhance::is_scan(&pg))).collect()
    };
    for (k, &page) in pages.iter().enumerate() {
        let Some(info) = d.info.pages.get(page) else { continue };
        let has_text = r.render(RenderRequest { page, kind: RequestKind::Text, scale: 1.0, ..Default::default() }).text.is_some_and(|t| t.glyphs.iter().any(|g| !g.text.trim().is_empty()));
        if has_text && !scans[k] {
            continue;
        }
        let shot = r.render(RenderRequest { page, scale: SCALE, ..Default::default() });
        if shot.error.is_some() || shot.width == 0 {
            continue;
        }
        let gray: Vec<u8> = shot.rgba.as_chunks::<4>().0.iter().map(|p| ((u32::from(p[0]) * 3 + u32::from(p[1]) * 6 + u32::from(p[2])) / 10) as u8).collect();
        let k = shot.width as f32 / info.width.max(1e-3);
        let (boxes, rules) = lines_in(&gray, shot.width as usize, (MIN_LINE_PT * k) as usize, MAX_THICK_PX);
        let user = |r: [usize; 4]| {
            let [a, b] = info.view_to_user(r[0] as f32 / k, r[1] as f32 / k);
            let [c, e] = info.view_to_user(r[2] as f32 / k, r[3] as f32 / k);
            [f64::from(a.min(c)), f64::from(b.min(e)), f64::from(a.max(c)), f64::from(b.max(e))]
        };
        // A rule is drawn under the line it is written on: its bottom edge stands for it.
        let shapes = Shapes { boxes: boxes.into_iter().map(user).collect(), rules: rules.into_iter().map(user).map(|r| [r[0], r[1], r[2], r[1]]).collect() };
        let words: Vec<Word> = match &ocr {
            Some(ocr) => {
                let settings = OcrSettings { dpi: 150.0, language: "en".into(), skip_text_pages: true };
                let job = doc.session().ocr_job(doc.id(), &[page], settings);
                job.map(|j| j.run(ocr, |_, _| true)).unwrap_or_default().into_iter().flat_map(|p| p.words).map(|w| {
                    // The box around the word's four corners (it may stand turned).
                    let corners = [[0.0, 0.0], w.across, w.up, [w.across[0] + w.up[0], w.across[1] + w.up[1]]].map(|[dx, dy]| [w.origin[0] + dx, w.origin[1] + dy]);
                    let (xs, ys) = (corners.map(|c| c[0]), corners.map(|c| c[1]));
                    let lo = |v: [f64; 4]| v.into_iter().fold(f64::INFINITY, f64::min);
                    let hi = |v: [f64; 4]| v.into_iter().fold(f64::NEG_INFINITY, f64::max);
                    Word { text: w.text, rect: [lo(xs), lo(ys), hi(xs), hi(ys)] }
                }).collect()
            }
            None => Vec::new(),
        };
        let here: Vec<[f64; 4]> = existing.iter().filter(|e| e.0 == page).map(|e| e.1).collect();
        for c in detect::detect(&words, &shapes, &here, &taken) {
            taken.push(c.name.clone());
            out.push((page, c));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A white image with dark rectangles painted (x0, y0, x1, y1, end exclusive).
    fn image(w: usize, h: usize, rects: &[[usize; 4]]) -> Vec<u8> {
        let mut g = vec![255u8; w * h];
        for r in rects {
            for y in r[1]..r[3] {
                g[y * w + r[0]..y * w + r[2]].fill(0);
            }
        }
        g
    }

    /// A box outline `t` pixels thick.
    fn outline(x0: usize, y0: usize, x1: usize, y1: usize, t: usize) -> [[usize; 4]; 4] {
        [[x0, y0, x1, y0 + t], [x0, y1 - t, x1, y1], [x0, y0, x0 + t, y1], [x1 - t, y0, x1, y1]]
    }

    #[test]
    fn a_scans_rules_and_boxes_are_found_and_its_dark_areas_and_strokes_are_not() {
        let mut rects = vec![[50, 100, 450, 102], [400, 200, 500, 230], [300, 160, 308, 162]];
        rects.extend(outline(50, 200, 70, 220, 2));
        rects.extend(outline(100, 300, 400, 340, 2));
        let (boxes, rules) = lines_in(&image(600, 400, &rects), 600, 10, 6);
        // The rule; not the filled area (too thick), not the short stroke (too short).
        assert_eq!(rules, [[50, 100, 450, 102]]);
        assert_eq!(boxes, [[50, 200, 70, 220], [100, 300, 400, 340]]);
    }

    #[test]
    fn two_lines_without_sides_are_two_rules() {
        let (boxes, rules) = lines_in(&image(300, 200, &[[20, 40, 220, 42], [20, 80, 220, 82]]), 300, 10, 6);
        assert!(boxes.is_empty() && rules.len() == 2, "{boxes:?} {rules:?}");
    }
}
