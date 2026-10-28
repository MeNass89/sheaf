//! PDF/E-1 (ISO 24517-1, Acrobat's Save as Other ▸ PDF/E): the engineering-document format. It is
//! PDF 1.6 with every font embedded, no encryption, no JavaScript and no external content, and it
//! keeps what drawings need (layers, transparency, 3D). Built on the PDF/A-2b conversion, which already
//! embeds fonts and removes encryption; the file then says it is PDF/E-1 in its XMP metadata
//! (`pdfe:ISO_PDFEVersion`) and declares PDF 1.6.

use crate::pages::Input;
use crate::pdfa::{PdfAIssue, PdfALevel};
use crate::paths::atomic_write;
use crate::{OpdfError, Result};
use lopdf::{Document, Object, Stream};
use std::path::{Path, PathBuf};

/// The XMP namespace ISO 24517-1 names for the identification.
const PDFE_NS: &str = "http://www.aiim.org/pdfe/ns/id/";

/// What [`to_pdfe`] wrote.
#[derive(Debug, Clone)]
pub struct PdfEReport {
    pub path: PathBuf,
    /// What the check of the written file found; empty = the built-in checks passed.
    pub issues: Vec<PdfAIssue>,
}

/// Converts `input` to PDF/E-1 and writes `out` (never overwriting). The written file is checked
/// before it gets its final name; with `allow_incomplete` false an unresolved check saves nothing.
pub fn to_pdfe(input: &Input, allow_incomplete: bool, out: &Path) -> Result<PdfEReport> {
    let tmp = tempfile::tempdir()?;
    let archived = crate::pdfa::to_pdfa_with_policy(input, PdfALevel::A2b, true, &tmp.path().join("a.pdf"))?;
    let bytes = identify(&std::fs::read(&archived.path)?)?;
    let mut issues = Vec::new();
    let path = atomic_write(out, |part| {
        std::fs::write(part, &bytes)?;
        issues = verify(&Input::new(part))?;
        if !allow_incomplete && !issues.is_empty() {
            let detail = issues.iter().map(|i| format!("{}: {}", i.clause, i.message)).collect::<Vec<_>>().join("\n");
            return Err(OpdfError::Conversion(format!("PDF/E-1 did not pass the built-in checks; no output saved.\n{detail}")));
        }
        Ok(())
    })?;
    Ok(PdfEReport { path, issues })
}

/// `bytes` (a PDF/A-2b file) declared PDF/E-1: the identification added to its XMP, PDF 1.6.
fn identify(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut pdf = Document::load_mem(bytes)?;
    let id = pdf.catalog()?.get(b"Metadata")?.as_reference()?;
    let stream = pdf.get_object(id)?.as_stream()?;
    let xmp = String::from_utf8(stream.get_plain_content()?).map_err(|_| OpdfError::Conversion("The metadata is not UTF-8".into()))?;
    let description = format!("<rdf:Description rdf:about=\"\" xmlns:pdfe=\"{PDFE_NS}\"><pdfe:ISO_PDFEVersion>PDF/E-1</pdfe:ISO_PDFEVersion></rdf:Description>");
    let xmp = match xmp.rfind("</rdf:RDF>") {
        Some(end) => format!("{}{description}{}", &xmp[..end], &xmp[end..]),
        None => return Err(OpdfError::Conversion("The metadata has no RDF to identify PDF/E in".into())),
    };
    let mut dict = stream.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    pdf.objects.insert(id, Object::Stream(Stream::new(dict, xmp.into_bytes())));
    pdf.version = "1.6".into();
    pdf.catalog_mut()?.remove(b"Version");
    let mut out = Vec::new();
    pdf.save_to(&mut out)?;
    Ok(out)
}

/// The PDF/E-1 rules `input` breaks: its identification, PDF 1.6 or earlier, no encryption, no
/// JavaScript, and the embedded-font and structure rules it shares with PDF/A-2b.
pub fn verify(input: &Input) -> Result<Vec<PdfAIssue>> {
    let issue = |clause: &str, message: &str| PdfAIssue { clause: clause.into(), message: message.into() };
    let mut issues: Vec<PdfAIssue> = crate::pdfa::verify(input, PdfALevel::A2b)?
        .into_iter()
        .filter(|i| !i.message.contains("PDF/A"))
        .collect();
    let bytes = std::fs::read(&input.path)?;
    let pdf = Document::load_mem(&bytes)?;
    if pdf.trailer.has(b"Encrypt") {
        issues.push(issue("ISO 24517-1 6.2", "The file is encrypted; PDF/E forbids encryption"));
    }
    if pdf.version.as_str() > "1.6" {
        issues.push(issue("ISO 24517-1 5", "The file declares a PDF version above 1.6"));
    }
    let has_js = pdf.objects.values().any(|o| {
        let d = match o { Object::Dictionary(d) => d, Object::Stream(s) => &s.dict, _ => return false };
        d.get(b"S").and_then(Object::as_name).is_ok_and(|n| n == b"JavaScript") || d.has(b"JS")
    });
    if has_js {
        issues.push(issue("ISO 24517-1 6.6", "The file holds JavaScript; PDF/E forbids it"));
    }
    let identified = pdf
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Metadata").ok()?.as_reference().ok())
        .and_then(|id| pdf.get_object(id).ok()?.as_stream().ok()?.get_plain_content().ok())
        .is_some_and(|x| String::from_utf8_lossy(&x).contains("<pdfe:ISO_PDFEVersion>PDF/E-1</pdfe:ISO_PDFEVersion>"));
    if !identified {
        issues.push(issue("ISO 24517-1 6.7", "The metadata does not identify the file as PDF/E-1"));
    }
    Ok(issues)
}
