//! Sheaf: revocation evidence a signature carries inside itself. Acrobat embeds the signer's CRLs
//! and OCSP responses in the CMS as the signed attribute `adbe-revocationInfoArchival`
//! (1.2.840.113583.1.1.8) and validates them offline, as it does the catalog's `/DSS`, which
//! PdfCraft already checks. Nothing is fetched.

use pdfcraft_sign::der::{tag, Tlv};
use pdfcraft_sign::pdf::{Detail, Fill};
use pdfcraft_sign::revocation::{CertificateList, OcspResponse, RevocationStatus};
use pdfcraft_sign::{SignatureInfo, Status};

const ARCHIVAL: &str = "1.2.840.113583.1.1.8";
const SIGNED_DATA: &str = "1.2.840.113549.1.7.2";

/// The CRLs and OCSP responses in the first signer's `adbe-revocationInfoArchival` attribute.
pub fn evidence(cms: &[u8]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    archive(cms).unwrap_or_default()
}

fn archive(cms: &[u8]) -> Option<(Vec<Vec<u8>>, Vec<Vec<u8>>)> {
    let (content_info, _) = Tlv::parse_ber(cms).ok()?;
    let ci = content_info.children().ok()?;
    if ci.first()?.oid().ok()? != SIGNED_DATA {
        return None;
    }
    let signed_data = ci.get(1)?.inner().ok()?.children().ok()?;
    // SignedData: version, digestAlgorithms, encapContentInfo, [0] certificates?, [1] crls?, signerInfos.
    let signer = signed_data.last()?.children().ok()?.into_iter().next()?;
    let attrs = signer.children().ok()?.into_iter().find(|t| t.tag == tag::ctx(0))?;
    let mut crls = Vec::new();
    let mut ocsps = Vec::new();
    for attr in attrs.children().ok()? {
        let parts = attr.children().ok()?;
        if parts.first().and_then(|o| o.oid().ok()).as_deref() != Some(ARCHIVAL) {
            continue;
        }
        for value in parts.get(1)?.children().ok()? {
            for item in value.children().ok()? {
                // [0] CRLs, [1] OCSP responses, each a SEQUENCE OF; [2] other kinds are skipped.
                let list = item.inner().ok().and_then(|s| s.children().ok()).unwrap_or_default();
                match item.tag {
                    t if t == tag::ctx(0) => crls.extend(list.iter().map(|c| c.raw.to_vec())),
                    t if t == tag::ctx(1) => ocsps.extend(list.iter().map(|c| c.raw.to_vec())),
                    _ => {}
                }
            }
        }
    }
    Some((crls, ocsps))
}

/// Judge `info` with the evidence its own CMS carries: a verified revocation makes it invalid
/// (Acrobat's verdict), a covering "good" answer is said; anything else leaves it as it was.
pub fn judge(info: &mut SignatureInfo, crls: &[Vec<u8>], ocsps: &[Vec<u8>]) {
    let (Some(signer), Some(at)) = (info.chain.first().or(info.certificate.as_ref()).cloned(), info.timestamp_time.or(info.signing_time)) else { return };
    let issuer = info.chain.get(1).cloned().unwrap_or_else(|| signer.clone());
    let mut covered = false;
    let checks = crls
        .iter()
        .filter_map(|c| CertificateList::parse(c).ok().map(|l| ("CRL", l.check(&signer, &issuer, at))))
        .chain(ocsps.iter().filter_map(|o| OcspResponse::parse(o).ok().map(|r| ("OCSP", r.check(&signer, &issuer, at)))));
    for (source, status) in checks {
        match status {
            RevocationStatus::Revoked { at } => {
                info.status = Status::Invalid;
                let pattern = "The signer's certificate has been revoked ({}).";
                let value = format!("{source} in the signature, {at}");
                info.details.push(Detail { message: pattern.replacen("{}", &value, 1), pattern, fill: Fill::Value(value) });
                return;
            }
            RevocationStatus::Good => covered = true,
            RevocationStatus::Unknown => {}
        }
    }
    let good = "The document's embedded revocation information shows that the signer's certificate has not been revoked.";
    if covered && !info.details.iter().any(|d| d.pattern == good) {
        info.details.push(Detail { message: good.into(), pattern: good, fill: Fill::Nothing });
    }
}

/// Every signed signature of `doc` judged again with its own embedded revocation evidence.
pub fn apply(doc: &pdfcraft_cos::Document, list: &mut [SignatureInfo]) {
    if !list.iter().any(|s| s.signed && !s.doc_timestamp) {
        return;
    }
    let fields = pdfcraft_forms::fields(doc);
    for info in list.iter_mut().filter(|s| s.signed && !s.doc_timestamp && s.status != Status::Invalid) {
        let Some(field) = fields.iter().find(|f| f.name == info.field) else { continue };
        let field_dict = doc.get(field.obj);
        let Some(v) = field_dict.as_dict().and_then(|d| d.get(b"V")).map(|v| doc.resolve(v)) else { continue };
        let Some(contents) = v.as_dict().and_then(|d| d.get(b"Contents")).map(|c| doc.resolve(c)) else { continue };
        let Some(cms) = contents.as_string().map(|s| s.bytes.clone()) else { continue };
        let (crls, ocsps) = evidence(&cms);
        if !crls.is_empty() || !ocsps.is_empty() {
            judge(info, &crls, &ocsps);
        }
    }
}
