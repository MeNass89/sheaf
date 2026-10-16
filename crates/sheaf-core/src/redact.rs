//! Redact PDF: find sensitive text, mark areas, and remove what lies under the marks for good.
//!
//! Search works on PdfCraft's text layer (the glyphs its renderer places, so what is found is
//! what the page shows), with PdfCraft's patterns for e-mail addresses, phone numbers, credit
//! card numbers, US Social Security numbers and dates. Marks are PDF redaction annotations
//! (`/Redact`) in user space; [`apply`] adds them, applies them (text, image pixels and
//! drawings under them are removed and black boxes drawn), writes the result, then re-opens
//! the written file and audits it. If the audit finds anything (text still extractable under a
//! box, a mark left unapplied) the output is deleted and the call fails: a redaction that only
//! looks done is never shipped.

use crate::pdfcraft::engine::{rect_quad, Edit, NewAnnotation, Shape, Style};
use crate::pdfcraft::{render, Doc};
use crate::{pages::Input, OpdfError, Result};
use std::path::{Path, PathBuf};

/// PdfCraft's search patterns.
pub use crate::pdfcraft::engine::RedactPattern as Pattern;

/// What to look for.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    /// The text as typed (runs of spaces match any spacing, words may wrap to the next line).
    Literal { text: String, case_insensitive: bool },
    Pattern(Pattern),
}

/// One match: its page (0-based), one quadrilateral per line it covers (user space, as PDF
/// `/QuadPoints`: top-left, top-right, bottom-left, bottom-right), and the text matched.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMatch {
    pub page: usize,
    pub quads: Vec<[f64; 8]>,
    pub text: String,
}

/// An area to redact on page `page` (0-based): one or more quadrilaterals in user space.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub page: usize,
    pub quads: Vec<[f64; 8]>,
}

/// What [`apply`] wrote.
#[derive(Debug, Clone)]
pub struct RedactReport {
    pub path: PathBuf,
    /// Marks applied (a match over two lines is one area).
    pub areas: usize,
    /// The input was password-protected; the output is not.
    pub was_encrypted: bool,
}

/// The bounding box `[x0, y0, x1, y1]` of quadrilaterals.
pub fn bounds(quads: &[[f64; 8]]) -> [f64; 4] {
    let mut r = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for q in quads {
        for p in q.chunks(2) {
            r = [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])];
        }
    }
    r
}

/// Every match of `query` in the document, page by page.
pub fn find(input: &Input, query: &Query) -> Result<Vec<TextMatch>> {
    find_all(input, std::slice::from_ref(query))
}

/// Every match of each of `queries` (opened and read once), page by page; on a page, in the
/// order of `queries`, then of the text.
pub fn find_all(input: &Input, queries: &[Query]) -> Result<Vec<TextMatch>> {
    let doc = Doc::open(input)?;
    let d = doc.document();
    let config = render::RenderConfig { password: d.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
    let mut r = render::PageRenderer::new(d.bytes.clone(), config);
    let mut out = Vec::new();
    for (page, info) in d.info.pages.iter().enumerate() {
        let res = r.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
        let Some(text) = res.text else {
            if let Some(e) = res.error {
                return Err(OpdfError::Engine(format!("page {}: {e}", page + 1)));
            }
            continue;
        };
        for q in queries {
            let ranges = match q {
                Query::Literal { text: needle, case_insensitive } => text.find_opts(needle, !case_insensitive, false),
                Query::Pattern(p) => text.find_with(|chars| crate::pdfcraft::engine::find_pattern(*p, chars)),
            };
            for g in ranges {
                let quads = text.line_rects(g.clone()).into_iter().map(|v| info.view_rect_to_quad(v)).collect();
                out.push(TextMatch { page, quads, text: text.text_of(g).replace('\n', " ") });
            }
        }
    }
    Ok(out)
}

/// Marks over the matches.
pub fn mark_text(matches: &[TextMatch]) -> Vec<Mark> {
    matches.iter().map(|m| Mark { page: m.page, quads: m.quads.clone() }).collect()
}

/// Marks over rectangles `[x0, y0, x1, y1]` in user space, one per `(page, rect)`.
pub fn mark_rects(rects: &[(usize, [f64; 4])]) -> Vec<Mark> {
    rects.iter().map(|(page, r)| Mark { page: *page, quads: vec![rect_quad(*r)] }).collect()
}

/// Marks over whole pages (0-based): their media box, so content outside the visible crop
/// area goes too.
pub fn mark_pages(input: &Input, pages: &[usize]) -> Result<Vec<Mark>> {
    let doc = Doc::open(input)?;
    let boxes = doc.document().page_boxes();
    pages
        .iter()
        .map(|p| {
            let b = boxes.get(*p).ok_or_else(|| OpdfError::Unsupported(format!("page {} does not exist (the document has {} pages)", p + 1, boxes.len())))?;
            Ok(Mark { page: *p, quads: vec![rect_quad(b[0])] })
        })
        .collect()
}

/// Applies `marks` and writes the result to `out` (never overwriting). The written file is
/// re-opened and audited; any finding deletes it and fails with [`OpdfError::Conversion`].
pub fn apply(input: &Input, marks: &[Mark], out: &Path) -> Result<RedactReport> {
    if marks.is_empty() {
        return Err(OpdfError::Unsupported("nothing is marked for redaction".into()));
    }
    let mut doc = Doc::open(input)?;
    let pages = doc.document().info.pages.len();
    if let Some(m) = marks.iter().find(|m| m.page >= pages) {
        return Err(OpdfError::Unsupported(format!("page {} does not exist (the document has {pages} pages)", m.page + 1)));
    }
    let edits = marks
        .iter()
        .map(|m| {
            let shape = Shape::Redact { quads: m.quads.clone(), overlay: String::new(), look: Default::default() };
            let style = Style::default_for(&shape);
            Edit::AddAnnotation(NewAnnotation { page: m.page, shape, style, contents: String::new(), author: "Sheaf".into() })
        })
        .collect();
    doc.apply(Edit::Batch { label: "Mark for redaction".into(), edits })?;
    doc.apply(Edit::ApplyRedactions { pages: None })?;
    let (path, was_encrypted) = doc.save_unprotected(out)?;
    if let Err(e) = verify(&path) {
        let _ = std::fs::remove_file(&path);
        return Err(e);
    }
    Ok(RedactReport { path, areas: marks.len(), was_encrypted })
}

/// Checks a written PDF for redactions that only look done: fails (with
/// [`OpdfError::Conversion`]) when PdfCraft's audit finds a redaction mark never applied, or an
/// opaque box with text still under it.
///
/// The audit works on whole text lines: a box over the middle of a line whose words on both
/// sides remain ("Mail ▮ today") counts as covering that line. Each such finding is therefore
/// checked glyph by glyph against PdfCraft's text layer (an independent reader: the renderer's)
/// and fails only when some glyph lies at least a fifth under the box, the same share redaction
/// itself removes. Anything that can't be read fails.
pub fn verify(path: &Path) -> Result<()> {
    let fail = |why: String| OpdfError::Conversion(format!("redaction audit failed: {why}"));
    let doc = Doc::open(&Input::new(path)).map_err(|e| fail(format!("the output can't be re-opened ({e})")))?;
    let d = doc.document();
    let findings = d.audit_redactions().map_err(fail)?;
    let config = render::RenderConfig { password: d.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
    let mut r = render::PageRenderer::new(d.bytes.clone(), config);
    let mut texts: std::collections::HashMap<usize, std::sync::Arc<render::PageText>> = Default::default();
    let mut real = Vec::new();
    for f in findings {
        if f.kind.clone().id() != "covered-text" {
            real.push(f);
            continue;
        }
        let text = match texts.get(&f.page) {
            Some(t) => t.clone(),
            None => {
                let res = r.render(render::RenderRequest { page: f.page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
                let t = res.text.ok_or_else(|| fail(format!("page {} can't be read back", f.page + 1)))?;
                texts.insert(f.page, t.clone());
                t
            }
        };
        let info = d.info.pages.get(f.page).ok_or_else(|| fail(format!("page {} is missing", f.page + 1)))?;
        let b = f.rect;
        let hidden = text.glyphs.iter().filter(|g| !g.text.trim().is_empty()).any(|g| {
            let [x0, y0, x1, y1] = info.view_rect_to_user(g.rect).map(f64::from);
            let area = (x1 - x0) * (y1 - y0);
            let inter = (x1.min(b[2]) - x0.max(b[0])).max(0.0) * (y1.min(b[3]) - y0.max(b[1])).max(0.0);
            area > 0.0 && inter >= 0.2 * area
        });
        if hidden {
            real.push(f);
        }
    }
    if let Some(f) = real.first() {
        return Err(fail(format!(
            "{} problem{}, first on page {} ({}{})",
            real.len(),
            if real.len() == 1 { "" } else { "s" },
            f.page + 1,
            f.kind.clone().id(),
            if f.covered_text.is_empty() { String::new() } else { format!(": \"{}\"", f.covered_text) }
        )));
    }
    Ok(())
}
