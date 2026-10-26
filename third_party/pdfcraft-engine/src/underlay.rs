//! Sheaf: another PDF's page drawn behind a page (Acrobat's Underlay, PDF Expert's Overlay ▸
//! Behind): a form drawn first in the page's content, filling its crop box, so the page's own
//! content and comments stay on top (Sol, 26/10: only an overlay in front ran page by page).

use pdfcraft_cos::{Dict, Document, ObjRef, Object, Stream};

use crate::EditError;

/// Form `form` (drawn in the unit square) under everything on page `page_ref`.
pub fn underlay(doc: &mut Document, page_ref: ObjRef, form: ObjRef) -> Result<(), EditError> {
    let page = doc.get(page_ref).as_dict().cloned().ok_or_else(|| EditError::Invalid("the page can't be read".into()))?;
    let boxes = |key: &[u8]| page.get(key).map(|b| doc.resolve(b)).and_then(|b| b.as_array().map(|a| a.iter().filter_map(Object::as_f64).collect::<Vec<_>>())).filter(|b| b.len() == 4);
    let b = boxes(b"CropBox").or_else(|| boxes(b"MediaBox")).unwrap_or_else(|| vec![0.0, 0.0, 612.0, 792.0]);
    let (x0, y0, w, h) = (b[0].min(b[2]), b[1].min(b[3]), (b[2] - b[0]).abs(), (b[3] - b[1]).abs());
    // The page's resources (its own, else its parents'), as a copy of its own with the form.
    let mut res = page.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned());
    let mut up = page.reference(b"Parent");
    while res.is_none() {
        let Some(p) = up else { break };
        let pd = doc.get(p).as_dict().cloned().unwrap_or_default();
        res = pd.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned());
        up = pd.reference(b"Parent");
    }
    let mut res = res.unwrap_or_default();
    let mut xobjects = res.get(b"XObject").map(|x| doc.resolve(x)).and_then(|x| x.as_dict().cloned()).unwrap_or_default();
    let mut k = 0;
    let name = loop {
        let n = format!("SheafUnder{k}");
        if xobjects.get(n.as_bytes()).is_none() {
            break n;
        }
        k += 1;
    };
    xobjects.set(name.as_bytes().to_vec(), form);
    res.set(b"XObject".to_vec(), Object::Dict(xobjects));
    let first = doc.add(Stream::from_raw(Dict::new(), format!("q {w} 0 0 {h} {x0} {y0} cm /{name} Do Q\n").into_bytes()));
    let contents = match page.get(b"Contents") {
        Some(Object::Array(a)) => std::iter::once(Object::Ref(first)).chain(a.iter().cloned()).collect(),
        Some(Object::Ref(r)) => match &*doc.get(*r) {
            Object::Array(a) => std::iter::once(Object::Ref(first)).chain(a.iter().cloned()).collect(),
            _ => vec![Object::Ref(first), Object::Ref(*r)],
        },
        _ => vec![Object::Ref(first)],
    };
    doc.update_dict(page_ref, |d| {
        d.set(b"Resources".to_vec(), Object::Dict(res));
        d.set(b"Contents".to_vec(), Object::Array(contents));
    })
    .map_err(|e| EditError::Write(e.to_string()))
}
