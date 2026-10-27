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
    ops.push(Op::new("Tf", vec![Object::name(&resource), num(t.size)]));
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
