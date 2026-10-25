//! What Document Properties shows beyond the engine's summary (Acrobat's Description and Security
//! tabs): the restrictions an owner password sets, PDF/A conformance, the dates, and page 1's
//! boxes when they differ from what is shown.

use lopdf::{Dictionary, Document, Object};

/// Facts read from the file; an entry is None or empty when the file does not say.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Facts {
    /// What the owner password forbids ("Printing", "Copying text and images"…); empty: nothing.
    pub restricted: Vec<&'static str>,
    /// "PDF/A-2b" from the XMP metadata.
    pub pdfa: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    /// Page 1's boxes in millimetres: media and crop always, trim, bleed and art when the file sets them.
    pub boxes: Vec<(&'static str, String)>,
}

/// The /P bits (PDF 32000 table 22) and what each forbids when it is off.
const BITS: [(u32, &str); 8] = [
    (3, "Printing"),
    (4, "Changing the document"),
    (5, "Copying text and images"),
    (6, "Adding comments"),
    (9, "Filling in forms"),
    (10, "Copying for accessibility"),
    (11, "Assembling pages"),
    (12, "Printing in high quality"),
];

/// Reads `bytes` (decrypted with `password` for the dates and metadata when it is protected).
pub fn read(bytes: &[u8], password: Option<&str>) -> Facts {
    let mut f = Facts::default();
    let Ok(raw) = Document::load_mem(bytes) else { return f };
    // /P from the encryption dictionary; a file with an empty open password is decrypted as it
    // loads, its dictionary gone: the permissions it read are kept with the encryption state.
    let p = match raw.trailer.get(b"Encrypt").and_then(|o| raw.dereference(o).map(|(_, o)| o.clone())) {
        Ok(Object::Dictionary(enc)) => enc.get(b"P").and_then(Object::as_i64).ok().map(|p| p as i32 as u32),
        _ => raw.encryption_state.as_ref().map(|e| e.permissions().bits() as u32),
    };
    if let Some(p) = p {
        f.restricted = restricted_by(p);
    }
    let doc = if raw.is_encrypted() {
        match password.and_then(|pw| Document::load_mem_with_options(bytes, lopdf::LoadOptions::with_password(pw)).ok()).or_else(|| Document::load_mem_with_options(bytes, lopdf::LoadOptions::with_password("")).ok()) {
            Some(d) => d,
            None => return f,
        }
    } else {
        raw
    };
    if let Ok(info) = doc.trailer.get(b"Info").and_then(|o| doc.dereference(o).map(|(_, o)| o.clone())).and_then(|o| o.as_dict().cloned()) {
        f.created = date(&info, b"CreationDate");
        f.modified = date(&info, b"ModDate");
    }
    f.pdfa = doc.catalog().ok().and_then(|c| c.get(b"Metadata").ok()).and_then(|m| m.as_reference().ok()).and_then(|r| doc.get_object(r).ok()).and_then(|o| o.as_stream().ok()).and_then(|s| s.decompressed_content().ok().or_else(|| Some(s.content.clone()))).and_then(|x| pdfa_of(&String::from_utf8_lossy(&x)));
    if let Some((_, &id)) = doc.get_pages().iter().next() {
        let page = |key: &[u8]| inherited(&doc, id, key).and_then(|o| rect(&o));
        let media = page(b"MediaBox");
        let mm = |r: [f64; 4]| {
            let mm = |v: f64| v / 72.0 * 25.4;
            format!("{:.0} × {:.0} mm", mm(r[2] - r[0]), mm(r[3] - r[1]))
        };
        // A missing crop box is the media box (PDF 32000 14.11.2); the others are listed when set.
        for (key, name) in [(&b"MediaBox"[..], "Media box"), (b"CropBox", "Crop box"), (b"TrimBox", "Trim box"), (b"BleedBox", "Bleed box"), (b"ArtBox", "Art box")] {
            let r = page(key).or(if key == b"CropBox" { media } else { None });
            if let Some(r) = r {
                f.boxes.push((name, mm(r)));
            }
        }
    }
    f
}

/// What a /P value forbids; no printing at all says the high-quality one too.
fn restricted_by(p: u32) -> Vec<&'static str> {
    let mut r: Vec<&str> = BITS.iter().filter(|(bit, _)| p & (1 << (bit - 1)) == 0).map(|(_, what)| *what).collect();
    if r.contains(&"Printing") {
        r.retain(|w| *w != "Printing in high quality");
    }
    r
}

/// A page attribute, from the page or the nearest parent holding it.
fn inherited(doc: &Document, mut id: lopdf::ObjectId, key: &[u8]) -> Option<Object> {
    for _ in 0..32 {
        let d = doc.get_dictionary(id).ok()?;
        if let Ok(v) = d.get(key) {
            return Some(doc.dereference(v).ok()?.1.clone());
        }
        id = d.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn rect(o: &Object) -> Option<[f64; 4]> {
    let a = o.as_array().ok()?;
    let v: Vec<f64> = a.iter().filter_map(|x| x.as_float().map(f64::from).ok().or_else(|| x.as_i64().ok().map(|i| i as f64))).collect();
    (v.len() == 4).then(|| [v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])])
}

/// "D:20261025143000+02'00'" as "2026-10-25 14:30".
fn date(info: &Dictionary, key: &[u8]) -> Option<String> {
    let s = info.get(key).ok()?.as_str().ok()?;
    let s = String::from_utf8_lossy(s);
    let d = s.trim().trim_start_matches("D:");
    let digits: String = d.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 8 {
        return None;
    }
    let mut out = format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8]);
    if digits.len() >= 12 {
        out += &format!(" {}:{}", &digits[8..10], &digits[10..12]);
    }
    Some(out)
}

/// "PDF/A-2b" from XMP's pdfaid:part and pdfaid:conformance (as attributes or elements).
fn pdfa_of(xmp: &str) -> Option<String> {
    let field = |name: &str| -> Option<String> {
        let at = xmp.find(name)? + name.len();
        let rest = xmp[at..].trim_start_matches(['=', '>', '"', '\'', ' ']);
        let v: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        (!v.is_empty()).then_some(v)
    };
    let part = field("pdfaid:part")?;
    let conf = field("pdfaid:conformance").map(|c| c.to_lowercase()).unwrap_or_default();
    Some(format!("PDF/A-{part}{conf}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_permission_bit_is_named() {
        let all = 0xFFFF_FFFC_u32;
        assert!(restricted_by(all).is_empty());
        assert_eq!(restricted_by(all & !(1 << 9)), ["Copying for accessibility"]);
        assert_eq!(restricted_by(all & !(1 << 11)), ["Printing in high quality"]);
        assert_eq!(restricted_by(all & !(1 << 2) & !(1 << 11)), ["Printing"]);
    }

    #[test]
    fn xmp_and_dates_read_as_people_write_them() {
        assert_eq!(pdfa_of(r#"<rdf:Description pdfaid:part="2" pdfaid:conformance="B"/>"#).as_deref(), Some("PDF/A-2b"));
        assert_eq!(pdfa_of("<pdfaid:part>1</pdfaid:part><pdfaid:conformance>A</pdfaid:conformance>").as_deref(), Some("PDF/A-1a"));
        assert_eq!(pdfa_of("<x:xmpmeta/>"), None);
        let mut info = Dictionary::new();
        info.set("CreationDate", Object::string_literal("D:20261025143000+02'00'"));
        info.set("ModDate", Object::string_literal("D:2026"));
        assert_eq!(date(&info, b"CreationDate").as_deref(), Some("2026-10-25 14:30"));
        assert_eq!(date(&info, b"ModDate"), None);
    }
}
