//! Compare PDF: the differences between an original and a changed version.
//!
//! Both documents open in one PdfCraft session. "Semantic text" diffs their words and writes
//! PdfCraft's compare report (a new PDF listing every change). "Content overlay" writes a copy
//! of the changed document with the differences marked as comments: highlights over replaced
//! (blue) and inserted (green) words, a note where words were deleted (red), and a red outline
//! around any other region that looks different when the pages are rendered (pictures, drawings).

use crate::paths::atomic_write;
use crate::pdfcraft::engine::{compare::Kind, Edit, NewAnnotation, Shape, Style};
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
    /// Overlay: regions that look different but carry no changed words.
    pub regions: usize,
    /// Overlay: the changed document was password-protected (its marked copy is not).
    pub was_encrypted: bool,
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
            s += &format!("; {} outlined", crate::tools::count(self.regions, "other changed area"));
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
        was_encrypted: false,
    };
    match mode {
        CompareMode::Text => {
            let bytes = session.compare_report(old_id, new_id).map_err(|e| OpdfError::Engine(e.to_string()))?;
            report.path = atomic_write(out, |part| Ok(std::fs::write(part, bytes.as_slice())?))?;
        }
        CompareMode::Overlay => {
            // Regions that look different, less those that text changes entirely account for:
            // the changed words, as they are now and as they were, and the words a change moved
            // along the line. A region that also holds another change (a picture next to a
            // changed caption) is kept whole.
            let visual = session.compare_visual(old_id, new_id, VISUAL_DPI).map_err(|e| OpdfError::Engine(e.to_string()))?;
            let doc_words = |id| -> Vec<Placed> { session.get(id).map(|d| d.words().into_iter().map(|w| (w.text, w.page, w.rect)).collect()).unwrap_or_default() };
            let mut words: Vec<(usize, [f64; 4])> = c
                .changes
                .iter()
                .flat_map(|ch| ch.new.rects.iter().map(move |r| (ch.new.page, *r)).chain(ch.old.rects.iter().map(move |r| (ch.old.page, *r))))
                .collect();
            words.extend(moved(&doc_words(old_id), &doc_words(new_id)));
            let edits: Vec<Edit> = visual
                .into_iter()
                .filter(|(page, r)| !explained_by(*r, &words.iter().filter(|(p, _)| p == page).map(|(_, w)| *w).collect::<Vec<_>>()))
                .map(|(page, rect)| {
                    let shape = Shape::Rectangle { rect };
                    let mut style = Style::default_for(&shape);
                    style.color = crate::pdfcraft::engine::compare::colour(Kind::Deleted);
                    style.width = 1.5;
                    Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: "Changed area".into(), author: "Compare".into() })
                })
                .collect();
            report.regions = edits.len();
            doc.session_mut().mark_differences(old_id, new_id).map_err(|e| OpdfError::Engine(e.to_string()))?;
            if !edits.is_empty() {
                doc.apply(Edit::Batch { label: "Mark changed areas".into(), edits })?;
            }
            (report.path, report.was_encrypted) = doc.save_unprotected(out)?;
        }
    }
    Ok(report)
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
