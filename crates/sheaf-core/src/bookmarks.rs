//! A bookmark added at a page for automation, from the command line (`sheaf bookmark`) and the
//! MCP server, so a script or the system's automation can do what Preview's Shortcuts actions and
//! Acrobat's `bookmarkRoot.createChild` do (`sheaf info` lists them). The reader edits bookmarks
//! itself (PdfCraft's outline).

use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use lopdf::{dictionary, Document, Object, ObjectId};
use std::path::{Path, PathBuf};

/// The outline's root dictionary id, if the document has one.
fn outlines_id(doc: &Document) -> Option<ObjectId> {
    doc.catalog().ok()?.get(b"Outlines").ok()?.as_reference().ok()
}

/// A PDF text string: as is when it is plain ASCII, else UTF-16 with its byte-order mark.
fn text_string(s: &str) -> Object {
    if s.is_ascii() {
        return Object::string_literal(s);
    }
    let mut bytes = vec![0xFE, 0xFF];
    for u in s.encode_utf16() {
        bytes.extend_from_slice(&u.to_be_bytes());
    }
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}

/// Adds a bookmark `title` opening page `page` (1-based, the whole page in view) after the
/// last top-level one, and writes the result to `out`.
pub fn add(input: &Input, title: &str, page: u32, out: &Path) -> Result<PathBuf> {
    let title = title.trim();
    if title.is_empty() {
        return Err(OpdfError::Unsupported("a bookmark needs a title".into()));
    }
    let mut doc = input.load_lopdf()?;
    let pages = doc.get_pages();
    let Some(&page_id) = pages.get(&page) else {
        return Err(OpdfError::Unsupported(format!("page {page}: the document has {} pages", pages.len())));
    };
    let root = match outlines_id(&doc) {
        Some(id) if doc.get_dictionary(id).is_ok() => id,
        _ => {
            let id = doc.add_object(dictionary! { "Type" => "Outlines", "Count" => 0 });
            let cat = doc.trailer.get(b"Root").and_then(Object::as_reference)?;
            doc.get_dictionary_mut(cat)?.set("Outlines", Object::Reference(id));
            id
        }
    };
    let last = doc.get_dictionary(root)?.get(b"Last").and_then(Object::as_reference).ok();
    let mut item = dictionary! {
        "Title" => text_string(title),
        "Parent" => Object::Reference(root),
        "Dest" => vec![Object::Reference(page_id), Object::Name(b"Fit".to_vec())],
    };
    if let Some(prev) = last {
        item.set("Prev", Object::Reference(prev));
    }
    let id = doc.add_object(item);
    match last {
        Some(prev) => doc.get_dictionary_mut(prev)?.set("Next", Object::Reference(id)),
        None => doc.get_dictionary_mut(root)?.set("First", Object::Reference(id)),
    }
    let r = doc.get_dictionary_mut(root)?;
    r.set("Last", Object::Reference(id));
    // The root's /Count is how many items show; the new one is at the top, so it shows.
    let count = r.get(b"Count").and_then(Object::as_i64).unwrap_or(0).abs();
    r.set("Count", count + 1);
    atomic_write(out, |part| {
        doc.save(part)?;
        Ok(())
    })
}
