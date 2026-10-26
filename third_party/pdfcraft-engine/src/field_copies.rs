//! Sheaf: a field copied in a grid of new fields of its own (Acrobat's Create Multiple Copies of
//! Fields): each copy is a separate field (its own name and value) with the original's type,
//! options, look, format and actions, placed side by side with it, down and across.

use pdfcraft_cos::{Document, ObjRef, Object, PdfString};
use pdfcraft_forms::FormError;

/// What a terminal field may take from its parents and must carry itself once it stands alone.
const INHERITED: [&[u8]; 7] = [b"FT", b"Ff", b"V", b"DV", b"DA", b"Q", b"Opt"];
/// A widget's own keys (the field's are taken whole).
const WIDGET_KEYS: [&[u8]; 9] = [b"Type", b"Subtype", b"AP", b"AS", b"MK", b"F", b"BS", b"Border", b"H"];

/// `rows` × `cols` copies of field `name`, the original in the top-left cell, each cell the
/// size of its first widget; copy (r, c) is named `name_r_c` (1-based; a free name if that one
/// is taken). Returns the new names.
pub fn copy_field(doc: &mut Document, name: &str, rows: usize, cols: usize) -> Result<Vec<String>, FormError> {
    let all = pdfcraft_forms::fields(doc);
    let f = all.iter().find(|f| f.name == name).ok_or_else(|| FormError::NoSuchField(name.into()))?;
    let w = f.widgets.first().ok_or_else(|| FormError::Invalid(format!("{name} has no widget")))?;
    let page = w.page.ok_or_else(|| FormError::Invalid(format!("{name} is on no page")))?;
    let pages = pdfcraft_annot::page_refs(doc).map_err(|e| FormError::Invalid(e.to_string()))?;
    let page_ref = *pages.get(page).ok_or_else(|| FormError::Invalid(format!("page {} does not exist", page + 1)))?;
    if rows * cols < 2 {
        return Err(FormError::Invalid("one copy at least".into()));
    }
    // The field standing alone: its own keys, those it inherits, its first widget's.
    let mut model = doc.get(f.obj).as_dict().cloned().unwrap_or_default();
    for k in [&b"Kids"[..], b"Parent", b"T", b"StructParent", b"P", b"Rect"] {
        model.remove(k);
    }
    let mut up = doc.get(f.obj).as_dict().and_then(|d| d.reference(b"Parent"));
    while let Some(p) = up {
        let pd = doc.get(p).as_dict().cloned().unwrap_or_default();
        for k in INHERITED {
            if model.get(k).is_none() {
                if let Some(v) = pd.get(k) {
                    model.set(k.to_vec(), v.clone());
                }
            }
        }
        up = pd.reference(b"Parent");
    }
    if w.obj != f.obj {
        let wd = doc.get(w.obj).as_dict().cloned().unwrap_or_default();
        for k in WIDGET_KEYS {
            if let Some(v) = wd.get(k) {
                model.set(k.to_vec(), v.clone());
            }
        }
    }
    let [x0, y0, x1, y1] = w.rect;
    let (cw, ch) = (x1 - x0, y1 - y0);
    let mut taken: Vec<String> = all.iter().map(|f| f.name.clone()).collect();
    let mut made = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if r == 0 && c == 0 {
                continue;
            }
            let base = format!("{}_{}_{}", name.rsplit('.').next().unwrap_or(name), r + 1, c + 1);
            let new = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}_{n}") }).find(|n| !taken.contains(n)).unwrap_or(base);
            let rect = [x0 + c as f64 * cw, y0 - r as f64 * ch, x1 + c as f64 * cw, y1 - r as f64 * ch];
            let mut d = model.clone();
            d.set(b"T".to_vec(), Object::String(PdfString::text(&new)));
            d.set(b"Rect".to_vec(), Object::Array(rect.iter().map(|v| Object::Real(*v)).collect()));
            d.set(b"P".to_vec(), Object::Ref(page_ref));
            let fr = doc.add(Object::Dict(d));
            push(doc, page_ref, b"Annots", fr)?;
            add_to_form(doc, fr)?;
            taken.push(new.clone());
            made.push(new);
        }
    }
    // Each copy drawn afresh: its appearance its own, not the original's shared streams.
    for n in &made {
        pdfcraft_forms::redraw_field(doc, n)?;
    }
    Ok(made)
}

/// `r` appended to the array under `key` in dictionary `owner` (inline or referenced).
fn push(doc: &mut Document, owner: ObjRef, key: &[u8], r: ObjRef) -> Result<(), FormError> {
    match doc.get(owner).as_dict().and_then(|d| d.get(key).cloned()) {
        Some(Object::Ref(ar)) => {
            let mut a = doc.get(ar).as_array().cloned().unwrap_or_default();
            a.push(Object::Ref(r));
            doc.set(ar, Object::Array(a));
        }
        other => {
            let mut a = other.and_then(|o| o.as_array().cloned()).unwrap_or_default();
            a.push(Object::Ref(r));
            doc.update_dict(owner, |d| d.set(key.to_vec(), Object::Array(a)))?;
        }
    }
    Ok(())
}

/// The form's `/Fields` gains `r` (the original field is there, so the form exists).
fn add_to_form(doc: &mut Document, r: ObjRef) -> Result<(), FormError> {
    let root = doc.root().ok_or_else(|| FormError::Invalid("no catalog".into()))?;
    match doc.get(root).as_dict().and_then(|d| d.get(b"AcroForm").cloned()) {
        Some(Object::Ref(af)) => push(doc, af, b"Fields", r),
        Some(Object::Dict(mut af)) => {
            let mut a = match af.get(b"Fields").cloned() {
                Some(Object::Ref(fr)) => {
                    let mut a = doc.get(fr).as_array().cloned().unwrap_or_default();
                    a.push(Object::Ref(r));
                    doc.set(fr, Object::Array(a));
                    return Ok(());
                }
                other => other.and_then(|o| o.as_array().cloned()).unwrap_or_default(),
            };
            a.push(Object::Ref(r));
            af.set(b"Fields".to_vec(), Object::Array(a));
            doc.update_dict(root, |d| d.set(b"AcroForm".to_vec(), Object::Dict(af)))?;
            Ok(())
        }
        _ => Err(FormError::Invalid("the document has no form".into())),
    }
}
