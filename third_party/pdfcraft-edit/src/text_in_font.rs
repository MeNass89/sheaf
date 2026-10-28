//! Sheaf: new text written on a page in a font of the computer's (an open-licensed full font
//! program, embedded; Acrobat's Add Text with its font list). The text becomes page content, a
//! paragraph like any other the Edit tool can then change; each typed line is one line, placed
//! from the top of its box (display space, as added text's) and aligned in it.

use pdfcraft_content::{Op, num, serialize_ops};
use pdfcraft_cos::{Dict, Document, Object, PdfString, Stream};
use pdfcraft_fonts::pdf::Metrics;

use crate::EditError;
use crate::added::{Align, AddedText};

/// `t` (its box, text, size, colour, alignment and underline; its family and weight are the
/// font's) written on `page` (0-based) in the font program `font`, embedded first.
pub fn add_text_in_font(doc: &mut Document, page: usize, t: &AddedText, font: &[u8]) -> Result<(), EditError> {
    if t.text.trim().is_empty() {
        return Err(EditError::Invalid("there is no text to add".into()));
    }
    if !(t.size.is_finite() && t.size > 0.0) || !t.rect.iter().chain(t.color.iter()).all(|v| v.is_finite()) {
        return Err(EditError::Invalid("invalid text size, colour or box".into()));
    }
    let resource = crate::text::register_text_font(doc, page, font)?;
    write_text(doc, page, t, &resource)
}

/// `t` written on `page` in the page's font resource `resource`, line by line as `t.text` breaks.
fn write_text(doc: &mut Document, page: usize, t: &AddedText, resource: &str) -> Result<(), EditError> {
    if t.text.trim().is_empty() {
        return Err(EditError::Invalid("there is no text to add".into()));
    }
    if !(t.size.is_finite() && t.size > 0.0) || !t.rect.iter().chain(t.color.iter()).all(|v| v.is_finite()) {
        return Err(EditError::Invalid("invalid text size, colour or box".into()));
    }
    let p = pdfcraft_model::pages(doc).into_iter().nth(page).ok_or(EditError::NoSuchPage(page))?;
    let fonts = p.dict.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().and_then(|d| d.get(b"Font").map(|f| doc.resolve(f))).and_then(|f| f.as_dict().cloned())).unwrap_or_default();
    let dict = fonts.get(resource.as_bytes()).map(|f| doc.resolve(f)).and_then(|f| f.as_dict().cloned()).ok_or_else(|| EditError::Invalid("the font was not embedded".into()))?;
    let m = Metrics::from_dict(doc, &dict);
    let lines: Vec<&str> = t.text.lines().collect();
    let mut encoded = Vec::with_capacity(lines.len());
    for line in &lines {
        let bytes = m.encode(line).ok_or_else(|| {
            let c = line.chars().find(|c| m.encode(&c.to_string()).is_none()).unwrap_or('?');
            EditError::Invalid(format!("the font has no \"{c}\": pick another font for this text"))
        })?;
        let width: f64 = m.codes(&bytes).iter().map(|(c, _)| m.width(*c)).sum::<f64>() * t.size;
        encoded.push((bytes, width));
    }
    let rect = [t.rect[0].min(t.rect[2]), t.rect[1].min(t.rect[3]), t.rect[0].max(t.rect[2]), t.rect[1].max(t.rect[3])];
    let box_width = (rect[2] - rect[0]).max(encoded.iter().map(|e| e.1).fold(0.0, f64::max));
    let [r, g, b] = t.color.map(|c| c.clamp(0.0, 1.0));
    // Drawn in display space (the page as shown), as added text is.
    let mut ops = vec![Op::new("q", vec![]), Op::new("cm", p.view_matrix(doc).iter().map(|v| num(*v)).collect())];
    ops.push(Op::new("rg", vec![num(r), num(g), num(b)]));
    ops.push(Op::new("BT", vec![]));
    ops.push(Op::new("Tf", vec![Object::name(resource), num(t.size)]));
    let mut lines_at = Vec::new();
    let mut y = rect[3] - t.size * 0.9;
    for (bytes, width) in &encoded {
        let x = match t.align {
            Align::Center => rect[0] + (box_width - width) / 2.0,
            Align::Right => rect[0] + box_width - width,
            _ => rect[0],
        };
        ops.push(Op::new("Tm", vec![num(1.0), num(0.0), num(0.0), num(1.0), num(x), num(y)]));
        ops.push(Op::new("Tj", vec![Object::String(PdfString::literal(bytes.clone()))]));
        lines_at.push((x, x + width, y));
        y -= t.size * 1.2;
    }
    ops.push(Op::new("ET", vec![]));
    if t.underline {
        ops.push(Op::new("RG", vec![num(r), num(g), num(b)]));
        ops.push(Op::new("w", vec![num(t.size * 0.06)]));
        for (x0, x1, y) in lines_at.into_iter().filter(|l| l.1 > l.0) {
            ops.push(Op::new("m", vec![num(x0), num(y - t.size * 0.12)]));
            ops.push(Op::new("l", vec![num(x1), num(y - t.size * 0.12)]));
            ops.push(Op::new("S", vec![]));
        }
    }
    ops.push(Op::new("Q", vec![]));
    // The page's own content in a state of its own (closing what it leaves open), then the text.
    let streams = crate::images::page_streams(doc, &p.dict);
    let depth = streams
        .iter()
        .flat_map(|(_, d)| pdfcraft_content::parse(d).ops)
        .fold(0usize, |depth, op| if op.is("q") { depth + 1 } else if op.is("Q") { depth.saturating_sub(1) } else { depth });
    let mut close = vec![Op::new("Q", vec![]); depth + 1];
    close.extend(ops);
    let mut contents = vec![Object::Ref(doc.add(Object::Stream(Stream::flate(Dict::new(), b"q"))))];
    contents.extend(streams.into_iter().map(|(o, _)| o));
    contents.push(Object::Ref(doc.add(Object::Stream(Stream::flate(Dict::new(), &serialize_ops(&close))))));
    doc.update_dict(p.obj, |d| d.set(b"Contents".to_vec(), Object::Array(contents)))?;
    Ok(())
}

/// Sheaf: the font dictionary named `resource` in the resources of page `page`.
fn page_font(doc: &Document, page: usize, resource: &str) -> Option<Object> {
    let p = pdfcraft_model::pages(doc).into_iter().nth(page)?;
    let fonts = p.dict.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().and_then(|d| d.get(b"Font").map(|f| doc.resolve(f))).and_then(|f| f.as_dict().cloned()))?;
    fonts.get(resource.as_bytes()).cloned()
}

/// Sheaf: whether `text` (each of its lines) can be written in page `page`'s font `resource`:
/// the font maps every character back to a code it holds (a subset holds the characters it showed).
pub fn font_writes(doc: &Document, page: usize, resource: &str, text: &str) -> bool {
    let Some(dict) = page_font(doc, page, resource).and_then(|f| doc.resolve(&f).as_dict().cloned()) else { return false };
    let m = Metrics::from_dict(doc, &dict);
    !text.trim().is_empty() && text.lines().all(|l| m.encode(l).is_some())
}

/// Sheaf: `t` written on `page` in the font `resource` of page `src_page` of `src`, that font (its
/// descriptor, embedded program, widths and Unicode map) copied in first under a new name. Pasted
/// paragraphs keep their embedded font as in PDF Expert and Acrobat (Sol, 27/10: they became
/// Helvetica, Times or Courier).
pub fn add_text_in_copied_font(doc: &mut Document, page: usize, t: &AddedText, src: &Document, src_page: usize, resource: &str) -> Result<(), EditError> {
    let font = page_font(src, src_page, resource).ok_or_else(|| EditError::Invalid("the copied text's font is missing".into()))?;
    let mut map = std::collections::HashMap::new();
    let copied = copy_value(doc, src, &font, &mut map, 0);
    let p = pdfcraft_model::pages(doc).into_iter().nth(page).ok_or(EditError::NoSuchPage(page))?;
    let mut resources = p.dict.get(b"Resources").and_then(|o| doc.dict(o)).unwrap_or_default();
    let mut fonts = resources.get(b"Font").and_then(|o| doc.dict(o)).unwrap_or_default();
    let name = (1..).map(|n| format!("SheafPaste{n}")).find(|n| !fonts.contains(n.as_bytes())).unwrap_or_default();
    fonts.set(name.as_bytes().to_vec(), copied);
    resources.set(b"Font".to_vec(), Object::Dict(fonts));
    doc.update_dict(p.obj, |d| d.set(b"Resources".to_vec(), Object::Dict(resources)))?;
    write_text(doc, page, t, &name)
}

/// `o` of `src` copied into `doc` with what it refers to (each object once; never up a /Parent).
fn copy_value(doc: &mut Document, src: &Document, o: &Object, map: &mut std::collections::HashMap<pdfcraft_cos::ObjRef, pdfcraft_cos::ObjRef>, depth: usize) -> Object {
    if depth > 32 {
        return Object::Null;
    }
    let dict = |doc: &mut Document, map: &mut std::collections::HashMap<_, _>, d: &Dict| {
        let mut out = Dict::new();
        for (k, v) in d.iter() {
            if k.as_slice() != b"Parent" {
                out.set(k.clone(), copy_value(doc, src, v, map, depth + 1));
            }
        }
        out
    };
    match o {
        Object::Ref(r) => {
            if let Some(n) = map.get(r) {
                return Object::Ref(*n);
            }
            let new = doc.add(Object::Null);
            map.insert(*r, new);
            let obj = src.get(*r);
            let copied = copy_value(doc, src, &obj, map, depth + 1);
            doc.set(new, copied);
            Object::Ref(new)
        }
        Object::Array(a) => Object::Array(a.iter().map(|x| copy_value(doc, src, x, map, depth + 1)).collect()),
        Object::Dict(d) => Object::Dict(dict(doc, map, d)),
        Object::Stream(s) => {
            let mut s = s.clone();
            s.dict = dict(doc, map, &s.dict);
            Object::Stream(s)
        }
        other => other.clone(),
    }
}
