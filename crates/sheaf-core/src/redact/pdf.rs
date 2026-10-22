//! PDF values read as text: text strings decoded, streams decompressed, XMP read as its values,
//! references and nested dictionaries resolved, and an object written back as PDF syntax.

use lopdf::{Dictionary, Object, ObjectId};
use std::collections::HashSet;

/// A PDF text string decoded (UTF-16BE or UTF-8 with their BOM, else PDFDocEncoding).
pub(super) fn decode(s: &Object) -> String {
    let t = lopdf::decode_text_string(s).unwrap_or_else(|_| s.as_str().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default());
    // lopdf keeps the UTF-8 byte-order mark as U+FEFF.
    t.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(t)
}

/// XMP as a reader of its values sees it: the text of its elements and the values of its
/// attributes (never element or attribute names, namespace declarations, comments or
/// processing instructions), with CDATA sections joined to the text around them and
/// character references (`&#115;`, `&#x73;`) and the named XML entities decoded. Each tag
/// separates text with a line break; a comment does not.
pub(super) fn xmp_text(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(s.len());
    let mut rest: &str = &s;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.find("-->").map_or("", |e| &r[e + 3..]);
        } else if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let e = r.find("]]>").unwrap_or(r.len());
            out.push_str(&r[..e]);
            rest = r.get(e + 3..).unwrap_or("");
        } else if let Some(r) = rest.strip_prefix("<?") {
            rest = r.find("?>").map_or("", |e| &r[e + 2..]);
        } else if let Some(r) = rest.strip_prefix('<') {
            let e = r.find('>').unwrap_or(r.len());
            out.push('\n');
            for v in attribute_values(&r[..e]) {
                out.push_str(&decode_entities(v));
                out.push('\n');
            }
            rest = r.get(e + 1..).unwrap_or("");
        } else {
            let n = rest.find('<').unwrap_or(rest.len());
            out.push_str(&decode_entities(&rest[..n]));
            rest = &rest[n..];
        }
    }
    out
}

/// The quoted attribute values of a tag's inside (`rdf:Description rdf:about="" dc:x='v'`),
/// namespace declarations (`xmlns…`) excepted.
fn attribute_values(tag: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = tag;
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq].split_whitespace().last().unwrap_or("");
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else { break };
        let body = &after[1..];
        let end = body.find(q).unwrap_or(body.len());
        if !name.starts_with("xmlns") {
            out.push(&body[..end]);
        }
        rest = body.get(end + 1..).unwrap_or("");
    }
    out
}

/// `s` with character references and the five named XML entities decoded.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find('&') {
        out.push_str(&rest[..a]);
        rest = &rest[a..];
        let end = rest.find(';').filter(|e| *e <= 12);
        let ch = end.map(|e| &rest[1..e]).and_then(|e| match e {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => e
                .strip_prefix("#x")
                .or_else(|| e.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| e.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        });
        match (ch, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A stream that is XMP by its own dictionary (`/Type /Metadata` or `/Subtype /XML`); see also
/// [`xmp_streams`], which adds every target of a `/Metadata` key.
pub(super) fn is_xmp(s: &lopdf::Stream) -> bool {
    s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Metadata".as_slice()) || s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"XML".as_slice())
}

pub(super) fn stream_text(s: &lopdf::Stream) -> String {
    String::from_utf8_lossy(&s.decompressed_content().unwrap_or_else(|_| s.content.clone())).into_owned()
}

pub(super) fn resolve<'a>(d: &'a lopdf::Document, o: &'a Object) -> Option<&'a Object> {
    d.dereference(o).ok().map(|(_, o)| o)
}

pub(super) fn sub_dict<'a>(d: &'a lopdf::Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Dictionary> {
    dict.get(key).ok().and_then(|o| resolve(d, o)).and_then(|o| match o {
        Object::Dictionary(x) => Some(x),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    })
}

/// The resources a page draws with (its own, else the nearest ancestor's).
pub(super) fn page_resources(d: &lopdf::Document, page: ObjectId) -> Dictionary {
    let mut id = Some(page);
    for _ in 0..64 {
        let Some(node) = id.and_then(|i| d.get_dictionary(i).ok()) else { break };
        if let Some(r) = sub_dict(d, node, b"Resources") {
            return r.clone();
        }
        id = node.get(b"Parent").and_then(Object::as_reference).ok();
    }
    Dictionary::new()
}

pub(super) fn decoded(s: &lopdf::Stream) -> Vec<u8> {
    s.decompressed_content().unwrap_or_else(|_| s.content.clone())
}

/// Streams that are XMP: the target of a `/Metadata` key anywhere, or typed `/Subtype /XML`.
pub(super) fn xmp_streams(d: &lopdf::Document) -> HashSet<ObjectId> {
    let mut out = HashSet::new();
    for (id, o) in &d.objects {
        let dict = match o {
            Object::Dictionary(x) => x,
            Object::Stream(s) => {
                if is_xmp(s) {
                    out.insert(*id);
                }
                &s.dict
            }
            _ => continue,
        };
        if let Ok(r) = dict.get(b"Metadata").and_then(Object::as_reference) {
            out.insert(r);
        }
    }
    out
}

/// One object written as PDF syntax (through lopdf's content writer).
pub(super) fn object_bytes(o: &Object) -> Option<Vec<u8>> {
    let c = lopdf::content::Content { operations: vec![lopdf::content::Operation::new("Zz", vec![o.clone()])] };
    let mut b = c.encode().ok()?;
    b.truncate(b.len().checked_sub(3)?);
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmp_text_keeps_values_and_drops_markup() {
        let t = xmp_text(b"<a b=\"x&amp;y\" xmlns:q=\"secret\"><!-- c -->v&lt;1<![CDATA[&#115;]]>&#115;</a>");
        assert_eq!(t.split('\n').filter(|l| !l.is_empty()).collect::<Vec<_>>(), ["x&y", "v<1&#115;s"]);
    }
}
