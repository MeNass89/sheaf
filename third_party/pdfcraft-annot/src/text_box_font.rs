//! Sheaf: a text box set in a font of the computer's (an open-licensed full font program; PDF
//! Expert's and Acrobat's font list for text comments). The font is embedded and put into the
//! box's appearance, `/DA` names it, and the box is drawn in its codes (see
//! [`crate::encoded_text_font`]): a later size, colour or alignment keeps it.

use pdfcraft_cos::{Document, Object, PdfString};

use crate::{AnnotError, Meta, annot_dict, annot_ref, appearance, draw_appearance, n, redraw_or_restore, rich_text, touch, unlocked};

/// Text box `index` on `page` (0-based) set in the font program `font`, its size and colour
/// kept; refused when the font lacks a character of its text.
pub fn set_text_box_font_program(doc: &mut Document, page: usize, index: usize, font: &[u8], meta: &Meta) -> Result<(), AnnotError> {
    let (_, r) = annot_ref(doc, page, index)?;
    unlocked(doc, r)?;
    let d = annot_dict(doc, r);
    if d.name(b"Subtype") != Some(b"FreeText") {
        return Err(AnnotError::Invalid("only text boxes have a font".into()));
    }
    pdfcraft_fonts::embedding::validate_document_input(font).map_err(AnnotError::Invalid)?;
    let text = d.get(b"Contents").and_then(|o| o.as_string()).map(PdfString::to_text).unwrap_or_default();
    if !pdfcraft_fonts::embedding::covers(font, &text) {
        return Err(AnnotError::Invalid("this font lacks some of the text box's characters".into()));
    }
    let (_, postscript) = pdfcraft_fonts::embedding::names(font).map_err(AnnotError::Invalid)?;
    // An appearance to put the font in (a box without one gets PdfCraft's first).
    let has_normal = |doc: &Document| d.get(b"AP").is_some_and(|ap| doc.resolve(ap).as_dict().is_some_and(|ap| ap.reference(b"N").is_some()));
    if !has_normal(&*doc) {
        draw_appearance(doc, r)?;
    }
    let d = annot_dict(doc, r);
    let normal = d
        .get(b"AP")
        .map(|ap| doc.resolve(ap))
        .and_then(|ap| ap.as_dict().and_then(|ap| ap.reference(b"N")))
        .ok_or_else(|| AnnotError::Invalid("the text box has no appearance".into()))?;
    let Object::Stream(mut stream) = doc.get(normal).as_ref().clone() else {
        return Err(AnnotError::Invalid("the text box's appearance is not a stream".into()));
    };
    let mut res = stream.dict.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned()).unwrap_or_default();
    let mut fonts = res.get(b"Font").map(|f| doc.resolve(f)).and_then(|f| f.as_dict().cloned()).unwrap_or_default();
    let name = pdfcraft_fonts::embedding::embed(doc, &mut fonts, font, &postscript).map_err(AnnotError::Invalid)?;
    res.set(b"Font".to_vec(), Object::Dict(fonts));
    stream.dict.set(b"Resources".to_vec(), Object::Dict(res));
    doc.set(normal, Object::Stream(stream));
    let before = d;
    doc.update_dict(r, |d| {
        let ([red, green, blue], size) = appearance::parse_da(d);
        d.set(b"DA".to_vec(), PdfString::literal(format!("{} {} {} rg /{name} {} Tf", n(red), n(green), n(blue), n(size)).into_bytes()));
        rich_text(d);
        touch(d, meta);
    })?;
    redraw_or_restore(doc, r, before)
}
