//! PDF to PDF/A: PdfCraft's converter, then its preflight check on the file written.

use crate::pdfcraft::engine::{pdfa, Edit};
use crate::pdfcraft::Doc;
use crate::{pages::Input, paths::atomic_write, Result};
use std::path::{Path, PathBuf};

#[path = "pdfa1.rs"]
mod part1;

/// The PDF/A conformance levels offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfALevel {
    /// PDF/A-1b: PDF 1.4 without transparency, layers or attachments.
    A1b,
    /// PDF/A-2b: the recommended archival level.
    #[default]
    A2b,
    /// PDF/A-3b: like 2b, and allows attachments of any type.
    A3b,
}

impl PdfALevel {
    fn engine(self) -> pdfa::Level {
        match self {
            PdfALevel::A1b | PdfALevel::A2b => pdfa::Level::A2b,
            PdfALevel::A3b => pdfa::Level::A3b,
        }
    }

    /// The requested archival identifier.
    pub fn label(self) -> &'static str {
        match self { Self::A1b => "PDF/A-1b", _ => self.engine().label() }
    }
}

/// The result line for a batch at `level` where `not_conforming` outputs still break rules, in
/// Sheaf's CLI and app alike.
pub fn summary(level: PdfALevel, not_conforming: usize) -> String {
    match not_conforming {
        0 => format!("Conforms to {}", level.label()),
        1 => format!("1 file does not fully conform to {}", level.label()),
        n => format!("{n} files do not fully conform to {}", level.label()),
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
    to_pdfa_with_policy(input, level, true, out)
}

/// Control incomplete archival copies: when false, unresolved built-in checks prevent
/// publication. This is a structural check, not a full ISO certification or level downgrade.
pub fn to_pdfa_with_policy(input: &Input, level: PdfALevel, allow_incomplete: bool, out: &Path) -> Result<PdfAReport> {
    to_pdfa_checked(input, level, allow_incomplete, out, verify)
}

fn to_pdfa_checked(input: &Input, level: PdfALevel, allow_incomplete: bool, out: &Path, check: impl FnOnce(&Input, PdfALevel) -> Result<Vec<PdfAIssue>>) -> Result<PdfAReport> {
    let mut doc = Doc::open(input)?;
    // Read before converting: PDF/A forbids encryption, so the conversion may drop it itself.
    let was_encrypted = doc.was_encrypted();
    doc.apply(Edit::ConvertPdfA { level: level.engine() })?;
    let (bytes, _) = doc.unprotected_bytes()?;
    drop(doc);
    let bytes = if level == PdfALevel::A1b { std::sync::Arc::new(part1::rewrite(bytes.as_slice())?) } else { bytes };
    let mut issues = Vec::new();
    let path = atomic_write(out, |part| {
        std::fs::write(part, &bytes)?;
        issues = check(&Input::new(part), level)?;
        if !allow_incomplete && !issues.is_empty() {
            let detail = issues.iter().map(|i| format!("{}: {}", i.clause, i.message)).collect::<Vec<_>>().join("\n");
            return Err(crate::OpdfError::Conversion(format!("{} did not pass the built-in checks; no output saved.\n{detail}", level.label())));
        }
        Ok(())
    })?;
    Ok(PdfAReport { path, was_encrypted, issues })
}

/// The rules of `level` that `input` breaks, as PdfCraft's preflight sees them.
pub fn verify(input: &Input, level: PdfALevel) -> Result<Vec<PdfAIssue>> {
    let doc = Doc::open(input)?;
    let mut issues: Vec<_> = doc.document().pdfa_verify(level.engine()).into_iter()
        // Part 1 has its own identifier check below; all other baseline checks still apply.
        .filter(|i| level != PdfALevel::A1b || i.pattern != "The metadata doesn't identify the file as {}")
        .map(|i| PdfAIssue { clause: i.clause.to_string(), message: i.message }).collect();
    if level == PdfALevel::A1b {
        let bytes = doc.full_bytes()?;
        issues.extend(part1::issues(&lopdf::Document::load_mem(bytes.as_slice())?)?);
    }
    Ok(issues)
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
        let r = to_pdfa_checked(&Input::new(&src), PdfALevel::A2b, true, &out, |i, _| {
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
