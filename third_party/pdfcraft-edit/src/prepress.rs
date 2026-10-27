//! Sheaf: Acrobat's Fix Hairlines for strokes in the page's own content.
//!
//! Width is measured when a path is painted, after its transform, rather than when `w` is set.
//! Text, images, patterns, grouped Form artwork and annotation appearances are left alone.

use pdfcraft_content::{Matrix, Op, Pieces};
use pdfcraft_cos::{Document, Object};

use crate::EditError;

#[derive(Clone, Copy)]
struct Stroke {
    matrix: Matrix,
    width: f64,
}

/// Raise page-content strokes below `minimum` points. The graphics state is restored after
/// each painted path, so a fix cannot alter later text or artwork that inherits its width.
pub fn fix_page_hairlines(doc: &mut Document, pages: &[usize], minimum: f64) -> Result<usize, EditError> {
    if !minimum.is_finite() || !(0.01..=10.0).contains(&minimum) {
        return Err(EditError::Invalid("the minimum stroke must be 0.01 to 10 points".into()));
    }
    let all = pdfcraft_model::pages(doc);
    let mut count = 0;
    for &page in pages {
        let p = all.get(page).ok_or(EditError::NoSuchPage(page))?;
        let unit = p.dict.get(b"UserUnit").map(|v| doc.resolve(v)).and_then(|v| v.as_f64()).filter(|v| v.is_finite() && *v > 0.0).unwrap_or(1.0).min(pdfcraft_model::MAX_USER_UNIT);
        let resources = p.dict.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned()).unwrap_or_default();
        let states = resources.get(b"ExtGState").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned()).unwrap_or_default();
        let streams = crate::images::page_streams(doc, &p.dict);
        let pieces = Pieces::join(&streams.iter().map(|s| s.1.as_slice()).collect::<Vec<_>>());
        let parsed = pieces.parse();
        if parsed.skipped != 0 {
            return Err(EditError::Invalid(format!("page {} has unreadable drawing operators", page + 1)));
        }
        let mut state = Stroke { matrix: Matrix::IDENTITY, width: 1.0 };
        let mut saved = Vec::new();
        let mut replacements = std::collections::BTreeMap::new();
        for (i, op) in parsed.ops.iter().enumerate() {
            match op.op.as_slice() {
                b"q" => saved.push(state),
                b"Q" => state = saved.pop().unwrap_or(Stroke { matrix: Matrix::IDENTITY, width: 1.0 }),
                b"cm" => {
                    if let Some(m) = op.nums::<6>() {
                        state.matrix = Matrix(m).then(&state.matrix);
                    }
                }
                b"w" => state.width = op.num(0).unwrap_or(state.width),
                b"gs" => {
                    let width = op.name(0).and_then(|name| states.get(name)).map(|s| doc.resolve(s))
                        .and_then(|s| s.as_dict().and_then(|s| s.get(b"LW")).map(|w| doc.resolve(w)))
                        .and_then(|w| w.as_f64());
                    if let Some(w) = width { state.width = w; }
                }
                b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*" => {
                    let [a, b, c, d, _, _] = state.matrix.0;
                    // The smaller singular value: the thinnest a stroke can be at this transform.
                    let largest = ((a + d).hypot(b - c) + (a - d).hypot(b + c)) / 2.0;
                    let scale = (a * d - b * c).abs() / largest * unit;
                    if scale.is_finite() && scale > 1e-9 && state.width.is_finite() && state.width >= 0.0 && state.width * scale < minimum {
                        let width = minimum / scale;
                        if width > 1e9 {
                            return Err(EditError::Invalid("the drawing scale is too small to repair its stroke".into()));
                        }
                        replacements.insert(i, vec![Op::new("w", vec![Object::Real(width)]), op.clone(), Op::new("w", vec![Object::Real(state.width)])]);
                    }
                }
                _ => {}
            }
        }
        if !replacements.is_empty() {
            count += replacements.len();
            let changed = crate::text::splice(&pieces, &parsed.ops, |i| match replacements.get(&i) {
                Some(ops) => (ops.clone(), false),
                None => (Vec::new(), true),
            });
            let contents = crate::text::rewritten_contents(doc, &streams, changed);
            doc.update_dict(p.obj, |d| d.set(b"Contents".to_vec(), contents))?;
        }
    }
    Ok(count)
}
