//! The reader's logic with no UI types: page themes, text selection on PdfCraft's text layer,
//! markup quadrilaterals for highlights, the outline as visible rows, and the annotation list.
//!
//! Points here are in a page's view space (points, y down, after `/Rotate`), the space
//! PdfCraft's text layer and the viewer use; [`crate::viewer::view_to_pdf`] maps them to PDF
//! user space.

use std::collections::HashSet;
use std::ops::Range;

use sheaf_core::pdfcraft::render::{self, PageText};

use crate::viewer::{view_to_pdf, PageBox};

/// How pages are coloured (View ▸ Theme in PDF Expert). The app's own chrome follows the app theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageTheme {
    #[default]
    Day,
    /// Pages inverted: white paper becomes near-black, black text near-white. Pictures invert
    /// too, as in PDF Expert's Night theme.
    Night,
    /// Warm paper, brown ink.
    Sepia,
}

impl PageTheme {
    pub fn from_index(i: i32) -> Self {
        match i {
            1 => PageTheme::Night,
            2 => PageTheme::Sepia,
            _ => PageTheme::Day,
        }
    }
    pub fn index(self) -> i32 {
        match self {
            PageTheme::Day => 0,
            PageTheme::Night => 1,
            PageTheme::Sepia => 2,
        }
    }
}

/// Paper and ink of the Night and Sepia themes (RGB). White maps to paper, black to ink, and
/// every grey in between along the line joining them; colours keep their hue relative to it.
const NIGHT_PAPER: [f32; 3] = [30.0, 30.0, 32.0];
const NIGHT_INK: [f32; 3] = [222.0, 222.0, 220.0];
const SEPIA_PAPER: [f32; 3] = [244.0, 236.0, 216.0];
const SEPIA_INK: [f32; 3] = [91.0, 70.0, 54.0];

/// Recolours an RGBA render in place for `theme`.
///
/// Night inverts the lightness only (HSL: hue and saturation kept): each pixel's lightness
/// `l = (max + min) / 2` goes to the other end of the paper–ink line and its offset from that grey
/// (`c - l`) is kept, so a yellow highlight stays yellow and a teal heading stays teal (a plain
/// inversion turns them blue and red). Sepia maps the grey scale onto warm paper and brown ink.
pub fn tint(rgba: &mut [u8], theme: PageTheme) {
    let (paper, ink) = match theme {
        PageTheme::Day => return,
        PageTheme::Night => (NIGHT_PAPER, NIGHT_INK),
        PageTheme::Sepia => (SEPIA_PAPER, SEPIA_INK),
    };
    // Per channel, the grey for each lightness: 255 (white) → paper, 0 (black) → ink. Night's paper
    // is dark and its ink light, which is the inversion.
    let mut grey = [[0i32; 256]; 3];
    for (c, table) in grey.iter_mut().enumerate() {
        for (y, out) in table.iter_mut().enumerate() {
            let t = y as f32 / 255.0;
            *out = (ink[c] + (paper[c] - ink[c]) * t).round() as i32;
        }
    }
    for px in rgba.as_chunks_mut::<4>().0 {
        let (r, g, b) = (px[0] as i32, px[1] as i32, px[2] as i32);
        let l = (r.max(g).max(b) + r.min(g).min(b)) / 2;
        let off = [r - l, g - l, b - l];
        for c in 0..3 {
            px[c] = (grey[c][l as usize] + off[c]).clamp(0, 255) as u8;
        }
    }
}

/// The background shown around pages for `theme` (RGB), so the gaps match the paper.
pub fn backdrop(theme: PageTheme) -> Option<[u8; 3]> {
    match theme {
        PageTheme::Day => None,
        PageTheme::Night => Some([18, 18, 20]),
        PageTheme::Sepia => Some([226, 216, 192]),
    }
}

/// The glyphs a drag from `a` to `b` selects, in reading order (`a` and `b` in view points).
/// `None` when the page has no text.
pub fn selection(text: &PageText, a: (f32, f32), b: (f32, f32)) -> Option<Range<usize>> {
    let i = text.nearest(a.0, a.1)?;
    let j = text.nearest(b.0, b.1)?;
    let (lo, hi) = if i <= j { (i, j) } else { (j, i) };
    Some(lo..hi + 1)
}

/// The word under a double-click.
pub fn word(text: &PageText, at: (f32, f32)) -> Option<Range<usize>> {
    let i = text.nearest(at.0, at.1)?;
    let (a, b) = text.word_at(i)?;
    Some(a..b + 1)
}

/// The text of a selection, lines joined by a newline and words by a space.
pub fn selected_text(text: &PageText, r: Range<usize>) -> String {
    let mut out = String::new();
    for i in r.clone() {
        let Some(g) = text.glyphs.get(i) else { break };
        if i > r.start {
            if text.line_of.get(i) != text.line_of.get(i - 1) {
                out.push('\n');
            } else if text.space_before.get(i).copied().unwrap_or(false) {
                out.push(' ');
            }
        }
        out.push_str(&g.text);
    }
    out
}

/// Markup quadrilaterals for `rects` (view points, one per line): top-left, top-right,
/// bottom-left, bottom-right, in PDF user space, the order Acrobat writes.
pub fn quads(page: &PageBox, rects: &[[f32; 4]]) -> Vec<[f64; 8]> {
    let size = page.view_size();
    rects
        .iter()
        .map(|r| {
            let p = |x: f32, y: f32| view_to_pdf(page, size, x as f64, y as f64);
            let (tl, tr, bl, br) = (p(r[0], r[1]), p(r[2], r[1]), p(r[0], r[3]), p(r[2], r[3]));
            [tl.0, tl.1, tr.0, tr.1, bl.0, bl.1, br.0, br.1]
        })
        .collect()
}

/// One visible row of the outline panel.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineRow {
    /// Path of child indices from the top level: the row's identity across expand/collapse.
    pub path: Vec<usize>,
    pub title: String,
    pub depth: usize,
    pub page: Option<usize>,
    pub has_children: bool,
    pub expanded: bool,
}

/// The rows the outline panel shows: every top-level item, and the children of each expanded
/// one. `toggled` holds the paths the user flipped from the file's own open/closed state.
pub fn outline_rows(items: &[render::OutlineItem], toggled: &HashSet<Vec<usize>>) -> Vec<OutlineRow> {
    fn walk(items: &[render::OutlineItem], depth: usize, prefix: &mut Vec<usize>, toggled: &HashSet<Vec<usize>>, out: &mut Vec<OutlineRow>) {
        for (i, it) in items.iter().enumerate() {
            prefix.push(i);
            let has_children = !it.children.is_empty();
            let expanded = has_children && (it.open != toggled.contains(prefix));
            out.push(OutlineRow { path: prefix.clone(), title: clean_title(&it.title), depth, page: it.page, has_children, expanded });
            if expanded {
                walk(&it.children, depth + 1, prefix, toggled, out);
            }
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    walk(items, 0, &mut Vec::new(), toggled, &mut out);
    out
}

/// Outline titles often carry line breaks and runs of spaces from the authoring tool.
fn clean_title(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { "Untitled".into() } else { t }
}

/// The outline row of the section `page` is in: the last row, in order, starting at or before it.
pub fn current_section(rows: &[OutlineRow], page: usize) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (i, r) in rows.iter().enumerate() {
        if let Some(p) = r.page
            && p <= page
            && best.is_none_or(|(bp, _)| p >= bp)
        {
            best = Some((p, i));
        }
    }
    best.map(|(_, i)| i)
}

/// What an annotation is, as the annotations panel names it.
pub fn kind_label(subtype: &str) -> &'static str {
    match subtype {
        "Highlight" => "Highlight",
        "Underline" => "Underline",
        "StrikeOut" => "Strikethrough",
        "Squiggly" => "Squiggly",
        "Text" => "Note",
        "FreeText" => "Text box",
        "Ink" => "Drawing",
        "Square" => "Rectangle",
        "Circle" => "Oval",
        "Line" => "Line",
        "Polygon" | "PolyLine" => "Shape",
        "Stamp" => "Stamp",
        _ => "Comment",
    }
}

/// Annotation subtypes the panel lists (links, widgets and pop-ups are not comments).
pub fn listed(subtype: &str) -> bool {
    !matches!(subtype, "Link" | "Widget" | "Popup")
}

/// The text a markup covers, for the annotations panel: the glyphs whose centre is inside one of
/// its quads (view points).
pub fn text_under(text: &PageText, rects: &[[f32; 4]]) -> String {
    let mut picked: Vec<usize> = rects.iter().flat_map(|r| text.glyphs_in(*r)).collect();
    picked.sort_unstable();
    picked.dedup();
    let mut out = String::new();
    for (k, &i) in picked.iter().enumerate() {
        if k > 0 {
            let prev = picked[k - 1];
            if text.line_of.get(i) != text.line_of.get(prev) || text.space_before.get(i).copied().unwrap_or(false) || i != prev + 1 {
                out.push(' ');
            }
        }
        out.push_str(&text.glyphs[i].text);
    }
    out
}

/// Highlight and pen colours (RGB 0–1): PDF Expert's five, in its order.
pub const COLOURS: [[f64; 3]; 5] = [
    [1.0, 0.86, 0.25], // yellow
    [0.42, 0.85, 0.42], // green
    [0.38, 0.72, 1.0], // blue
    [1.0, 0.45, 0.6],  // pink
    [0.75, 0.5, 1.0],  // purple
];

/// Ink colours for pen, underline, strikethrough and text: darker versions read better as strokes.
pub const INK: [[f64; 3]; 5] = [
    [0.85, 0.2, 0.15], // red
    [0.1, 0.1, 0.12],  // black
    [0.1, 0.4, 0.9],   // blue
    [0.1, 0.6, 0.3],   // green
    [0.6, 0.25, 0.85], // purple
];

/// Opacities offered for shapes and drawings.
pub const OPACITIES: [f64; 4] = [1.0, 0.75, 0.5, 0.25];

/// Shapes tool: 0 rectangle, 1 oval, 2 line, 3 arrow. The outline shown while a shape is dragged
/// from `a` to `b` (view points), as one path.
pub fn shape_outline(kind: i32, a: (f64, f64), b: (f64, f64)) -> Vec<(f64, f64)> {
    let (x0, y0, x1, y1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
    match kind {
        0 => vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)],
        1 => {
            let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0);
            (0..=48).map(|i| i as f64 / 48.0 * std::f64::consts::TAU).map(|t| (cx + rx * t.cos(), cy + ry * t.sin())).collect()
        }
        2 => vec![a, b],
        _ => {
            let [h1, h2] = arrow_head(a, b, 10.0);
            vec![a, b, h1, b, h2]
        }
    }
}

/// The two ends of an open arrowhead at `b`, `len` long, 30° either side of the line from `a`.
pub fn arrow_head(a: (f64, f64), b: (f64, f64), len: f64) -> [(f64, f64); 2] {
    let angle = (b.1 - a.1).atan2(b.0 - a.0);
    let side = |s: f64| (b.0 - len * (angle + s).cos(), b.1 - len * (angle + s).sin());
    [side(-std::f64::consts::FRAC_PI_6), side(std::f64::consts::FRAC_PI_6)]
}

/// A shape's fill: its stroke colour mixed with 75 % white.
pub fn fill_tint(c: [f64; 3]) -> [f64; 3] {
    c.map(|v| v + (1.0 - v) * 0.75)
}

#[cfg(test)]
mod tests {
    use super::*;
    use render::{DestView, OutlineItem, TextGlyph};

    fn glyph(c: &str, x: f32, y: f32) -> TextGlyph {
        let rect = [x, y, x + 6.0, y + 10.0];
        TextGlyph { text: c.into(), rect, quad: [[x, y + 10.0], [x + 6.0, y + 10.0], [x + 6.0, y], [x, y]], direction: [1.0, 0.0] }
    }

    /// "ab cd" on line 0, "ef" on line 1.
    fn sample() -> PageText {
        PageText {
            glyphs: vec![glyph("a", 10.0, 10.0), glyph("b", 16.0, 10.0), glyph("c", 28.0, 10.0), glyph("d", 34.0, 10.0), glyph("e", 10.0, 30.0), glyph("f", 16.0, 30.0)],
            line_of: vec![0, 0, 0, 0, 1, 1],
            space_before: vec![false, false, true, false, false, false],
        }
    }

    #[test]
    fn a_drag_selects_in_reading_order_whichever_way_it_goes() {
        let t = sample();
        assert_eq!(selection(&t, (11.0, 12.0), (35.0, 12.0)), Some(0..4));
        assert_eq!(selection(&t, (17.0, 32.0), (17.0, 12.0)), Some(1..6));
        assert_eq!(selected_text(&t, 0..6), "ab cd\nef");
        assert_eq!(word(&t, (29.0, 12.0)), Some(2..4));
        assert_eq!(selection(&PageText { glyphs: vec![], line_of: vec![], space_before: vec![] }, (0.0, 0.0), (1.0, 1.0)), None);
    }

    #[test]
    fn quads_are_top_left_top_right_bottom_left_bottom_right_in_user_space() {
        let page = PageBox { crop: [0.0, 0.0, 600.0, 800.0], rotation: 0, unit: 1.0 };
        let q = quads(&page, &[[10.0, 20.0, 50.0, 30.0]]);
        assert_eq!(q, vec![[10.0, 780.0, 50.0, 780.0, 10.0, 770.0, 50.0, 770.0]]);
    }

    #[test]
    fn themes_map_white_to_paper_and_black_to_ink() {
        let mut px = [255, 255, 255, 255, 0, 0, 0, 255];
        tint(&mut px, PageTheme::Night);
        assert_eq!(&px[..3], &[30, 30, 32]);
        assert_eq!(&px[4..7], &[222, 222, 220]);
        let mut px = [255, 255, 255, 255, 0, 0, 0, 255];
        tint(&mut px, PageTheme::Sepia);
        assert_eq!(&px[..3], &[244, 236, 216]);
        assert_eq!(&px[4..7], &[91, 70, 54]);
        let mut px = [12, 34, 56, 255];
        tint(&mut px, PageTheme::Day);
        assert_eq!(px, [12, 34, 56, 255]);
        // A yellow highlight stays yellow at night: red and green above blue.
        let mut px = [255, 219, 64, 255];
        tint(&mut px, PageTheme::Night);
        assert!(px[0] > px[2] + 100 && px[1] > px[2] + 100, "{px:?}");
    }

    fn item(title: &str, page: usize, open: bool, children: Vec<OutlineItem>) -> OutlineItem {
        OutlineItem { title: title.into(), page: Some(page), view: DestView::Top, children, open }
    }

    #[test]
    fn outline_rows_follow_the_files_open_state_and_the_users_toggles() {
        let items = vec![item("1  Intro\n", 0, false, vec![item("1.1", 1, false, vec![])]), item("2 Methods", 4, true, vec![item("2.1", 5, false, vec![])])];
        let rows = outline_rows(&items, &HashSet::new());
        assert_eq!(rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["1 Intro", "2 Methods", "2.1"]);
        assert!(rows[0].has_children && !rows[0].expanded && rows[1].expanded);
        let toggled: HashSet<Vec<usize>> = [vec![0], vec![1]].into_iter().collect();
        let rows = outline_rows(&items, &toggled);
        assert_eq!(rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["1 Intro", "1.1", "2 Methods"]);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(current_section(&rows, 3), Some(1));
        assert_eq!(current_section(&rows, 9), Some(2));
    }

    #[test]
    fn the_text_under_a_highlight_is_read_back() {
        let t = sample();
        assert_eq!(text_under(&t, &[[9.0, 9.0, 41.0, 21.0]]), "ab cd");
        assert_eq!(kind_label("StrikeOut"), "Strikethrough");
        assert!(!listed("Link") && listed("Highlight"));
    }

    #[test]
    fn shape_outlines() {
        let r = shape_outline(0, (30.0, 40.0), (10.0, 20.0));
        assert_eq!(r, vec![(10.0, 20.0), (30.0, 20.0), (30.0, 40.0), (10.0, 40.0), (10.0, 20.0)]);
        let o = shape_outline(1, (0.0, 0.0), (20.0, 10.0));
        assert!(o.iter().all(|p| ((p.0 - 10.0) / 10.0).powi(2) + ((p.1 - 5.0) / 5.0).powi(2) < 1.0001));
        assert_eq!(shape_outline(2, (1.0, 2.0), (3.0, 4.0)), vec![(1.0, 2.0), (3.0, 4.0)]);
        // An arrow pointing right: its head's ends are behind the tip, one above, one below.
        let [h1, h2] = arrow_head((0.0, 0.0), (100.0, 0.0), 10.0);
        assert!(h1.0 < 100.0 && h2.0 < 100.0 && (h1.1 + h2.1).abs() < 1e-9 && h1.1 != 0.0);
        assert!(((100.0 - h1.0).hypot(h1.1) - 10.0).abs() < 1e-9);
        assert_eq!(fill_tint([0.0, 1.0, 0.2]), [0.75, 1.0, 0.8]);
    }

}
