//! The deny-by-default audit of the final bytes ([`check_scrubbed`]): every string, name, key
//! and decoded stream of every object is read for the searched text, with a short list of
//! exemptions; exempt places that still hold it are noted for the user.

use super::content::{cmap_text, inline_dict, marked_op, scan_content, shown_strings};
use super::matching::contains_any;
use super::names::{is_standard_name, role_names, STANDARD_KEYS};
use super::pdf::{decode, decoded, is_xmp, xmp_streams, xmp_text};
use super::reach::Reach;
use super::Needle;
use crate::{OpdfError, Result};
use lopdf::{Dictionary, Object, ObjectId};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Told to the user when a searched text is still in a place the audit exempts (item 10).
const NOTE_LINK: &str = "Link address still contains the searched text";
const NOTE_DEST: &str = "A named destination or name-tree entry still contains the searched text";
const NOTE_FIELD: &str = "A form field name still contains the searched text";
const NOTE_FONT_NAME: &str = "A font name still contains the searched text";
const NOTE_GLYPH: &str = "A glyph name still contains the searched text";
const NOTE_PROGRAM: &str = "A font program still contains the searched text";
const NOTE_CMAP: &str = "A font's character map still contains the searched text";

/// Resource categories: their keys are resource names a producer picks (/F1, /Im3), not text.
const RESOURCE_KINDS: &[&[u8]] = &[b"Font", b"XObject", b"ExtGState", b"ColorSpace", b"Pattern", b"Shading", b"Properties"];

/// Action types (`/S` of an action dictionary).
const ACTIONS: &[&[u8]] = &[
    b"GoTo", b"GoToR", b"GoToE", b"GoToDp", b"Launch", b"Thread", b"URI", b"Sound", b"Movie", b"Hide", b"Named", b"SubmitForm", b"ResetForm", b"ImportData",
    b"JavaScript", b"SetOCGState", b"Rendition", b"Trans", b"GoTo3DView", b"RichMediaExecute",
];

/// For every object, the key it is first referenced under (`/Font`, `/Resources`, `/Dests`, …),
/// so an indirect dictionary is read in the context it is used in.
fn under_map(d: &lopdf::Document) -> HashMap<ObjectId, Vec<u8>> {
    fn scan(o: &Object, key: Option<&[u8]>, out: &mut HashMap<ObjectId, Vec<u8>>) {
        match o {
            Object::Reference(r) => {
                if let Some(k) = key {
                    out.entry(*r).or_insert_with(|| k.to_vec());
                }
            }
            Object::Array(a) => a.iter().for_each(|x| scan(x, key, out)),
            Object::Dictionary(x) => x.iter().for_each(|(k, v)| scan(v, Some(k), out)),
            Object::Stream(s) => s.dict.iter().for_each(|(k, v)| scan(v, Some(k), out)),
            _ => {}
        }
    }
    let mut out = HashMap::new();
    for o in d.objects.values() {
        scan(o, None, &mut out);
    }
    for (k, v) in d.trailer.iter() {
        scan(v, Some(k), &mut out);
    }
    out
}

/// Something the audit found.
struct Finding {
    id: ObjectId,
    /// "/ActualText", "stream content", …
    what: String,
}

/// The deny-by-default reader: every string, name and key of what it is given is checked,
/// with the exemptions of [`check_scrubbed`]; exempt places that still hold a needle are noted.
struct Audit<'n> {
    needles: &'n [Needle],
    roles: HashSet<Vec<u8>>,
    id: ObjectId,
    hits: Vec<Finding>,
    notes: BTreeSet<&'static str>,
}

impl Audit<'_> {
    fn has(&self, s: &str) -> bool {
        contains_any(s, self.needles)
    }

    /// Whether a value holds a needle in a string, a non-standard name or a key.
    fn holds(&self, o: &Object) -> bool {
        match o {
            Object::String(..) => self.has(&decode(o)),
            Object::Name(n) => !is_standard_name(n, &self.roles) && self.has(&String::from_utf8_lossy(n)),
            Object::Array(a) => a.iter().any(|x| self.holds(x)),
            Object::Dictionary(x) => x.iter().any(|(k, v)| self.has(&String::from_utf8_lossy(k)) || self.holds(v)),
            _ => false,
        }
    }

    fn hit(&mut self, what: String) {
        self.hits.push(Finding { id: self.id, what });
    }

    /// Reads `dict`, found under key `under` (its context).
    fn walk_dict(&mut self, dict: &Dictionary, under: Option<&[u8]>) {
        let name = |k: &[u8]| dict.get(k).and_then(Object::as_name).ok();
        let (ty, sub) = (name(b"Type"), name(b"Subtype"));
        let font = ty == Some(b"Font".as_slice()) || (dict.has(b"BaseFont") && sub.is_some());
        let descriptor = ty == Some(b"FontDescriptor".as_slice());
        let cmap = ty == Some(b"CMap".as_slice());
        let field = is_field_or_widget(dict);
        let annot = ty == Some(b"Annot".as_slice()) || (sub.is_some() && dict.has(b"Rect"));
        let acroform = dict.has(b"Fields");
        let action = name(b"S").is_some_and(|s| ACTIONS.contains(&s));
        let outline = dict.has(b"Title") && (dict.has(b"Parent") || dict.has(b"Dest") || dict.has(b"A"));
        let link = sub == Some(b"Link".as_slice());
        let structure = ty == Some(b"StructElem".as_slice()) || (dict.has(b"S") && dict.has(b"P") && dict.has(b"K"));
        let xref = ty == Some(b"XRef".as_slice());
        let xobject = ty == Some(b"XObject".as_slice()) || matches!(sub, Some(b"Image" | b"Form"));
        let resource_names = under.is_some_and(|u| RESOURCE_KINDS.contains(&u));
        let custom = matches!(under, Some(b"RoleMap" | b"ClassMap" | b"Colorants"));
        let dests = under == Some(b"Dests".as_slice());
        let glyphs = under == Some(b"CharProcs".as_slice());
        for (k, v) in dict.iter() {
            // The key itself: a standard key, or a name the context explains, or text.
            let ks = String::from_utf8_lossy(k);
            if self.has(&ks) {
                if dests {
                    self.notes.insert(NOTE_DEST);
                } else if glyphs {
                    self.notes.insert(NOTE_GLYPH);
                } else if !(resource_names || custom || STANDARD_KEYS.binary_search(&ks.as_ref()).is_ok()) {
                    self.hit(format!("key /{ks}"));
                }
            }
            let note = match k.as_slice() {
                b"T" if field => Some(NOTE_FIELD),
                b"URI" => Some(NOTE_LINK),
                b"D" if action || outline || link => Some(NOTE_DEST),
                b"Dest" => Some(NOTE_DEST),
                b"Differences" if v.as_array().is_ok_and(|a| a.iter().all(|x| matches!(x, Object::Name(_) | Object::Integer(_) | Object::Real(_)))) => Some(NOTE_GLYPH),
                b"CharSet" if descriptor => Some(NOTE_GLYPH),
                b"BaseFont" if font => Some(NOTE_FONT_NAME),
                b"FontName" | b"FontFamily" if descriptor => Some(NOTE_FONT_NAME),
                b"CMapName" if cmap => Some(NOTE_FONT_NAME),
                b"Registry" | b"Ordering" if under == Some(b"CIDSystemInfo".as_slice()) => Some(NOTE_FONT_NAME),
                _ => None,
            };
            if let Some(n) = note {
                if self.holds(v) {
                    self.notes.insert(n);
                }
                if let Object::Dictionary(x) = v {
                    self.walk_dict(x, Some(k));
                }
                continue;
            }
            // Codes and identifiers: a language tag, a text style, a unique name, our own marks.
            let silent = match k.as_slice() {
                b"Lang" | b"PCMark" => true,
                b"DA" | b"DS" => field || annot || acroform,
                b"ID" => xref || structure,
                b"NM" => annot,
                b"Name" => font || xobject,
                b"Panose" => under == Some(b"Style".as_slice()),
                b"S" => structure && v.as_name().is_ok_and(|n| is_standard_name(n, &self.roles)),
                _ => false,
            };
            if silent {
                continue;
            }
            // Name trees (/Names [key value …], /Limits [first last]) and the XFA packet list.
            if matches!(k.as_slice(), b"Names" | b"Limits" | b"XFA")
                && let Object::Array(a) = v
            {
                for (i, x) in a.iter().enumerate() {
                    if (k == b"Limits" || i % 2 == 0) && matches!(x, Object::String(..)) {
                        if self.holds(x) {
                            self.notes.insert(NOTE_DEST);
                        }
                    } else {
                        self.walk(x, k);
                    }
                }
                continue;
            }
            self.walk(v, k);
        }
    }

    /// Reads `o`, found under `key`.
    fn walk(&mut self, o: &Object, key: &[u8]) {
        let what = || format!("/{}", String::from_utf8_lossy(key));
        match o {
            Object::String(..) => {
                if self.has(&decode(o)) {
                    self.hit(what());
                }
            }
            Object::Name(n) => {
                if !is_standard_name(n, &self.roles) && self.has(&String::from_utf8_lossy(n)) {
                    self.hit(what());
                }
            }
            Object::Array(a) => {
                let first = a.first().and_then(|x| x.as_name().ok());
                for (i, x) in a.iter().enumerate() {
                    // An indexed colour space's lookup table is pixels; a separation's colorant
                    // names are inks.
                    let skip = match first {
                        Some(b"Indexed" | b"I") => matches!(x, Object::String(..)),
                        Some(b"Separation" | b"DeviceN" | b"NChannel") => i == 1,
                        _ => false,
                    };
                    if !skip {
                        self.walk(x, key);
                    }
                }
            }
            Object::Dictionary(x) => self.walk_dict(x, Some(key)),
            Object::Stream(s) => self.walk_dict(&s.dict, Some(key)),
            _ => {}
        }
    }

    /// What exemption (c) still reads in a painted content stream: comments, marked-content
    /// tags and property lists, and inline image dictionaries.
    fn painted(&mut self, raw: &[u8]) {
        for s in scan_content(raw) {
            if s.comments.iter().any(|&(cs, ce)| self.has(&String::from_utf8_lossy(&raw[cs + 1..ce]))) {
                self.hit("a comment in page content".into());
            }
            match s.op.as_str() {
                "%" => {
                    if self.has(&String::from_utf8_lossy(&raw[s.start + 1..s.end])) {
                        self.hit("a comment in page content".into());
                    }
                }
                "BMC" | "BDC" | "MP" | "DP" => {
                    let Some(op) = marked_op(raw, &s) else { continue };
                    if let Some(Object::Name(t)) = op.operands.first()
                        && !is_standard_name(t, &self.roles)
                        && self.has(&String::from_utf8_lossy(t))
                    {
                        self.hit("a marked-content tag in page content".into());
                    }
                    for o in op.operands.iter().skip(1) {
                        if let Object::Dictionary(x) = o {
                            self.walk_dict(x, Some(b"BDC"));
                        }
                    }
                }
                "BI" => match inline_dict(raw, &s) {
                    Some(x) => self.walk_dict(&x, Some(b"BI")),
                    None => {
                        if self.has(&String::from_utf8_lossy(&raw[s.start..s.id_at])) {
                            self.hit("an inline image in page content".into());
                        }
                    }
                },
                _ => {}
            }
        }
    }
}

fn is_field_or_widget(d: &Dictionary) -> bool {
    d.has(b"FT") || d.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Widget".as_slice()) || (d.has(b"T") && (d.has(b"Kids") || d.has(b"V")) && !d.has(b"Subtype"))
}

/// Every place where one of `needles` still occurs (see [`check_scrubbed`]), and the notes for
/// exempt places that still hold one.
fn findings(d: &lopdf::Document, needles: &[Needle]) -> (Vec<Finding>, BTreeSet<&'static str>) {
    let reach = Reach::of(d);
    let xmp = xmp_streams(d);
    let under = under_map(d);
    let mut a = Audit { needles, roles: role_names(d), id: (0, 0), hits: Vec::new(), notes: BTreeSet::new() };
    // Objects packed in object streams, read again from the packed bytes (an object the
    // cross-reference table no longer lists is still in the file).
    let mut packed: Vec<(ObjectId, Object)> = Vec::new();
    for o in d.objects.values() {
        if let Object::Stream(s) = o
            && s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"ObjStm".as_slice())
            && let Ok(os) = lopdf::ObjectStream::new(s)
        {
            packed.extend(os.objects);
        }
    }
    let all = d.objects.iter().map(|(i, o)| (*i, o)).chain(packed.iter().map(|(i, o)| (*i, o)));
    for (id, o) in all {
        a.id = id;
        let ctx = under.get(&id).map(Vec::as_slice);
        match o {
            Object::Stream(s) => {
                let ty = s.dict.get(b"Type").and_then(Object::as_name).ok();
                if reach.programs.contains(&id) {
                    if a.has(&String::from_utf8_lossy(&decoded(s))) {
                        a.notes.insert(NOTE_PROGRAM);
                    }
                } else if reach.cmaps.contains(&id) {
                    if a.has(&cmap_text(&decoded(s))) {
                        a.notes.insert(NOTE_CMAP);
                    }
                } else if reach.assets.contains(&id) || matches!(ty, Some(b"ObjStm" | b"XRef")) {
                    // Exemption (b): pixels, profiles and encodings (their dictionaries are read).
                } else if reach.painted.contains(&id) {
                    // Exemption (c): drawn text is checked on the page; the rest is read here.
                    a.painted(&decoded(s));
                } else {
                    let raw = decoded(s);
                    let text = if xmp.contains(&id) || is_xmp(s) {
                        xmp_text(&raw)
                    } else {
                        let mut t = String::from_utf8_lossy(&raw).into_owned();
                        t.push('\n');
                        t.push_str(&shown_strings(&raw));
                        t
                    };
                    if a.has(&text) {
                        a.hit(if xmp.contains(&id) { "XMP metadata".into() } else { "stream content".into() });
                    }
                }
                a.walk_dict(&s.dict, ctx);
            }
            Object::Dictionary(x) => a.walk_dict(x, ctx),
            other => a.walk(other, ctx.unwrap_or(b"value")),
        }
    }
    // The trailer's document information when written inline.
    if let Ok(Object::Dictionary(info)) = d.trailer.get(b"Info") {
        a.id = (0, 0);
        a.walk_dict(info, Some(b"Info"));
    }
    (a.hits, a.notes)
}

/// The deny-by-default audit of the final bytes: fails when one of `needles` (compared in
/// Unicode form C, case-insensitive, any spacing) is still in any string, name, key or decoded
/// stream of any object, object streams included, except:
/// - (a) name-tree keys and destinations (`/Dest`; `/D` of an action, outline item or link),
///   `/URI`, `/DA` and `/DS` of a field, annotation or form, a field's `/T`, `/Lang`, an
///   annotation's `/NM`, structure roles the document declares, and XMP markup (element and
///   attribute names; XMP values are read, see [`xmp_text`]);
/// - (b) the bytes of fonts (programs, CMaps), images (with their masks) and ICC profiles
///   reached from a page's content, an annotation appearance, the form's default resources or
///   an output intent; their dictionaries are read, except font names (`/BaseFont`, a
///   descriptor's `/FontName` and `/FontFamily`), glyph names (`/Differences` of names,
///   `/CharSet`, `/CharProcs` keys) and indexed-colour lookup tables;
/// - (c) the operators of page content streams and of the Form XObjects, patterns and Type3
///   glyphs they paint, whose text [`verify`] checks on the page; their comments, marked-content
///   tags and property lists and inline image dictionaries are read.
///
/// Returns a note for each kind of exempt place that still holds a searched text (a link
/// address, a named destination, a field name, a font or glyph name, a font program or
/// character map), so the success line never hides it.
pub(super) fn check_scrubbed(bytes: &[u8], needles: &[Needle]) -> Result<Vec<String>> {
    if needles.is_empty() {
        return Ok(Vec::new());
    }
    let d = lopdf::Document::load_mem(bytes).map_err(|e| OpdfError::Conversion(format!("redaction audit failed: the output can't be re-read ({e})")))?;
    let (hits, notes) = findings(&d, needles);
    match hits.first() {
        None => Ok(notes.into_iter().map(String::from).collect()),
        Some(f) => Err(OpdfError::Conversion(format!(
            "redaction audit failed: a redacted text is still in the document ({}, object {} {})",
            f.what, f.id.0, f.id.1
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One page, `/Metadata` → an XMP stream holding `inner`.
    fn with_xmp(inner: &str) -> Vec<u8> {
        let mut d = lopdf::Document::with_version("1.7");
        let xmp = format!("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF><rdf:Description rdf:about=\"\">{inner}</rdf:Description></rdf:RDF></x:xmpmeta>");
        let mut dict = Dictionary::new();
        dict.set("Type", Object::Name(b"Metadata".to_vec()));
        dict.set("Subtype", Object::Name(b"XML".to_vec()));
        let m = d.add_object(lopdf::Stream::new(dict, xmp.into_bytes()));
        let mut cat = Dictionary::new();
        cat.set("Type", Object::Name(b"Catalog".to_vec()));
        cat.set("Metadata", m);
        let root = d.add_object(cat);
        d.trailer.set("Root", root);
        let mut out = Vec::new();
        d.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn the_check_reads_xmp_through_references_comments_and_cdata() {
        let n: [Needle; 1] = ["secretword".into()];
        for inner in ["<dc:title>&#115;ecretword</dc:title>", "<dc:title>s&#x65;cretword</dc:title>", "<dc:title>secret<!-- x -->word</dc:title>", "<dc:title>secret<![CDATA[word]]></dc:title>", "<dc:title>SECRETWORD</dc:title>"] {
            let e = check_scrubbed(&with_xmp(inner), &n).unwrap_err().to_string();
            assert!(e.contains("XMP metadata"), "{inner}: {e}");
        }
        // The word in a tag or attribute name, or split by a tag, is not the text.
        for inner in ["<secretword:x/>", "<dc:title>secret</dc:title><dc:x>word</dc:x>", "<dc:title>Report</dc:title>"] {
            check_scrubbed(&with_xmp(inner), &n).unwrap_or_else(|e| panic!("{inner}: {e}"));
        }
    }

    /// A one-page PDF: catalog (1) with `cat`, pages (2), page (3) with `page` and resources
    /// /F1 (5, Helvetica) plus `res`, content (4) `content`, then `more` from object 6 on.
    fn doc(cat: &str, page: &str, res: &str, content: &[u8], more: &[(&str, Option<&[u8]>)]) -> Vec<u8> {
        let objs: Vec<(String, Option<Vec<u8>>)> = [
            (format!("<< /Type /Catalog /Pages 2 0 R {cat} >>"), None),
            ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(), None),
            (format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> {res} >> {page} >>"), None),
            ("<< >>".to_string(), Some(content.to_vec())),
            ("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(), None),
        ]
        .into_iter()
        .chain(more.iter().map(|(d, s)| (d.to_string(), s.map(<[u8]>::to_vec))))
        .collect();
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offs = Vec::new();
        for (i, (d, st)) in objs.iter().enumerate() {
            offs.push(out.len());
            match st {
                Some(st) => {
                    out.extend(format!("{} 0 obj {} /Length {} >>\nstream\n", i + 1, d.trim_end_matches(">>").trim_end(), st.len()).as_bytes());
                    out.extend(st);
                    out.extend(b"\nendstream endobj\n");
                }
                None => out.extend(format!("{} 0 obj {d} endobj\n", i + 1).as_bytes()),
            }
        }
        let x = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for o in offs {
            out.extend(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend(format!("trailer << /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n", objs.len() + 1).as_bytes());
        out
    }

    fn audit(bytes: &[u8], needle: &str) -> std::result::Result<Vec<String>, String> {
        check_scrubbed(bytes, &[needle.into()]).map_err(|e| e.to_string())
    }

    const SHOWN: &[u8] = b"BT /F1 12 Tf 72 700 Td (keep) Tj ET";

    #[test]
    fn painted_content_is_read_outside_its_drawing() {
        // Drawn text is exempt (verify reads it on the page)...
        audit(&doc("", "", "", b"BT /F1 12 Tf 72 700 Td (secretword) Tj ET", &[]), "secretword").unwrap();
        // ...but not marked-content properties, comments or inline image dictionaries.
        for content in [
            &b"/Span << /ActualText (secretword) >> BDC BT /F1 12 Tf (x) Tj ET EMC"[..],
            b"/P << /E (a secretword) >> DP BT /F1 12 Tf (x) Tj ET",
            b"% draft secretword\nBT /F1 12 Tf (x) Tj ET",
            b"q 10 0 0 10 0 0 cm BI /W 1 /H 1 /CS /G /BPC 8 /Note (secretword) ID \x00 EI Q",
        ] {
            let e = match audit(&doc("", "", "", content, &[]), "secretword") {
                Ok(n) => panic!("{} passed: {n:?}", String::from_utf8_lossy(content)),
                Err(e) => e,
            };
            assert!(e.contains("redaction audit failed"), "{}: {e}", String::from_utf8_lossy(content));
        }
    }

    #[test]
    fn reached_fonts_have_their_dictionaries_read() {
        // A font the page uses: its program is exempt, its dictionary is not.
        let content = b"BT /F2 12 Tf 72 700 Td (x) Tj ET";
        let stash = doc("", "", "/Font << /F2 6 0 R >>", content, &[("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Stash (secretword) >>", None)]);
        assert!(audit(&stash, "secretword").unwrap_err().contains("/Stash"));
        let caption = doc("", "", "", SHOWN, &[]);
        audit(&caption, "secretword").unwrap();
        // Its name is only noted.
        let named = doc("", "", "/Font << /F2 6 0 R >>", content, &[("<< /Type /Font /Subtype /Type1 /BaseFont /SecretwordSans >>", None)]);
        assert_eq!(audit(&named, "secretword").unwrap(), [NOTE_FONT_NAME]);
    }

    #[test]
    fn exempt_keys_only_count_in_their_context() {
        let cases: [(&str, bool); 8] = [
            ("/Extra << /D (secretword) >>", false),
            ("/OpenAction << /S /GoTo /D (secretword) >>", true),
            ("/Extra << /DA (/secretword 0 Tf) >>", false),
            ("/AcroForm << /Fields [] /DA (/Helv 0 Tf secretword) >>", true),
            ("/Extra << /Differences [(secretword)] >>", false),
            ("/Extra << /Differences [1 /secretword] >>", true),
            ("/Extra << /FontFamily (secretword) >>", false),
            ("/Extra << /Type /FontDescriptor /FontFamily (secretword) >>", true),
        ];
        for (cat, exempt) in cases {
            let r = audit(&doc(cat, "", "", SHOWN, &[]), "secretword");
            assert_eq!(r.is_ok(), exempt, "{cat}: {r:?}");
        }
    }

    #[test]
    fn keys_that_are_not_pdf_keys_are_read() {
        let e = audit(&doc("/Extra << /secretword 1 >>", "", "", SHOWN, &[]), "secretword").unwrap_err();
        assert!(e.contains("key /secretword"), "{e}");
        // Resource names are names a producer picks, not text.
        audit(&doc("", "", "/XObject << /secretword 6 0 R >>", b"q /secretword Do Q", &[("<< /Type /XObject /Subtype /Form /BBox [0 0 1 1] >>", Some(b""))]), "secretword").unwrap();
    }

    #[test]
    fn text_split_across_strings_of_a_block_is_one_text() {
        assert!(contains_any(&shown_strings(b"BT (SEC) Tj (RET) Tj ET"), &["secret".into()]));
        assert!(!contains_any(&shown_strings(b"BT (SEC) Tj ET BT (RET) Tj ET"), &["secret".into()]));
        // An unpainted stream holding it that way is refused.
        let e = audit(&doc("/Extra 6 0 R", "", "", SHOWN, &[("<< >>", Some(b"BT /F1 9 Tf (secr) Tj [(et) -20 (word)] TJ ET"))]), "secretword").unwrap_err();
        assert!(e.contains("stream content"), "{e}");
    }

    #[test]
    fn a_self_painting_appearance_finishes_quickly() {
        // The appearance paints itself eight times over: without a visited set, every level of
        // the depth limit multiplies the walk by eight.
        let bytes = doc(
            "",
            "/Annots [6 0 R]",
            "",
            SHOWN,
            &[
                ("<< /Type /Annot /Subtype /Square /Rect [0 0 10 10] /AP << /N 7 0 R >> >>", None),
                ("<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] /Resources << /XObject << /X 7 0 R >> >> >>", Some(b"/X Do /X Do /X Do /X Do /X Do /X Do /X Do /X Do")),
            ],
        );
        let t = std::time::Instant::now();
        audit(&bytes, "secretword").unwrap();
        assert!(t.elapsed() < std::time::Duration::from_secs(1), "{:?}", t.elapsed());
    }

    #[test]
    fn our_own_marks_and_language_tags_are_not_text() {
        let pc = doc("/Lang (en-US)", "", "", b"/Redaction BMC EMC BT (x) Tj ET", &[("<< /PCMark /Redaction /PCAdded << /Kind /Added >> >>", None)]);
        let pc = {
            let mut d = lopdf::Document::load_mem(&pc).unwrap();
            let id = d.add_object(lopdf::Stream::new(Dictionary::from_iter([("PCMark", Object::Name(b"Redaction".to_vec()))]), b"0 g".to_vec()));
            d.catalog_mut().unwrap().set("Extra2", id);
            let mut b = Vec::new();
            d.save_to(&mut b).unwrap();
            b
        };
        for w in ["red", "add", "en", "us", "redaction", "added"] {
            audit(&pc, w).unwrap_or_else(|e| panic!("{w}: {e}"));
        }
        // A PdfCraft mark written as its own object, as PdfCraft writes it.
        let wrap = doc("/Extra 6 0 R", "", "", SHOWN, &[("<< /PCMark /Wrap >>", Some(b"q Q"))]);
        audit(&wrap, "wrap").unwrap();
    }

    #[test]
    fn output_intent_profiles_are_reached() {
        let bytes = doc("/OutputIntents [<< /Type /OutputIntent /S /GTS_PDFA1 /DestOutputProfile 6 0 R >>]", "", "", SHOWN, &[("<< /N 3 >>", Some(b"desc secretword profile"))]);
        audit(&bytes, "secretword").unwrap();
    }
}
