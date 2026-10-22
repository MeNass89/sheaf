//! Typed signatures in a choice of handwriting fonts.
//!
//! Dancing Script is PdfCraft's own (`engine::script_outline`); the others are bundled here
//! (`assets/fonts`, OFL and Apache licences beside them) and drawn the same way: each glyph
//! flattened to polygons in em units, characters the font lacks taken from Inter, slanted and
//! scaled to its capitals (adapted from pdfcraft-fonts' `script_outline`, MIT OR Apache-2.0).

use crate::pdfcraft::engine::{self, ScriptOutline};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};

/// The fonts a name can be typed in, in the order the signature pad offers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScriptFont {
    #[default]
    Dancing,
    GreatVibes,
    Satisfy,
    Caveat,
}

impl ScriptFont {
    pub const ALL: [ScriptFont; 4] = [ScriptFont::Dancing, ScriptFont::GreatVibes, ScriptFont::Satisfy, ScriptFont::Caveat];

    /// The font at `i` of [`Self::ALL`] (the first for anything else).
    pub fn from_index(i: usize) -> ScriptFont {
        Self::ALL.get(i).copied().unwrap_or_default()
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            ScriptFont::Dancing => "Dancing Script",
            ScriptFont::GreatVibes => "Great Vibes",
            ScriptFont::Satisfy => "Satisfy",
            ScriptFont::Caveat => "Caveat",
        }
    }

    fn bytes(self) -> Option<&'static [u8]> {
        match self {
            ScriptFont::Dancing => None,
            ScriptFont::GreatVibes => Some(include_bytes!("../assets/fonts/GreatVibes-Regular.ttf")),
            ScriptFont::Satisfy => Some(include_bytes!("../assets/fonts/Satisfy-Regular.ttf")),
            ScriptFont::Caveat => Some(include_bytes!("../assets/fonts/Caveat.ttf")),
        }
    }
}

/// Forward slant of fallback glyphs (about 12°), so they lean like the script.
const FALLBACK_SLANT: f64 = 0.21;
/// Bound on the points of a whole name.
const MAX_POINTS: usize = 262_144;
/// Segments per curve.
const STEPS: usize = 8;

/// The outlines of `text` in `font` (empty when it draws nothing: no letters it or the fallback
/// has, or more than `engine::MAX_SIGNATURE_CHARS` characters).
pub fn outline(font: ScriptFont, text: &str) -> ScriptOutline {
    let Some(bytes) = font.bytes() else { return engine::script_outline(text) };
    if text.chars().take(engine::MAX_SIGNATURE_CHARS + 1).count() > engine::MAX_SIGNATURE_CHARS {
        return ScriptOutline::default();
    }
    let Ok(face) = FontRef::new(bytes) else { return ScriptOutline::default() };
    let loc = LocationRef::default();
    let metrics = face.metrics(Size::unscaled(), loc);
    let scale = 1.0 / metrics.units_per_em.max(1) as f64;
    let (charmap, glyphs, advances) = (face.charmap(), face.outline_glyphs(), face.glyph_metrics(Size::unscaled(), loc));
    // The fallback face, scaled so its capitals are as tall as the script's.
    let fallback = FontRef::new(pdfcraft_fonts::INTER_REGULAR).ok().map(|f| {
        let m = f.metrics(Size::unscaled(), loc);
        let em = 1.0 / m.units_per_em.max(1) as f64;
        let ratio = match (metrics.cap_height, m.cap_height) {
            (Some(ours), Some(theirs)) if ours > 0.0 && theirs > 0.0 => (ours as f64 * scale) / (theirs as f64 * em),
            _ => 1.0,
        };
        let ratio = if ratio.is_finite() { ratio.clamp(0.5, 2.0) } else { 1.0 };
        (f.charmap(), f.outline_glyphs(), f.glyph_metrics(Size::unscaled(), loc), em * ratio)
    });
    let mut pen = Flatten { scale, ..Default::default() };
    for ch in text.chars() {
        if let Some(gid) = charmap.map(ch) {
            (pen.scale, pen.slant) = (scale, 0.0);
            if let Some(g) = glyphs.get(gid) {
                let _ = g.draw(DrawSettings::unhinted(Size::unscaled(), loc), &mut pen);
                pen.end();
            }
            pen.dx += advances.advance_width(gid).unwrap_or(0.0) as f64 * scale;
        } else if let Some((map, outlines, adv, em)) = &fallback
            && let Some(gid) = map.map(ch)
        {
            (pen.scale, pen.slant) = (*em, FALLBACK_SLANT);
            if let Some(g) = outlines.get(gid) {
                let _ = g.draw(DrawSettings::unhinted(Size::unscaled(), loc), &mut pen);
                pen.end();
            }
            pen.dx += adv.advance_width(gid).unwrap_or(0.0) as f64 * em;
        }
    }
    pen.end();
    if pen.too_complex {
        return ScriptOutline::default();
    }
    ScriptOutline { contours: pen.contours, width: pen.dx, ascent: metrics.ascent as f64 * scale, descent: metrics.descent as f64 * scale }
}

/// Glyph outlines flattened to polygons, in em units.
#[derive(Default)]
struct Flatten {
    contours: Vec<Vec<[f64; 2]>>,
    cur: Vec<[f64; 2]>,
    scale: f64,
    /// Horizontal shear (x += slant × y), for slanted fallback glyphs.
    slant: f64,
    dx: f64,
    points: usize,
    too_complex: bool,
}

impl Flatten {
    fn pt(&self, x: f32, y: f32) -> [f64; 2] {
        let (x, y) = (x as f64 * self.scale, y as f64 * self.scale);
        [self.dx + x + self.slant * y, y]
    }

    fn last(&self) -> [f64; 2] {
        self.cur.last().copied().unwrap_or([self.dx, 0.0])
    }

    fn push(&mut self, p: [f64; 2]) {
        if self.points >= MAX_POINTS {
            self.too_complex = true;
            return;
        }
        self.points += 1;
        self.cur.push(p);
    }

    fn end(&mut self) {
        if self.cur.len() > 2 {
            self.contours.push(std::mem::take(&mut self.cur));
        } else {
            self.cur.clear();
        }
    }
}

impl OutlinePen for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        self.end();
        self.push(self.pt(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.push(self.pt(x, y));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (p0, c, p) = (self.last(), self.pt(cx, cy), self.pt(x, y));
        for i in 1..=STEPS {
            let t = i as f64 / STEPS as f64;
            let u = 1.0 - t;
            self.push([u * u * p0[0] + 2.0 * u * t * c[0] + t * t * p[0], u * u * p0[1] + 2.0 * u * t * c[1] + t * t * p[1]]);
        }
    }

    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        let (p0, c0, c1, p) = (self.last(), self.pt(c0x, c0y), self.pt(c1x, c1y), self.pt(x, y));
        for i in 1..=STEPS {
            let t = i as f64 / STEPS as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.push([a * p0[0] + b * c0[0] + c * c1[0] + d * p[0], a * p0[1] + b * c0[1] + c * c1[1] + d * p[1]]);
        }
    }

    fn close(&mut self) {
        self.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_font_draws_a_name_and_falls_back_for_cyrillic() {
        for f in ScriptFont::ALL {
            let o = outline(f, "Ada Lovelace");
            assert!(o.contours.len() >= 8 && o.width > 1.0, "{}: {} contours, width {}", f.name(), o.contours.len(), o.width);
            assert!(o.ascent > 0.4 && o.descent < 0.0, "{}", f.name());
            let c = outline(f, "Саша");
            assert!(c.contours.len() >= 4, "{}: Cyrillic from the fallback ({} contours)", f.name(), c.contours.len());
        }
        // The fonts differ: the same name is a different width in each.
        let widths: Vec<f64> = ScriptFont::ALL.iter().map(|f| (outline(*f, "Ada Lovelace").width * 1000.0).round()).collect();
        assert!(widths.windows(2).all(|w| w[0] != w[1]), "{widths:?}");
        assert!(outline(ScriptFont::Satisfy, "").contours.is_empty());
        assert!(outline(ScriptFont::Caveat, &"W".repeat(engine::MAX_SIGNATURE_CHARS + 1)).contours.is_empty());
        assert_eq!(ScriptFont::from_index(2), ScriptFont::Satisfy);
        assert_eq!(ScriptFont::from_index(9), ScriptFont::Dancing);
    }
}
