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
}

/// Resolution the pages are rendered at for the visual comparison.
const VISUAL_DPI: f32 = 72.0;

/// Whether box `a` overlaps any of `bs` (all `[x0, y0, x1, y1]`).
fn overlaps_any(a: [f64; 4], bs: &[[f64; 4]]) -> bool {
    bs.iter().any(|b| a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3])
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
            // Regions that look different, less those the word highlights already cover.
            let visual = session.compare_visual(old_id, new_id, VISUAL_DPI).map_err(|e| OpdfError::Engine(e.to_string()))?;
            let words: Vec<(usize, [f64; 4])> = c.changes.iter().flat_map(|ch| ch.new.rects.iter().map(move |r| (ch.new.page, *r))).collect();
            let edits: Vec<Edit> = visual
                .into_iter()
                .filter(|(page, r)| !overlaps_any(*r, &words.iter().filter(|(p, _)| p == page).map(|(_, w)| *w).collect::<Vec<_>>()))
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
