//! Sheaf: Acrobat's Fix Hairlines for page strokes and nested Form XObjects.
//!
//! Each repaired Form placement gets a private copy: a shared logo on an unselected page must
//! stay unchanged, and the same Form drawn at two scales needs two different local widths.
//! Text strokes, tiling patterns, Type 3 glyphs and annotation appearances remain out of scope.

use pdfcraft_content::{Matrix, Op, Pieces};
use pdfcraft_cos::{Dict, Document, Object, Stream};

use crate::EditError;

#[derive(Clone, Copy)]
struct Stroke {
    matrix: Matrix,
    width: f64,
}

struct Repair {
    contents: Vec<Vec<u8>>,
    resources: Dict,
    count: usize,
}

struct Context {
    minimum: f64,
    unit: f64,
    remaining: usize,
}

/// Raise strokes below `minimum` physical points. Graphics state is restored after each path;
/// Forms inherit the calling width and transform, including their own Matrix and page UserUnit.
pub fn fix_page_hairlines(doc: &mut Document, pages: &[usize], minimum: f64) -> Result<usize, EditError> {
    if !minimum.is_finite() || !(0.01..=10.0).contains(&minimum) {
        return Err(EditError::Invalid("the minimum stroke must be 0.01 to 10 points".into()));
    }
    let all = pdfcraft_model::pages(doc);
    let mut count = 0;
    for &page in pages {
        let p = all.get(page).ok_or(EditError::NoSuchPage(page))?;
        let unit = p.dict.get(b"UserUnit").map(|v| doc.resolve(v)).and_then(|v| v.as_f64()).filter(|v| v.is_finite() && *v > 0.0).unwrap_or(1.0).min(pdfcraft_model::MAX_USER_UNIT);
        let resources = p.dict.get(b"Resources").and_then(|r| doc.dict(r)).unwrap_or_default();
        let streams = crate::images::page_streams(doc, &p.dict);
        let data = streams.iter().map(|s| s.1.as_slice()).collect::<Vec<_>>();
        let mut context = Context { minimum, unit, remaining: 1_000_000 };
        let repair = context.repair(doc, &data, resources, Stroke { matrix: Matrix::IDENTITY, width: 1.0 }, 0)?;
        if repair.count > 0 {
            count += repair.count;
            let contents = crate::text::rewritten_contents(doc, &streams, repair.contents);
            doc.update_dict(p.obj, |d| {
                d.set(b"Contents".to_vec(), contents);
                d.set(b"Resources".to_vec(), Object::Dict(repair.resources));
            })?;
        }
    }
    Ok(count)
}

impl Context {
    fn repair(&mut self, doc: &mut Document, streams: &[&[u8]], mut resources: Dict, initial: Stroke, depth: usize) -> Result<Repair, EditError> {
        if depth > 16 {
            return Err(EditError::Invalid("grouped artwork is cyclic or deeper than 16 Forms; no hairline repair was applied".into()));
        }
        let pieces = Pieces::join(streams);
        let parsed = pieces.parse();
        if parsed.skipped != 0 {
            return Err(EditError::Invalid("artwork has unreadable drawing operators; no hairline repair was applied".into()));
        }
        self.remaining = self.remaining.checked_sub(parsed.ops.len()).ok_or_else(|| EditError::Invalid("artwork exceeds the hairline repair limit; split the job into fewer pages".into()))?;
        let states = resources.get(b"ExtGState").and_then(|r| doc.dict(r)).unwrap_or_default();
        let mut xobjects = resources.get(b"XObject").and_then(|r| doc.dict(r)).unwrap_or_default();
        let mut state = initial;
        let mut saved = Vec::new();
        let mut replacements = std::collections::BTreeMap::new();
        let mut count = 0;
        for (i, op) in parsed.ops.iter().enumerate() {
            match op.op.as_slice() {
                b"q" => saved.push(state),
                b"Q" => state = saved.pop().unwrap_or(initial),
                b"cm" => {
                    if let Some(m) = op.nums::<6>() { state.matrix = Matrix(m).then(&state.matrix); }
                }
                b"w" => state.width = op.num(0).unwrap_or(state.width),
                b"gs" => {
                    let width = op.name(0).and_then(|name| states.get(name)).and_then(|s| doc.dict(s))
                        .and_then(|s| s.get(b"LW").map(|w| doc.resolve(w))).and_then(|w| w.as_f64());
                    if let Some(w) = width { state.width = w; }
                }
                b"Do" => {
                    let form = op.name(0).and_then(|n| xobjects.get(n)).map(|o| doc.resolve(o));
                    if let Some(Object::Stream(form)) = form.as_deref().filter(|o| o.as_dict().and_then(|d| d.get(b"Subtype")).and_then(Object::as_name) == Some(b"Form")) {
                        let data = form.decoded_within(32 * 1024 * 1024)?;
                        let matrix = form.dict.get(b"Matrix").map(|m| doc.resolve(m)).and_then(|m| m.as_array().map(|a| a.iter().filter_map(Object::as_f64).collect::<Vec<_>>()))
                            .and_then(|v| <[f64; 6]>::try_from(v).ok()).map(Matrix).unwrap_or(Matrix::IDENTITY);
                        let local = form.dict.get(b"Resources").and_then(|r| doc.dict(r)).unwrap_or_else(|| resources.clone());
                        let repair = self.repair(doc, &[data.as_slice()], local, Stroke { matrix: matrix.then(&state.matrix), width: state.width }, depth + 1)?;
                        if repair.count > 0 {
                            let mut dict = form.dict.clone();
                            dict.remove(b"Length");
                            dict.set(b"Resources".to_vec(), Object::Dict(repair.resources));
                            let changed = repair.contents.concat();
                            let copy = doc.add(Object::Stream(Stream::flate(dict, &changed)));
                            let mut suffix = i;
                            let name = loop {
                                let name = format!("SheafHairline{suffix}").into_bytes();
                                if !xobjects.contains(&name) { break name; }
                                suffix += 1;
                            };
                            xobjects.set(name.clone(), Object::Ref(copy));
                            replacements.insert(i, vec![Op::new("Do", vec![Object::Name(name)])]);
                            count += repair.count;
                        }
                    }
                }
                b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*" => {
                    let [a, b, c, d, _, _] = state.matrix.0;
                    // Smaller singular value: the thinnest this stroke can be at this placement.
                    let largest = ((a + d).hypot(b - c) + (a - d).hypot(b + c)) / 2.0;
                    let scale = (a * d - b * c).abs() / largest * self.unit;
                    if scale.is_finite() && scale > 1e-9 && state.width.is_finite() && state.width >= 0.0 && state.width * scale < self.minimum {
                        let width = self.minimum / scale;
                        if width > 1e9 { return Err(EditError::Invalid("the drawing scale is too small to repair its stroke".into())); }
                        replacements.insert(i, vec![Op::new("w", vec![Object::Real(width)]), op.clone(), Op::new("w", vec![Object::Real(state.width)])]);
                        count += 1;
                    }
                }
                _ => {}
            }
        }
        resources.set(b"XObject".to_vec(), Object::Dict(xobjects));
        let contents = crate::text::splice(&pieces, &parsed.ops, |i| match replacements.get(&i) {
            Some(ops) => (ops.clone(), false),
            None => (Vec::new(), true),
        });
        Ok(Repair { contents, resources, count })
    }
}
