//! PDF portfolios (Acrobat's Create PDF Portfolio): a cover plus embedded original files,
//! not concatenated pages. Stirling's details view and attachment-pane fallback are preserved.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use lopdf::{dictionary, Object, Stream};
use crate::{pdfcraft::engine::Session, paths::atomic_write, OpdfError, Result};

fn text(value: &str) -> Object {
    let bytes: Vec<u8> = [0xFE, 0xFF].into_iter().chain(value.encode_utf16().flat_map(u16::to_be_bytes)).collect();
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}

fn unique_name(name: &str, used: &BTreeSet<String>) -> String {
    if !used.contains(name) { return name.into(); }
    let path = Path::new(name);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|i| format!("{stem} ({i}){ext}")).find(|n| !used.contains(n)).unwrap()
}

/// Bundle files verbatim. Duplicate basenames get distinct attachment names; neither inputs
/// nor an existing destination are replaced. PDF and non-PDF files can coexist in the bundle.
pub fn create(files: &[PathBuf], title: &str, out: &Path) -> Result<PathBuf> {
    if files.is_empty() { return Err(OpdfError::Unsupported("Choose at least one file for the portfolio".into())); }
    let title = if title.trim().is_empty() { "PDF Portfolio" } else { title.trim() };
    let body = format!("{title}\n\n{} files are included in this PDF Portfolio.\nOpen the Attachments panel to browse or save the original files.\n\n{}", files.len(), files.iter().map(|p| p.file_name().unwrap_or_default().to_string_lossy()).collect::<Vec<_>>().join("\n"));
    let bytes = Session::new().create_from_text(title, &body).map_err(|e| OpdfError::Engine(e.to_string()))?;
    let mut doc = lopdf::Document::load_mem(&bytes)?;
    let mut used = BTreeSet::new();
    let mut entries = Vec::new();
    for path in files {
        let name = path.file_name().ok_or_else(|| OpdfError::Unsupported("A portfolio item needs a file name".into()))?.to_string_lossy();
        let name = unique_name(&name, &used);
        used.insert(name.clone());
        let data = std::fs::read(path)?;
        let mut stream = Stream::new(dictionary! { "Type" => "EmbeddedFile", "Params" => dictionary! { "Size" => data.len() as i64 } }, data);
        stream.compress()?;
        let embedded = doc.add_object(stream);
        let spec = doc.add_object(dictionary! { "Type" => "Filespec", "F" => text(&name), "UF" => text(&name), "Desc" => text(&format!("Portfolio item: {name}")), "EF" => dictionary! { "F" => embedded, "UF" => embedded } });
        entries.push((name, spec));
    }
    let first = entries[0].0.clone();
    entries.sort_by_key(|(name, _)| name.encode_utf16().collect::<Vec<_>>());
    let names: Vec<Object> = entries.into_iter().flat_map(|(name, spec)| [text(&name), spec.into()]).collect();
    let tree = doc.add_object(dictionary! { "Names" => names });
    let field = |subtype: &str, name: &str, order: i64| dictionary! { "Type" => "CollectionField", "Subtype" => Object::Name(subtype.as_bytes().to_vec()), "N" => text(name), "O" => order, "V" => true };
    let schema = dictionary! { "Type" => "CollectionSchema", "Name" => field("F", "Name", 0), "Description" => field("Desc", "Description", 1), "Size" => field("Size", "Size", 2) };
    let catalog = doc.catalog_mut()?;
    catalog.set("Collection", dictionary! { "Type" => "Collection", "View" => "D", "D" => text(&first), "Schema" => schema });
    catalog.set("Names", dictionary! { "EmbeddedFiles" => tree });
    catalog.set("PageMode", Object::Name(b"UseAttachments".to_vec()));
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}
