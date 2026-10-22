//! Sign: the signature as a picture for the panel and the page overlay (black ink on
//! transparent, straight RGBA), and the drawing pad's strokes in the form the engine takes.
//! No UI types, so it is tested on its own.

use sheaf_core::sign::Graphic;

/// A picture `w` × `h` pixels, straight RGBA, rows from the top.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Picture {
    fn new(w: u32, h: u32) -> Self {
        Self { w, h, rgba: vec![0; (w * h * 4) as usize] }
    }

    fn ink(&mut self, x: i64, y: i64) {
        if x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h {
            let i = ((y as u32 * self.w + x as u32) * 4) as usize;
            self.rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }

    /// Pixels with ink.
    pub fn inked(&self) -> usize {
        self.rgba.chunks(4).filter(|p| p[3] > 0).count()
    }
}

/// The pad's strokes, as the Slint pad writes them ("M x y L x y L … M …", pad pixels, y down)
/// on a pad `w` pixels wide and `h` high → strokes, y up, in units where the pad is 1 wide.
/// Points outside the pad are held to its edges; a stroke of one point stays (a dot).
pub fn parse_pad(commands: &str, w: f64, h: f64) -> Vec<Vec<[f64; 2]>> {
    let w = w.max(1.0);
    let mut strokes: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut words = commands.split_whitespace();
    while let Some(op) = words.next() {
        let (Some(x), Some(y)) = (words.next().and_then(|v| v.parse::<f64>().ok()), words.next().and_then(|v| v.parse::<f64>().ok())) else {
            break;
        };
        if !(x.is_finite() && y.is_finite()) {
            continue;
        }
        let p = [x.clamp(0.0, w) / w, (h - y.clamp(0.0, h)) / w];
        match op {
            "M" => strokes.push(vec![p]),
            "L" => match strokes.last_mut() {
                Some(s) => s.push(p),
                None => strokes.push(vec![p]),
            },
            _ => {}
        }
    }
    strokes
}

/// `graphic` drawn as large as fits in `w` × `h` pixels, centred, black on transparent. An
/// image keeps its own colours. `None` when there is nothing to draw (or the image can't be
/// read).
pub fn render(graphic: &Graphic, w: u32, h: u32) -> Option<Picture> {
    if w == 0 || h == 0 {
        return None;
    }
    match graphic {
        Graphic::Typed(t, font) => typed(t, *font, w, h),
        Graphic::Drawn(s) => drawn(s, w, h),
        Graphic::Image(p) => picture(p, w, h),
    }
}

/// `(scale, x offset, y offset)` fitting a `bw` × `bh` box in `w` × `h` with a margin.
fn fit(bw: f64, bh: f64, w: u32, h: u32, margin: f64) -> (f64, f64, f64) {
    let (aw, ah) = (w as f64 - 2.0 * margin, h as f64 - 2.0 * margin);
    let k = (aw / bw.max(1e-9)).min(ah / bh.max(1e-9));
    (k, margin + (aw - bw * k) / 2.0, margin + (ah - bh * k) / 2.0)
}

fn typed(text: &str, font: sheaf_core::script::ScriptFont, w: u32, h: u32) -> Option<Picture> {
    let o = sheaf_core::script::outline(font, text.trim());
    if !o.contours.iter().any(|c| c.len() > 2) {
        return None;
    }
    let [l, b, r, t] = o.bounds();
    let (k, ox, oy) = fit(r - l, t - b, w, h, 1.0);
    // Pixel space, y down, then an even-odd scanline fill.
    let polys: Vec<Vec<(f64, f64)>> = o.contours.iter().map(|c| c.iter().map(|p| (ox + (p[0] - l) * k, oy + (t - p[1]) * k)).collect()).collect();
    let mut pic = Picture::new(w, h);
    for y in 0..h {
        let sy = y as f64 + 0.5;
        let mut xs: Vec<f64> = Vec::new();
        for poly in &polys {
            for i in 0..poly.len() {
                let (a, c) = (poly[i], poly[(i + 1) % poly.len()]);
                if (a.1 <= sy) != (c.1 <= sy) {
                    xs.push(a.0 + (sy - a.1) / (c.1 - a.1) * (c.0 - a.0));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            for x in pair[0].round().max(0.0) as i64..pair[1].round().min(w as f64) as i64 {
                pic.ink(x, y as i64);
            }
        }
    }
    Some(pic)
}

fn drawn(strokes: &[Vec<[f64; 2]>], w: u32, h: u32) -> Option<Picture> {
    let pts: Vec<[f64; 2]> = strokes.iter().flatten().copied().filter(|p| p.iter().all(|v| v.is_finite())).collect();
    if pts.is_empty() {
        return None;
    }
    let b = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    // As the engine places it: a dot or a line still gets some extent.
    let (bw, bh) = ((b[2] - b[0]).max(0.02), (b[3] - b[1]).max(0.02));
    let pen = (w.min(h) as f64 / 40.0).clamp(1.0, 4.0);
    let (k, ox, oy) = fit(bw, bh, w, h, pen + 1.0);
    let px = |p: &[f64; 2]| (ox + (p[0] - b[0]) * k + ((bw - (b[2] - b[0])) * k) / 2.0, oy + (b[1] + bh - p[1]) * k - ((bh - (b[3] - b[1])) * k) / 2.0);
    let mut pic = Picture::new(w, h);
    let r = pen.ceil() as i64;
    let mut dot = |cx: f64, cy: f64| {
        for dy in -r..=r {
            for dx in -r..=r {
                if ((dx * dx + dy * dy) as f64) <= pen * pen {
                    pic.ink(cx.round() as i64 + dx, cy.round() as i64 + dy);
                }
            }
        }
    };
    for s in strokes {
        let s: Vec<(f64, f64)> = s.iter().filter(|p| p.iter().all(|v| v.is_finite())).map(px).collect();
        if let [only] = s.as_slice() {
            dot(only.0, only.1);
        }
        for seg in s.windows(2) {
            let (a, c) = (seg[0], seg[1]);
            let n = ((c.0 - a.0).hypot(c.1 - a.1) / (pen / 2.0)).ceil().max(1.0) as usize;
            for i in 0..=n {
                let f = i as f64 / n as f64;
                dot(a.0 + (c.0 - a.0) * f, a.1 + (c.1 - a.1) * f);
            }
        }
    }
    Some(pic)
}

fn picture(path: &std::path::Path, w: u32, h: u32) -> Option<Picture> {
    // The engine's own reader: the same formats and size limits the signing accepts.
    let img = sheaf_core::pdfcraft::engine::SignatureImage::read(std::fs::File::open(path).ok()?).ok()?;
    let [iw, ih] = img.size();
    let (k, ox, oy) = fit(iw as f64, ih as f64, w, h, 0.0);
    let mut pic = Picture::new(w, h);
    let src = img.rgba();
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (((x as f64 + 0.5 - ox) / k).floor(), ((y as f64 + 0.5 - oy) / k).floor());
            if sx < 0.0 || sy < 0.0 || sx >= iw as f64 || sy >= ih as f64 {
                continue;
            }
            let s = (sy as usize * iw + sx as usize) * 4;
            let d = ((y * w + x) * 4) as usize;
            pic.rgba[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    Some(pic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_commands_become_strokes_y_up_in_pad_widths() {
        let s = parse_pad("M 0 100 L 200 0 L 300 50 M 150 50", 200.0, 100.0);
        // x held to the pad, y flipped, both in pad widths.
        assert_eq!(s, vec![vec![[0.0, 0.0], [1.0, 0.5], [1.0, 0.25]], vec![[0.75, 0.25]]]);
        assert!(parse_pad("", 200.0, 100.0).is_empty());
        assert_eq!(parse_pad("M 1 2 L x", 10.0, 10.0).len(), 1, "a broken tail is dropped, the rest kept");
    }

    #[test]
    fn each_kind_draws_ink_inside_its_box() {
        let t = render(&Graphic::Typed("Ada Lovelace".into(), Default::default()), 240, 60).unwrap();
        assert_eq!((t.w, t.h), (240, 60));
        assert!(t.inked() > 300, "typed ink {}", t.inked());
        let d = render(&Graphic::Drawn(vec![vec![[0.0, 0.0], [1.0, 0.3]]]), 240, 60).unwrap();
        assert!(d.inked() > 200, "drawn ink {}", d.inked());
        assert!(render(&Graphic::Typed("   ".into(), Default::default()), 240, 60).is_none());
        assert!(render(&Graphic::Drawn(vec![]), 240, 60).is_none());
        assert!(render(&Graphic::Image("/no/such/file.png".into()), 240, 60).is_none());
    }

    #[test]
    fn an_image_is_scaled_into_the_box_keeping_its_colours() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.png");
        image::RgbaImage::from_pixel(40, 10, image::Rgba([10, 20, 200, 255])).save(&p).unwrap();
        let pic = render(&Graphic::Image(p), 80, 80).unwrap();
        // 40 × 10 fits as 80 × 20, centred vertically.
        let at = |x: u32, y: u32| &pic.rgba[((y * 80 + x) * 4) as usize..][..4];
        assert_eq!(at(40, 40), [10, 20, 200, 255]);
        assert_eq!(at(40, 5)[3], 0);
        assert_eq!(pic.inked(), 80 * 20);
    }
}
