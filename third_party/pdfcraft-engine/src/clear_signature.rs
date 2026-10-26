//! Sheaf: a digital signature cleared from its field (Acrobat's Clear Signature, needed before
//! signing it again): the field stays, unsigned, its widgets drawn empty, and a certification the
//! signature made is withdrawn with it. Saved as an update, the signatures before stay valid.

use pdfcraft_cos::{Dict, Document, Object, Stream};
use pdfcraft_forms::{FieldKind, FormError};

pub fn clear_signature(doc: &mut Document, name: &str) -> Result<(), FormError> {
    let all = pdfcraft_forms::fields(doc);
    let f = all.iter().find(|f| f.name == name).ok_or_else(|| FormError::NoSuchField(name.into()))?;
    if f.kind != FieldKind::Signature {
        return Err(FormError::Invalid(format!("{name} is not a signature field")));
    }
    let value = doc.get(f.obj).as_dict().and_then(|d| d.get(b"V").cloned());
    let Some(value) = value.filter(|v| !matches!(v, Object::Null)) else {
        return Err(FormError::Invalid(format!("{name} is not signed")));
    };
    doc.update_dict(f.obj, |d| {
        d.remove(b"V");
    })
    .map_err(|e| FormError::Invalid(e.to_string()))?;
    for w in &f.widgets {
        let [x0, y0, x1, y1] = w.rect;
        let mut form = Dict::new();
        form.set(b"Type".to_vec(), Object::name("XObject"));
        form.set(b"Subtype".to_vec(), Object::name("Form"));
        form.set(b"BBox".to_vec(), vec![Object::Real(0.0), Object::Real(0.0), Object::Real((x1 - x0).abs()), Object::Real((y1 - y0).abs())]);
        let blank = doc.add(Stream::from_raw(form, Vec::new()));
        doc.update_dict(w.obj, |d| {
            let mut ap = Dict::new();
            ap.set(b"N".to_vec(), blank);
            d.set(b"AP".to_vec(), ap);
        })
        .map_err(|e| FormError::Invalid(e.to_string()))?;
    }
    // A certification (DocMDP) names the signature it came with: it goes with it.
    let root = doc.root().ok_or(FormError::NoForm)?;
    let perms = doc.get(root).as_dict().and_then(|d| d.get(b"Perms").cloned());
    if let Some(perms) = perms {
        let mut p = doc.resolve(&perms).as_dict().cloned().unwrap_or_default();
        if p.get(b"DocMDP") == Some(&value) {
            p.remove(b"DocMDP");
            let empty = p.is_empty();
            doc.update_dict(root, |d| {
                if empty {
                    d.remove(b"Perms");
                } else {
                    d.set(b"Perms".to_vec(), p);
                }
            })
            .map_err(|e| FormError::Invalid(e.to_string()))?;
        }
    }
    Ok(())
}
