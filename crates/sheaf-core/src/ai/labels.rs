//! Stirling's Classify keeps each document's category in the PDF itself: the label is written in
//! the document information (`/Category`, a custom property Acrobat lists under Document
//! Properties ▸ Custom, and added to `/Keywords` so file managers and searches find it), as an
//! incremental update: the original bytes stay as they were, so signatures remain valid.

use crate::pages::Input;
use crate::pdfcraft::engine::Edit;
use crate::pdfcraft::Doc;
use crate::Result;
use std::path::Path;

/// The category label written in the PDF at `path` ("Invoice"), if any.
pub fn read(path: &Path) -> Option<String> {
    let doc = lopdf::Document::load(path).ok()?;
    let info = doc.trailer.get(b"Info").ok()?;
    let info = match info {
        lopdf::Object::Reference(r) => doc.get_dictionary(*r).ok()?,
        lopdf::Object::Dictionary(d) => d,
        _ => return None,
    };
    let v = info.get(b"Category").ok()?;
    let bytes = v.as_str().ok()?;
    let text = if bytes.starts_with(&[0xFE, 0xFF]) {
        String::from_utf16_lossy(&bytes[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

/// Writes `label` as the category of the PDF at `path`, in place, as an incremental update.
pub fn write(path: &Path, label: &str) -> Result<()> {
    let mut doc = Doc::open(&Input::new(path))?;
    let mut keywords = doc.document().info.keywords.clone().unwrap_or_default();
    let tag = format!("Category: {label}");
    // A former label is replaced in the keywords, not stacked.
    let kept: Vec<&str> = keywords.split(';').map(str::trim).filter(|k| !k.is_empty() && !k.starts_with("Category: ")).collect();
    keywords = kept.into_iter().chain(std::iter::once(tag.as_str())).collect::<Vec<_>>().join("; ");
    doc.apply(Edit::Batch {
        label: "Classify".into(),
        edits: vec![Edit::SetInfo { key: "Category".into(), value: label.into() }, Edit::SetInfo { key: "Keywords".into(), value: keywords }],
    })?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp = tempfile::Builder::new().prefix(".sheaf-label-").suffix(".pdf").tempfile_in(dir)?.into_temp_path();
    let tmp_path = tmp.to_path_buf();
    tmp.close()?;
    let written = doc.save_incremental(&tmp_path)?;
    drop(doc);
    std::fs::rename(&written, path)?;
    Ok(())
}
