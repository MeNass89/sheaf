//! Acrobat's Preflight report: print checks on the current document, without changing it.

use crate::pdfcraft::Doc;
use crate::{paths::atomic_write, Result};
use std::path::{Path, PathBuf};

/// A report with its profile and source identified, suitable for keeping beside a print job.
/// The structural checks deliberately do not certify PDF/X or simulate ink separations.
pub fn report(doc: &Doc, name: &str, checked_at: &str) -> String {
    format!(
        "Sheaf print preflight\nDocument: {name}\nChecked: {checked_at}\nProfile: close-up colour print\nImage thresholds: fail below 150 ppi; warn below 300 ppi\nBleed: warn below 2 mm on any trim edge\n\n{}",
        doc.document().print_readiness().text()
    )
}

/// Write a complete report or leave no file. An existing report gets a free neighbouring name.
pub fn save_report(doc: &Doc, name: &str, checked_at: &str, out: &Path) -> Result<PathBuf> {
    let text = report(doc, name, checked_at);
    atomic_write(out, |part| {
        std::fs::write(part, text.as_bytes())?;
        Ok(())
    })
}
