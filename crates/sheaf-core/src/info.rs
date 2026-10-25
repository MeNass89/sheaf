//! The document information a PDF Sheaf makes carries (Acrobat's Create PDF writes the user's
//! name as Author).

use crate::pdfcraft::engine::Edit;
use crate::pdfcraft::Doc;
use crate::{pages::Input, Result};
use std::path::Path;

/// Writes `name` as the Author of the PDF at `path` when it has none (a converted Office file
/// keeps its own). Returns whether it was written; an empty `name` writes nothing.
pub fn set_author_if_empty(path: &Path, name: &str) -> Result<bool> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(false);
    }
    let mut doc = Doc::open(&Input::new(path))?;
    if doc.document().info.author.as_deref().is_some_and(|a| !a.trim().is_empty()) {
        return Ok(false);
    }
    doc.apply(Edit::SetInfo { key: "Author".into(), value: name.into() })?;
    let bytes = doc.full_bytes()?;
    drop(doc);
    let part = path.with_extension("pdf.part");
    std::fs::write(&part, bytes.as_slice())?;
    std::fs::rename(&part, path)?;
    Ok(true)
}
