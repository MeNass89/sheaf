//! Acrobat's Print Production ▸ Trap Presets. Acrobat only stores the preset; the traps are drawn by
//! an Adobe In-RIP Trapping device at print time. Sheaf draws them itself on the separated plates
//! (raster trapping, as a RIP does): where two abutting colours each have an ink the other lacks, the
//! lighter colour's inks spread under the darker one, so a slightly misregistered plate leaves no
//! paper gap. Ghostscript's own TrapX option is not used: on 10.02 it corrupts the plates.

use crate::separations::Plate;
use crate::{OpdfError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One trap preset, with Acrobat's settings and its defaults (Trap Presets ▸ Create).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrapPreset {
    pub name: String,
    /// Trap width between two inks other than black, in points.
    pub width_pt: f32,
    /// Trap width into black and rich black, in points.
    pub black_width_pt: f32,
    /// How much an ink must change between two colours before they are trapped, 0–1.
    pub step: f32,
    /// The black coverage, 0–1, from which a colour counts as black for the black width.
    pub black_limit: f32,
    /// The neutral density from which a colour counts as black for the black width.
    pub black_density: f32,
    /// When the lighter colour's neutral density is at least this share of the darker one's, the
    /// trap straddles the edge instead of spreading the lighter colour (Acrobat's Sliding Trap), 0–1.
    pub sliding: f32,
    /// How much of the lighter colour's ink the trap carries, 0–1 (Acrobat's Trap Color Reduction).
    pub reduction: f32,
}

impl Default for TrapPreset {
    fn default() -> Self {
        TrapPreset { name: "Default".into(), width_pt: 0.25, black_width_pt: 0.5, step: 0.10, black_limit: 1.0, black_density: 1.6, sliding: 0.70, reduction: 1.0 }
    }
}

impl TrapPreset {
    /// The settings out of Acrobat's ranges, in words; None when the preset can be used.
    pub fn problem(&self) -> Option<&'static str> {
        let unit = |v: f32| (0.0..=1.0).contains(&v);
        if !(0.0..=8.0).contains(&self.width_pt) || !(0.0..=8.0).contains(&self.black_width_pt) { return Some("Use trap widths from 0 to 8 points"); }
        if !(0.01..=1.0).contains(&self.step) { return Some("Use a step limit from 1 to 100%"); }
        if !unit(self.black_limit) || !unit(self.sliding) || !unit(self.reduction) { return Some("Use black limit, sliding trap and colour reduction from 0 to 100%"); }
        if !(0.001..=10.0).contains(&self.black_density) { return Some("Use a black density from 0.001 to 10"); }
        None
    }
}

/// The neutral density of full coverage of each process ink (Adobe In-RIP Trapping's defaults).
const PROCESS_ND: [f32; 4] = [0.61, 0.76, 0.16, 1.70];

fn ink_density(plate: &Plate) -> f32 {
    plate.cmyk.iter().zip(PROCESS_ND).map(|(c, nd)| c * nd).sum::<f32>().max(0.05)
}

/// Traps `plates` (one page, all the same size, rendered at `dpi`) in place with `preset`. Returns the
/// number of pixels a trap changed.
pub fn apply(plates: &mut [Plate], preset: &TrapPreset, dpi: f32) -> usize {
    let Some(first) = plates.first() else { return 0 };
    let (w, h) = (first.width as usize, first.height as usize);
    if plates.iter().any(|p| p.width as usize != w || p.height as usize != h || p.rgba.len() != w * h * 4) { return 0; }
    let n = plates.len();
    let black = plates.iter().position(|p| p.name == "Black");
    let nd: Vec<f32> = plates.iter().map(ink_density).collect();
    // Coverage 0–1 per pixel and ink (black on the plate = full ink).
    let cov: Vec<f32> = (0..w * h).flat_map(|i| plates.iter().map(move |p| 1.0 - f32::from(p.rgba[i * 4]) / 255.0)).collect();
    let density: Vec<f32> = (0..w * h).map(|i| (0..n).map(|k| cov[i * n + k] * nd[k]).sum()).collect();
    let is_black: Vec<bool> = (0..w * h).map(|i| density[i] >= preset.black_density || black.is_some_and(|b| cov[i * n + b] >= preset.black_limit - 1e-3)).collect();
    let px = |pt: f32| (pt * dpi / 72.0).round().max(if pt > 0.0 { 1.0 } else { 0.0 }) as isize;
    let (r_colour, r_black) = (px(preset.width_pt), px(preset.black_width_pt));
    let reach = r_colour.max(r_black);
    let near = near_edges(&cov, n, w, h, preset.step, reach);
    // Rows of the result in parallel: each pixel's trap reads only the original coverage.
    let mut out = cov.clone();
    let threads = std::thread::available_parallelism().map_or(1, |t| t.get()).min(h.max(1));
    let rows_each = h.div_ceil(threads);
    let changed: usize = std::thread::scope(|s| {
        let workers: Vec<_> = out.chunks_mut(rows_each * w * n).enumerate().map(|(t, chunk)| {
            let (cov, density, is_black, near) = (&cov, &density, &is_black, &near);
            s.spawn(move || {
                let mut changed = 0;
                for (row, line) in chunk.chunks_mut(w * n).enumerate() {
                    let y = (t * rows_each + row) as isize;
                    for x in 0..w as isize {
                        let q = y as usize * w + x as usize;
                        if near[q] && trap_pixel(cov, density, is_black, n, w, h, x, y, &mut line[x as usize * n..x as usize * n + n], preset, (r_colour, r_black, reach)) {
                            changed += 1;
                        }
                    }
                }
                changed
            })
        }).collect();
        workers.into_iter().map(|w| w.join().unwrap_or(0)).sum()
    });
    for (k, plate) in plates.iter_mut().enumerate() {
        for (i, px) in plate.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let v = ((1.0 - out[i * n + k]) * 255.0).round() as u8;
            *px = [v, v, v, 255];
        }
    }
    changed
}

/// The trap pixel (`x`, `y`) takes from its neighbours into `ink` (its coverage, already copied);
/// true when it changed. `radii`: the colour width, the black width and the larger of the two, in pixels.
#[allow(clippy::too_many_arguments)]
fn trap_pixel(cov: &[f32], density: &[f32], is_black: &[bool], n: usize, w: usize, h: usize, x: isize, y: isize, ink: &mut [f32], preset: &TrapPreset, radii: (isize, isize, isize)) -> bool {
    let (r_colour, r_black, reach) = radii;
    let q = y as usize * w + x as usize;
    let (b, dq, q_black) = (&cov[q * n..q * n + n], density[q], is_black[q]);
    let mut grown = false;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (sx, sy) = (x + dx, y + dy);
            if (dx == 0 && dy == 0) || sx < 0 || sy < 0 || sx >= w as isize || sy >= h as isize { continue; }
            let p = sy as usize * w + sx as usize;
            let a = &cov[p * n..p * n + n];
            // A trap only where each colour has an ink the other lacks (a gap would show paper).
            if !(a.iter().zip(b).any(|(a, b)| a - b > preset.step) && b.iter().zip(a).any(|(b, a)| b - a > preset.step)) { continue; }
            let dp = density[p];
            // q receives p's inks when p is the lighter colour, or the two straddle the edge.
            let sliding = dp.min(dq) >= preset.sliding * dp.max(dq);
            if dp > dq && !sliding { continue; }
            let mut radius = if q_black || is_black[p] { r_black } else { r_colour };
            if sliding { radius = (radius + 1) / 2; }
            if dx * dx + dy * dy > radius * radius { continue; }
            for (o, a) in ink.iter_mut().zip(a) {
                let v = a * preset.reduction;
                if v > *o + 1e-3 { *o = v; grown = true; }
            }
        }
    }
    grown
}

/// The pixels within `reach` of a colour edge (a 4-neighbour whose ink differs by more than `step`):
/// only they can take a trap, so the neighbourhood scan skips the rest. A two-pass chamfer distance
/// (3 straight, 4 diagonal) over-estimates the true distance by at most 8%, so the reach is widened
/// to match.
fn near_edges(cov: &[f32], n: usize, w: usize, h: usize, step: f32, reach: isize) -> Vec<bool> {
    let differs = |a: usize, b: usize| (0..n).any(|k| (cov[a * n + k] - cov[b * n + k]).abs() > step);
    const FAR: u32 = u32::MAX / 2;
    let mut d: Vec<u32> = (0..w * h).map(|i| {
        let (x, y) = (i % w, i / w);
        let edge = (x > 0 && differs(i, i - 1)) || (x + 1 < w && differs(i, i + 1)) || (y > 0 && differs(i, i - w)) || (y + 1 < h && differs(i, i + w));
        if edge { 0 } else { FAR }
    }).collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if x > 0 { v = v.min(d[i - 1] + 3); }
            if y > 0 {
                v = v.min(d[i - w] + 3);
                if x > 0 { v = v.min(d[i - w - 1] + 4); }
                if x + 1 < w { v = v.min(d[i - w + 1] + 4); }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if x + 1 < w { v = v.min(d[i + 1] + 3); }
            if y + 1 < h {
                v = v.min(d[i + w] + 3);
                if x + 1 < w { v = v.min(d[i + w + 1] + 4); }
                if x > 0 { v = v.min(d[i + w - 1] + 4); }
            }
            d[i] = v;
        }
    }
    let limit = (reach as u32 + 1) * 3 * 109 / 100;
    d.into_iter().map(|v| v <= limit).collect()
}

/// Output Preview with traps: the preview's plates of `page` trapped at their true width on screen.
pub fn preview(bytes: &[u8], password: Option<&str>, page: usize, preset: &TrapPreset, progress: &mut dyn FnMut() -> bool) -> Result<Vec<Plate>> {
    if let Some(problem) = preset.problem() { return Err(OpdfError::Unsupported(problem.into())); }
    let mut plates = crate::separations::preview(bytes, password, page, progress)?;
    let (w, h) = page_size(bytes, page).unwrap_or((612.0, 792.0));
    apply(&mut plates, preset, crate::separations::preview_dpi(w, h));
    Ok(plates)
}

/// The crop box of `page` (else its media box, inherited from the page tree) in points.
fn page_size(bytes: &[u8], page: usize) -> Option<(f32, f32)> {
    let pdf = lopdf::Document::load_mem(bytes).ok()?;
    let mut id = *pdf.get_pages().get(&(page as u32 + 1))?;
    for _ in 0..32 {
        let dict = pdf.get_dictionary(id).ok()?;
        for key in [&b"CropBox"[..], b"MediaBox"] {
            if let Ok(b) = dict.get(key).and_then(|o| pdf.dereference(o)).and_then(|(_, o)| o.as_array()) {
                let v: Vec<f32> = b.iter().filter_map(|n| n.as_float().ok()).collect();
                if v.len() == 4 { return Some(((v[2] - v[0]).abs(), (v[3] - v[1]).abs())); }
            }
        }
        id = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

/// Saves the trapped plates of `pages` (0-based) rendered at `dpi`, one 8-bit grey TIFF per page and ink
/// (black = full ink, as plate-setters read them), named `<prefix> p<N> <ink>.tif` (`prefix`: a folder
/// and the file stem).
pub fn save_trapped_plates(bytes: &[u8], password: Option<&str>, pages: &[usize], dpi: u32, preset: &TrapPreset, prefix: &Path, progress: &mut dyn FnMut() -> bool) -> Result<Vec<PathBuf>> {
    if let Some(problem) = preset.problem() { return Err(OpdfError::Unsupported(problem.into())); }
    if !(72..=2400).contains(&dpi) { return Err(OpdfError::Unsupported("Use a plate resolution from 72 to 2400 dpi".into())); }
    let mut written = Vec::new();
    for &page in pages {
        let mut plates = crate::separations::render(bytes, password, page, Some(dpi), progress)?;
        apply(&mut plates, preset, dpi as f32);
        for plate in &plates {
            let path = PathBuf::from(format!("{} p{} {}.tif", prefix.display(), page + 1, plate.name));
            let grey: Vec<u8> = plate.rgba.as_chunks::<4>().0.iter().map(|p| p[0]).collect();
            let image = image::GrayImage::from_raw(plate.width, plate.height, grey).ok_or_else(|| OpdfError::Conversion("A plate has the wrong size".into()))?;
            crate::paths::atomic_write(&path, |part| image.save_with_format(part, image::ImageFormat::Tiff).map_err(|e| OpdfError::Conversion(e.to_string())))?;
            written.push(path);
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plates from rows of CMYK coverage (0–255 ink), one row of `w` pixels per call.
    fn plates(row: &[[u8; 4]]) -> Vec<Plate> {
        ["Cyan", "Magenta", "Yellow", "Black"].iter().enumerate().map(|(k, name)| {
            let mut cmyk = [0.0; 4];
            cmyk[k] = 1.0;
            Plate { name: (*name).into(), width: row.len() as u32, height: 1, rgba: row.iter().flat_map(|c| [255 - c[k], 255 - c[k], 255 - c[k], 255]).collect(), cmyk }
        }).collect()
    }

    fn ink(plates: &[Plate], k: usize) -> Vec<u8> {
        plates[k].rgba.as_chunks::<4>().0.iter().map(|p| 255 - p[0]).collect()
    }

    /// Yellow abutting cyan: yellow (lighter) spreads under cyan by the trap width, cyan stays put.
    #[test]
    fn the_lighter_colour_spreads_under_the_darker_by_the_trap_width() {
        let (y, c) = ([0, 0, 255, 0], [255, 0, 0, 0]);
        let mut p = plates(&[y, y, y, y, c, c, c, c]);
        // 0.5 pt at 288 dpi = 2 px.
        let preset = TrapPreset { width_pt: 0.5, ..TrapPreset::default() };
        assert!(apply(&mut p, &preset, 288.0) > 0);
        assert_eq!(ink(&p, 2), [255, 255, 255, 255, 255, 255, 0, 0], "yellow two pixels into the cyan");
        assert_eq!(ink(&p, 0), [0, 0, 0, 0, 255, 255, 255, 255], "cyan does not move");
    }

    /// Colours that share their inks (a tint next to its full colour) or meet paper are not trapped.
    #[test]
    fn colours_without_a_possible_gap_and_paper_are_left_alone() {
        let mut p = plates(&[[128, 0, 0, 0], [128, 0, 0, 0], [255, 0, 0, 0], [255, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]]);
        let before: Vec<Vec<u8>> = (0..4).map(|k| ink(&p, k)).collect();
        assert_eq!(apply(&mut p, &TrapPreset::default(), 600.0), 0);
        assert_eq!((0..4).map(|k| ink(&p, k)).collect::<Vec<_>>(), before);
        // Below the step limit: 5% magenta against 5% cyan does not trap at 10%.
        let mut faint = plates(&[[0, 13, 0, 0], [13, 0, 0, 0]]);
        assert_eq!(apply(&mut faint, &TrapPreset::default(), 600.0), 0);
    }

    /// Into black the trap takes the black width; colour reduction lightens what spreads.
    #[test]
    fn black_takes_the_black_width_and_reduction_lightens_the_trap() {
        let (m, k) = ([0, 255, 0, 0], [0, 0, 0, 255]);
        let mut p = plates(&[m, m, m, m, k, k, k, k]);
        // 0.25 pt colour, 0.75 pt black at 288 dpi: 1 px and 3 px.
        let preset = TrapPreset { black_width_pt: 0.75, reduction: 0.5, ..TrapPreset::default() };
        apply(&mut p, &preset, 288.0);
        assert_eq!(ink(&p, 1), [255, 255, 255, 255, 127, 127, 127, 0], "half-strength magenta three pixels under the black; the magenta itself unchanged");
    }

    /// Close densities straddle the edge (sliding trap): each side takes the other's ink.
    #[test]
    fn close_densities_trap_on_both_sides_of_the_edge() {
        let (c, m) = ([255, 0, 0, 0], [0, 255, 0, 0]);
        let mut p = plates(&[c, c, c, m, m, m]);
        // 1 pt at 144 dpi = 2 px, halved on each side.
        apply(&mut p, &TrapPreset { width_pt: 1.0, ..TrapPreset::default() }, 144.0);
        assert_eq!(ink(&p, 0), [255, 255, 255, 255, 0, 0]);
        assert_eq!(ink(&p, 1), [0, 0, 255, 255, 255, 255]);
        // Sliding off: only the lighter cyan (0.61 against 0.76) spreads, by the whole width.
        let mut q = plates(&[c, c, c, m, m, m]);
        apply(&mut q, &TrapPreset { width_pt: 1.0, sliding: 1.0, ..TrapPreset::default() }, 144.0);
        assert_eq!(ink(&q, 0), [255, 255, 255, 255, 255, 0]);
        assert_eq!(ink(&q, 1), [0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn presets_out_of_range_are_refused() {
        assert_eq!(TrapPreset::default().problem(), None);
        assert!(TrapPreset { width_pt: 9.0, ..TrapPreset::default() }.problem().is_some());
        assert!(TrapPreset { step: 0.0, ..TrapPreset::default() }.problem().is_some());
        assert!(TrapPreset { sliding: 1.5, ..TrapPreset::default() }.problem().is_some());
    }

    /// The real page: a cyan box on a yellow page, rendered by Ghostscript and saved as trapped plates.
    #[test]
    fn trapped_plates_of_a_real_page_are_saved_one_tiff_per_ink() {
        use lopdf::{dictionary, Document, Stream};
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let content = pdf.add_object(Stream::new(dictionary! {}, b"0 0 1 0 k 0 0 72 72 re f 1 0 0 0 k 18 18 36 36 re f".to_vec()));
        let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 72.into(), 72.into()], "Contents" => content });
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()] }.into());
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        pdf.trailer.set("Root", root);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let preset = TrapPreset { width_pt: 1.0, ..TrapPreset::default() };
        let files = save_trapped_plates(&bytes, None, &[0], 144, &preset, &dir.path().join("box"), &mut || true).unwrap();
        assert_eq!(files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>(),
            ["box p1 Cyan.tif", "box p1 Magenta.tif", "box p1 Yellow.tif", "box p1 Black.tif"]);
        let yellow = image::open(&files[2]).unwrap().to_luma8();
        assert_eq!(yellow.dimensions(), (144, 144));
        // Yellow (lighter) now runs 2 px into the cyan box, whose edge is at 36 px; inside stays clear.
        assert!(yellow.get_pixel(37, 72)[0] < 64 && yellow.get_pixel(72, 72)[0] > 192, "{:?} {:?}", yellow.get_pixel(37, 72), yellow.get_pixel(72, 72));
        assert!(save_trapped_plates(&bytes, None, &[0], 10, &preset, &dir.path().join("box"), &mut || true).is_err());
        // The preview fits the 72-point page in 1,200 pixels (1,200 dpi): 1 pt of yellow is ~17 px into the cyan.
        assert_eq!(page_size(&bytes, 0), Some((72.0, 72.0)));
        let shown = preview(&bytes, None, 0, &preset, &mut || true).unwrap();
        let yellow = &shown[2].rgba;
        let at = |x: usize| yellow[(600 * 1200 + x) * 4];
        assert!(at(310) < 64 && at(330) > 192, "{} {}", at(310), at(330));
    }
}
