use crate::{pages::Input, paths::atomic_write, Result};
use std::path::{Path, PathBuf};

/// lopdf's reader rebuilds the xref by scanning objects when the table is broken;
/// pdfium then re-serialises a clean file. Falls back to the lopdf save if pdfium can't open it.
/// Like every tool but Protect, the output is not password-protected: a protected input's
/// pdfium copy is decrypted through lopdf before it is written.
pub fn repair(input: &Input, out: &Path) -> Result<PathBuf> {
    let _g = crate::engine::pdfium_lock();
    if let Ok(doc) = input.open() {
        if !crate::engine::is_protected(&doc) {
            return atomic_write(out, |part| Ok(doc.save_to_file(part)?));
        }
        let bytes = doc.save_to_bytes()?;
        let mut d = match &input.password {
            Some(pw) => lopdf::Document::load_mem_with_options(&bytes, lopdf::LoadOptions::with_password(pw))?,
            None => lopdf::Document::load_mem(&bytes)?,
        };
        d.max_id = d.objects.keys().map(|id| id.0).max().unwrap_or(0);
        return atomic_write(out, |part| { d.save(part)?; Ok(()) });
    }
    let mut doc = input.load_lopdf()?;
    doc.renumber_objects();
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}
