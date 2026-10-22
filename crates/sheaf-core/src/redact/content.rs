//! Content streams by byte range: split into operations and comments, the text their text
//! operators show, the text a ToUnicode CMap maps to, and painted content rewritten without the
//! searched text in its comments, marked-content properties and inline image dictionaries.

use super::clean::scrub_dict_all;
use super::matching::contains_any;
use super::names::PDF_NAMES;
use super::pdf::{decode, object_bytes};
use super::Needle;
use lopdf::{Dictionary, Object};

/// One piece of a content stream: an operation (its operands and operator) or a comment.
pub(super) struct Span {
    pub(super) start: usize,
    pub(super) end: usize,
    /// The operator, "%" for a comment, "BI" for a whole inline image.
    pub(super) op: String,
    /// Inline image: where its `ID` keyword starts (its dictionary is `start + 2 .. id_at`).
    pub(super) id_at: usize,
    /// Comments inside an operation (between its operands, or in an inline image dictionary),
    /// as byte ranges from their `%`. Comments outside operations are spans of their own, so
    /// spans never overlap.
    pub(super) comments: Vec<(usize, usize)>,
}

/// Splits content `raw` into operations and comments by byte range, skipping strings, arrays,
/// dictionaries and inline image data, so each can be read or rewritten on its own. The spans
/// are in order and never overlap: a comment inside an operation belongs to that operation.
pub(super) fn scan_content(raw: &[u8]) -> Vec<Span> {
    let ws = |c: u8| b" \t\r\n\x0c\0".contains(&c);
    let delim = |c: u8| b"()<>[]{}/%".contains(&c);
    let n = raw.len();
    let (mut out, mut i, mut depth) = (Vec::new(), 0usize, 0usize);
    let (mut op_start, mut bi): (Option<usize>, Option<usize>) = (None, None);
    // Comments met since the current operation (or inline image) started.
    let mut inner: Vec<(usize, usize)> = Vec::new();
    let flush = |inner: &mut Vec<(usize, usize)>, out: &mut Vec<Span>| {
        out.extend(inner.drain(..).map(|(start, end)| Span { start, end, op: "%".into(), id_at: 0, comments: Vec::new() }));
    };
    while i < n {
        let c = raw[i];
        if ws(c) {
            i += 1;
            continue;
        }
        let start = i;
        match c {
            b'%' => {
                while i < n && raw[i] != b'\n' && raw[i] != b'\r' {
                    i += 1;
                }
                if op_start.is_some() || bi.is_some() {
                    inner.push((start, i));
                } else {
                    out.push(Span { start, end: i, op: "%".into(), id_at: 0, comments: Vec::new() });
                }
                continue;
            }
            b'(' => {
                let mut level = 0usize;
                while i < n {
                    match raw[i] {
                        b'\\' => i += 1,
                        b'(' => level += 1,
                        b')' => {
                            level -= 1;
                            if level == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            b'<' if raw.get(i + 1) == Some(&b'<') => {
                depth += 1;
                i += 2;
            }
            b'>' if raw.get(i + 1) == Some(&b'>') => {
                depth = depth.saturating_sub(1);
                i += 2;
            }
            b'<' => {
                while i < n && raw[i] != b'>' {
                    i += 1;
                }
                i += 1;
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'/' => {
                i += 1;
                while i < n && !ws(raw[i]) && !delim(raw[i]) {
                    i += 1;
                }
            }
            _ => {
                while i < n && !ws(raw[i]) && !delim(raw[i]) {
                    i += 1;
                }
                if i == start {
                    i += 1;
                    continue;
                }
                let tok = &raw[start..i];
                let keyword = tok[0].is_ascii_alphabetic() || tok[0] == b'\'' || tok[0] == b'"';
                if keyword && depth == 0 && !matches!(tok, b"true" | b"false" | b"null") {
                    if tok == b"BI" {
                        // Stray operands before BI are dropped; their comments stand alone.
                        flush(&mut inner, &mut out);
                        bi = Some(start);
                        op_start = None;
                        continue;
                    }
                    if tok == b"ID"
                        && let Some(b0) = bi.take()
                    {
                        // The data: after one white-space byte, up to white space + EI + white space.
                        let data = (i + 1).min(n);
                        let mut end = n;
                        let mut j = data;
                        while j + 2 <= n {
                            if &raw[j..j + 2] == b"EI" && j >= 1 && ws(raw[j - 1]) && raw.get(j + 2).is_none_or(|c| ws(*c)) {
                                end = j + 2;
                                break;
                            }
                            j += 1;
                        }
                        out.push(Span { start: b0, end, op: "BI".into(), id_at: start, comments: std::mem::take(&mut inner) });
                        i = end;
                        op_start = None;
                        continue;
                    }
                    if bi.is_none() {
                        out.push(Span { start: op_start.unwrap_or(start), end: i, op: String::from_utf8_lossy(tok).into_owned(), id_at: 0, comments: std::mem::take(&mut inner) });
                        op_start = None;
                    }
                    continue;
                }
            }
        }
        if op_start.is_none() && bi.is_none() {
            op_start = Some(start);
        }
    }
    // Operands with no operator: their comments stand alone, after every span.
    flush(&mut inner, &mut out);
    out
}

/// `raw[from..to]` without the comments of `s` that `drop` selects (their line ends stay, so
/// the tokens around them stay apart).
fn uncommented(raw: &[u8], s: &Span, from: usize, to: usize, drop: impl Fn(&[u8]) -> bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(to.saturating_sub(from));
    let mut at = from;
    for &(cs, ce) in &s.comments {
        if cs >= at && ce <= to && drop(&raw[cs..ce]) {
            b.extend_from_slice(&raw[at..cs]);
            at = ce;
        }
    }
    b.extend_from_slice(raw.get(at..to).unwrap_or_default());
    b
}

/// The dictionary of the inline image `raw[s.start..s.end]` (its comments left out).
pub(super) fn inline_dict(raw: &[u8], s: &Span) -> Option<Dictionary> {
    let mut b = b"<<".to_vec();
    b.extend_from_slice(&uncommented(raw, s, s.start + 2, s.id_at, |_| true));
    b.extend_from_slice(b">> Zz");
    let c = lopdf::content::Content::decode(&b).ok()?;
    c.operations.into_iter().next()?.operands.into_iter().next()?.as_dict().ok().cloned()
}

/// A marked-content or marked-point operation (`BMC`, `BDC`, `MP`, `DP`) of `raw`, decoded
/// (its comments left out).
pub(super) fn marked_op(raw: &[u8], s: &Span) -> Option<lopdf::content::Operation> {
    lopdf::content::Content::decode(&uncommented(raw, s, s.start, s.end, |_| true)).ok()?.operations.into_iter().next()
}

/// The text a ToUnicode CMap maps codes to (its UTF-16BE hex strings, decoded).
pub(super) fn cmap_text(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut out = String::new();
    for part in s.split('<').skip(1) {
        let Some(hex) = part.split('>').next() else { continue };
        let hex: String = hex.chars().filter(char::is_ascii_hexdigit).collect();
        if hex.len() >= 4 && hex.len().is_multiple_of(4) {
            let units: Vec<u16> = (0..hex.len() / 4).filter_map(|i| u16::from_str_radix(&hex[i * 4..i * 4 + 4], 16).ok()).collect();
            out.push_str(&String::from_utf16_lossy(&units));
            out.push(' ');
        }
    }
    out
}

/// Text drawn by the text operators of `raw` read as content, the strings of one `BT … ET`
/// block joined with nothing between them, for streams that are not drawn and so are read here.
pub(super) fn shown_strings(raw: &[u8]) -> String {
    let Ok(c) = lopdf::content::Content::decode(raw) else { return String::new() };
    let mut out = String::new();
    for op in &c.operations {
        match op.operator.as_str() {
            "ET" => out.push('\n'),
            "Tj" | "TJ" | "'" | "\"" => {
                for o in &op.operands {
                    match o {
                        Object::String(..) => out.push_str(&decode(o)),
                        Object::Array(a) => a.iter().filter(|x| matches!(x, Object::String(..))).for_each(|x| out.push_str(&decode(x))),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// `raw` (painted content) with `needles` removed from what [`Audit::painted`] reads: comments
/// holding one are dropped; marked-content property lists and inline image dictionaries are
/// scrubbed like any value; a marked-content tag holding one becomes `/Span`. `None` when
/// nothing changed.
pub(super) fn scrub_painted(raw: &[u8], needles: &[Needle]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(raw.len());
    let mut at = 0;
    let mut changed = false;
    for s in scan_content(raw) {
        let replacement: Option<Vec<u8>> = match s.op.as_str() {
            "%" => contains_any(&String::from_utf8_lossy(&raw[s.start + 1..s.end]), needles).then(Vec::new),
            "BMC" | "BDC" | "MP" | "DP" => marked_op(raw, &s).and_then(|mut op| {
                let mut hit = false;
                if let Some(Object::Name(t)) = op.operands.first_mut() {
                    let ts = String::from_utf8_lossy(t).into_owned();
                    if !PDF_NAMES.contains(&ts.as_str()) && contains_any(&ts, needles) {
                        *t = b"Span".to_vec();
                        hit = true;
                    }
                }
                for o in op.operands.iter_mut().skip(1) {
                    if let Object::Dictionary(x) = o {
                        hit |= scrub_dict_all(x, needles);
                    }
                }
                hit.then(|| lopdf::content::Content { operations: vec![op] }.encode().ok()).flatten()
            }),
            "BI" => inline_dict(raw, &s).and_then(|mut x| {
                if !scrub_dict_all(&mut x, needles) {
                    return None;
                }
                let body = object_bytes(&Object::Dictionary(x))?;
                // "BI" + the dictionary's entries (without << >>) + everything from "ID" on.
                let mut b = b"BI ".to_vec();
                b.extend_from_slice(body.get(2..body.len().saturating_sub(2))?);
                b.push(b' ');
                b.extend_from_slice(&raw[s.id_at..s.end]);
                Some(b)
            }),
            _ => None,
        };
        // A comment inside an operation left as it was: dropped on its own.
        let comment_hit = |c: &[u8]| contains_any(&String::from_utf8_lossy(&c[1..]), needles);
        let replacement = replacement.or_else(|| s.comments.iter().any(|&(cs, ce)| comment_hit(&raw[cs..ce])).then(|| uncommented(raw, &s, s.start, s.end, comment_hit)));
        if let Some(r) = replacement {
            out.extend_from_slice(&raw[at..s.start]);
            out.extend_from_slice(&r);
            at = s.end;
            changed = true;
        }
    }
    if !changed {
        return None;
    }
    out.extend_from_slice(&raw[at..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOWN: &[u8] = b"BT /F1 12 Tf 72 700 Td (keep) Tj ET";

    #[test]
    fn painted_content_is_scrubbed_outside_its_drawing() {
        let n: [Needle; 1] = ["secretword".into()];
        let raw = b"/Span << /ActualText (secretword) /Lang (en) >> BDC BT (x) Tj ET EMC\n% secretword\nBI /W 1 /H 1 /CS /G /BPC 8 /Note (a secretword) ID \x00 EI";
        let out = scrub_painted(raw, &n).unwrap();
        let t = String::from_utf8_lossy(&out);
        assert!(!t.contains("secretword") && t.contains("/Lang") && t.contains("BI") && t.contains("ID \x00 EI") && t.contains("EMC"), "{t}");
        // Still valid content (lopdf's parser does not read the abbreviated /G inline image, so
        // the part before it is checked).
        let before_bi = &out[..t.find("BI ").unwrap()];
        assert!(lopdf::content::Content::decode(before_bi).is_ok(), "still valid content: {t}");
        assert!(scrub_painted(SHOWN, &n).is_none());
    }

    #[test]
    fn comments_next_to_and_inside_operations_never_overlap_and_are_scrubbed() {
        let n: [Needle; 1] = ["secretword".into()];
        // Every pairing of a comment with operands, operators and inline image dictionaries,
        // with and without white space around it.
        let parts: [&[u8]; 12] = [
            b"/secretword",
            b"% secretword\n",
            b"%secretword\r",
            b"% keep\n",
            b"BMC",
            b"EMC",
            b"<< /A (secretword) >>",
            b"BDC",
            b"BT (x) Tj ET",
            b"BI /W 1 /H 1 /CS /G /BPC 8",
            b"ID \x00 EI",
            b"/Note (secretword)",
        ];
        let mut runs = 0;
        for a in parts {
            for b in parts {
                for c in parts {
                    for sep in [&b" "[..], b""] {
                        let raw = [a, sep, b, sep, c, b" % secretword\n BMC EMC"].concat();
                        let spans = scan_content(&raw);
                        let mut end = 0;
                        for s in &spans {
                            let mut all = vec![(s.start, s.end)];
                            all.extend(s.comments.iter().copied());
                            for &(x, y) in &all {
                                assert!(x <= y && y <= raw.len(), "{}", String::from_utf8_lossy(&raw));
                            }
                            assert!(s.start >= end, "spans overlap: {}", String::from_utf8_lossy(&raw));
                            end = s.end;
                        }
                        let out = scrub_painted(&raw, &n).unwrap_or_else(|| raw.clone());
                        // No comment of the result holds the word.
                        for s in scan_content(&out) {
                            let mut cs = s.comments.clone();
                            if s.op == "%" {
                                cs.push((s.start, s.end));
                            }
                            for (x, y) in cs {
                                assert!(!String::from_utf8_lossy(&out[x..y]).contains("secretword"), "{} -> {}", String::from_utf8_lossy(&raw), String::from_utf8_lossy(&out));
                            }
                        }
                        runs += 1;
                    }
                }
            }
        }
        assert_eq!(runs, 12 * 12 * 12 * 2);
        // A comment inside an inline image dictionary (once an overlapping span and a panic).
        let raw = b"q BI /W 1 /H 1 /CS /G /BPC 8 %secretword\n/Note (secretword) ID \x00 EI Q";
        let out = scrub_painted(raw, &n).unwrap();
        let t = String::from_utf8_lossy(&out);
        assert!(!t.contains("secretword") && t.contains("ID \x00 EI Q"), "{t}");
        // A comment between a tag and its operator: the tag is still read and replaced.
        let out = scrub_painted(b"/secretword % secretword\n BMC EMC", &n).unwrap();
        assert_eq!(String::from_utf8_lossy(&out), "/Span BMC EMC");
    }
}
