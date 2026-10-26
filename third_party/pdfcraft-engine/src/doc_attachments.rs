//! Sheaf: document-level attachments (`/Names /EmbeddedFiles`) added, renamed and removed, as
//! Acrobat's Attachments pane does. Page-level ones are file attachment comments (DeleteAnnotation).
//! Only a flat name tree (a `/Names` array, what writers make) is edited; one with `/Kids` is refused.

use pdfcraft_cos::{Dict, Document, Object, ObjRef, PdfString, Stream};

/// What to do with the attachment named `key`.
#[derive(Clone, Debug, PartialEq)]
pub enum DocAttachment {
    /// Embed `bytes` under `key` (an attachment of that name is replaced).
    Add { bytes: std::sync::Arc<Vec<u8>> },
    Remove,
    /// The same file under another name.
    Rename { to: String },
}

/// Applies `action` to the document's attachment `key`.
pub fn apply(doc: &mut Document, key: &str, action: &DocAttachment) -> Result<(), String> {
    let root = doc.root().ok_or("the file has no catalog")?;
    let catalog = doc.get(root).as_dict().cloned().ok_or("the catalog is not a dictionary")?;
    // The /Names dictionary, made an object of its own when it is inline or missing.
    let names_ref = match catalog.get(b"Names") {
        Some(Object::Ref(r)) => *r,
        other => {
            let d = other.and_then(Object::as_dict).cloned().unwrap_or_default();
            let r = doc.add(Object::Dict(d));
            doc.update_dict(root, |c| c.set(b"Names".to_vec(), Object::Ref(r))).map_err(|e| e.to_string())?;
            r
        }
    };
    let names = doc.get(names_ref).as_dict().cloned().unwrap_or_default();
    let tree_ref = match names.get(b"EmbeddedFiles") {
        Some(Object::Ref(r)) => *r,
        other => {
            let d = other.and_then(Object::as_dict).cloned().unwrap_or_default();
            let r = doc.add(Object::Dict(d));
            doc.update_dict(names_ref, |n| n.set(b"EmbeddedFiles".to_vec(), Object::Ref(r))).map_err(|e| e.to_string())?;
            r
        }
    };
    let tree = doc.get(tree_ref).as_dict().cloned().unwrap_or_default();
    if tree.get(b"Kids").is_some() {
        return Err("this file's attachments are kept in a nested list Sheaf does not edit yet".into());
    }
    let mut pairs: Vec<(String, Object)> = tree
        .get(b"Names")
        .and_then(Object::as_array)
        .map(|a| a.chunks_exact(2).filter_map(|c| Some((c[0].as_string()?.to_text(), c[1].clone()))).collect())
        .unwrap_or_default();
    let at = pairs.iter().position(|(k, _)| k == key);
    match action {
        DocAttachment::Add { bytes } => {
            let spec = filespec(doc, key, bytes);
            match at {
                Some(i) => pairs[i].1 = Object::Ref(spec),
                None => pairs.push((key.to_string(), Object::Ref(spec))),
            }
        }
        DocAttachment::Remove => {
            let i = at.ok_or_else(|| format!("no attachment named {key}"))?;
            pairs.remove(i);
        }
        DocAttachment::Rename { to } => {
            let to = to.trim();
            if to.is_empty() {
                return Err("an attachment needs a name".into());
            }
            if pairs.iter().any(|(k, _)| k == to) {
                return Err(format!("an attachment is already named {to}"));
            }
            let i = at.ok_or_else(|| format!("no attachment named {key}"))?;
            pairs[i].0 = to.to_string();
            // The file specification carries the name the reader shows (/F, /UF).
            if let Object::Ref(r) = pairs[i].1 {
                doc.update_dict(r, |d| {
                    d.set(b"F".to_vec(), Object::String(PdfString::text(to)));
                    d.set(b"UF".to_vec(), Object::String(PdfString::text(to)));
                })
                .map_err(|e| e.to_string())?;
            }
        }
    }
    // Name trees are sorted by key.
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let array: Vec<Object> = pairs.into_iter().flat_map(|(k, v)| [Object::String(PdfString::text(&k)), v]).collect();
    doc.update_dict(tree_ref, |t| t.set(b"Names".to_vec(), Object::Array(array))).map_err(|e| e.to_string())
}

/// An embedded file stream and its file specification named `name`.
fn filespec(doc: &mut Document, name: &str, bytes: &[u8]) -> ObjRef {
    let mut params = Dict::new();
    params.set(b"Size".to_vec(), Object::Int(bytes.len() as i64));
    let mut sd = Dict::new();
    sd.set(b"Type".to_vec(), Object::name("EmbeddedFile"));
    sd.set(b"Params".to_vec(), Object::Dict(params));
    let file = doc.add(Object::Stream(Stream::flate(sd, bytes)));
    let mut ef = Dict::new();
    ef.set(b"F".to_vec(), Object::Ref(file));
    ef.set(b"UF".to_vec(), Object::Ref(file));
    let mut spec = Dict::new();
    spec.set(b"Type".to_vec(), Object::name("Filespec"));
    spec.set(b"F".to_vec(), Object::String(PdfString::text(name)));
    spec.set(b"UF".to_vec(), Object::String(PdfString::text(name)));
    spec.set(b"EF".to_vec(), Object::Dict(ef));
    doc.add(Object::Dict(spec))
}
