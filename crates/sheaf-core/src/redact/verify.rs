//! The check of the written pages ([`verify`]): PdfCraft's redaction audit, refined glyph by
//! glyph, then pdfium's text under every mark.

use super::Mark;
use crate::engine::{open_pdf, pdfium_lock};
use crate::pdfcraft::{render, Doc};
use crate::{pages::Input, OpdfError, Result};
use pdfium_render::prelude::*;
use std::collections::BTreeSet;
use std::path::Path;

/// Checks a written PDF for redactions that only look done. Fails when:
/// - PdfCraft's audit finds a redaction mark never applied, or an opaque filled rectangle
///   painted over text that is still extractable under it (when that text lies under none of
///   `marks`, the box was in the input: the message names its pages and asks to mark it);
/// - pdfium (a second, independent reader, which also reads text inside Form XObjects) still
///   finds a character whose centre lies inside one of `marks`, or any character on a page
///   marked whole.
///
/// PdfCraft's audit works on whole text lines: a box over the middle of a line whose words on
/// both sides remain ("Mail ▮ today") counts as covering that line. Each such finding is
/// therefore checked glyph by glyph against PdfCraft's text layer and fails only when some
/// glyph lies at least a fifth under the box, the same share redaction itself removes.
///
/// Blind spots. Outside `marks`, a "box" is only a filled rectangle in the page's own content:
/// a box drawn in a Form XObject, as a path that is not `re`, as a black image, or as an
/// annotation (/Square, a stamp) is not seen, nor is text no reader can decode (a Type3 or
/// other font without /ToUnicode or a known encoding). Inside `marks` those cases are caught
/// when pdfium can decode the text. Text in a page's structure tree (/ActualText, /Alt) is not
/// checked here; outside the pages [`apply`] runs a deny-by-default audit of every object
/// ([`check_scrubbed`]), whose exemptions are its blind spots: fonts, images and ICC profiles
/// a page uses (glyph names, a subset's name, text drawn as pixels), name-tree keys,
/// destinations, URIs, /DA, field names and XMP markup. Anything that can't be read fails.
pub fn verify(path: &Path, marks: &[Mark]) -> Result<()> {
    let fail = |why: String| OpdfError::Conversion(format!("redaction audit failed: {why}"));
    let doc = Doc::open(&Input::new(path)).map_err(|e| fail(format!("the output can't be re-opened ({e})")))?;
    let d = doc.document();
    let findings = d.audit_redactions().map_err(fail)?;
    let config = render::RenderConfig { password: d.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
    let mut r = render::PageRenderer::new(d.bytes.clone(), config);
    let mut texts: std::collections::HashMap<usize, std::sync::Arc<render::PageText>> = Default::default();
    let mut real = Vec::new();
    // Per finding of `real`: the text it hides lies under no mark (the input's own box).
    let mut theirs = Vec::new();
    for f in findings {
        if f.kind.clone().id() != "covered-text" {
            real.push(f);
            theirs.push(false);
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
        let hidden: Vec<((f64, f64), &str)> = text
            .glyphs
            .iter()
            .filter(|g| !g.text.trim().is_empty())
            .filter_map(|g| {
                let [x0, y0, x1, y1] = info.view_rect_to_user(g.rect).map(f64::from);
                let area = (x1 - x0) * (y1 - y0);
                let inter = (x1.min(b[2]) - x0.max(b[0])).max(0.0) * (y1.min(b[3]) - y0.max(b[1])).max(0.0);
                (area > 0.0 && inter >= 0.2 * area).then_some((((x0 + x1) / 2.0, (y0 + y1) / 2.0), g.text.as_str()))
            })
            .collect();
        if hidden.is_empty() {
            continue;
        }
        let under_mark = hidden.iter().any(|(c, _)| marks.iter().any(|m| m.page == f.page && (m.whole_page || m.quads.iter().any(|q| in_quad(q, *c)))));
        // A box the input already had hides text only over a letter or digit: a frame over
        // private-use glyphs, spaces or punctuation is decoration.
        if under_mark || hidden.iter().any(|(_, t)| hides_text(t)) {
            theirs.push(!under_mark);
            real.push(f);
        }
    }
    // Text hidden under a box but under no mark was hidden in the input already.
    if !real.is_empty() && theirs.iter().all(|t| *t) {
        let pages: BTreeSet<usize> = real.iter().map(|f| f.page + 1).collect();
        let list = pages.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ");
        let page = if pages.len() == 1 { "page" } else { "pages" };
        return Err(OpdfError::Engine(format!("This PDF already hides text under a box on {page} {list}. Mark that area too to remove it.")));
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
    drop(r);
    drop(doc);
    text_under_marks(path, marks).map_err(|e| match e {
        OpdfError::Conversion(_) => e,
        e => fail(format!("the output can't be read by pdfium ({e})")),
    })
}

/// Whether covered text `t` is text a box can hide: it has a Unicode letter or digit.
fn hides_text(t: &str) -> bool {
    t.chars().any(char::is_alphanumeric)
}

/// Whether point `p` lies inside quadrilateral `q` (PDF /QuadPoints order: top-left,
/// top-right, bottom-left, bottom-right; any orientation).
fn in_quad(q: &[f64; 8], p: (f64, f64)) -> bool {
    let corners = [(q[0], q[1]), (q[2], q[3]), (q[6], q[7]), (q[4], q[5])];
    let side = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
    let s: Vec<f64> = (0..4).map(|i| side(corners[i], corners[(i + 1) % 4])).collect();
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}

/// pdfium's text under each mark: any non-blank character whose centre is inside a mark (or
/// anywhere on a page marked whole) fails.
fn text_under_marks(path: &Path, marks: &[Mark]) -> Result<()> {
    if marks.is_empty() {
        return Ok(());
    }
    let _g = pdfium_lock();
    let doc = open_pdf(path, None)?;
    let pages: BTreeSet<usize> = marks.iter().map(|m| m.page).collect();
    for p in pages {
        let page = doc.pages().get(p as PdfPageIndex)?;
        let text = page.text()?;
        let here: Vec<&Mark> = marks.iter().filter(|m| m.page == p).collect();
        let whole = here.iter().any(|m| m.whole_page);
        let mut left = String::new();
        for c in text.chars().iter() {
            let Some(ch) = c.unicode_char() else { continue };
            if ch.is_whitespace() || ch == '\u{fffe}' || c.is_generated().unwrap_or(false) {
                continue;
            }
            let b = c.tight_bounds().or_else(|_| c.loose_bounds())?;
            let centre = (((b.left().value + b.right().value) / 2.0) as f64, ((b.bottom().value + b.top().value) / 2.0) as f64);
            if whole || here.iter().any(|m| m.quads.iter().any(|q| in_quad(q, centre))) {
                left.push(ch);
            }
        }
        if !left.is_empty() {
            return Err(OpdfError::Conversion(format!("redaction audit failed: text is still under a black box on page {} (\"{left}\")", p + 1)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_letters_and_digits_count_as_hidden_text() {
        assert!(!hides_text("\u{f001}") && !hides_text(" ") && !hides_text("•–.") && !hides_text(""));
        assert!(hides_text("ab") && hides_text("7") && hides_text("é"));
    }
}
