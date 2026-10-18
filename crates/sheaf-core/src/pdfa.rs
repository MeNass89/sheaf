//! PDF to PDF/A: PdfCraft's converter, then its preflight check on the file written.

use crate::pdfcraft::engine::{pdfa, Edit};
use crate::pdfcraft::Doc;
use crate::{pages::Input, paths::atomic_write, Result};
use std::path::{Path, PathBuf};

/// The PDF/A conformance levels offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfALevel {
    /// PDF/A-2b: the recommended archival level.
    #[default]
    A2b,
    /// PDF/A-3b: like 2b, and allows attachments of any type.
    A3b,
}

impl PdfALevel {
    fn engine(self) -> pdfa::Level {
        match self {
            PdfALevel::A2b => pdfa::Level::A2b,
            PdfALevel::A3b => pdfa::Level::A3b,
        }
    }

    /// "PDF/A-2b" or "PDF/A-3b".
    pub fn label(self) -> &'static str {
        self.engine().label()
    }
}

/// A rule of the standard the output still breaks.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfAIssue {
    /// The ISO 19005 clause, e.g. "6.2.11.4".
    pub clause: String,
    pub message: String,
}

/// What [`to_pdfa`] wrote.
#[derive(Debug, Clone)]
pub struct PdfAReport {
    pub path: PathBuf,
    /// The input was password-protected (the output never is).
    pub was_encrypted: bool,
    /// What the check of the written file found; empty = conforms.
    pub issues: Vec<PdfAIssue>,
}

/// Converts `input` to PDF/A at `level` and writes `out` (never overwriting). The written
/// file is re-opened and checked against `level` before it gets its final name: when that
/// check cannot run, nothing is written.
pub fn to_pdfa(input: &Input, level: PdfALevel, out: &Path) -> Result<PdfAReport> {
    to_pdfa_checked(input, level, out, verify)
}

fn to_pdfa_checked(input: &Input, level: PdfALevel, out: &Path, check: impl FnOnce(&Input, PdfALevel) -> Result<Vec<PdfAIssue>>) -> Result<PdfAReport> {
    let mut doc = Doc::open(input)?;
    // Read before converting: PDF/A forbids encryption, so the conversion may drop it itself.
    let was_encrypted = doc.was_encrypted();
    doc.apply(Edit::ConvertPdfA { level: level.engine() })?;
    let (bytes, _) = doc.unprotected_bytes()?;
    drop(doc);
    let mut issues = Vec::new();
    let path = atomic_write(out, |part| {
        std::fs::write(part, bytes.as_slice())?;
        issues = check(&Input::new(part), level)?;
        Ok(())
    })?;
    Ok(PdfAReport { path, was_encrypted, issues })
}

/// The rules of `level` that `input` breaks, as PdfCraft's preflight sees them.
pub fn verify(input: &Input, level: PdfALevel) -> Result<Vec<PdfAIssue>> {
    let doc = Doc::open(input)?;
    Ok(doc.document().pdfa_verify(level.engine()).into_iter().map(|i| PdfAIssue { clause: i.clause.to_string(), message: i.message }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_that_cannot_run_writes_nothing() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("a.pdf");
        let mut d = lopdf::Document::with_version("1.7");
        let pages = d.new_object_id();
        let page = d.add_object(lopdf::dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()] });
        d.objects.insert(pages, lopdf::dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }.into());
        let root = d.add_object(lopdf::dictionary! { "Type" => "Catalog", "Pages" => pages });
        d.trailer.set("Root", root);
        d.save(&src).unwrap();
        let out = t.path().join("a-pdfa.pdf");
        let mut checked = None;
        let r = to_pdfa_checked(&Input::new(&src), PdfALevel::A2b, &out, |i, _| {
            checked = Some(i.path.clone());
            Err(crate::OpdfError::Engine("the written file could not be read".into()))
        });
        assert!(r.is_err());
        // The check read the file before it was published, and nothing is left behind.
        assert_ne!(checked.as_deref(), Some(out.as_path()));
        assert!(!out.exists());
        let left: Vec<_> = std::fs::read_dir(t.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["a.pdf"]);
        // A check that runs publishes the file.
        let r = to_pdfa(&Input::new(&src), PdfALevel::A2b, &out).unwrap();
        assert_eq!(r.path, out);
    }
}
