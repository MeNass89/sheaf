//! Sheaf: drawings (vector paths) as objects of Edit a PDF, picked and moved with paragraphs and
//! images. A path is its construction operators (`m l c v y h re`) up to the operator that
//! paints it (`S s f F f* B B* b b*`); a clipping path (`W`/`W*`) or one only ended (`n`) is
//! none. Moving one wraps it in `q cm … Q`, as images are moved, so nothing else changes.

use std::collections::BTreeSet;

use pdfcraft_content::{Matrix, Op, Pieces};
use pdfcraft_cos::{Document, ObjRef, Object};

use crate::EditError;

/// More paths than this on a page (a map, a dense chart) and none is offered: picking stays
/// usable and the inventory under its limit.
pub(crate) const MAX_PATHS: usize = 4_000;

/// A painted path: where it is in the page's content (stream, first and painting operator) and
/// its box in user space (control points included, the stroke's width not).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PagePath {
    pub stream: usize,
    pub start: usize,
    pub end: usize,
    pub rect: [f64; 4],
    pub ctm: Matrix,
}

const PAINT: [&[u8]; 9] = [b"S", b"s", b"f", b"F", b"f*", b"B", b"B*", b"b", b"b*"];

/// The painted paths of the page's own content streams, not in `excluded` streams (added items);
/// empty when there are more than [`MAX_PATHS`].
pub(crate) fn page_paths(doc: &Document, page: usize, excluded: &BTreeSet<ObjRef>) -> Result<Vec<PagePath>, EditError> {
    let p = pdfcraft_model::pages(doc).into_iter().nth(page).ok_or(EditError::NoSuchPage(page))?;
    let streams = crate::images::page_streams(doc, &p.dict);
    let mut out = Vec::new();
    let mut ctm = Matrix::IDENTITY;
    let mut stack: Vec<Matrix> = Vec::new();
    for (si, (obj, bytes)) in streams.iter().enumerate() {
        let skip = obj.as_ref().is_some_and(|r| excluded.contains(&r));
        let ops = Pieces::join(&[bytes.as_slice()]).parse().ops;
        let mut start: Option<usize> = None;
        let mut points: Vec<(f64, f64)> = Vec::new();
        let mut clip = false;
        let mut in_text = false;
        for (i, op) in ops.iter().enumerate() {
            let name = op.op.as_slice();
            match name {
                b"q" => stack.push(ctm),
                b"Q" => ctm = stack.pop().unwrap_or(Matrix::IDENTITY),
                b"cm" => {
                    if let Some(m) = op.nums::<6>() {
                        ctm = Matrix(m).then(&ctm);
                    }
                }
                b"BT" => in_text = true,
                b"ET" => in_text = false,
                b"m" | b"l" | b"c" | b"v" | b"y" | b"re" | b"h" => {
                    if start.is_none() {
                        start = Some(i);
                        points.clear();
                        clip = false;
                    }
                    let n: Vec<f64> = op.operands.iter().filter_map(Object::as_f64).collect();
                    if name == b"re" && n.len() == 4 {
                        points.extend([(n[0], n[1]), (n[0] + n[2], n[1] + n[3])]);
                    } else {
                        points.extend(n.chunks_exact(2).map(|c| (c[0], c[1])));
                    }
                }
                b"W" | b"W*" => clip = true,
                b"n" => start = None,
                _ if PAINT.contains(&name) => {
                    if let Some(s) = start.take()
                        && !clip
                        && !in_text
                        && !skip
                        && !points.is_empty()
                    {
                        let user: Vec<(f64, f64)> = points.iter().map(|&(x, y)| ctm.apply(x, y)).collect();
                        let fold = |f: fn(f64, f64) -> f64, k: usize, init: f64| user.iter().map(|p| if k == 0 { p.0 } else { p.1 }).fold(init, f);
                        let mut rect = [fold(f64::min, 0, f64::MAX), fold(f64::min, 1, f64::MAX), fold(f64::max, 0, f64::MIN), fold(f64::max, 1, f64::MIN)];
                        // A line has no area: a point of thickness to hold it by.
                        if rect[2] - rect[0] < 1.0 {
                            rect[0] -= 0.5;
                            rect[2] += 0.5;
                        }
                        if rect[3] - rect[1] < 1.0 {
                            rect[1] -= 0.5;
                            rect[3] += 0.5;
                        }
                        if rect.iter().all(|v| v.is_finite()) {
                            out.push(PagePath { stream: si, start: s, end: i, rect, ctm });
                            if out.len() > MAX_PATHS {
                                return Ok(Vec::new());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

/// Moves paths `indexes` (of [`page_paths`]) by `offset` (user space): each is wrapped in `q`, a
/// translation in its own space, and `Q`.
pub(crate) fn translate_paths(doc: &mut Document, page: usize, indexes: &[usize], offset: [f64; 2], excluded: &BTreeSet<ObjRef>) -> Result<(), EditError> {
    if indexes.is_empty() {
        return Ok(());
    }
    let paths = page_paths(doc, page, excluded)?;
    let p = pdfcraft_model::pages(doc).into_iter().nth(page).ok_or(EditError::NoSuchPage(page))?;
    let all = crate::images::page_streams(doc, &p.dict);
    let mut starts = std::collections::BTreeMap::new();
    let mut ends = BTreeSet::new();
    for i in indexes {
        let path = paths.get(*i).ok_or_else(|| EditError::Invalid("a selected drawing no longer exists".into()))?;
        let inverse = path.ctm.invert().ok_or_else(|| EditError::Invalid("a selected drawing has no area".into()))?;
        let [a, b, c, d, _, _] = inverse.0;
        let local = [a * offset[0] + c * offset[1], b * offset[0] + d * offset[1]];
        if !local.iter().all(|v| v.is_finite() && v.abs() <= 1e12) {
            return Err(EditError::Invalid("the drawing move is too large".into()));
        }
        starts.insert((path.stream, path.start), local);
        ends.insert((path.stream, path.end));
    }
    let mut data = Vec::with_capacity(all.len());
    for (stream, (_, bytes)) in all.iter().enumerate() {
        let pieces = Pieces::join(&[bytes.as_slice()]);
        let ops = pieces.parse().ops;
        let changed = crate::text::splice(&pieces, &ops, |index| {
            let mut out = Vec::new();
            if let Some(l) = starts.get(&(stream, index)) {
                out.push(Op::new("q", vec![]));
                out.push(Op::new("cm", vec![Object::Real(1.0), Object::Real(0.0), Object::Real(0.0), Object::Real(1.0), Object::Real(l[0]), Object::Real(l[1])]));
            }
            if ends.contains(&(stream, index)) {
                // The painting operator, then the end of the wrap.
                out.extend(ops.get(index).cloned());
                out.push(Op::new("Q", vec![]));
                return (out, false);
            }
            (out, true)
        });
        data.push(changed.into_iter().next().unwrap_or_default());
    }
    let contents = crate::text::rewritten_contents(doc, &all, data);
    doc.update_dict(p.obj, |d| d.set(b"Contents".to_vec(), contents))?;
    Ok(())
}
