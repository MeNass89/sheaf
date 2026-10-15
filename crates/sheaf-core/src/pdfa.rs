//! PDF to PDF/A: PdfCraft's converter, then its preflight check on the file written.

use crate::pdfcraft::engine::{pdfa, Edit};
use crate::pdfcraft::Doc;
use crate::{pages::Input, Result};
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

/// Converts `input` to PDF/A at `level`, writes `out` (never overwriting), then re-opens the
/// written file and checks it against `level`.
pub fn to_pdfa(input: &Input, level: PdfALevel, out: &Path) -> Result<PdfAReport> {
    let mut doc = Doc::open(input)?;
    let was_encrypted = doc.was_encrypted();
    if was_encrypted {
        doc.apply(Edit::RemoveProtection)?;
    }
    doc.apply(Edit::ConvertPdfA { level: level.engine() })?;
    let path = doc.save(out)?;
    let issues = verify(&Input::new(&path), level)?;
    Ok(PdfAReport { path, was_encrypted, issues })
}

/// The rules of `level` that `input` breaks, as PdfCraft's preflight sees them.
pub fn verify(input: &Input, level: PdfALevel) -> Result<Vec<PdfAIssue>> {
    let doc = Doc::open(input)?;
    Ok(doc.document().pdfa_verify(level.engine()).into_iter().map(|i| PdfAIssue { clause: i.clause.to_string(), message: i.message }).collect())
}
