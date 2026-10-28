//! Sheaf: a document time stamp (Acrobat's Time Stamp Document; ISO 32000-2 §12.8.5, PAdES
//! ETSI EN 319 142-1 §5.4) written as the value of an invisible signature field, as Acrobat
//! writes it. Upstream's `pdfcraft_sign::timestamp_document` adds the `/DocTimeStamp`
//! dictionary without a field: PdfCraft finds it by scanning, but poppler (`pdfsig`) and
//! Acrobat list signatures from the form's fields and did not see it.

use std::collections::HashSet;

use pdfcraft_cos::{Dict, Document, Object, PdfString, SaveOptions};
use pdfcraft_sign::{DigestAlg, SignError, TimestampAuthority, TimestampQuery};

/// Room for the token in `/Contents`: a TSA's token with its certificate chain is 2–8 KB.
const TOKEN_RESERVE: usize = 16 * 1024;
/// The `/ByteRange` placeholder, patched once the file is written (as pdfcraft-sign's).
const BR_MARK: [i64; 3] = [1_111_111_111, 2_222_222_222, 3_333_333_333];

/// The document as it is, with a document time stamp appended as an incremental update. The
/// request goes through `tsa`; a refused, malformed or mismatched reply produces no file.
pub fn timestamp_document(doc: &Document, tsa: &dyn TimestampAuthority, date: &str) -> Result<Vec<u8>, SignError> {
    // A pending security change would make the incremental save a full rewrite.
    if doc.encryption_changed() {
        return Err(SignError::Pdf("save the document's new security settings before time stamping it".into()));
    }
    if doc.permissions().is_some_and(|p| !p.fill_forms()) {
        return Err(SignError::Pdf("the document's security settings don't allow signing; open it with the permissions password to time stamp it".into()));
    }
    let mut doc = doc.clone();
    let root = doc.root().ok_or_else(|| SignError::Pdf("the document has no catalog".into()))?;
    let mut v = Dict::new();
    v.set(b"Type".to_vec(), Object::name("DocTimeStamp"));
    v.set(b"Filter".to_vec(), Object::name("Adobe.PPKLite"));
    v.set(b"SubFilter".to_vec(), Object::name("ETSI.RFC3161"));
    v.set(b"ByteRange".to_vec(), Object::Array([0].iter().chain(BR_MARK.iter()).map(|n| Object::Int(*n)).collect()));
    v.set(b"Contents".to_vec(), Object::String(PdfString { bytes: vec![0; TOKEN_RESERVE], hex: true }));
    let sig = doc.add(Object::Dict(v));
    // The invisible field holding it, a widget on the first page (Print + Locked, empty Rect).
    let pages = pdfcraft_annot::page_refs(&doc).map_err(|e| SignError::Pdf(e.to_string()))?;
    let page = *pages.first().ok_or_else(|| SignError::Pdf("the document has no pages".into()))?;
    let taken: HashSet<String> = pdfcraft_forms::fields(&doc).into_iter().map(|f| f.name).collect();
    let name = (1..).map(|i| format!("Timestamp{i}")).find(|n| !taken.contains(n)).unwrap_or_default();
    let mut w = Dict::new();
    w.set(b"Type".to_vec(), Object::name("Annot"));
    w.set(b"Subtype".to_vec(), Object::name("Widget"));
    w.set(b"FT".to_vec(), Object::name("Sig"));
    w.set(b"T".to_vec(), PdfString::text(&name));
    w.set(b"V".to_vec(), Object::Ref(sig));
    w.set(b"Rect".to_vec(), Object::Array(vec![Object::Int(0); 4]));
    w.set(b"P".to_vec(), Object::Ref(page));
    w.set(b"F".to_vec(), Object::Int(4 | 128));
    let wr = doc.add(Object::Dict(w));
    let pd = doc.get(page).as_dict().cloned().unwrap_or_default();
    match pd.get(b"Annots") {
        Some(Object::Ref(ar)) => {
            let mut a = doc.get(*ar).as_array().cloned().unwrap_or_default();
            a.push(Object::Ref(wr));
            doc.set(*ar, Object::Array(a));
        }
        other => {
            let mut a = other.and_then(|o| o.as_array().cloned()).unwrap_or_default();
            a.push(Object::Ref(wr));
            doc.update_dict(page, |d| d.set(b"Annots".to_vec(), Object::Array(a)))?;
        }
    }
    let catalog = doc.get(root).as_dict().cloned().unwrap_or_default();
    let (af_ref, mut af) = match catalog.get(b"AcroForm") {
        Some(Object::Ref(r)) => (Some(*r), doc.get(*r).as_dict().cloned().unwrap_or_default()),
        Some(Object::Dict(d)) => (None, d.clone()),
        _ => (None, Dict::new()),
    };
    match af.get(b"Fields") {
        Some(Object::Ref(r)) => {
            let r = *r;
            let mut fields = doc.get(r).as_array().cloned().unwrap_or_default();
            fields.push(Object::Ref(wr));
            doc.set(r, Object::Array(fields));
        }
        other => {
            let mut fields = other.and_then(|o| o.as_array().cloned()).unwrap_or_default();
            fields.push(Object::Ref(wr));
            af.set(b"Fields".to_vec(), Object::Array(fields));
        }
    }
    af.set(b"SigFlags".to_vec(), Object::Int(3));
    match af_ref {
        Some(r) => doc.set(r, Object::Dict(af)),
        None => doc.update_dict(root, |c| c.set(b"AcroForm".to_vec(), Object::Dict(af)))?,
    }
    let save = SaveOptions { mod_date: Some(date.to_string()), object_streams: false, ..SaveOptions::default() };
    let mut out = pdfcraft_cos::write_incremental(&doc, &save)?;
    let (br_at, (start, end)) = locate(&out)?;
    let mark = format!("0 {} {} {}", BR_MARK[0], BR_MARK[1], BR_MARK[2]);
    let mut br_text = format!("0 {start} {end} {}", out.len() - end).into_bytes();
    if br_text.len() > mark.len() {
        return Err(SignError::Pdf("the document is too large to time stamp".into()));
    }
    br_text.resize(mark.len(), b' ');
    out.get_mut(br_at..br_at + mark.len()).ok_or_else(|| SignError::Pdf("ByteRange placeholder not found".into()))?.copy_from_slice(&br_text);
    let q = TimestampQuery::new(DigestAlg::Sha256, DigestAlg::Sha256.digest(&[&out[..start], &out[end..]]))?;
    let reply = tsa.timestamp(&q.encode()?)?;
    let token = pdfcraft_sign::timestamp::parse_response(&reply, &q)?;
    let hex: Vec<u8> = token.raw.iter().flat_map(|b| format!("{b:02X}").into_bytes()).collect();
    if hex.len() > TOKEN_RESERVE * 2 {
        return Err(SignError::Pdf("the time stamp token is larger than the room kept for it".into()));
    }
    out[start + 1..start + 1 + hex.len()].copy_from_slice(&hex);
    Ok(out)
}

/// The ByteRange numbers' offset and the `<…>` placeholder's span in the written update.
fn locate(out: &[u8]) -> Result<(usize, (usize, usize)), SignError> {
    let mark = format!("0 {} {} {}", BR_MARK[0], BR_MARK[1], BR_MARK[2]);
    let hits: Vec<usize> = out.windows(mark.len()).enumerate().filter(|(_, w)| *w == mark.as_bytes()).map(|(i, _)| i).collect();
    let [br] = hits.as_slice() else { return Err(SignError::Pdf("ByteRange placeholder not found".into())) };
    let start = out[..*br].windows(3).rposition(|w| w == b"obj").ok_or_else(|| SignError::Pdf("time stamp object not found".into()))?;
    let end = br + out[*br..].windows(6).position(|w| w == b"endobj").ok_or_else(|| SignError::Pdf("unterminated time stamp object".into()))?;
    let mut zeros = vec![b'<'];
    zeros.extend(std::iter::repeat_n(b'0', TOKEN_RESERVE * 2));
    zeros.push(b'>');
    let c = out[start..end].windows(zeros.len()).position(|w| w == zeros.as_slice()).ok_or_else(|| SignError::Pdf("Contents placeholder not found".into()))?;
    Ok((*br, (start + c, start + c + zeros.len())))
}
