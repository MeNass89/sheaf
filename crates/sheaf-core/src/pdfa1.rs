//! Extra restrictions for Acrobat/iLovePDF's older PDF/A-1b archival target.
//! Reuse PdfCraft's repairs, then write PDF 1.4 and inspect features forbidden in part 1.
//! This is a conservative structural check, not an ISO conformance certificate.

use crate::{pdfa::PdfAIssue, OpdfError, Result};
use lopdf::{Document, Object, Stream};

pub(super) fn rewrite(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut pdf = Document::load_mem(bytes)?;
    let id = pdf.catalog()?.get(b"Metadata")?.as_reference()?;
    let stream = pdf.get_object(id)?.as_stream()?;
    let xmp = String::from_utf8(stream.get_plain_content()?).map_err(|_| OpdfError::Conversion("The archival metadata is not UTF-8".into()))?;
    let xmp = xmp.replace("<pdfaid:part>2</pdfaid:part>", "<pdfaid:part>1</pdfaid:part>");
    let mut dict = stream.dict.clone();
    dict.remove(b"Filter");
    dict.remove(b"DecodeParms");
    pdf.objects.insert(id, Object::Stream(Stream::new(dict, xmp.into_bytes())));
    pdf.version = "1.4".into();
    pdf.catalog_mut()?.remove(b"Version");
    // lopdf expands object streams while reading. The rewritten file uses a classic xref table.
    pdf.objects.retain(|_, o| !o.type_name().is_ok_and(|n| n == b"ObjStm" || n == b"XRef"));
    pdf.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    pdf.trailer.remove(b"Prev");
    pdf.trailer.remove(b"XRefStm");
    let mut out = Vec::new();
    pdf.save_to(&mut out)?;
    Ok(out)
}

pub(super) fn issues(pdf: &Document, identified: bool) -> Result<Vec<PdfAIssue>> {
    let mut problems = std::collections::BTreeSet::new();
    if pdf.version != "1.4" { problems.insert("The PDF/A-1 file must use PDF 1.4".to_string()); }
    if !matches!(pdf.reference_table.cross_reference_type, lopdf::xref::XrefType::CrossReferenceTable) {
        problems.insert("Cross-reference streams are not allowed in PDF/A-1".into());
    }
    let cat = pdf.catalog()?;
    if cat.get(b"Version").is_ok_and(|v| v.as_name().is_ok_and(|v| v > b"1.4".as_slice())) {
        problems.insert("The catalog declares a newer PDF version than PDF/A-1 permits".into());
    }
    if !identified { problems.insert("The metadata does not identify PDF/A-1b".into()); }
    for object in pdf.objects.values() { inspect(pdf, object, 0, &mut problems); }
    Ok(problems.into_iter().map(|message| PdfAIssue { clause: "PDF/A-1 additional restrictions".into(), message }).collect())
}

fn inspect(pdf: &Document, object: &Object, depth: usize, problems: &mut std::collections::BTreeSet<String>) {
    if depth > 64 { problems.insert("An object is too deeply nested to check PDF/A-1 safely".into()); return; }
    let dict = match object {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &s.dict,
        Object::Array(a) => { for o in a { inspect(pdf, o, depth + 1, problems); } return; }
        _ => return,
    };
    let value = |key: &[u8]| dict.get(key).ok().and_then(|v| pdf.dereference(v).ok().map(|(_, v)| v));
    let name = |key: &[u8], value: &[u8]| dict.get(key).ok().and_then(|v| pdf.dereference(v).ok().map(|(_, v)| v)).is_some_and(|v| v.as_name().is_ok_and(|n| n == value));
    if dict.has(b"OCProperties") || dict.has(b"OC") || name(b"Type", b"OCG") {
        problems.insert("Optional content (layers) is not allowed in PDF/A-1; choose PDF/A-2b to preserve it".into());
    }
    if name(b"Type", b"EmbeddedFile") || dict.has(b"EF") || dict.has(b"EmbeddedFiles") {
        problems.insert("Attachments are not allowed in PDF/A-1; choose PDF/A-3b to preserve them".into());
    }
    let alpha = [b"ca".as_slice(), b"CA".as_slice()].into_iter().any(|k| dict.has(k) && !value(k).is_some_and(|v| v.as_float().is_ok_and(|n| n == 1.0)));
    let mask = dict.has(b"SMask") && !value(b"SMask").is_some_and(|v| matches!(v, Object::Name(n) if n == b"None"));
    let blend = dict.has(b"BM") && !value(b"BM").is_some_and(|v| matches!(v, Object::Name(n) if n == b"Normal" || n == b"Compatible"));
    if alpha || mask || blend || name(b"S", b"Transparency") {
        problems.insert("Transparency is not allowed in PDF/A-1; choose PDF/A-2b to preserve it (flattening is not performed)".into());
    }
    if dict.has(b"SMaskInData") || name(b"Subtype", b"OpenType") || name(b"Type", b"ObjStm") || name(b"Type", b"XRef") {
        problems.insert("A PDF 1.5+ image or font feature cannot be preserved in PDF/A-1".into());
    }
    if let Some(filters) = value(b"Filter") {
        let forbidden = |v: &Object| pdf.dereference(v).is_ok_and(|(_, v)| v.as_name().is_ok_and(|n| matches!(n, b"JPXDecode" | b"JBIG2Decode" | b"Crypt")));
        if forbidden(filters) || filters.as_array().is_ok_and(|a| a.iter().any(forbidden)) {
            problems.insert("An image or stream uses compression unsupported by this PDF/A-1 converter".into());
        }
    }
    for (_, value) in dict { inspect(pdf, value, depth + 1, problems); }
}
