//! Enhance scanned pages (Acrobat's Scan & OCR ▸ Enhance): a page that is only a picture is
//! straightened (its lines made level again) and cleaned (the paper white, the ink dark), then
//! drawn again as a JPEG on a page of the same size. A page with text of its own is copied as it
//! is. OCR runs after it, so its words land on level lines.

use crate::engine::{pdfium, pdfium_lock};
use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use image::{codecs::jpeg::JpegEncoder, GrayImage, Rgb, RgbImage};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

/// Resolution a scanned page is drawn again at.
const DPI: f32 = 200.0;
/// A tilt past this is left alone: a page turned on purpose, not a sheet fed askew.
const MAX_SKEW: f32 = 5.0;
/// Under this tilt nothing is turned (each turn softens the picture a little).
const MIN_SKEW: f32 = 0.15;

/// What [`enhance`] did.
#[derive(Debug, Clone)]
pub struct EnhanceReport {
    pub path: PathBuf,
    /// Scanned pages straightened and cleaned, and pages with text of their own left as they were.
    pub enhanced: usize,
    pub kept: usize,
}

/// Writes `out` from `input` with its scanned pages straightened and cleaned. `progress` gets
/// 0..=1 before each page; returning `false` stops, with nothing written.
pub fn enhance(input: &Input, out: &Path, progress: &mut dyn FnMut(f32) -> bool) -> Result<EnhanceReport> {
    // Held throughout: a page takes a fraction of a second at 200 dpi.
    let _g = pdfium_lock();
    let src = input.open()?;
    let mut dst = pdfium().create_new_pdf()?;
    let n = src.pages().len();
    let (mut enhanced, mut kept) = (0, 0);
    let tmp = tempfile::tempdir()?;
    for i in 0..n {
        if !progress(i as f32 / n.max(1) as f32) {
            return Err(OpdfError::Cancelled);
        }
        let page = src.pages().get(i)?;
        let at = dst.pages().len();
        if !is_scan(&page) {
            dst.pages_mut().copy_page_range_from_document(&src, i..=i, at)?;
            kept += 1;
            continue;
        }
        let (w, h) = (page.width(), page.height());
        let px = ((w.value * DPI / 72.0).round() as i32).max(1);
        let rgb = page.render_with_config(&PdfRenderConfig::new().set_target_width(px))?.as_image()?.to_rgb8();
        let clean = clean_page(&rgb);
        let jpeg = tmp.path().join(format!("{i}.jpg"));
        JpegEncoder::new_with_quality(std::fs::File::create(&jpeg)?, 85).encode_image(&clean)?;
        let mut new = dst.pages_mut().create_page_at_index(PdfPagePaperSize::Custom(w, h), at)?;
        let mut obj = PdfPageImageObject::new_from_jpeg_file(&dst, &jpeg)?;
        obj.scale(w.value, h.value)?;
        new.objects_mut().add_image_object(obj)?;
        enhanced += 1;
    }
    progress(1.0);
    let path = atomic_write(out, |part| Ok(dst.save_to_file(part)?))?;
    Ok(EnhanceReport { path, enhanced, kept })
}

/// How many words a recogniser reads in `img` that look like words (three letters or more, a vowel
/// among them): an upside-down or sideways scan reads few.
fn real_words(ocr: &crate::pdfcraft::engine::ocr::Ocr, img: &image::RgbaImage) -> usize {
    let vowel = |w: &str| w.chars().any(|c| "aeiouyAEIOUYàâéèêëîïôûüÀÂÉÈÊËÎÏÔÛÜ".contains(c));
    ocr.recognize(img.as_raw(), img.width(), img.height())
        // Upright only: the reader also reads words standing on their side, so a page turned a
        // quarter would count as many as one set straight.
        .map(|lines| {
            lines
                .iter()
                .flat_map(|l| &l.words)
                .filter(|w| matches!(w.turn, crate::pdfcraft::engine::ocr::Turn::Upright) && w.text.chars().filter(|c| c.is_alphabetic()).count() >= 3 && vowel(&w.text))
                .count()
        })
        .unwrap_or(0)
}

/// For each scanned page among `pages` (a picture and no text: [`is_scan`]), the clockwise turn (90,
/// 180 or 270) that sets it upright, when reading it so finds clearly more words than as it is.
pub fn scan_upright_turns(bytes: &[u8], password: Option<&str>, pages: &[usize]) -> Vec<(usize, i64)> {
    let Ok(ocr) = crate::ocr::engine() else { return Vec::new() };
    // One page's picture at a time (Sol, 27/10: every page was rendered first, several GB held for
    // a big scan). The document is opened again for each page so PDFium's lock is not held while
    // the page is read.
    let shot = |p: usize| -> Option<image::RgbaImage> {
        let _g = crate::engine::pdfium_lock();
        let doc = crate::engine::open_pdf_bytes(bytes, password).ok()?;
        let page = doc.pages().get(p as i32).ok()?;
        if !is_scan(&page) {
            return None;
        }
        // About 110 dpi, enough to read body text.
        let w = (page.width().value * 1.5).round().clamp(64.0, 2000.0) as i32;
        Some(page.render_with_config(&PdfRenderConfig::new().set_target_width(w)).ok()?.as_image().ok()?.to_rgba8())
    };
    let mut out = Vec::new();
    for &p in pages {
        let Some(img) = shot(p) else { continue };
        let as_is = real_words(&ocr, &img);
        let tries = [(90, image::imageops::rotate90(&img)), (180, image::imageops::rotate180(&img)), (270, image::imageops::rotate270(&img))];
        let best = tries.iter().map(|(d, i)| (*d, real_words(&ocr, i))).max_by_key(|t| t.1);
        if let Some((d, n)) = best
            && n >= 5
            && n * 2 > as_is * 3
        {
            out.push((p, d));
        }
    }
    out
}

/// Which pages of PDF `bytes` are scans ([`is_scan`]), in order; all false when it cannot be read.
pub fn scanned_pages(bytes: &[u8], password: Option<&str>) -> Vec<bool> {
    let _g = crate::engine::pdfium_lock();
    let Ok(doc) = crate::engine::open_pdf_bytes(bytes, password) else { return Vec::new() };
    doc.pages().iter().map(|p| is_scan(&p)).collect()
}

/// A scanned page: a picture over most of it, and no text drawn of its own. An OCR layer
/// (invisible text over the picture) does not make it a text page (Sol, 26/10: OCR'd scans were
/// kept as they were).
pub fn is_scan(page: &PdfPage) -> bool {
    let drawn_text = page.objects().iter().any(|o| o.as_text_object().is_some_and(|t| t.render_mode() != PdfPageTextRenderMode::Invisible && !t.text().trim().is_empty()));
    let area = page.width().value * page.height().value;
    !drawn_text
        && page.objects().iter().any(|o| {
            o.object_type() == PdfPageObjectType::Image && o.bounds().is_ok_and(|b| b.width().value * b.height().value >= 0.6 * area)
        })
}

/// A page's picture straightened, its paper made white and its ink dark.
pub fn clean_page(rgb: &RgbImage) -> RgbImage {
    let gray = image::imageops::grayscale(rgb);
    let skew = skew_angle(&gray);
    let mut out = if skew.abs() >= MIN_SKEW { rotate_white(rgb, skew) } else { rgb.clone() };
    levels(&mut out);
    out
}

/// How far a page's lines are tilted, in degrees clockwise as seen (−5 to 5; 0 when it cannot
/// tell): the angle at which its dark pixels pile into the fewest, fullest rows, as text lines
/// do when level.
pub fn skew_angle(gray: &GrayImage) -> f32 {
    // Read on a copy about 600 px wide: enough for a tenth of a degree.
    let k = (gray.width() / 600).max(1);
    let small = image::imageops::thumbnail(gray, gray.width() / k, gray.height() / k);
    let paper = percentile(small.as_raw(), 0.5);
    let dark: Vec<(f32, f32)> = small.enumerate_pixels().filter(|(_, _, p)| (p.0[0] as f32) < paper as f32 * 0.6).map(|(x, y, _)| (x as f32, y as f32)).collect();
    if dark.len() < 50 {
        return 0.0;
    }
    let rows = small.height() as usize;
    let score = |deg: f32| {
        let t = deg.to_radians().tan();
        let mut hist = vec![0u32; rows * 3];
        for &(x, y) in &dark {
            let r = (y - x * t).round() as isize + rows as isize;
            if let Some(c) = usize::try_from(r).ok().and_then(|r| hist.get_mut(r)) {
                *c += 1;
            }
        }
        hist.iter().map(|&c| (c as f64) * (c as f64)).sum::<f64>()
    };
    let best = |from: f32, to: f32, step: f32| {
        let steps = ((to - from) / step).round() as i32;
        (0..=steps).map(|i| from + i as f32 * step).max_by(|a, b| score(*a).total_cmp(&score(*b))).unwrap_or(0.0)
    };
    let coarse = best(-MAX_SKEW, MAX_SKEW, 0.5);
    let fine = best(coarse - 0.5, coarse + 0.5, 0.05);
    if fine.abs() > MAX_SKEW { 0.0 } else { fine }
}

/// `img` turned `deg` degrees anticlockwise about its centre (undoing a clockwise tilt of `deg`),
/// the corners it uncovers white.
pub fn rotate_white(img: &RgbImage, deg: f32) -> RgbImage {
    let (w, h) = img.dimensions();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let (s, c) = deg.to_radians().sin_cos();
    RgbImage::from_fn(w, h, |x, y| {
        // The source point: this one turned clockwise by `deg`.
        let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
        let (sx, sy) = (cx + dx * c - dy * s - 0.5, cy + dx * s + dy * c - 0.5);
        bilinear(img, sx, sy)
    })
}

fn bilinear(img: &RgbImage, x: f32, y: f32) -> Rgb<u8> {
    let (w, h) = img.dimensions();
    if x < 0.0 || y < 0.0 || x > (w - 1) as f32 || y > (h - 1) as f32 {
        return Rgb([255, 255, 255]);
    }
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let p = |x, y| img.get_pixel(x, y).0;
    let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
    Rgb(std::array::from_fn(|i| {
        let top = a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx;
        let bottom = c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx;
        (top * (1.0 - fy) + bottom * fy).round() as u8
    }))
}

/// Stretches the page's tones: the paper (its usual brightness, less a little) becomes white, the
/// darkest ink black, colours kept in proportion.
pub fn levels(img: &mut RgbImage) {
    let luma: Vec<u8> = img.pixels().map(|p| ((p.0[0] as u32 * 3 + p.0[1] as u32 * 6 + p.0[2] as u32) / 10) as u8).collect();
    let (ink, paper) = (percentile(&luma, 0.01) as f32, percentile(&luma, 0.5) as f32 * 0.92);
    if paper - ink < 32.0 {
        return;
    }
    let scale = 255.0 / (paper - ink);
    for p in img.pixels_mut() {
        p.0 = p.0.map(|v| ((v as f32 - ink) * scale).clamp(0.0, 255.0).round() as u8);
    }
}

/// The value below which `q` (0–1) of `values` lie.
fn percentile(values: &[u8], q: f32) -> u8 {
    let mut counts = [0usize; 256];
    values.iter().for_each(|&v| counts[v as usize] += 1);
    let target = (values.len() as f32 * q) as usize;
    let mut seen = 0;
    for (v, &c) in counts.iter().enumerate() {
        seen += c;
        if seen > target {
            return v as u8;
        }
    }
    255
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page of text lines (dark bars with gaps, as words), 1000 × 1300 px, on greyish paper.
    fn lines() -> RgbImage {
        RgbImage::from_fn(1000, 1300, |x, y| {
            let on_line = (100..1200).contains(&y) && y % 40 < 12 && (100..900).contains(&x) && (x / 30) % 4 != 3;
            if on_line { Rgb([60, 60, 70]) } else { Rgb([215, 210, 200]) }
        })
    }

    #[test]
    fn a_tilted_page_is_measured_and_straightened() {
        let gray = |i: &RgbImage| image::imageops::grayscale(i);
        assert!(skew_angle(&gray(&lines())).abs() < 0.1);
        // Fed 2.5° clockwise askew.
        let tilted = rotate_white(&lines(), -2.5);
        let skew = skew_angle(&gray(&tilted));
        assert!((skew - 2.5).abs() < 0.15, "{skew}");
        let straight = clean_page(&tilted);
        assert!(skew_angle(&gray(&straight)).abs() < 0.15);
    }

    #[test]
    fn the_paper_turns_white_and_the_ink_dark() {
        let mut page = lines();
        levels(&mut page);
        assert_eq!(page.get_pixel(5, 5).0, [255, 255, 255]);
        assert!(page.get_pixel(130, 121).0.iter().all(|&v| v < 25), "{:?}", page.get_pixel(130, 121));
    }
}
