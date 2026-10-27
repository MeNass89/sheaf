//! PDF to PDF/A: PdfCraft's converter, then its preflight check on the file written.

use crate::pdfcraft::engine::{pdfa, Edit};
use crate::pdfcraft::Doc;
use crate::{pages::Input, paths::atomic_write, Result};
use std::path::{Path, PathBuf};

#[path = "pdfa1.rs"]
mod part1;
#[path = "pdfa_unicode.rs"]
mod unicode;
#[path = "pdfa_profiles.rs"]
mod profiles;

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
    /// PDF/A-2u: 2b plus Unicode mapping for every text code.
    A2u,
    /// PDF/A-3u: 3b plus Unicode mapping for every text code.
    A3u,
    /// PDF/A-1a: 1b plus Unicode and an existing logical tag structure.
    A1a,
    /// PDF/A-2a: 2b plus Unicode and an existing logical tag structure.
    A2a,
    /// PDF/A-3a: 3b plus Unicode and an existing logical tag structure.
    A3a,
}

impl PdfALevel {
    /// iLovePDF's complete conformance-level inventory, in its version order.
    pub const ALL: [Self; 8] = [Self::A1b, Self::A1a, Self::A2b, Self::A2u, Self::A2a, Self::A3b, Self::A3u, Self::A3a];

    fn engine(self) -> pdfa::Level {
        if self.part() == 3 { pdfa::Level::A3b } else { pdfa::Level::A2b }
    }

    pub fn part(self) -> u8 {
        match self { Self::A1b | Self::A1a => 1, Self::A3b | Self::A3u | Self::A3a => 3, _ => 2 }
    }

    pub fn conformance(self) -> &'static str {
        match self { Self::A1a | Self::A2a | Self::A3a => "A", Self::A2u | Self::A3u => "U", _ => "B" }
    }

    /// The requested archival identifier; no level is silently downgraded.
    pub fn label(self) -> &'static str {
        match self {
            Self::A1b => "PDF/A-1b", Self::A1a => "PDF/A-1a",
            Self::A2b => "PDF/A-2b", Self::A2u => "PDF/A-2u", Self::A2a => "PDF/A-2a",
            Self::A3b => "PDF/A-3b", Self::A3u => "PDF/A-3u", Self::A3a => "PDF/A-3a",
        }
    }
}

/// The same check result in the CLI and app. An unsupported check is not proof that the
/// file violates ISO, and passing our structural checks is not a full ISO certificate.
pub fn summary(level: PdfALevel, not_passed: usize) -> String {
    match not_passed {
        0 => format!("Checks passed for {}", level.label()),
        1 => format!("1 file did not pass {} checks", level.label()),
        n => format!("{n} files did not pass {} checks", level.label()),
    }
}

/// A failed or unsupported built-in archival check.
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
    /// What the check of the written file found; empty = built-in checks passed.
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
    let bytes = if level.part() == 1 { std::sync::Arc::new(part1::rewrite(bytes.as_slice())?) } else { bytes };
    let bytes = if level.conformance() != "B" { std::sync::Arc::new(profiles::rewrite(bytes.as_slice(), level.conformance())?) } else { bytes };
    let mut issues = Vec::new();
    let path = atomic_write(out, |part| {
        std::fs::write(part, bytes.as_slice())?;
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
    let extended = level.part() == 1 || level.conformance() != "B";
    let mut issues: Vec<_> = doc.document().pdfa_verify(level.engine()).into_iter()
        // The baseline b-level checker still checks every visual/archive rule. Identification
        // is checked against the actual selected part/conformance below for a/u and part 1.
        .filter(|i| !extended || i.pattern != "The metadata doesn't identify the file as {}")
        .map(|i| PdfAIssue { clause: i.clause.to_string(), message: i.message }).collect();
    if extended {
        let pdf = lopdf::Document::load(&input.path)?;
        let identified = doc.document().standards().pdfa.is_some_and(|(part, conformance)| part == level.part() && conformance == level.conformance());
        if !identified { issues.push(PdfAIssue { clause: "PDF/A identification".into(), message: format!("The metadata does not identify {}", level.label()) }); }
        if level.part() == 1 { issues.extend(part1::issues(&pdf, true)?); }
        if level.conformance() != "B" { issues.extend(profiles::issues(&doc, &pdf, level.conformance() == "A")); }
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
