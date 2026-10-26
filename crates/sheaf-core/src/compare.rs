//! Compare PDF: the differences between an original and a changed version.
//!
//! Both documents open in one PdfCraft session. "Semantic text" diffs their words and writes
//! PdfCraft's compare report (a new PDF listing every change). "Content overlay" writes a copy
//! of the changed document with the differences marked as comments: highlights over replaced
//! (blue) and inserted (green) words, a note where words were deleted (red), and a red outline
//! around any other region that looks different when the pages are rendered (pictures, drawings).

use crate::paths::atomic_write;
use crate::pdfcraft::engine::{compare::Kind, AddedText, Edit, NewAnnotation, Shape, Style};
use crate::pdfcraft::Doc;
use crate::{pages::Input, OpdfError, Result};
use std::path::{Path, PathBuf};

/// What the comparison writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompareMode {
    /// A report PDF listing the word changes.
    #[default]
    Text,
    /// The changed document with the differences highlighted.
    Overlay,
}

/// What [`compare`] found and wrote.
#[derive(Debug, Clone)]
pub struct CompareReport {
    pub path: PathBuf,
    /// Word changes, by PdfCraft's kinds: a run of deleted words directly followed by inserted
    /// ones is one replacement.
    pub added: usize,
    pub removed: usize,
    pub replaced: usize,
    /// Regions that look different but carry no changed words: outlined by Overlay, listed on the
    /// report's last page.
    pub regions: usize,
    /// The report was asked for (the areas are listed, not outlined).
    pub listed: bool,
    /// Overlay: the changed document was password-protected (its marked copy is not).
    pub was_encrypted: bool,
    /// Pages one document has past the other's last (a scanned page added or removed at the end
    /// has no words to compare): pages only the changed one has, pages only the original has.
    pub pages_added: usize,
    pub pages_removed: usize,
    /// Lines of the changed document whose words are the same but moved (no change on that line to
    /// have pushed them): Acrobat reports moved text.
    pub moved: usize,
}

impl CompareReport {
    /// The documents' text is the same.
    pub fn identical(&self) -> bool {
        self.added + self.removed + self.replaced == 0
    }

    /// "1 added, 2 removed, 3 changed".
    pub fn counts(&self) -> String {
        format!("{} added, {} removed, {} changed", self.added, self.removed, self.replaced)
    }

    /// The result line, in Sheaf's CLI and app alike.
    pub fn summary(&self) -> String {
        let mut s = if self.identical() { "No differences: the documents' text is identical.".to_string() } else { self.counts() };
        if self.regions > 0 {
            s += &format!("; {} {}", crate::tools::count(self.regions, "other changed area"), if self.listed { "listed" } else { "outlined" });
        }
        if self.moved > 0 {
            s += &format!("; {} moved", crate::tools::count(self.moved, "line"));
        }
        if self.pages_added > 0 {
            s += &format!("; {} at the end", crate::tools::count(self.pages_added, "page added"));
        }
        if self.pages_removed > 0 {
            s += &format!("; {} at the end", crate::tools::count(self.pages_removed, "page removed"));
        }
        s
    }
}

/// Resolution the pages are rendered at for the visual comparison.
const VISUAL_DPI: f32 = 72.0;

/// How far (points) a visual region may reach past the word boxes that explain it: the regions
/// are built from cells 8 pixels wide at [`VISUAL_DPI`], so they overshoot the glyphs they
/// enclose by up to a cell, plus anti-aliasing.
const WORD_SLACK: f64 = 10.0;

/// Whether box `a` lies entirely within the union of `bs` grown by [`WORD_SLACK`] (all
/// `[x0, y0, x1, y1]`): a region that only shows changed words. Checked on a grid of points.
fn explained_by(a: [f64; 4], bs: &[[f64; 4]]) -> bool {
    let grown: Vec<[f64; 4]> = bs.iter().map(|b| [b[0] - WORD_SLACK, b[1] - WORD_SLACK, b[2] + WORD_SLACK, b[3] + WORD_SLACK]).collect();
    let inside = |x: f64, y: f64| grown.iter().any(|b| b[0] <= x && x <= b[2] && b[1] <= y && y <= b[3]);
    // About one point apart, at most 200 steps a side.
    let steps = |d: f64| (d.ceil() as usize).clamp(1, 200);
    let (nx, ny) = (steps(a[2] - a[0]), steps(a[3] - a[1]));
    (0..=nx).all(|i| (0..=ny).all(|j| inside(a[0] + (a[2] - a[0]) * i as f64 / nx as f64, a[1] + (a[3] - a[1]) * j as f64 / ny as f64)))
}

/// A word: its text, page and box.
type Placed = (String, usize, [f64; 4]);

/// The boxes of words found in one document and not at the same place in the other (in
/// either document), by page.
fn moved(old: &[Placed], new: &[Placed]) -> Vec<(usize, [f64; 4])> {
    let same = |a: &Placed, b: &Placed| a.1 == b.1 && a.0 == b.0 && a.2.iter().zip(b.2).all(|(x, y)| (x - y).abs() < 0.5);
    let lone = |a: &[Placed], b: &[Placed]| a.iter().filter(|w| !b.iter().any(|v| same(w, v))).map(|w| (w.1, w.2)).collect::<Vec<_>>();
    let mut out = lone(old, new);
    out.extend(lone(new, old));
    out
}

/// The lines of `new` whose words were in `old` but are not at the same place, on lines where
/// `changed` sees no changed word: text moved as it was. One box per line, by page.
/// The horizontal span of the run of words on `page` that holds `r`: its neighbours on the same
/// line closer than a column gutter (18 pt), one after the other.
fn run_of(words: &[Placed], page: usize, r: &[f64; 4]) -> (f64, f64) {
    const GUTTER: f64 = 18.0;
    let line: Vec<[f64; 4]> = words.iter().filter(|w| w.1 == page && (w.2[1] - r[1]).abs() < 3.0).map(|w| w.2).collect();
    let (mut x0, mut x1) = (r[0], r[2]);
    loop {
        let wider = line.iter().fold((x0, x1), |(a, b), w| if w[2] >= a - GUTTER && w[0] <= b + GUTTER { (a.min(w[0]), b.max(w[2])) } else { (a, b) });
        if wider == (x0, x1) {
            return (x0, x1);
        }
        (x0, x1) = wider;
    }
}

fn moved_lines(old: &[Placed], new: &[Placed], changed: impl Fn(usize, &[f64; 4]) -> bool) -> Vec<(usize, [f64; 4])> {
    let same = |a: &Placed, b: &Placed| a.1 == b.1 && a.0 == b.0 && a.2.iter().zip(b.2).all(|(x, y)| (x - y).abs() < 0.5);
    let mut lines: Vec<(usize, [f64; 4])> = Vec::new();
    for w in new.iter().filter(|w| !old.iter().any(|o| same(o, w)) && old.iter().any(|o| o.0 == w.0) && !changed(w.1, &w.2)) {
        match lines.iter_mut().find(|(p, l)| *p == w.1 && (l[1] - w.2[1]).abs() < 3.0) {
            Some((_, l)) => *l = [l[0].min(w.2[0]), l[1].min(w.2[1]), l[2].max(w.2[2]), l[3].max(w.2[3])],
            None => lines.push((w.1, w.2)),
        }
    }
    lines
}

/// Compares `old` (the original) with `new` (the changed version) and writes `out` (never
/// overwriting): the report for [`CompareMode::Text`], the marked copy of `new` for
/// [`CompareMode::Overlay`].
pub fn compare(old: &Input, new: &Input, mode: CompareMode, out: &Path) -> Result<CompareReport> {
    let mut doc = Doc::open(new)?;
    let old_id = doc.open_also(old)?;
    let new_id = doc.id();
    let session = doc.session();
    let c = session.compare(old_id, new_id).map_err(|e| OpdfError::Engine(e.to_string()))?;
    let mut report = CompareReport {
        path: PathBuf::new(),
        added: c.count(Kind::Inserted),
        removed: c.count(Kind::Deleted),
        replaced: c.count(Kind::Replaced),
        regions: 0,
        listed: mode == CompareMode::Text,
        was_encrypted: false,
        pages_added: 0,
        pages_removed: 0,
        moved: 0,
    };
    // Regions that look different, less those that text changes entirely account for: the
    // changed words, as they are now and as they were, and the words a change moved along the
    // line. A region that also holds another change (a picture next to a changed caption) is
    // kept whole. What is left changed outside the words: images, drawings, scanned pages.
    let visual = session.compare_visual(old_id, new_id, VISUAL_DPI).map_err(|e| OpdfError::Engine(e.to_string()))?;
    let doc_words = |id| -> Vec<Placed> { session.get(id).map(|d| d.words().into_iter().map(|w| (w.text, w.page, w.rect)).collect()).unwrap_or_default() };
    let mut words: Vec<(usize, [f64; 4])> = c
        .changes
        .iter()
        .flat_map(|ch| ch.new.rects.iter().map(move |r| (ch.new.page, *r)).chain(ch.old.rects.iter().map(move |r| (ch.old.page, *r))))
        .collect();
    let (old_words, new_words) = (doc_words(old_id), doc_words(new_id));
    let shifted = moved(&old_words, &new_words);
    // Words moved where no change is on their line moved on their own (a paragraph moved down): a
    // line of them is reported (Sol, 25/10: a move with the same words showed no difference).
    // A change on the same line counts only in the same column (Sol, 25/10: a word changed in the
    // other column at the same height hid the move).
    let changed_line = |page: usize, r: &[f64; 4]| {
        let (x0, x1) = run_of(&new_words, page, r);
        c.changes.iter().any(|ch| ch.new.page == page && ch.new.rects.iter().any(|q| (q[1] - r[1]).abs() < 3.0 && q[2] >= x0 - 1.0 && q[0] <= x1 + 1.0))
    };
    let moved_lines = moved_lines(&old_words, &new_words, changed_line);
    report.moved = moved_lines.len();
    words.extend(shifted);
    let mut areas: Vec<(usize, [f64; 4])> =
        visual.into_iter().filter(|(page, r)| !explained_by(*r, &words.iter().filter(|(p, _)| p == page).map(|(_, w)| *w).collect::<Vec<_>>())).collect();
    report.regions = areas.len();
    // Pages past the other document's last: the visual comparison stops at the pages both have
    // (Sol, 25/10: a scanned page added or removed at the end went unreported). A page only the
    // changed document has is outlined whole; pages only the original has are said.
    let pages_of = |id| session.get(id).map(|d| d.info.pages.clone()).unwrap_or_default();
    let (old_pages, new_pages) = (pages_of(old_id), pages_of(new_id));
    for (page, info) in new_pages.iter().enumerate().skip(old_pages.len()) {
        let (a, b) = (info.view_to_user(0.0, 0.0), info.view_to_user(info.width, info.height));
        areas.push((page, [a[0].min(b[0]) as f64, a[1].min(b[1]) as f64, a[0].max(b[0]) as f64, a[1].max(b[1]) as f64]));
    }
    report.pages_added = new_pages.len().saturating_sub(old_pages.len());
    report.pages_removed = old_pages.len().saturating_sub(new_pages.len());
    let mut notes = Vec::new();
    // Two documents that share almost no words are said to (Acrobat warns that documents are too
    // different to compare; Sol, 25/10): the report is still written.
    let total = old_words.len() + new_words.len();
    let changed: usize = c.changes.iter().map(|ch| ch.new.rects.len() + ch.old.rects.len()).sum();
    if total >= 40 && changed * 10 >= total * 8 {
        notes.push(format!("The documents differ almost everywhere ({} % of their words): they may not be versions of one another.", changed * 100 / total));
    }
    if report.pages_added > 0 {
        notes.push(format!("{} only in the changed document.", pages_span(old_pages.len() + 1, new_pages.len(), "is", "are")));
    }
    let mut moved_pages: Vec<(usize, usize)> = Vec::new();
    for (page, _) in &moved_lines {
        match moved_pages.last_mut() {
            Some((p, k)) if p == page => *k += 1,
            _ => moved_pages.push((*page, 1)),
        }
    }
    for (page, k) in &moved_pages {
        notes.push(format!("Page {}: {} moved, the words the same.", page + 1, crate::tools::count(*k, "line")));
    }
    if report.pages_removed > 0 {
        notes.push(format!("{} not in the changed document.", pages_span(new_pages.len() + 1, old_pages.len(), "of the original is", "of the original are")));
    }
    match mode {
        CompareMode::Text => {
            let bytes = session.compare_report(old_id, new_id).map_err(|e| OpdfError::Engine(e.to_string()))?;
            if areas.is_empty() && notes.is_empty() {
                report.path = atomic_write(out, |part| Ok(std::fs::write(part, bytes.as_slice())?))?;
            } else {
                report.path = with_areas_page(&bytes, &areas, &notes, out)?;
            }
        }
        CompareMode::Overlay => {
            // Changed areas outlined in red, moved lines in blue.
            let outline = |(page, rect): (usize, [f64; 4]), kind: Kind, contents: &str| {
                let shape = Shape::Rectangle { rect };
                let mut style = Style::default_for(&shape);
                style.color = crate::pdfcraft::engine::compare::colour(kind);
                style.width = 1.5;
                Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: contents.into(), author: "Compare".into() })
            };
            let edits: Vec<Edit> = areas
                .into_iter()
                .map(|a| outline(a, Kind::Deleted, "Changed area"))
                .chain(moved_lines.iter().map(|&a| outline(a, Kind::Replaced, "Moved text")))
                .collect();
            doc.session_mut().mark_differences(old_id, new_id).map_err(|e| OpdfError::Engine(e.to_string()))?;
            if !edits.is_empty() {
                doc.apply(Edit::Batch { label: "Mark changed areas".into(), edits })?;
            }
            (report.path, report.was_encrypted) = doc.save_unprotected(out)?;
        }
    }
    Ok(report)
}

/// "Page 3 is" or "Pages 3–5 are" (1-based pages, `one` and `many` the words after them).
fn pages_span(first: usize, last: usize, one: &str, many: &str) -> String {
    if first == last { format!("Page {first} {one}") } else { format!("Pages {first}–{last} {many}") }
}

/// The text report with a last page listing the areas that changed outside the words (Acrobat's
/// report lists image changes too): a scanned page, a picture or a drawing changed shows there,
/// page by page; Overlay marks them on the page.
fn with_areas_page(report: &[u8], areas: &[(usize, [f64; 4])], notes: &[String], out: &Path) -> Result<PathBuf> {
    let tmp = tempfile::Builder::new().suffix(".pdf").tempfile()?;
    std::fs::write(tmp.path(), report)?;
    let mut doc = Doc::open(&Input::new(tmp.path()))?;
    let n = doc.document().info.pages.len();
    let mut per_page: Vec<(usize, usize)> = Vec::new();
    for (page, _) in areas {
        match per_page.last_mut() {
            Some((p, k)) if p == page => *k += 1,
            _ => per_page.push((*page, 1)),
        }
    }
    let lines: Vec<String> = per_page.iter().map(|(p, k)| format!("Page {}: {k} area{}", p + 1, if *k == 1 { "" } else { "s" })).collect();
    let pages = if notes.is_empty() { String::new() } else { format!("{}\n\n", notes.join("\n")) };
    let text = format!(
        "Changes outside the text\n\n{pages}These areas of the changed document look different though their words are the same: images, drawings or scanned pages. Compare's Overlay marks them on the pages.\n\n{}",
        lines.join("\n")
    );
    doc.apply(Edit::InsertBlankPage { at: n, width: 612.0, height: 792.0 })?;
    doc.apply(Edit::AddText { page: n, text: AddedText { rect: [54.0, 60.0, 558.0, 740.0], text, size: 11.0, ..Default::default() } })?;
    doc.save(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_is_explained_only_when_the_words_cover_it_whole() {
        let word = [100.0, 500.0, 140.0, 512.0];
        // The word's own region, a cell larger on every side: explained.
        assert!(explained_by([92.0, 492.0, 148.0, 520.0], &[word]));
        // A picture above it that the region also takes in: not explained, though it touches.
        assert!(!explained_by([92.0, 492.0, 300.0, 700.0], &[word]));
        // Two words side by side cover their joint region.
        assert!(explained_by([92.0, 492.0, 208.0, 520.0], &[word, [150.0, 500.0, 200.0, 512.0]]));
        assert!(!explained_by([0.0, 0.0, 10.0, 10.0], &[]));
    }
}
