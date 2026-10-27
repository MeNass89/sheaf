//! Sheaf: Acrobat's Fix Hairlines for page strokes and nested Form XObjects.
//!
//! Each repaired Form placement gets a private copy: a shared logo on an unselected page must
//! stay unchanged, and the same Form drawn at two scales needs two different local widths.
//! Tiling patterns and Type 3 charprocs get private resources too; annotation appearances and
//! stroked outlines in ordinary fonts remain unchanged.

use pdfcraft_content::{Matrix, Op, Pieces};
use pdfcraft_cos::{Dict, Document, Object, Stream};

use crate::EditError;

#[derive(Clone, Copy)]
struct Stroke {
    matrix: Matrix,
    width: f64,
    font: Option<usize>,
    size: f64,
    horizontal: f64,
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
        let objects = match p.dict.get(b"Contents") {
            None => Vec::new(),
            Some(value) => match &*doc.resolve(value) {
                Object::Array(values) => values.clone(),
                _ => vec![value.clone()],
            },
        };
        let mut streams = Vec::new();
        let mut remaining = 32 * 1024 * 1024;
        for object in objects {
            let value = doc.resolve(&object);
            let Object::Stream(stream) = &*value else {
                return Err(EditError::Invalid("page contents are not readable streams; no hairline repair was applied".into()));
            };
            let bytes = stream.decoded_within(remaining)?;
            // A stream that decodes to nothing from bytes that are there is damaged: refused, not
            // written back empty (its drawing would be lost without a word).
            if bytes.is_empty() && !stream.raw.is_empty() {
                return Err(EditError::Invalid("a page's content cannot be read; no hairline repair was applied".into()));
            }
            remaining -= bytes.len();
            streams.push((object, bytes));
        }
        let data = streams.iter().map(|s| s.1.as_slice()).collect::<Vec<_>>();
        let mut context = Context { minimum, unit, remaining: 1_000_000 };
        let repair = context.repair(doc, &data, resources, Stroke { matrix: Matrix::IDENTITY, width: 1.0, font: None, size: 12.0, horizontal: 1.0 }, 0)?;
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
            return Err(EditError::Invalid("grouped artwork is cyclic or deeper than 16 resources; no hairline repair was applied".into()));
        }
        let pieces = Pieces::join(streams);
        let parsed = pieces.parse();
        if parsed.skipped != 0 {
            return Err(EditError::Invalid("artwork has unreadable drawing operators; no hairline repair was applied".into()));
        }
        self.remaining = self.remaining.checked_sub(parsed.ops.len()).ok_or_else(|| EditError::Invalid("artwork exceeds the hairline repair limit; split the job into fewer pages".into()))?;
        let states = resources.get(b"ExtGState").and_then(|r| doc.dict(r)).unwrap_or_default();
        let mut xobjects = resources.get(b"XObject").and_then(|r| doc.dict(r)).unwrap_or_default();
        let mut patterns = resources.get(b"Pattern").and_then(|r| doc.dict(r)).unwrap_or_default();
        let mut fonts = resources.get(b"Font").and_then(|r| doc.dict(r)).unwrap_or_default();
        let font_names: Vec<_> = fonts.iter().map(|(name, _)| name.clone()).collect();
        let mut text_matrix = Matrix::IDENTITY;
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
                b"Tf" => {
                    state.font = op.name(0).and_then(|name| font_names.iter().position(|n| n == name));
                    state.size = op.num(1).unwrap_or(state.size);
                }
                b"Tz" => state.horizontal = op.num(0).unwrap_or(100.0) / 100.0,
                b"BT" => text_matrix = Matrix::IDENTITY,
                b"Tm" => { if let Some(m) = op.nums::<6>() { text_matrix = Matrix(m); } }
                b"Tj" | b"TJ" | b"'" | b"\"" => {
                    let name = state.font.and_then(|k| font_names.get(k));
                    let font = name.and_then(|n| fonts.get(n)).and_then(|f| doc.dict(f));
                    if let (Some(name), Some(font)) = (name, font.filter(|f| f.name(b"Subtype") == Some(b"Type3"))) {
                        let matrix = matrix(doc, &font, b"FontMatrix", Matrix([0.001, 0.0, 0.0, 0.001, 0.0, 0.0]));
                        let size = Matrix([state.size * state.horizontal, 0.0, 0.0, state.size, 0.0, 0.0]);
                        let placed = matrix.then(&size).then(&text_matrix).then(&state.matrix);
                        let mut chars = font.get(b"CharProcs").and_then(|v| doc.dict(v)).ok_or_else(|| EditError::Invalid("a Type 3 font has no readable glyphs".into()))?;
                        let mut local = font.get(b"Resources").and_then(|v| doc.dict(v)).unwrap_or_else(|| resources.clone());
                        let mut fixed = 0;
                        // The font copy is private to this text-show operation. Fix all its glyphs;
                        // the encoding and text advance remain exactly the original font's.
                        for (glyph, object) in chars.clone().iter() {
                            let value = doc.resolve(object);
                            let Object::Stream(stream) = &*value else { return Err(EditError::Invalid("a Type 3 glyph is unreadable".into())) };
                            let data = decode_artwork(stream)?;
                            let repair = self.repair(doc, &[&data], local.clone(), Stroke { matrix: placed, ..state }, depth + 1)?;
                            if repair.count > 0 {
                                let mut dict = stream.dict.clone();
                                dict.remove(b"Length");
                                let copy = doc.add(Object::Stream(Stream::flate(dict, &repair.contents.concat())));
                                chars.set(glyph.clone(), Object::Ref(copy));
                                local = repair.resources;
                                fixed += repair.count;
                            }
                        }
                        if fixed > 0 {
                            let mut changed = font.clone();
                            changed.set(b"Resources".to_vec(), Object::Dict(local));
                            changed.set(b"CharProcs".to_vec(), Object::Dict(chars));
                            let copy = doc.add(Object::Dict(changed));
                            let private = private_name(&fonts, "SheafHairlineFont", i);
                            fonts.set(private.clone(), Object::Ref(copy));
                            replacements.insert(i, vec![Op::new("Tf", vec![Object::Name(private), Object::Real(state.size)]), op.clone(), Op::new("Tf", vec![Object::Name(name.clone()), Object::Real(state.size)])]);
                            count += fixed;
                        }
                    }
                }
                b"scn" | b"SCN" => {
                    if let Some(Object::Name(name)) = op.operands.last() {
                        let value = patterns.get(name).map(|v| doc.resolve(v));
                        if let Some(Object::Stream(pattern)) = value.as_deref().filter(|o| o.as_dict().and_then(|d| d.int(b"PatternType")) == Some(1)) {
                            // Pattern matrices refer to the parent stream's initial user space,
                            // independently of later cm operators used to draw the filled path.
                            let placed = matrix(doc, &pattern.dict, b"Matrix", Matrix::IDENTITY).then(&initial.matrix);
                            let local = pattern.dict.get(b"Resources").and_then(|v| doc.dict(v)).unwrap_or_else(|| resources.clone());
                            let data = decode_artwork(pattern)?;
                            let repair = self.repair(doc, &[&data], local, Stroke { matrix: placed, ..state }, depth + 1)?;
                            if repair.count > 0 {
                                let mut dict = pattern.dict.clone();
                                dict.remove(b"Length");
                                dict.set(b"Resources".to_vec(), Object::Dict(repair.resources));
                                let copy = doc.add(Object::Stream(Stream::flate(dict, &repair.contents.concat())));
                                let private = private_name(&patterns, "SheafHairlinePattern", i);
                                patterns.set(private.clone(), Object::Ref(copy));
                                let mut changed = op.clone();
                                if let Some(last) = changed.operands.last_mut() { *last = Object::Name(private); }
                                replacements.insert(i, vec![changed]);
                                count += repair.count;
                            }
                        }
                    }
                }
                b"Do" => {
                    let form = op.name(0).and_then(|n| xobjects.get(n)).map(|o| doc.resolve(o));
                    if let Some(Object::Stream(form)) = form.as_deref().filter(|o| o.as_dict().and_then(|d| d.get(b"Subtype")).and_then(Object::as_name) == Some(b"Form")) {
                        let data = decode_artwork(form)?;
                        let matrix = form.dict.get(b"Matrix").map(|m| doc.resolve(m)).and_then(|m| m.as_array().map(|a| a.iter().filter_map(Object::as_f64).collect::<Vec<_>>()))
                            .and_then(|v| <[f64; 6]>::try_from(v).ok()).map(Matrix).unwrap_or(Matrix::IDENTITY);
                        let local = form.dict.get(b"Resources").and_then(|r| doc.dict(r)).unwrap_or_else(|| resources.clone());
                        let repair = self.repair(doc, &[data.as_slice()], local, Stroke { matrix: matrix.then(&state.matrix), ..state }, depth + 1)?;
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
        resources.set(b"Font".to_vec(), Object::Dict(fonts));
        resources.set(b"Pattern".to_vec(), Object::Dict(patterns));
        let contents = crate::text::splice(&pieces, &parsed.ops, |i| match replacements.get(&i) {
            Some(ops) => (ops.clone(), false),
            None => (Vec::new(), true),
        });
        Ok(Repair { contents, resources, count })
    }
}

fn matrix(doc: &Document, dict: &Dict, key: &[u8], default: Matrix) -> Matrix {
    dict.get(key).map(|v| doc.resolve(v)).and_then(|v| v.as_array().map(|a| a.iter().map(|o| doc.resolve(o).as_f64()).collect::<Option<Vec<_>>>()))
        .flatten().and_then(|v| <[f64; 6]>::try_from(v).ok()).map(Matrix).unwrap_or(default)
}

fn private_name(dict: &Dict, prefix: &str, start: usize) -> Vec<u8> {
    let mut i = start;
    loop {
        let name = format!("{prefix}{i}").into_bytes();
        if !dict.contains(&name) { return name; }
        i += 1;
    }
}

fn decode_artwork(stream: &Stream) -> Result<Vec<u8>, EditError> {
    let bytes = stream.decoded_within(32 * 1024 * 1024)?;
    if bytes.is_empty() && !stream.raw.is_empty() {
        return Err(EditError::Invalid("grouped artwork, pattern or glyph content cannot be read; no hairline repair was applied".into()));
    }
    Ok(bytes)
}
