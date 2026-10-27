//! iLovePDF's a/u profiles preserve existing tags and check the extra archival requirements.
//! They never invent reading order, language or alternative text to manufacture conformance.

use super::{PdfAIssue, unicode};
use crate::{pdfcraft::Doc, OpdfError, Result};
use crate::pdfcraft::engine::a11y::{Options, Rule, Status};
use lopdf::{Document, Object, Stream};

pub(super) fn rewrite(bytes: &[u8], conformance: &str) -> Result<Vec<u8>> {
    let mut pdf = Document::load_mem(bytes)?;
    let metadata = pdf.catalog()?.get(b"Metadata")?.as_reference()?;
    let stream = pdf.get_object(metadata)?.as_stream()?;
    let xmp = String::from_utf8(stream.get_plain_content()?).map_err(|_| OpdfError::Conversion("Archival metadata is not UTF-8".into()))?;
    let marker = "<pdfaid:conformance>B</pdfaid:conformance>";
    if !xmp.contains(marker) { return Err(OpdfError::Conversion("The archival converter did not produce the expected baseline identifier".into())); }
    let xmp = xmp.replace(marker, &format!("<pdfaid:conformance>{conformance}</pdfaid:conformance>"));
    let mut dict = stream.dict.clone();
    dict.remove(b"Filter"); dict.remove(b"DecodeParms");
    pdf.objects.insert(metadata, Object::Stream(Stream::new(dict, xmp.into_bytes())));
    let mut out = Vec::new();
    pdf.save_to(&mut out)?;
    Ok(out)
}

pub(super) fn issues(doc: &Doc, pdf: &Document, tagged: bool) -> Vec<PdfAIssue> {
    let mut issues = unicode::issues(pdf);
    if !tagged { return issues; }
    let rules = [Rule::TaggedPdf, Rule::PrimaryLanguage, Rule::TaggedContent, Rule::TaggedAnnotations,
        Rule::TaggedFormFields, Rule::FieldDescriptions, Rule::FiguresAltText, Rule::OtherElementsAltText,
        Rule::TableRows, Rule::TableCells, Rule::ListItems, Rule::LblLBody];
    let options = Options { rules: rules.into_iter().collect(), pages: None };
    let Some(report) = doc.document().accessibility_check(&options) else {
        issues.push(PdfAIssue { clause: "PDF/A-a logical structure".into(), message: "The existing tag tree could not be checked".into() });
        return issues;
    };
    for result in report.results.into_iter().filter(|r| r.status == Status::Failed) {
        for finding in result.findings {
            let page = finding.page.map_or_else(String::new, |p| format!("Page {}: ", p + 1));
            issues.push(PdfAIssue { clause: format!("PDF/A-a: {}", result.rule.name()), message: format!("{page}{}", finding.message) });
        }
    }
    issues
}
