//! The reader's logic with no UI types: page themes, text selection on PdfCraft's text layer,
//! markup quadrilaterals for highlights, the outline as visible rows, and the annotation list.
//!
//! Points here are in a page's view space (points, y down, after `/Rotate`), the space
//! PdfCraft's text layer and the viewer use; [`crate::viewer::view_to_pdf`] maps them to PDF
//! user space.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

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

/// A drag over text in the Redact tool, as Acrobat marks text for redaction: the glyphs from `a`
/// to `b` (view points of page `page`) as one quadrilateral per line, with their text (redacted
/// from the metadata and bookmarks too). `None` when the drag does not start on text (it marks an
/// area then).
pub fn redact_selection(text: &PageText, page_box: &PageBox, page: usize, a: (f32, f32), b: (f32, f32)) -> Option<sheaf_core::redact::Mark> {
    // On text: inside a glyph's box, give or take 2 points.
    let on = text.glyphs.iter().any(|g| a.0 >= g.rect[0] - 2.0 && a.0 <= g.rect[2] + 2.0 && a.1 >= g.rect[1] - 2.0 && a.1 <= g.rect[3] + 2.0);
    if !on {
        return None;
    }
    // Between letters, as a caret: a letter is in once the pointer is past its middle (taking the
    // nearest whole, a drag stopping on the last letter of a word left it readable).
    let (i, j) = (caret(text, a)?, caret(text, b)?);
    let range = i.min(j)..i.max(j);
    if range.is_empty() {
        return None;
    }
    redact_range(text, page_box, page, range)
}

/// The place between two glyphs nearest `p`: before the nearest glyph, or after it when `p` is
/// past its middle (along its writing direction).
fn caret(text: &PageText, p: (f32, f32)) -> Option<usize> {
    let i = text.nearest(p.0, p.1)?;
    let g = &text.glyphs[i];
    let (cx, cy) = ((g.rect[0] + g.rect[2]) / 2.0, (g.rect[1] + g.rect[3]) / 2.0);
    let along = (p.0 - cx) * g.direction[0] + (p.1 - cy) * g.direction[1];
    Some(if along > 0.0 { i + 1 } else { i })
}

/// The words of a line, for a triple click: the glyphs of the line holding the glyph nearest `at`.
pub fn line(text: &PageText, at: (f32, f32)) -> Option<Range<usize>> {
    let i = text.nearest(at.0, at.1)?;
    let l = text.line_of.get(i)?;
    let start = (0..=i).rev().take_while(|&k| text.line_of.get(k) == Some(l)).last()?;
    let end = (i..text.glyphs.len()).take_while(|&k| text.line_of.get(k) == Some(l)).last()?;
    Some(start..end + 1)
}

/// A redaction mark for glyphs `range` of page `page` (a word or line clicked).
pub fn redact_range(text: &PageText, page_box: &PageBox, page: usize, range: Range<usize>) -> Option<sheaf_core::redact::Mark> {
    let words = selected_text(text, range.clone());
    if words.trim().is_empty() {
        return None;
    }
    Some(sheaf_core::redact::Mark {
        page,
        quads: quads(page_box, &text.line_rects(range)),
        text: words.split_whitespace().collect::<Vec<_>>().join(" "),
        match_case: true,
        whole_word: false,
        whole_page: false,
    })
}

/// Edit tool, several objects: the smallest of `rects` (user space) holding point `p`, within 2
/// points (an image over a paragraph is the image).
pub fn object_at(rects: &[[f64; 4]], p: (f64, f64)) -> Option<usize> {
    rects
        .iter()
        .enumerate()
        .filter(|(_, r)| p.0 >= r[0] - 2.0 && p.0 <= r[2] + 2.0 && p.1 >= r[1] - 2.0 && p.1 <= r[3] + 2.0)
        .min_by(|a, b| ((a.1[2] - a.1[0]) * (a.1[3] - a.1[1])).total_cmp(&((b.1[2] - b.1[0]) * (b.1[3] - b.1[1]))))
        .map(|(i, _)| i)
}

/// The objects a marquee (user space) touches, as Acrobat's selection rectangle picks them.
pub fn objects_in(rects: &[[f64; 4]], m: [f64; 4]) -> Vec<usize> {
    rects.iter().enumerate().filter(|(_, r)| r[0] < m[2] && r[2] > m[0] && r[1] < m[3] && r[3] > m[1]).map(|(i, _)| i).collect()
}

/// After a move, the objects now where the picked ones went: for each of `moved` (their boxes
/// moved by the offset), the one of `rects` (the page read again) overlapping it most, if by half.
pub fn picked_again(moved: &[[f64; 4]], rects: &[[f64; 4]]) -> Vec<usize> {
    let overlap = |a: &[f64; 4], b: &[f64; 4]| {
        let (w, h) = (a[2].min(b[2]) - a[0].max(b[0]), a[3].min(b[3]) - a[1].max(b[1]));
        if w <= 0.0 || h <= 0.0 { 0.0 } else { w * h / ((a[2] - a[0]) * (a[3] - a[1])).max((b[2] - b[0]) * (b[3] - b[1])).max(1e-9) }
    };
    moved
        .iter()
        .filter_map(|m| rects.iter().enumerate().map(|(i, r)| (i, overlap(m, r))).filter(|(_, o)| *o >= 0.5).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i))
        .collect()
}

/// The height of paragraph `drawn` (its rows as drawn, one per line, in box `rect`) rewrapped to
/// width `w`: its line height and mean character width kept, its words wrapped greedily.
pub fn rewrapped_height(drawn: &str, rect: [f64; 4], w: f64) -> f64 {
    let rows: Vec<&str> = drawn.lines().filter(|l| !l.trim().is_empty()).collect();
    let n = rows.len().max(1) as f64;
    let line_h = (rect[3] - rect[1]).max(1.0) / n;
    let longest = rows.iter().map(|r| r.chars().count()).max().unwrap_or(1).max(1) as f64;
    let char_w = ((rect[2] - rect[0]).max(1.0) / longest).max(0.1);
    let per_line = (w / char_w).floor().max(1.0) as usize;
    let mut lines = 1;
    let mut used = 0usize;
    for word in drawn.split_whitespace() {
        let len = word.chars().count();
        if used > 0 && used + 1 + len > per_line {
            lines += 1;
            used = len;
        } else {
            used += if used > 0 { 1 } else { 0 } + len;
        }
    }
    lines as f64 * line_h
}

/// A link's target as the Link editor shows it to change: its page number (1-based) or address
/// (what `link_action` reads back).
pub fn link_text(t: &render::LinkTarget) -> String {
    match t {
        render::LinkTarget::Page(p, _) => (p + 1).to_string(),
        render::LinkTarget::Uri(u) => u.clone(),
        _ => String::new(),
    }
}

/// Where a link typed in the Link editor goes: a page number of the document (1 to `pages`), or
/// a web address ("example.org" becomes "https://example.org", an e-mail address a `mailto:`).
/// `None` when it is neither (a page past the end, a blank).
pub fn link_action(text: &str, pages: usize) -> Option<sheaf_core::pdfcraft::engine::LinkAction> {
    use sheaf_core::pdfcraft::engine::LinkAction;
    let t = text.trim();
    // "4", "p4", "p. 4".
    if let Ok(n) = t.trim_start_matches(['p', 'P']).trim_start_matches(['.', ' ']).parse::<usize>() {
        return (1..=pages).contains(&n).then(|| LinkAction::Page(n - 1));
    }
    if t.is_empty() || t.contains(char::is_whitespace) {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    if ["http://", "https://", "mailto:", "ftp://", "file://"].iter().any(|s| lower.starts_with(s)) {
        return Some(LinkAction::Uri(t.to_string()));
    }
    if t.contains('@') && !t.contains('/') {
        return Some(LinkAction::Uri(format!("mailto:{t}")));
    }
    t.contains('.').then(|| LinkAction::Uri(format!("https://{t}")))
}

/// One visible row of the outline panel.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineRow {
    /// Path of child indices from the top level: the row's identity across expand/collapse.
    pub path: Vec<usize>,
    pub title: String,
    pub depth: usize,
    pub page: Option<usize>,
    /// Where on `page` it goes.
    pub view: render::DestView,
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
            out.push(OutlineRow { path: prefix.clone(), title: clean_title(&it.title), depth, page: it.page, view: it.view, has_children, expanded });
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

/// The part of a page a destination points at, brought into view by a jump there (None: the
/// page's top): an /XYZ or /FitH top edge, a /FitR rectangle.
pub fn dest_focus(view: &render::DestView, crop: [f64; 4]) -> Option<[f64; 4]> {
    match *view {
        render::DestView::Xyz { top: Some(t), .. } | render::DestView::FitH { top: Some(t) } => Some([crop[0], t as f64 - 1.0, crop[2], t as f64]),
        render::DestView::FitR { rect } => Some(rect.map(f64::from)),
        _ => None,
    }
}

/// The XFDF element of the comment named `name` in an export of the document's comments (one
/// element a line, PdfCraft's writer), its replies left out.
pub fn xfdf_element(xfdf: &str, name: &str) -> Option<String> {
    let attr = format!(" name=\"{}\"", xml_escape(name));
    xfdf.lines().find(|l| l.starts_with('<') && l.contains(&attr) && !l.contains(" inreplyto=\"")).map(str::to_string)
}

/// A comment without a name (/NM) in the file, found by its page and box (within half a point):
/// copied as [`xfdf_element`] copies a named one (Sol, 25/10: unnamed comments were refused).
pub fn xfdf_element_at(xfdf: &str, page: usize, rect: [f64; 4]) -> Option<String> {
    let attr = |l: &str, key: &str| l.split(&format!(" {key}=\"")).nth(1).and_then(|v| v.split('"').next()).map(str::to_string);
    xfdf.lines().filter(|l| l.starts_with('<') && !l.contains(" inreplyto=\"") && attr(l, "name").is_none()).find(|l| {
        let r: Vec<f64> = attr(l, "rect").map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_default();
        attr(l, "page").and_then(|p| p.parse::<usize>().ok()) == Some(page) && r.len() == 4 && (0..4).all(|k| (r[k] - rect[k]).abs() < 0.5)
    })
    .map(str::to_string)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A copied comment's XFDF element made ready to paste: on `page` (0-based), moved by (dx, dy)
/// points (PDF space, y up), under the fresh `name` (an import replaces a comment of the same
/// name); wrapped as a whole XFDF file.
pub fn paste_xfdf(element: &str, page: usize, (dx, dy): (f64, f64), name: &str) -> String {
    // Each "x,y,x,y…" attribute that holds points, and the "x,y;x,y" contents of points.
    let shift = |v: &str, sep: char| -> String {
        v.split(sep)
            .map(|pt| {
                pt.split(',')
                    .enumerate()
                    .map(|(i, n)| n.trim().parse::<f64>().map_or_else(|_| n.to_string(), |x| format!("{}", x + if i % 2 == 0 { dx } else { dy })))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join(&sep.to_string())
    };
    let mut out = String::new();
    let mut rest = element;
    // Attributes: page, name, and the point lists.
    while let Some(eq) = rest.find("=\"") {
        let key_start = rest[..eq].rfind(' ').map_or(0, |k| k + 1);
        let key = &rest[key_start..eq];
        let Some(end) = rest[eq + 2..].find('"') else { break };
        let value = &rest[eq + 2..eq + 2 + end];
        out.push_str(&rest[..eq + 2]);
        out.push_str(&match key {
            "page" => page.to_string(),
            "name" => xml_escape(name),
            "rect" | "coords" | "start" | "end" | "callout" => shift(value, ';'),
            _ => value.to_string(),
        });
        out.push('"');
        rest = &rest[eq + 3 + end..];
    }
    out.push_str(rest);
    // Ink strokes and polygon points.
    for tag in ["gesture", "vertices"] {
        let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
        let mut s = String::new();
        let mut r = out.as_str();
        while let Some(a) = r.find(&open) {
            let Some(b) = r[a..].find(&close) else { break };
            s.push_str(&r[..a + open.len()]);
            s.push_str(&shift(&r[a + open.len()..a + b], ';'));
            r = &r[a + b..];
        }
        s.push_str(r);
        out = s;
    }
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n<annots>\n{}\n</annots>\n</xfdf>\n", out.trim_end())
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

/// The text layer of pages `0..n` of `bytes`: `known` pages as they are, the others read on every
/// core (one PdfCraft renderer each, at most four). `found(page, text)` gets each page in no
/// particular order and returns false to stop; `stale()` true stops reading too.
pub fn read_texts(
    bytes: &Arc<Vec<u8>>,
    config: &render::RenderConfig,
    n: usize,
    known: &std::collections::HashMap<usize, Arc<PageText>>,
    stale: &(dyn Fn() -> bool + Sync),
    found: impl Fn(usize, Option<Arc<PageText>>) -> bool + Sync,
) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let readers = std::thread::available_parallelism().map_or(2, |k| k.get()).clamp(1, 4);
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..readers {
            let (next, found) = (&next, &found);
            scope.spawn(move || {
                let mut rr: Option<render::PageRenderer> = None;
                loop {
                    let page = next.fetch_add(1, Ordering::Relaxed);
                    if page >= n || stale() {
                        return;
                    }
                    let text = match known.get(&page) {
                        Some(t) => Some(t.clone()),
                        None => {
                            let rr = rr.get_or_insert_with(|| render::PageRenderer::new(bytes.clone(), config.clone()));
                            rr.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() }).text
                        }
                    };
                    if !found(page, text) {
                        return;
                    }
                }
            });
        }
    });
}

/// The Notes panel's filter: 0 everything, 1 text markup, 2 notes and text boxes, 3 drawings and
/// shapes, 4 stamps and signatures. Whether a comment of `subtype` (a signature: `fill_sign`) shows.
/// Review statuses the Notes filter offers (Acrobat's), then "" for none.
const STATUSES: [&str; 5] = ["Accepted", "Rejected", "Cancelled", "Completed", ""];

pub fn notes_filter(filter: i32, subtype: &str, fill_sign: bool, status: &str, checked: bool) -> bool {
    // 10: checked with Acrobat's check mark.
    if filter == 10 {
        return checked;
    }
    // 5–9: by review status, as Acrobat's comment filter (Accepted … Completed, then none).
    if let Some(want) = usize::try_from(filter - 5).ok().and_then(|k| STATUSES.get(k)) {
        return status.eq_ignore_ascii_case(want);
    }
    let group = match subtype {
        _ if fill_sign => 4,
        "Highlight" | "Underline" | "StrikeOut" | "Squiggly" => 1,
        "Text" | "FreeText" => 2,
        "Stamp" => 4,
        _ => 3,
    };
    filter == 0 || filter == group
}

/// A zoom typed by the user ("150", "150%", " 75 % "), as a percentage of actual size, held to
/// the viewer's range (10 … 1600 %).
pub fn parse_zoom(text: &str) -> Option<u32> {
    let n: f64 = text.trim().trim_end_matches('%').trim().replace(',', ".").parse().ok()?;
    (n.is_finite() && n > 0.0).then(|| n.round().clamp(10.0, 1600.0) as u32)
}

/// Signature inks (PDF Expert's black, blue, dark blue) and pen widths in points.
pub const SIGNATURE_INKS: [[f64; 3]; 3] = [[0.1, 0.1, 0.12], [0.1, 0.32, 0.85], [0.05, 0.15, 0.45]];
pub const SIGNATURE_WIDTHS: [f64; 3] = [1.0, 1.8, 2.8];

/// Opacities offered for shapes and drawings.
pub const OPACITIES: [f64; 4] = [1.0, 0.75, 0.5, 0.25];

/// A `file:` link's target: the path decoded (%20…), relative to the document's folder when it
/// is not absolute. None for anything else.
pub fn file_link_path(uri: &str, doc_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let rest = uri.strip_prefix("file://").or_else(|| uri.strip_prefix("file:"))?;
    // "file://host/path": only the local host is ours.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?, 16) {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let path = std::path::PathBuf::from(String::from_utf8(out).ok()?);
    (!path.as_os_str().is_empty()).then(|| if path.is_absolute() { path } else { doc_dir.join(path) })
}

/// Shapes tool with ⇧ held at the press (PDF Expert, Acrobat): a rectangle becomes a square and an
/// oval a circle (the longer side wins), a line or arrow snaps to the nearest 45°.
pub fn constrained(kind: i32, a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if kind <= 1 {
        let side = dx.abs().max(dy.abs());
        return (a.0 + side.copysign(dx), a.1 + side.copysign(dy));
    }
    let len = dx.hypot(dy);
    let angle = (dy.atan2(dx) / std::f64::consts::FRAC_PI_4).round() * std::f64::consts::FRAC_PI_4;
    (a.0 + len * angle.cos(), a.1 + len * angle.sin())
}

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

/// The Arrow's heads the options bar cycles through, with their names (Acrobat's Line Ends).
pub const ARROW_HEADS: [(sheaf_core::pdfcraft::engine::LineEnding, &str); 5] = {
    use sheaf_core::pdfcraft::engine::LineEnding as E;
    [(E::OpenArrow, "Open arrow"), (E::ClosedArrow, "Closed arrow"), (E::Circle, "Circle"), (E::Square, "Square"), (E::Diamond, "Diamond")]
};

/// Shape tool kinds clicked out as closed outlines: polygon (4), cloud (5), area (9).
pub fn closed_shape(kind: i32) -> bool {
    matches!(kind, 4 | 5 | 9)
}

/// The Measuring tools' scales (Acrobat's Scale Ratio): paper to real, and the real unit.
pub const MEASURE_SCALES: [(f64, &str); 8] = [(1.0, "cm"), (1.0, "in"), (50.0, "m"), (100.0, "m"), (200.0, "m"), (500.0, "m"), (1000.0, "m"), (25000.0, "km")];

/// Real units per user-space unit at scale `k` of [`MEASURE_SCALES`] on a page whose /UserUnit
/// is `user_unit` (points per user-space unit; Sol, 25/10: a page at 2 read half its lengths).
pub fn measure_factor(k: usize, user_unit: f64) -> (f64, &'static str) {
    let (ratio, unit) = MEASURE_SCALES[k.min(MEASURE_SCALES.len() - 1)];
    let cm_per_unit = match unit {
        "in" => 2.54,
        "m" => 100.0,
        "km" => 100_000.0,
        _ => 1.0,
    };
    (2.54 / 72.0 * user_unit * ratio / cm_per_unit, unit)
}

/// A measure's label: a distance or perimeter along the points (kind 7, 8), an area inside them
/// (9), `per_unit` real `unit`s for each user-space unit ([`measure_factor`]).
pub fn measure_label(kind: i32, pts: &[[f64; 2]], (per_unit, unit): (f64, &str)) -> String {
    match kind {
        9 => {
            let n = pts.len();
            let twice: f64 = (0..n).map(|i| pts[i][0] * pts[(i + 1) % n][1] - pts[(i + 1) % n][0] * pts[i][1]).sum();
            format!("{:.2} {unit}²", (twice / 2.0).abs() * per_unit * per_unit)
        }
        _ => format!("{:.2} {unit}", pts.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>() * per_unit),
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

    #[test]
    fn file_links_are_decoded_and_taken_from_the_document_folder() {
        let d = std::path::Path::new("/docs");
        assert_eq!(file_link_path("file:///home/a/My%20file.pdf", d), Some("/home/a/My file.pdf".into()));
        assert_eq!(file_link_path("file://localhost/tmp/x.pdf", d), Some("/tmp/x.pdf".into()));
        assert_eq!(file_link_path("file:annex.pdf", d), Some("/docs/annex.pdf".into()));
        assert_eq!(file_link_path("https://x.org", d), None);
    }

    #[test]
    fn shift_makes_squares_circles_and_45_degree_lines() {
        assert_eq!(constrained(0, (10.0, 10.0), (40.0, 20.0)), (40.0, 40.0));
        assert_eq!(constrained(1, (10.0, 10.0), (0.0, 50.0)), (-30.0, 50.0));
        let (x, y) = constrained(2, (0.0, 0.0), (100.0, 8.0));
        assert!((y - 0.0).abs() < 1e-9 && (x - 100.0_f64.hypot(8.0)).abs() < 1e-9);
        let (x, y) = constrained(3, (0.0, 0.0), (50.0, 45.0));
        assert!((x - y).abs() < 1e-9);
    }
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
    fn a_drag_over_text_marks_its_lines_for_redaction() {
        let t = sample();
        let page = PageBox { crop: [0.0, 0.0, 600.0, 800.0], rotation: 0, unit: 1.0 };
        let m = redact_selection(&t, &page, 3, (17.0, 12.0), (17.0, 32.0)).expect("starts on text");
        assert_eq!((m.page, m.text.as_str(), m.quads.len()), (3, "b cd e", 2), "one quad per line");
        // A letter is in once the pointer is past its middle: "d" (34–40) taken at 38, not at 36.
        assert_eq!(redact_selection(&t, &page, 0, (9.0, 12.0), (38.0, 12.0)).map(|m| m.text), Some("ab cd".into()));
        assert_eq!(redact_selection(&t, &page, 0, (9.0, 12.0), (36.0, 12.0)).map(|m| m.text), Some("ab c".into()));
        // A triple click takes the line, a double click the word.
        assert_eq!(line(&t, (17.0, 12.0)), Some(0..4));
        assert_eq!(redact_range(&t, &page, 0, word(&t, (29.0, 12.0)).unwrap()).map(|m| m.text), Some("cd".into()));
        // Off the text: an area, not a text mark.
        assert!(redact_selection(&t, &page, 3, (300.0, 300.0), (400.0, 400.0)).is_none());
    }

    #[test]
    fn a_paragraph_pulled_narrower_is_as_tall_as_its_new_lines() {
        // One row of 20 characters, 230 points wide (11.5 a character), 24 high; 120 wide: 10 a line.
        let h = rewrapped_height("Alpha paragraph here", [72.0, 700.0, 302.0, 724.0], 120.0);
        assert_eq!(h, 72.0, "Alpha / paragraph / here");
        assert_eq!(rewrapped_height("Alpha paragraph here", [72.0, 700.0, 302.0, 724.0], 400.0), 24.0);
    }

    #[test]
    fn objects_are_picked_by_point_by_marquee_and_found_again_after_a_move() {
        // A paragraph and an image inside it.
        let rects = [[100.0, 400.0, 500.0, 600.0], [200.0, 450.0, 260.0, 500.0], [100.0, 100.0, 200.0, 150.0]];
        assert_eq!(object_at(&rects, (230.0, 470.0)), Some(1), "the smaller, the image");
        assert_eq!(object_at(&rects, (120.0, 420.0)), Some(0));
        assert_eq!(object_at(&rects, (50.0, 50.0)), None);
        assert_eq!(objects_in(&rects, [90.0, 90.0, 210.0, 420.0]), [0, 2]);
        // Moved 10 right: found again where they went, in the page read again (another order).
        let after = [[110.0, 100.0, 210.0, 150.0], [110.0, 400.0, 510.0, 600.0]];
        assert_eq!(picked_again(&[[110.0, 400.0, 510.0, 600.0], [110.0, 100.0, 210.0, 150.0]], &after), [1, 0]);
    }

    #[test]
    fn a_link_goes_to_a_page_number_or_a_web_address() {
        use sheaf_core::pdfcraft::engine::LinkAction;
        assert_eq!(link_action("3", 10), Some(LinkAction::Page(2)));
        assert_eq!(link_action(" p. 4 ", 10), Some(LinkAction::Page(3)));
        assert_eq!(link_action("11", 10), None, "past the last page");
        assert_eq!(link_action("0", 10), None);
        assert_eq!(link_action("example.org/x", 10), Some(LinkAction::Uri("https://example.org/x".into())));
        assert_eq!(link_action("HTTP://a.b", 10), Some(LinkAction::Uri("HTTP://a.b".into())));
        assert_eq!(link_action("ada@example.com", 10), Some(LinkAction::Uri("mailto:ada@example.com".into())));
        assert_eq!(link_action("hello", 10), None);
        assert_eq!(link_action("  ", 10), None);
        // An existing link is shown as typed, and reads back as it was.
        assert_eq!(link_text(&render::LinkTarget::Page(1, render::DestView::Fit)), "2");
        assert_eq!(link_action(&link_text(&render::LinkTarget::Page(1, render::DestView::Fit)), 10), Some(LinkAction::Page(1)));
        assert_eq!(link_text(&render::LinkTarget::Uri("https://example.org".into())), "https://example.org");
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
    fn measures_read_in_true_size() {
        // One inch along, then a 72 × 72 pt square: 2.54 cm, 6.45 cm².
        let true_size = measure_factor(0, 1.0);
        assert_eq!(measure_label(7, &[[0.0, 0.0], [72.0, 0.0]], true_size), "2.54 cm");
        assert_eq!(measure_label(8, &[[0.0, 0.0], [72.0, 0.0], [72.0, 72.0]], true_size), "5.08 cm");
        assert_eq!(measure_label(9, &[[0.0, 0.0], [72.0, 0.0], [72.0, 72.0], [0.0, 72.0]], true_size), "6.45 cm²");
        // A page whose /UserUnit is 2: each unit is two points.
        assert_eq!(measure_label(7, &[[0.0, 0.0], [72.0, 0.0]], measure_factor(0, 2.0)), "5.08 cm");
        assert_eq!(measure_label(9, &[[0.0, 0.0], [72.0, 0.0], [72.0, 72.0], [0.0, 72.0]], measure_factor(0, 2.0)), "25.81 cm²");
        // A plan at 1:100 in metres: 1 inch of paper is 2.54 m; in inches, 1 inch is 1 inch.
        assert_eq!(measure_label(7, &[[0.0, 0.0], [72.0, 0.0]], measure_factor(3, 1.0)), "2.54 m");
        assert_eq!(measure_label(7, &[[0.0, 0.0], [72.0, 0.0]], measure_factor(1, 1.0)), "1.00 in");
        assert!(closed_shape(9) && closed_shape(5) && !closed_shape(6) && !closed_shape(7));
    }

    #[test]
    fn a_copied_comment_is_pasted_on_another_page_moved_and_renamed() {
        let x = "<?xml?>\n<xfdf><annots>\n<ink page=\"0\" rect=\"10,20,30,40\" name=\"a&amp;1\" color=\"#FF0000\"><inklist><gesture>10,20;30,40</gesture></inklist><popup page=\"0\" rect=\"100,100,200,150\" open=\"no\"/></ink>\n<text page=\"0\" rect=\"0,0,1,1\" name=\"r\" inreplyto=\"a&amp;1\"><contents>Reply</contents></text>\n</annots></xfdf>";
        let el = xfdf_element(x, "a&1").unwrap();
        assert!(el.starts_with("<ink ") && !el.contains("Reply"));
        assert_eq!(xfdf_element(x, "missing"), None);
        // Unnamed: found by its page and box.
        let unnamed = "<xfdf><annots>\n<square page=\"1\" rect=\"10,20,30,40\" color=\"#00FF00\"/>\n</annots></xfdf>";
        assert!(xfdf_element_at(unnamed, 1, [10.2, 20.0, 30.0, 39.8]).is_some_and(|e| e.starts_with("<square ")));
        assert_eq!(xfdf_element_at(unnamed, 0, [10.0, 20.0, 30.0, 40.0]), None);
        let p = paste_xfdf(&el, 2, (12.0, -12.0), "copy-1");
        assert!(p.contains("<ink page=\"2\" rect=\"22,8,42,28\" name=\"copy-1\" color=\"#FF0000\">"), "{p}");
        assert!(p.contains("<gesture>22,8;42,28</gesture>") && p.contains("<popup page=\"2\" rect=\"112,88,212,138\""), "{p}");
        assert!(p.starts_with("<?xml") && p.contains("<annots>"));
    }

    #[test]
    fn a_destination_points_at_its_top_edge_or_rectangle() {
        let crop = [0.0, 0.0, 612.0, 792.0];
        assert_eq!(dest_focus(&DestView::Xyz { left: None, top: Some(300.0), zoom: None }, crop), Some([0.0, 299.0, 612.0, 300.0]));
        assert_eq!(dest_focus(&DestView::FitH { top: Some(500.0) }, crop), Some([0.0, 499.0, 612.0, 500.0]));
        assert_eq!(dest_focus(&DestView::FitR { rect: [10.0, 20.0, 30.0, 40.0] }, crop), Some([10.0, 20.0, 30.0, 40.0]));
        assert_eq!(dest_focus(&DestView::Fit, crop), None);
        assert_eq!(dest_focus(&DestView::Xyz { left: None, top: None, zoom: None }, crop), None);
    }

    #[test]
    fn the_text_under_a_highlight_is_read_back() {
        let t = sample();
        assert_eq!(text_under(&t, &[[9.0, 9.0, 41.0, 21.0]]), "ab cd");
        assert_eq!(kind_label("StrikeOut"), "Strikethrough");
        assert!(!listed("Link") && listed("Highlight"));
    }

    #[test]
    fn zoom_typed_by_the_user() {
        assert_eq!(parse_zoom("150"), Some(150));
        assert_eq!(parse_zoom(" 75 % "), Some(75));
        assert_eq!(parse_zoom("12,5%"), Some(13));
        assert_eq!(parse_zoom("5000"), Some(1600));
        assert_eq!(parse_zoom("abc"), None);
        assert_eq!(parse_zoom("0"), None);
    }

    #[test]
    fn notes_filter_groups() {
        assert!(notes_filter(0, "Ink", false, "", false) && notes_filter(1, "Underline", false, "", false) && notes_filter(2, "FreeText", false, "", false));
        assert!(notes_filter(3, "Square", false, "", false) && notes_filter(3, "Ink", false, "", false) && notes_filter(4, "Ink", true, "", false) && notes_filter(4, "Stamp", false, "", false));
        assert!(!notes_filter(1, "Text", false, "", false) && !notes_filter(3, "Ink", true, "", false));
        // By status: Accepted (5) … Completed (8), no status (9).
        assert!(notes_filter(5, "Text", false, "Accepted", false) && !notes_filter(5, "Text", false, "Rejected", false));
        assert!(notes_filter(9, "Highlight", false, "", false) && !notes_filter(9, "Highlight", false, "Completed", false));
        assert!(notes_filter(10, "Text", false, "Accepted", true) && !notes_filter(10, "Text", false, "", false));
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
