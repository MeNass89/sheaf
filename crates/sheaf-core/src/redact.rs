//! Redact PDF: find sensitive text, mark areas, and remove what lies under the marks for good.
//!
//! Search works on PdfCraft's text layer (the glyphs its renderer places, so what is found is
//! what the page shows), with PdfCraft's patterns for e-mail addresses, phone numbers, credit
//! card numbers, US Social Security numbers and dates. Marks are PDF redaction annotations
//! (`/Redact`) in user space; [`apply`] adds them, applies them (text, image pixels and
//! drawings under them are removed and black boxes drawn), empties whole-page marks, removes
//! the matched text from metadata, bookmarks, comments and form fields, then audits the bytes
//! before they get their final name (see [`verify`]). If the audit finds anything nothing is
//! written and the call fails: a redaction that only looks done is never shipped.

use crate::engine::{open_pdf, pdfium_lock};
use crate::pdfcraft::engine::{rect_quad, Edit, NewAnnotation, Shape, Style};
use crate::pdfcraft::{render, Doc};
use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use lopdf::{Dictionary, Object, ObjectId};
use pdfium_render::prelude::*;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

/// PdfCraft's search patterns.
pub use crate::pdfcraft::engine::RedactPattern as Pattern;

/// What to look for.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    /// The text as typed (runs of spaces match any spacing, words may wrap to the next line).
    Literal { text: String, case_insensitive: bool },
    Pattern(Pattern),
}

/// One match: its page (0-based), one quadrilateral per line it covers (user space, as PDF
/// `/QuadPoints`: top-left, top-right, bottom-left, bottom-right), and the text matched.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMatch {
    pub page: usize,
    pub quads: Vec<[f64; 8]>,
    pub text: String,
}

/// An area to redact on page `page` (0-based): one or more quadrilaterals in user space.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mark {
    pub page: usize,
    pub quads: Vec<[f64; 8]>,
    /// The text matched under this mark (empty for a drawn area): [`apply`] also removes it
    /// from the document information, XMP metadata, bookmarks, comments and form fields.
    pub text: String,
    /// The mark is the whole page: its content is replaced by a black fill of the page and its
    /// annotations dropped, so nothing outside the media box survives either.
    pub whole_page: bool,
}

/// The line shown when [`apply`] succeeds (its audit passed).
pub const VERIFIED: &str = "No text remains under the black boxes. Matches also removed from metadata, bookmarks and comments.";

/// What [`apply`] wrote.
#[derive(Debug, Clone)]
pub struct RedactReport {
    pub path: PathBuf,
    /// Marks applied (a match over two lines is one area).
    pub areas: usize,
    /// The input was password-protected; the output is not.
    pub was_encrypted: bool,
    /// What was removed whole besides the marks, one line each ("XMP metadata removed: it
    /// contained the redacted text", "Attachment a.txt removed: …").
    pub warnings: Vec<String>,
}

/// The bounding box `[x0, y0, x1, y1]` of quadrilaterals.
pub fn bounds(quads: &[[f64; 8]]) -> [f64; 4] {
    let mut r = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for q in quads {
        for p in q.chunks(2) {
            r = [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])];
        }
    }
    r
}

/// Every match of `query` in the document, page by page.
pub fn find(input: &Input, query: &Query) -> Result<Vec<TextMatch>> {
    find_all(input, std::slice::from_ref(query))
}

/// Every match of each of `queries` (opened and read once), page by page; on a page, in the
/// order of `queries`, then of the text.
pub fn find_all(input: &Input, queries: &[Query]) -> Result<Vec<TextMatch>> {
    let doc = Doc::open(input)?;
    let d = doc.document();
    let config = render::RenderConfig { password: d.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
    let mut r = render::PageRenderer::new(d.bytes.clone(), config);
    let mut out = Vec::new();
    for (page, info) in d.info.pages.iter().enumerate() {
        let res = r.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
        let Some(text) = res.text else {
            if let Some(e) = res.error {
                return Err(OpdfError::Engine(format!("page {}: {e}", page + 1)));
            }
            continue;
        };
        for q in queries {
            let ranges = match q {
                // Typed text in either Unicode form ("É" as one character or "E" + accent) finds
                // the page's text in either.
                Query::Literal { text: needle, case_insensitive } => {
                    use unicode_normalization::UnicodeNormalization;
                    let (c, dcomp): (String, String) = (needle.nfc().collect(), needle.nfd().collect());
                    let mut r = text.find_opts(&c, !case_insensitive, false);
                    if dcomp != c {
                        for g in text.find_opts(&dcomp, !case_insensitive, false) {
                            if !r.contains(&g) {
                                r.push(g);
                            }
                        }
                    }
                    r
                }
                Query::Pattern(p) => text.find_with(|chars| crate::pdfcraft::engine::find_pattern(*p, chars)),
            };
            for g in ranges {
                let quads = text.line_rects(g.clone()).into_iter().map(|v| info.view_rect_to_quad(v)).collect();
                out.push(TextMatch { page, quads, text: text.text_of(g).replace('\n', " ") });
            }
        }
    }
    Ok(out)
}

/// Marks over the matches.
pub fn mark_text(matches: &[TextMatch]) -> Vec<Mark> {
    matches.iter().map(|m| Mark { page: m.page, quads: m.quads.clone(), text: m.text.clone(), whole_page: false }).collect()
}

/// Marks over rectangles `[x0, y0, x1, y1]` in user space, one per `(page, rect)`.
pub fn mark_rects(rects: &[(usize, [f64; 4])]) -> Vec<Mark> {
    rects.iter().map(|(page, r)| Mark { page: *page, quads: vec![rect_quad(*r)], ..Default::default() }).collect()
}

/// Marks over whole pages (0-based). [`apply`] replaces each such page's content with a black
/// fill of its media box and drops its annotations and resources, so nothing of the page is
/// left, not even content drawn outside the media box.
pub fn mark_pages(input: &Input, pages: &[usize]) -> Result<Vec<Mark>> {
    let doc = Doc::open(input)?;
    let boxes = doc.document().page_boxes();
    pages
        .iter()
        .map(|p| {
            let b = boxes.get(*p).ok_or_else(|| OpdfError::Unsupported(format!("page {} does not exist (the document has {} pages)", p + 1, boxes.len())))?;
            Ok(Mark { page: *p, quads: vec![rect_quad(b[0])], text: String::new(), whole_page: true })
        })
        .collect()
}

/// Applies `marks` and writes the result to `out` (never overwriting).
///
/// `searched` are the texts typed for the search, removed with the marks' own texts from the
/// document information, XMP metadata, bookmarks, comments and form fields (see [`Mark::text`]).
/// The bytes are audited ([`verify`]) under their temporary name; any finding fails the call
/// and nothing is written.
pub fn apply(input: &Input, marks: &[Mark], searched: &[String], out: &Path) -> Result<RedactReport> {
    if marks.is_empty() {
        return Err(OpdfError::Unsupported("nothing is marked for redaction".into()));
    }
    let mut doc = Doc::open(input)?;
    let pages = doc.document().info.pages.len();
    if let Some(m) = marks.iter().find(|m| m.page >= pages) {
        return Err(OpdfError::Unsupported(format!("page {} does not exist (the document has {pages} pages)", m.page + 1)));
    }
    // Whole pages are emptied below; PdfCraft applies the others.
    let edits: Vec<Edit> = marks
        .iter()
        .filter(|m| !m.whole_page)
        .map(|m| {
            let shape = Shape::Redact { quads: m.quads.clone(), overlay: String::new(), look: Default::default() };
            let style = Style::default_for(&shape);
            Edit::AddAnnotation(NewAnnotation { page: m.page, shape, style, contents: String::new(), author: "Sheaf".into() })
        })
        .collect();
    if !edits.is_empty() {
        doc.apply(Edit::Batch { label: "Mark for redaction".into(), edits })?;
        doc.apply(Edit::ApplyRedactions { pages: None })?;
    }
    let (bytes, was_encrypted) = doc.unprotected_bytes()?;
    drop(doc);
    let needles = needles(marks, searched);
    let (bytes, warnings) = post_pass(&bytes, marks, &needles)?;
    let path = atomic_write(out, |part| {
        std::fs::write(part, &bytes)?;
        verify(part, marks)?;
        check_scrubbed(&bytes, &needles)
    })?;
    Ok(RedactReport { path, areas: marks.len(), was_encrypted, warnings })
}

/// The texts to remove outside the page content: the typed searches and every match's text.
fn needles(marks: &[Mark], searched: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in searched.iter().chain(marks.iter().map(|m| &m.text)) {
        let t = nfc(&t.split_whitespace().collect::<Vec<_>>().join(" "));
        if !t.is_empty() && !out.iter().any(|o| o.to_lowercase() == t.to_lowercase()) {
            out.push(t);
        }
    }
    out
}

/// `s` in Unicode normalisation form C ("E" + combining acute becomes "É").
fn nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
}

/// Where `needle` occurs in `hay` (byte range of `hay`): case-insensitive, any run of white
/// space in `needle` matching any run in `hay`, both compared in Unicode form C. `hay` must
/// already be in form C (see [`nfc`]) for the range to be meaningful.
fn find_loose(hay: &str, needle: &str) -> Option<(usize, usize)> {
    let needle = nfc(needle);
    let words: Vec<Vec<char>> = needle.split_whitespace().map(|w| w.chars().collect()).collect();
    if words.is_empty() {
        return None;
    }
    let same = |a: char, b: char| a == b || a.to_lowercase().eq(b.to_lowercase());
    let chars: Vec<(usize, char)> = hay.char_indices().collect();
    'start: for s in 0..chars.len() {
        let mut i = s;
        for (k, w) in words.iter().enumerate() {
            if k > 0 {
                let ws = i;
                while i < chars.len() && chars[i].1.is_whitespace() {
                    i += 1;
                }
                if i == ws {
                    continue 'start;
                }
            }
            for c in w {
                match chars.get(i) {
                    Some(&(_, h)) if same(h, *c) => i += 1,
                    _ => continue 'start,
                }
            }
        }
        let end = chars.get(i).map_or(hay.len(), |c| c.0);
        return Some((chars[s].0, end));
    }
    None
}

fn contains_any(hay: &str, needles: &[String]) -> bool {
    let hay = nfc(hay);
    needles.iter().any(|n| find_loose(&hay, n).is_some())
}

/// `text` (in form C) without any of `needles`, or `None` when none occurs.
fn scrub(text: &str, needles: &[String]) -> Option<String> {
    let mut t = nfc(text);
    let mut changed = false;
    while let Some((a, b)) = needles.iter().find_map(|n| find_loose(&t, n)) {
        t.replace_range(a..b, "");
        changed = true;
    }
    changed.then_some(t)
}

/// A PDF text string decoded (UTF-16BE or UTF-8 with their BOM, else PDFDocEncoding).
fn decode(s: &Object) -> String {
    let t = lopdf::decode_text_string(s).unwrap_or_else(|_| s.as_str().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default());
    // lopdf keeps the UTF-8 byte-order mark as U+FEFF.
    t.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(t)
}

/// XMP as a reader of its values sees it: the text of its elements and the values of its
/// attributes (never element or attribute names, namespace declarations, comments or
/// processing instructions), with CDATA sections joined to the text around them and
/// character references (`&#115;`, `&#x73;`) and the named XML entities decoded. Each tag
/// separates text with a line break; a comment does not.
fn xmp_text(bytes: &[u8]) -> String {
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
fn is_xmp(s: &lopdf::Stream) -> bool {
    s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Metadata".as_slice()) || s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"XML".as_slice())
}

fn stream_text(s: &lopdf::Stream) -> String {
    String::from_utf8_lossy(&s.decompressed_content().unwrap_or_else(|_| s.content.clone())).into_owned()
}

/// What a dictionary is, for the scrub's allow-list.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Info,
    Outline,
    Annot,
    Field,
    /// A widget's appearance characteristics (`/MK`): its captions.
    Captions,
    /// A structure element: its alternate and replacement texts.
    Struct,
}

/// The keys whose text values are scrubbed, by kind: what a reader shows as text. Names-tree
/// keys, destinations, URIs, default appearances, field names and anything else are left alone.
fn scrub_keys(k: Kind) -> &'static [&'static [u8]] {
    match k {
        Kind::Info => &[],
        Kind::Outline => &[b"Title"],
        Kind::Annot => &[b"Contents", b"T", b"Subj", b"RC"],
        Kind::Field => &[b"V", b"DV", b"TU", b"Opt"],
        Kind::Captions => &[b"CA", b"RC", b"AC"],
        Kind::Struct => &[b"Alt", b"ActualText", b"E"],
    }
}

/// The kinds of `d` (an annotation widget can be its own field, so possibly two).
/// `captions` are the `/MK` dictionaries written as their own objects.
fn kinds(d: &Dictionary, outline_items: &HashSet<ObjectId>, captions: &HashSet<ObjectId>, id: ObjectId) -> Vec<Kind> {
    let name = |k: &[u8]| d.get(k).and_then(Object::as_name).ok();
    let mut out = Vec::new();
    if outline_items.contains(&id) {
        out.push(Kind::Outline);
    }
    if captions.contains(&id) {
        out.push(Kind::Captions);
    }
    if name(b"Type") == Some(b"StructElem".as_slice()) || (d.has(b"S") && d.has(b"P") && d.has(b"K")) {
        out.push(Kind::Struct);
    }
    let annot = name(b"Type") == Some(b"Annot".as_slice()) || (d.has(b"Subtype") && d.has(b"Rect") && name(b"Type").is_none_or(|t| t == b"Annot"));
    // A widget's /T is its field's name: a widget is only scrubbed as a field.
    if annot && name(b"Subtype") != Some(b"Widget".as_slice()) {
        out.push(Kind::Annot);
    }
    if d.has(b"FT") || (d.has(b"T") && (d.has(b"V") || d.has(b"Kids")) && name(b"Subtype") != Some(b"Text".as_slice())) {
        out.push(Kind::Field);
    }
    out
}

/// The outline items reachable from the catalog's /Outlines (by /First and /Next).
fn outline_items(d: &lopdf::Document) -> HashSet<ObjectId> {
    let mut out = HashSet::new();
    let first = |id: ObjectId| d.get_dictionary(id).ok().and_then(|x| x.get(b"First").and_then(Object::as_reference).ok());
    let Some(root) = d.catalog().ok().and_then(|c| c.get(b"Outlines").and_then(Object::as_reference).ok()) else { return out };
    let mut todo: Vec<ObjectId> = first(root).into_iter().collect();
    while let Some(id) = todo.pop() {
        if !out.insert(id) || out.len() > 100_000 {
            continue;
        }
        if let Some(c) = first(id) {
            todo.push(c);
        }
        if let Some(n) = d.get_dictionary(id).ok().and_then(|x| x.get(b"Next").and_then(Object::as_reference).ok()) {
            todo.push(n);
        }
    }
    out
}

/// Scrubs the allow-listed keys of `d` (through arrays and nested dictionaries: a choice
/// field's /Opt); the document information is scrubbed whole with [`scrub_dict_all`].
fn scrub_dict(d: &mut Dictionary, kinds: &[Kind], needles: &[String]) -> bool {
    if kinds.contains(&Kind::Info) {
        return scrub_dict_all(d, needles);
    }
    let keys: Vec<Vec<u8>> = d.iter().filter(|(k, _)| kinds.iter().any(|kd| scrub_keys(*kd).contains(&k.as_slice()))).map(|(k, _)| k.clone()).collect();
    let mut changed = false;
    for k in keys {
        let Ok(v) = d.get_mut(&k) else { continue };
        match scrub_any(v, needles) {
            Scrubbed::Kept => {}
            Scrubbed::Changed => changed = true,
            Scrubbed::Drop => {
                d.remove(&k);
                changed = true;
            }
        }
    }
    // A widget's captions written inline.
    if let Ok(Object::Dictionary(mk)) = d.get_mut(b"MK") {
        changed |= scrub_dict(mk, &[Kind::Captions], needles);
    }
    changed
}

/// What the post-pass did that the user must be told.
type Warnings = Vec<String>;

/// The written bytes, after PdfCraft: whole-page marks emptied; `needles` removed from the
/// document information (through arrays and dictionaries; a name holding one loses its key),
/// bookmark titles, comments, form field values and options, widget captions and structure
/// alternate texts; XMP metadata, attachments, JavaScript, private application data
/// (/PieceInfo) and Form XObjects no page paints that contain one removed whole (each kind
/// named in the warnings). Unchanged bytes are returned as they are.
fn post_pass(bytes: &[u8], marks: &[Mark], needles: &[String]) -> Result<(Vec<u8>, Warnings)> {
    let whole: BTreeSet<usize> = marks.iter().filter(|m| m.whole_page).map(|m| m.page).collect();
    let mut warnings = Vec::new();
    if whole.is_empty() && needles.is_empty() {
        return Ok((bytes.to_vec(), warnings));
    }
    let mut d = lopdf::Document::load_mem(bytes)?;
    let mut changed = false;
    // Whole pages: one black fill of the mark's box, nothing else.
    let page_ids = d.get_pages();
    let mut dropped: HashSet<ObjectId> = HashSet::new();
    for &p in &whole {
        let id = *page_ids.get(&(p as u32 + 1)).ok_or_else(|| OpdfError::Engine(format!("page {} is missing", p + 1)))?;
        let quads: Vec<[f64; 8]> = marks.iter().filter(|m| m.whole_page && m.page == p).flat_map(|m| m.quads.clone()).collect();
        let [x0, y0, x1, y1] = bounds(&quads);
        let fill = format!("q 0 0 0 rg {x0} {y0} {} {} re f Q", x1 - x0, y1 - y0).into_bytes();
        let content = d.add_object(lopdf::Stream::new(Dictionary::new(), fill));
        let annots: Vec<ObjectId> = match d.get_dictionary(id)?.get(b"Annots") {
            Ok(a) => d.dereference(a)?.1.as_array().map(|a| a.iter().filter_map(|x| x.as_reference().ok()).collect()).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        dropped.extend(annots);
        let page = d.get_object_mut(id)?.as_dict_mut()?;
        page.set("Contents", content);
        page.set("Resources", Dictionary::new());
        for k in [&b"Annots"[..], b"Thumb", b"PieceInfo", b"Metadata", b"B"] {
            page.remove(k);
        }
        changed = true;
    }
    if !dropped.is_empty() {
        prune_fields(&mut d, &dropped)?;
    }
    if needles.is_empty() {
        d.prune_objects();
        let mut out = Vec::new();
        d.save_to(&mut out)?;
        return Ok((out, warnings));
    }
    // Text values under the allow-list.
    let outlines = outline_items(&d);
    let captions: HashSet<ObjectId> = d.objects.values().filter_map(|o| o.as_dict().ok()).filter_map(|x| x.get(b"MK").and_then(Object::as_reference).ok()).collect();
    let ids: Vec<ObjectId> = d.objects.keys().copied().collect();
    let mut fields_changed = false;
    for id in ids {
        let Some(Object::Dictionary(dict)) = d.objects.get_mut(&id) else { continue };
        let mut ks = kinds(dict, &outlines, &captions, id);
        if dict.has(b"MK") {
            ks.push(Kind::Annot);
            ks.dedup();
        }
        let widget = dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Widget".as_slice());
        if widget && !ks.contains(&Kind::Field) {
            // A widget is scrubbed as a field (never its /T), plus its captions.
            ks.retain(|k| *k != Kind::Annot);
        }
        if ks.is_empty() || !scrub_dict(dict, &ks, needles) {
            continue;
        }
        changed = true;
        // An annotation or a field whose text changed loses its appearance (it still shows
        // the old text); viewers draw it again from the new value.
        dict.remove(b"AP");
        if ks.contains(&Kind::Field) {
            fields_changed = true;
            let kids: Vec<ObjectId> = dict.get(b"Kids").and_then(Object::as_array).map(|a| a.iter().filter_map(|x| x.as_reference().ok()).collect()).unwrap_or_default();
            for k in kids {
                if let Ok(kd) = d.get_object_mut(k).and_then(Object::as_dict_mut) {
                    kd.remove(b"AP");
                }
            }
        }
    }
    // The document information, direct or not.
    match d.trailer.get(b"Info").ok().cloned() {
        Some(Object::Reference(r)) => {
            if let Ok(info) = d.get_object_mut(r).and_then(Object::as_dict_mut) {
                changed |= scrub_dict(info, &[Kind::Info], needles);
            }
        }
        Some(Object::Dictionary(mut info)) => {
            if scrub_dict(&mut info, &[Kind::Info], needles) {
                d.trailer.set("Info", info);
                changed = true;
            }
        }
        _ => {}
    }
    if fields_changed
        && let Ok(af) = d.catalog().and_then(|c| c.get(b"AcroForm")).cloned()
    {
        match af {
            Object::Reference(r) => {
                if let Ok(a) = d.get_object_mut(r).and_then(Object::as_dict_mut) {
                    a.set("NeedAppearances", true);
                }
            }
            Object::Dictionary(mut a) => {
                a.set("NeedAppearances", true);
                let root = d.trailer.get(b"Root")?.as_reference()?;
                d.get_object_mut(root)?.as_dict_mut()?.set("AcroForm", a);
            }
            _ => {}
        }
    }
    // Streams with a match outside page content: XMP, attachments and JavaScript go whole.
    let mut gone: HashSet<ObjectId> = HashSet::new();
    let mut xmp_gone = false;
    for id in xmp_streams(&d) {
        if let Ok(Object::Stream(s)) = d.get_object(id)
            && contains_any(&xmp_text(&decoded(s)), needles)
        {
            gone.insert(id);
            xmp_gone = true;
        }
    }
    if xmp_gone {
        warnings.push("XMP metadata removed: it contained the redacted text".into());
    }
    let (attachments, scripts) = embedded_and_scripts(&d);
    for (name, file) in attachments {
        if d.get_object(file).ok().and_then(|o| o.as_stream().ok()).is_some_and(|s| contains_any(&stream_text(s), needles)) {
            gone.insert(file);
            warnings.push(format!("Attachment {name} removed: it contained the redacted text"));
        }
    }
    for (name, js) in scripts {
        let text = match d.get_object(js) {
            Ok(Object::Stream(s)) => stream_text(s),
            Ok(o @ Object::String(..)) => decode(o),
            _ => continue,
        };
        if contains_any(&text, needles) {
            gone.insert(js);
            warnings.push(format!("JavaScript {name} removed: it contained the redacted text"));
        }
    }
    // A JavaScript action written inline (`/JS (…)`) is a string: dropped with its key.
    for o in d.objects.values_mut() {
        if let Object::Dictionary(dict) = o
            && dict.get(b"S").and_then(Object::as_name).ok() == Some(b"JavaScript".as_slice())
            && dict.get(b"JS").ok().is_some_and(|j| matches!(j, Object::String(..)) && contains_any(&decode(j), needles))
        {
            dict.remove(b"JS");
            warnings.push("JavaScript removed: it contained the redacted text".into());
            changed = true;
        }
    }
    if !gone.is_empty() {
        changed = true;
        remove_objects(&mut d, &gone);
    }
    // Private application data (/PieceInfo, on the catalog, pages or forms) holding the text.
    let ids: Vec<ObjectId> = d.objects.keys().copied().collect();
    for id in ids {
        let piece = match d.objects.get(&id) {
            Some(Object::Dictionary(x)) => x.get(b"PieceInfo").ok().cloned(),
            Some(Object::Stream(s)) => s.dict.get(b"PieceInfo").ok().cloned(),
            _ => None,
        };
        if let Some(p) = piece
            && holds(&d, &p, needles, &mut HashSet::new())
        {
            match d.objects.get_mut(&id) {
                Some(Object::Dictionary(x)) => x.remove(b"PieceInfo"),
                Some(Object::Stream(s)) => s.dict.remove(b"PieceInfo"),
                _ => None,
            };
            warn(&mut warnings, "Private application data removed: it contained the redacted text");
            changed = true;
        }
    }
    // Form XObjects no page paints (an unused resource, a form hung off private data, an
    // annotation appearance) that hold the text: removed, with every reference to them. When a
    // page's content can't be read, what it paints is unknown: nothing is removed, and the
    // audit refuses if the text is left.
    // What nothing points to any more (a removed /PieceInfo's forms) goes first.
    d.prune_objects();
    let reach = Reach::of(&d);
    if !reach.broken {
        let hidden: HashSet<ObjectId> = d
            .objects
            .iter()
            .filter(|(id, o)| {
                !reach.painted.contains(id)
                    && o.as_stream().is_ok_and(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form".as_slice()))
            })
            .filter(|(_, o)| holds(&d, o, needles, &mut HashSet::new()))
            .map(|(id, _)| *id)
            .collect();
        if !hidden.is_empty() {
            remove_objects(&mut d, &hidden);
            warn(&mut warnings, "Hidden content removed: it contained the redacted text");
            changed = true;
        }
    }
    if !changed {
        return Ok((bytes.to_vec(), warnings));
    }
    d.prune_objects();
    let mut out = Vec::new();
    d.save_to(&mut out)?;
    Ok((out, warnings))
}

/// Streams named for the user: ("a.txt", its object).
type Named = Vec<(String, ObjectId)>;

/// The embedded files (`/EmbeddedFile` streams, with the file name of their file
/// specification) and the JavaScript held in streams (`/JS` of every JavaScript action).
fn embedded_and_scripts(d: &lopdf::Document) -> (Named, Named) {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    let mut scripts = Vec::new();
    for o in d.objects.values() {
        let dict = match o {
            Object::Dictionary(x) => x,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        if let Ok(Object::Dictionary(ef)) = dict.get(b"EF") {
            let name = [&b"UF"[..], b"F"].iter().find_map(|k| dict.get(k).ok().map(decode)).unwrap_or_else(|| "(unnamed)".into());
            for (_, v) in ef.iter() {
                if let Ok(r) = v.as_reference()
                    && seen.insert(r)
                {
                    files.push((name.clone(), r));
                }
            }
        }
        if dict.get(b"S").and_then(Object::as_name).ok() == Some(b"JavaScript".as_slice())
            && let Ok(r) = dict.get(b"JS").and_then(Object::as_reference)
        {
            scripts.push(("in an action".to_string(), r));
        }
    }
    // Streams typed /EmbeddedFile that no file specification names.
    for (id, o) in &d.objects {
        if let Object::Stream(s) = o
            && s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"EmbeddedFile".as_slice())
            && seen.insert(*id)
        {
            files.push(("(unnamed)".into(), *id));
        }
    }
    (files, scripts)
}

/// Removes from the form every field whose widget was dropped (with a parent left without
/// kids), so a blacked-out page keeps no field values.
fn prune_fields(d: &mut lopdf::Document, dropped: &HashSet<ObjectId>) -> Result<()> {
    fn keep(d: &mut lopdf::Document, list: Vec<Object>, dropped: &HashSet<ObjectId>) -> Vec<Object> {
        let mut out = Vec::new();
        for o in list {
            let Ok(id) = o.as_reference() else {
                out.push(o);
                continue;
            };
            if dropped.contains(&id) {
                continue;
            }
            let kids = d.get_dictionary(id).ok().and_then(|f| f.get(b"Kids").ok()).and_then(|k| k.as_array().ok()).cloned();
            if let Some(kids) = kids {
                let had = !kids.is_empty();
                let left = keep(d, kids, dropped);
                if had && left.is_empty() {
                    continue;
                }
                if let Ok(f) = d.get_object_mut(id).and_then(Object::as_dict_mut) {
                    f.set("Kids", left);
                }
            }
            out.push(o);
        }
        out
    }
    let Ok(af) = d.catalog().and_then(|c| c.get(b"AcroForm")).cloned() else { return Ok(()) };
    let af_id = af.as_reference().ok();
    let af_dict = match af_id {
        Some(r) => d.get_dictionary(r)?.clone(),
        None => af.as_dict()?.clone(),
    };
    let Ok(fields) = af_dict.get(b"Fields").and_then(|f| d.dereference(f)).map(|(_, f)| f.as_array().cloned().unwrap_or_default()) else { return Ok(()) };
    let fields = keep(d, fields, dropped);
    let mut af_dict = af_dict;
    af_dict.set("Fields", fields);
    match af_id {
        Some(r) => d.objects.insert(r, Object::Dictionary(af_dict)).map(|_| ()).unwrap_or(()),
        None => {
            let root = d.trailer.get(b"Root")?.as_reference()?;
            d.get_object_mut(root)?.as_dict_mut()?.set("AcroForm", af_dict);
        }
    }
    Ok(())
}

// ---- Deny-by-default audit ---------------------------------------------------------------------

/// What the audit may skip because it is drawn (and so checked on the page by [`verify`]) or
/// is a drawing resource.
#[derive(Default)]
struct Reach {
    /// Page content streams, Form XObjects painted by `Do` from them (recursively), tiling
    /// patterns and Type3 glyph procedures they use.
    painted: HashSet<ObjectId>,
    /// Font dictionaries (with their descriptors, CMaps and programs), images (with their
    /// masks) and ICC profiles used by the painted streams or by annotation appearances.
    assets: HashSet<ObjectId>,
    /// Some content could not be parsed: what it paints is unknown.
    broken: bool,
}

fn resolve<'a>(d: &'a lopdf::Document, o: &'a Object) -> Option<&'a Object> {
    d.dereference(o).ok().map(|(_, o)| o)
}

fn sub_dict<'a>(d: &'a lopdf::Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Dictionary> {
    dict.get(key).ok().and_then(|o| resolve(d, o)).and_then(|o| match o {
        Object::Dictionary(x) => Some(x),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    })
}

/// The resources a page draws with (its own, else the nearest ancestor's).
fn page_resources(d: &lopdf::Document, page: ObjectId) -> Dictionary {
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

fn decoded(s: &lopdf::Stream) -> Vec<u8> {
    s.decompressed_content().unwrap_or_else(|_| s.content.clone())
}

impl Reach {
    fn of(d: &lopdf::Document) -> Reach {
        let mut r = Reach::default();
        for (_, page) in d.get_pages() {
            let res = page_resources(d, page);
            for c in d.get_page_contents(page) {
                if r.painted.insert(c) {
                    r.content(d, c, &res, true, 0);
                }
            }
            // Annotation appearances: their resources count as reached; the appearance
            // streams themselves are not exempt (verify does not read annotations).
            let annots = d.get_dictionary(page).ok().and_then(|p| p.get(b"Annots").ok()).and_then(|a| resolve(d, a)).and_then(|a| a.as_array().ok()).cloned().unwrap_or_default();
            for a in annots {
                let Some(ap) = resolve(d, &a).and_then(|a| a.as_dict().ok()).and_then(|a| sub_dict(d, a, b"AP")) else { continue };
                for (_, v) in ap.iter() {
                    let mut forms: Vec<ObjectId> = v.as_reference().into_iter().collect();
                    if let Some(Object::Dictionary(states)) = resolve(d, v) {
                        forms.extend(states.iter().filter_map(|(_, x)| x.as_reference().ok()));
                    }
                    for f in forms {
                        r.content(d, f, &res, false, 0);
                    }
                }
            }
        }
        r
    }

    /// Walks the stream `id` as content drawn with `parent` resources (or its own).
    fn content(&mut self, d: &lopdf::Document, id: ObjectId, parent: &Dictionary, painted: bool, depth: usize) {
        if depth > 32 {
            return;
        }
        let Ok(Object::Stream(s)) = d.get_object(id) else { return };
        let res = sub_dict(d, &s.dict, b"Resources").cloned().unwrap_or_else(|| parent.clone());
        let Ok(ops) = lopdf::content::Content::decode(&decoded(s)) else {
            self.broken = true;
            return;
        };
        let named = |cat: &[u8], op: &lopdf::content::Operation| -> Option<ObjectId> {
            let name = op.operands.iter().rev().find_map(|o| o.as_name().ok())?;
            sub_dict(d, &res, cat)?.get(name).ok()?.as_reference().ok()
        };
        for op in &ops.operations {
            match op.operator.as_str() {
                "Do" => {
                    let Some(x) = named(b"XObject", op) else { continue };
                    let Ok(Object::Stream(xs)) = d.get_object(x) else { continue };
                    match xs.dict.get(b"Subtype").and_then(Object::as_name).ok() {
                        Some(b"Form") => {
                            if !painted || self.painted.insert(x) {
                                self.content(d, x, &res, painted, depth + 1);
                            }
                        }
                        Some(b"Image") => self.image(d, x),
                        _ => {}
                    }
                }
                "Tf" => {
                    if let Some(f) = named(b"Font", op) {
                        self.font(d, f, &res, depth);
                    }
                }
                "cs" | "CS" => {
                    if let Some(cs) = op.operands.first().and_then(|o| o.as_name().ok()).and_then(|n| sub_dict(d, &res, b"ColorSpace")?.get(n).ok().cloned()) {
                        self.color_space(d, &cs, 0);
                    }
                }
                "scn" | "SCN" => {
                    if let Some(p) = named(b"Pattern", op)
                        && d.get_object(p).is_ok_and(|o| o.as_stream().is_ok())
                        && (!painted || self.painted.insert(p))
                    {
                        self.content(d, p, &res, painted, depth + 1);
                    }
                }
                _ => {}
            }
        }
    }

    fn image(&mut self, d: &lopdf::Document, id: ObjectId) {
        if !self.assets.insert(id) {
            return;
        }
        let Ok(Object::Stream(s)) = d.get_object(id) else { return };
        for k in [&b"SMask"[..], b"Mask"] {
            if let Ok(m) = s.dict.get(k).and_then(Object::as_reference) {
                self.image(d, m);
            }
        }
        if let Ok(cs) = s.dict.get(b"ColorSpace") {
            self.color_space(d, &cs.clone(), 0);
        }
    }

    fn color_space(&mut self, d: &lopdf::Document, cs: &Object, depth: usize) {
        if depth > 8 {
            return;
        }
        if let Ok(r) = cs.as_reference() {
            if d.get_object(r).is_ok_and(|o| o.as_stream().is_ok()) {
                self.assets.insert(r);
            }
            if let Ok(o) = d.get_object(r) {
                let o = o.clone();
                self.color_space(d, &o, depth + 1);
            }
            return;
        }
        if let Object::Array(a) = cs {
            for x in a {
                if let Ok(r) = x.as_reference()
                    && d.get_object(r).is_ok_and(|o| o.as_stream().is_ok())
                {
                    // An ICC profile, an indexed lookup table or a tint function.
                    self.assets.insert(r);
                } else if matches!(x, Object::Array(_) | Object::Reference(_)) {
                    self.color_space(d, x, depth + 1);
                }
            }
        }
    }

    /// A font and everything it is made of: descriptor, programs, CMaps, descendants, Type3
    /// glyph procedures.
    fn font(&mut self, d: &lopdf::Document, id: ObjectId, parent: &Dictionary, depth: usize) {
        if !self.assets.insert(id) {
            return;
        }
        let Ok(f) = d.get_dictionary(id) else { return };
        let f = f.clone();
        for (k, v) in f.iter() {
            match k.as_slice() {
                b"FontDescriptor" | b"ToUnicode" | b"Encoding" | b"CIDSystemInfo" | b"CIDToGIDMap" => self.font_part(d, v),
                b"DescendantFonts" => {
                    let kids = resolve(d, v).and_then(|a| a.as_array().ok()).cloned().unwrap_or_default();
                    for kid in kids {
                        if let Ok(r) = kid.as_reference() {
                            self.font(d, r, parent, depth);
                        }
                    }
                }
                b"CharProcs" => {
                    let res = sub_dict(d, &f, b"Resources").cloned().unwrap_or_else(|| parent.clone());
                    let procs: Vec<ObjectId> = resolve(d, v).and_then(|p| p.as_dict().ok()).map(|p| p.iter().filter_map(|(_, x)| x.as_reference().ok()).collect()).unwrap_or_default();
                    for p in procs {
                        if self.painted.insert(p) {
                            self.content(d, p, &res, true, depth + 1);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// A descriptor, CMap or encoding of a font, and the streams it points to (FontFile*,
    /// CIDSet, an embedded CMap's /UseCMap, …).
    fn font_part(&mut self, d: &lopdf::Document, v: &Object) {
        let Ok(r) = v.as_reference() else { return };
        if !self.assets.insert(r) {
            return;
        }
        let dict = match d.get_object(r) {
            Ok(Object::Dictionary(x)) => x.clone(),
            Ok(Object::Stream(s)) => s.dict.clone(),
            _ => return,
        };
        for (_, x) in dict.iter() {
            if let Ok(xr) = x.as_reference()
                && d.get_object(xr).is_ok_and(|o| o.as_stream().is_ok())
            {
                self.assets.insert(xr);
            }
        }
    }
}

/// Names PDF itself defines (types, subtypes, filters, colour spaces, actions, annotation and
/// field kinds, standard structure roles, …): structure, never the user's text.
const PDF_NAMES: &[&str] = &[
    "Catalog", "Pages", "Page", "Font", "FontDescriptor", "XObject", "Form", "Image", "Annot", "Annots", "Link", "Widget", "Text", "FreeText", "Line",
    "Square", "Circle", "Polygon", "PolyLine", "Highlight", "Underline", "Squiggly", "StrikeOut", "Stamp", "Caret", "Ink", "Popup", "FileAttachment",
    "Sound", "Movie", "Screen", "PrinterMark", "TrapNet", "Watermark", "3D", "Redact", "Projection", "RichMedia", "Outlines", "Metadata", "XML",
    "Filespec", "EmbeddedFile", "Encoding", "ExtGState", "Pattern", "Shading", "Group", "Transparency", "Mask", "Alpha", "Luminosity", "OCG", "OCMD",
    "OCProperties", "StructTreeRoot", "StructElem", "MCR", "OBJR", "ObjStm", "XRef", "Sig", "DocTimeStamp", "SigRef", "TransformParams", "Action",
    "Type0", "Type1", "Type1C", "Type3", "TrueType", "MMType1", "CIDFontType0", "CIDFontType0C", "CIDFontType2", "OpenType", "CMap", "Identity",
    "Identity-H", "Identity-V", "WinAnsiEncoding", "MacRomanEncoding", "MacExpertEncoding", "StandardEncoding", "PDFDocEncoding", "FlateDecode",
    "LZWDecode", "ASCIIHexDecode", "ASCII85Decode", "RunLengthDecode", "CCITTFaxDecode", "JBIG2Decode", "DCTDecode", "JPXDecode", "Crypt", "Fl",
    "AHx", "A85", "LZW", "RL", "CCF", "DCT", "DeviceGray", "DeviceRGB", "DeviceCMYK", "CalGray", "CalRGB", "Lab", "ICCBased", "Indexed",
    "Separation", "DeviceN", "NChannel", "All", "None", "G", "RGB", "CMYK", "I", "GoTo", "GoToR", "GoToE", "Launch", "Thread", "URI", "Named",
    "SubmitForm", "ResetForm", "ImportData", "JavaScript", "SetOCGState", "Rendition", "Trans", "GoTo3DView", "Hide", "NextPage", "PrevPage",
    "FirstPage", "LastPage", "Print", "View", "Design", "Export", "XYZ", "Fit", "FitH", "FitV", "FitR", "FitB", "FitBH", "FitBV", "Tx", "Btn", "Ch",
    "Off", "On", "Yes", "No", "True", "False", "Unknown", "UseNone", "UseOutlines", "UseThumbs", "FullScreen", "UseOC", "UseAttachments",
    "SinglePage", "OneColumn", "TwoColumnLeft", "TwoColumnRight", "TwoPageLeft", "TwoPageRight", "L2R", "R2L", "Simplex", "DuplexFlipShortEdge",
    "DuplexFlipLongEdge", "AppDefault", "Normal", "Multiply", "Overlay", "Darken", "Lighten", "ColorDodge", "ColorBurn", "HardLight", "SoftLight",
    "Difference", "Exclusion", "Hue", "Saturation", "Color", "Compatible", "Perceptual", "RelativeColorimetric", "Saturation", "AbsoluteColorimetric",
    "S", "D", "B", "U", "R", "P", "N", "C", "F", "H", "O", "T", "V", "W", "Q", "Comment", "Key", "Note", "Help", "NewParagraph", "Paragraph", "Insert",
    "Check", "Cross", "Star", "Diamond", "OpenArrow", "ClosedArrow", "Butt", "ROpenArrow", "RClosedArrow", "Slash", "Approved", "Experimental",
    "NotApproved", "AsIs", "Expired", "NotForPublicRelease", "Confidential", "Final", "Sold", "Departmental", "ForComment", "TopSecret", "Draft",
    "ForPublicRelease", "FreeTextCallout", "FreeTextTypeWriter", "LineArrow", "LineDimension", "PolygonCloud", "PolyLineDimension", "PolygonDimension",
    "Group", "Inline", "Document", "DocumentFragment", "Part", "Art", "Sect", "Div", "BlockQuote", "Caption", "TOC", "TOCI", "Index", "NonStruct",
    "Private", "H1", "H2", "H3", "H4", "H5", "H6", "Title", "FENote", "Sub", "Lbl", "LBody", "LI", "Table", "TR", "TH", "TD", "THead", "TBody",
    "TFoot", "Span", "Quote", "Reference", "BibEntry", "Code", "Em", "Strong", "Ruby", "RB", "RT", "RP", "Warichu", "WT", "WP", "Figure", "Formula",
    "Artifact", "Layout", "List", "PrintField", "Pagination", "Background", "Header", "Footer", "Watermark", "Disc", "Decimal", "UpperRoman",
    "LowerRoman", "UpperAlpha", "LowerAlpha", "Ordered", "Unordered", "Row", "Column", "Both", "Block", "Start", "Center", "End", "Justify",
    "Before", "Middle", "After", "Auto", "Solid", "Dashed", "Dotted", "Double", "Groove", "Ridge", "Inset", "Outset", "Hidden", "NoView",
    "Toggle", "ReadOnly", "Locked", "LockedContents", "AlternatePresentations", "SlideShow", "Media", "MediaClip", "MediaClipData", "MediaPlayers",
    "Rendition", "SR", "MR", "Pagelabels", "Lang", "Marked", "Supplement", "Registry", "Ordering", "Adobe", "UCS", "Event", "Ve", "Va", "Vf",
    "Distiller", "PageElement", "PageItem", "Unchecked", "Checked", "Matte", "Default", "DefaultGray", "DefaultRGB", "DefaultCMYK", "Halftone",
    "Function", "Sampled", "Interpolate", "BBox", "Matrix", "Resources", "Contents", "Kids", "Parent", "Count", "Type", "Subtype",
];

/// Keys whose values are references to names, URIs, styles or font names, never the user's
/// text (exemption (a), plus font names and destination references).
fn exempt_key(key: &[u8]) -> bool {
    matches!(key, b"URI" | b"DA" | b"Dest" | b"D" | b"BaseFont" | b"FontName" | b"FontFamily" | b"FontStretch" | b"Differences" | b"Filter" | b"DecodeParms")
}

/// Something the audit found.
struct Finding {
    id: ObjectId,
    /// "/ActualText", "stream content", …
    what: String,
}

/// Text drawn by the text operators of `raw` read as content (each operator on its own line,
/// the strings of `TJ` joined), for streams that are not drawn and so are read here.
fn shown_strings(raw: &[u8]) -> String {
    let Ok(c) = lopdf::content::Content::decode(raw) else { return String::new() };
    let mut out = String::new();
    for op in &c.operations {
        if !matches!(op.operator.as_str(), "Tj" | "TJ" | "'" | "\"") {
            continue;
        }
        for o in &op.operands {
            match o {
                Object::String(..) => out.push_str(&decode(o)),
                Object::Array(a) => a.iter().filter(|x| matches!(x, Object::String(..))).for_each(|x| out.push_str(&decode(x))),
                _ => {}
            }
        }
        out.push('\n');
    }
    out
}

fn is_field_or_widget(d: &Dictionary) -> bool {
    d.has(b"FT") || d.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Widget".as_slice()) || (d.has(b"T") && (d.has(b"Kids") || d.has(b"V")) && !d.has(b"Subtype"))
}

/// Streams that are XMP: the target of a `/Metadata` key anywhere, or typed `/Subtype /XML`.
fn xmp_streams(d: &lopdf::Document) -> HashSet<ObjectId> {
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

/// Every place outside drawn content where one of `needles` still occurs: every string and
/// name of every object (object streams opened), and every stream's decoded content, except
/// the exemptions of [`check_scrubbed`].
fn findings(d: &lopdf::Document, needles: &[String]) -> Vec<Finding> {
    let reach = Reach::of(d);
    let xmp = xmp_streams(d);
    let mut out = Vec::new();
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
        // Fonts, images and ICC profiles in use: structure and pixels (exemption (b)).
        if reach.assets.contains(&id) {
            continue;
        }
        let dict = match o {
            Object::Stream(s) => {
                let ty = s.dict.get(b"Type").and_then(Object::as_name).ok();
                if !matches!(ty, Some(b"ObjStm" | b"XRef")) && !reach.painted.contains(&id) {
                    let raw = decoded(s);
                    let text = if xmp.contains(&id) || is_xmp(s) {
                        xmp_text(&raw)
                    } else {
                        let mut t = String::from_utf8_lossy(&raw).into_owned();
                        t.push('\n');
                        t.push_str(&shown_strings(&raw));
                        t
                    };
                    if contains_any(&text, needles) {
                        out.push(Finding { id, what: if xmp.contains(&id) { "XMP metadata".into() } else { "stream content".into() } });
                    }
                }
                &s.dict
            }
            Object::Dictionary(x) => x,
            other => {
                walk(other, None, None, needles, &mut |what| out.push(Finding { id, what }));
                continue;
            }
        };
        walk_dict(dict, needles, &mut |what| out.push(Finding { id, what }));
    }
    // The trailer's document information when written inline.
    if let Ok(Object::Dictionary(info)) = d.trailer.get(b"Info") {
        walk_dict(info, needles, &mut |what| out.push(Finding { id: (0, 0), what: format!("document information {what}") }));
    }
    out
}

fn walk_dict(dict: &Dictionary, needles: &[String], hit: &mut dyn FnMut(String)) {
    let field = is_field_or_widget(dict);
    for (k, v) in dict.iter() {
        // A field's name (exemption (a)).
        if field && k == b"T" {
            continue;
        }
        walk(v, Some(k), Some(dict), needles, hit);
    }
}

/// Checks `o`, found under `key` of `parent`, and what it contains.
fn walk(o: &Object, key: Option<&[u8]>, parent: Option<&Dictionary>, needles: &[String], hit: &mut dyn FnMut(String)) {
    let key_name = || key.map(|k| format!("/{}", String::from_utf8_lossy(k))).unwrap_or_else(|| "a value".into());
    if key.is_some_and(exempt_key) {
        // A URI, style, destination or font name: only its nested dictionaries are read.
        if let Object::Dictionary(x) = o {
            walk_dict(x, needles, hit);
        }
        return;
    }
    match o {
        Object::String(..) => {
            if contains_any(&decode(o), needles) {
                hit(key_name());
            }
        }
        Object::Name(n) => {
            let n = String::from_utf8_lossy(n);
            if !PDF_NAMES.contains(&n.as_ref()) && contains_any(&n, needles) {
                hit(key_name());
            }
        }
        Object::Array(a) => {
            // A name tree's keys (/Names [key value …], /Limits [first last]): exemption (a).
            let tree = matches!(key, Some(b"Names") | Some(b"Limits")) && parent.is_some();
            for (i, x) in a.iter().enumerate() {
                if tree && (key == Some(b"Limits") || i % 2 == 0) && matches!(x, Object::String(..)) {
                    continue;
                }
                walk(x, key, parent, needles, hit);
            }
        }
        Object::Dictionary(x) => walk_dict(x, needles, hit),
        _ => {}
    }
}

/// The deny-by-default audit of the final bytes: fails when one of `needles` (compared in
/// Unicode form C, case-insensitive, any spacing) is still in any string, name or decoded
/// stream of any object, object streams included, except:
/// - (a) name-tree keys and destination references (`/Dest`, `/D`), `/URI`, `/DA`, a field's
///   `/T`, and XMP markup (element and attribute names; XMP values are read, see [`xmp_text`]);
/// - (b) fonts (dictionaries, descriptors, CMaps, programs), images (with their masks) and ICC
///   profiles, only when reached from a page's content or an annotation appearance; font names
///   (`/BaseFont`, `/FontName`, `/FontFamily`) and names PDF itself defines ([`PDF_NAMES`]);
/// - (c) the content of page content streams and of the Form XObjects, patterns and Type3
///   glyphs they paint, whose text [`verify`] checks on the page.
fn check_scrubbed(bytes: &[u8], needles: &[String]) -> Result<()> {
    if needles.is_empty() {
        return Ok(());
    }
    let d = lopdf::Document::load_mem(bytes).map_err(|e| OpdfError::Conversion(format!("redaction audit failed: the output can't be re-read ({e})")))?;
    match findings(&d, needles).first() {
        None => Ok(()),
        Some(f) => Err(OpdfError::Conversion(format!(
            "redaction audit failed: a redacted text is still in the document ({}, object {} {})",
            f.what, f.id.0, f.id.1
        ))),
    }
}

/// What [`scrub_any`] did to a value.
#[derive(PartialEq)]
enum Scrubbed {
    Kept,
    Changed,
    /// Nothing of the value is left (an emptied string, a name holding the text): its key goes.
    Drop,
}

/// Removes `needles` from `o` and everything nested in it: strings lose the matched text, names
/// holding it (other than [`PDF_NAMES`]) go, arrays and dictionaries are read through.
/// References are not followed. In an array an emptied string stays as `()`, so the positions
/// of the others (a choice field's /Opt pairs) do not move.
fn scrub_any(o: &mut Object, needles: &[String]) -> Scrubbed {
    match o {
        Object::String(..) => match scrub(&decode(o), needles) {
            None => Scrubbed::Kept,
            Some(t) if t.trim().is_empty() => Scrubbed::Drop,
            Some(t) => {
                *o = lopdf::text_string(&t);
                Scrubbed::Changed
            }
        },
        Object::Name(n) => {
            let n = String::from_utf8_lossy(n);
            if !PDF_NAMES.contains(&n.as_ref()) && contains_any(&n, needles) { Scrubbed::Drop } else { Scrubbed::Kept }
        }
        Object::Array(a) => {
            let mut changed = false;
            let mut i = 0;
            while i < a.len() {
                match scrub_any(&mut a[i], needles) {
                    Scrubbed::Kept => {}
                    Scrubbed::Changed => changed = true,
                    Scrubbed::Drop if matches!(a[i], Object::String(..)) => {
                        a[i] = Object::String(Vec::new(), lopdf::StringFormat::Literal);
                        changed = true;
                    }
                    Scrubbed::Drop => {
                        a.remove(i);
                        changed = true;
                        continue;
                    }
                }
                i += 1;
            }
            match (changed, a.is_empty()) {
                (true, true) => Scrubbed::Drop,
                (true, false) => Scrubbed::Changed,
                _ => Scrubbed::Kept,
            }
        }
        Object::Dictionary(d) => {
            if scrub_dict_all(d, needles) { Scrubbed::Changed } else { Scrubbed::Kept }
        }
        _ => Scrubbed::Kept,
    }
}

/// [`scrub_any`] on every value of `d`; a dropped value loses its key.
fn scrub_dict_all(d: &mut Dictionary, needles: &[String]) -> bool {
    let keys: Vec<Vec<u8>> = d.iter().map(|(k, _)| k.clone()).collect();
    let mut changed = false;
    for k in keys {
        let Ok(v) = d.get_mut(&k) else { continue };
        match scrub_any(v, needles) {
            Scrubbed::Kept => {}
            Scrubbed::Changed => changed = true,
            Scrubbed::Drop => {
                d.remove(&k);
                changed = true;
            }
        }
    }
    changed
}

/// Whether `o`, or anything it holds or points to (references followed once), contains one of
/// `needles`: strings, names, and streams read as text, as XMP and as content.
fn holds(d: &lopdf::Document, o: &Object, needles: &[String], seen: &mut HashSet<ObjectId>) -> bool {
    match o {
        Object::Reference(r) => seen.insert(*r) && d.get_object(*r).is_ok_and(|x| holds(d, x, needles, seen)),
        Object::String(..) => contains_any(&decode(o), needles),
        Object::Name(n) => {
            let n = String::from_utf8_lossy(n);
            !PDF_NAMES.contains(&n.as_ref()) && contains_any(&n, needles)
        }
        Object::Array(a) => a.iter().any(|x| holds(d, x, needles, seen)),
        Object::Dictionary(x) => x.iter().any(|(_, v)| holds(d, v, needles, seen)),
        Object::Stream(s) => {
            let raw = decoded(s);
            contains_any(&String::from_utf8_lossy(&raw), needles)
                || contains_any(&xmp_text(&raw), needles)
                || contains_any(&shown_strings(&raw), needles)
                || s.dict.iter().any(|(_, v)| holds(d, v, needles, seen))
        }
        _ => false,
    }
}

/// Removes the objects `gone` and every reference to them: dictionary keys pointing to one
/// (/Metadata, an /EF entry, an /XObject resource, an /AP state) and array entries.
fn remove_objects(d: &mut lopdf::Document, gone: &HashSet<ObjectId>) {
    fn unref(o: &mut Object, gone: &HashSet<ObjectId>) {
        match o {
            Object::Array(a) => {
                a.retain(|x| !x.as_reference().is_ok_and(|r| gone.contains(&r)));
                a.iter_mut().for_each(|x| unref(x, gone));
            }
            Object::Dictionary(x) => unref_dict(x, gone),
            Object::Stream(s) => unref_dict(&mut s.dict, gone),
            _ => {}
        }
    }
    fn unref_dict(x: &mut Dictionary, gone: &HashSet<ObjectId>) {
        let keys: Vec<Vec<u8>> = x.iter().filter(|(_, v)| v.as_reference().is_ok_and(|r| gone.contains(&r))).map(|(k, _)| k.clone()).collect();
        for k in keys {
            x.remove(&k);
        }
        x.iter_mut().for_each(|(_, v)| unref(v, gone));
    }
    for id in gone {
        d.objects.remove(id);
    }
    for o in d.objects.values_mut() {
        unref(o, gone);
    }
    unref_dict(&mut d.trailer, gone);
}

fn warn(warnings: &mut Warnings, w: &str) {
    if !warnings.iter().any(|x| x == w) {
        warnings.push(w.to_string());
    }
}

/// Checks a written PDF for redactions that only look done. Fails when:
/// - PdfCraft's audit finds a redaction mark never applied, or an opaque filled rectangle
///   painted over text that is still extractable under it (when that text lies under none of
///   `marks`, the box was in the input: the message names its pages and asks to mark it);
/// - pdfium (a second, independent reader, which also reads text inside Form XObjects) still
///   finds a character whose centre lies inside one of `marks`, or any character on a page
///   marked whole.
///
/// PdfCraft's audit works on whole text lines: a box over the middle of a line whose words on
/// both sides remain ("Mail ▮ today") counts as covering that line. Each such finding is
/// therefore checked glyph by glyph against PdfCraft's text layer and fails only when some
/// glyph lies at least a fifth under the box, the same share redaction itself removes.
///
/// Blind spots. Outside `marks`, a "box" is only a filled rectangle in the page's own content:
/// a box drawn in a Form XObject, as a path that is not `re`, as a black image, or as an
/// annotation (/Square, a stamp) is not seen, nor is text no reader can decode (a Type3 or
/// other font without /ToUnicode or a known encoding). Inside `marks` those cases are caught
/// when pdfium can decode the text. Text in a page's structure tree (/ActualText, /Alt) is not
/// checked here; outside the pages [`apply`] runs a deny-by-default audit of every object
/// ([`check_scrubbed`]), whose exemptions are its blind spots: fonts, images and ICC profiles
/// a page uses (glyph names, a subset's name, text drawn as pixels), name-tree keys,
/// destinations, URIs, /DA, field names and XMP markup. Anything that can't be read fails.
pub fn verify(path: &Path, marks: &[Mark]) -> Result<()> {
    let fail = |why: String| OpdfError::Conversion(format!("redaction audit failed: {why}"));
    let doc = Doc::open(&Input::new(path)).map_err(|e| fail(format!("the output can't be re-opened ({e})")))?;
    let d = doc.document();
    let findings = d.audit_redactions().map_err(fail)?;
    let config = render::RenderConfig { password: d.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
    let mut r = render::PageRenderer::new(d.bytes.clone(), config);
    let mut texts: std::collections::HashMap<usize, std::sync::Arc<render::PageText>> = Default::default();
    let mut real = Vec::new();
    // Per finding of `real`: the text it hides lies under no mark (the input's own box).
    let mut theirs = Vec::new();
    for f in findings {
        if f.kind.clone().id() != "covered-text" {
            real.push(f);
            theirs.push(false);
            continue;
        }
        let text = match texts.get(&f.page) {
            Some(t) => t.clone(),
            None => {
                let res = r.render(render::RenderRequest { page: f.page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
                let t = res.text.ok_or_else(|| fail(format!("page {} can't be read back", f.page + 1)))?;
                texts.insert(f.page, t.clone());
                t
            }
        };
        let info = d.info.pages.get(f.page).ok_or_else(|| fail(format!("page {} is missing", f.page + 1)))?;
        let b = f.rect;
        let hidden: Vec<(f64, f64)> = text
            .glyphs
            .iter()
            .filter(|g| !g.text.trim().is_empty())
            .filter_map(|g| {
                let [x0, y0, x1, y1] = info.view_rect_to_user(g.rect).map(f64::from);
                let area = (x1 - x0) * (y1 - y0);
                let inter = (x1.min(b[2]) - x0.max(b[0])).max(0.0) * (y1.min(b[3]) - y0.max(b[1])).max(0.0);
                (area > 0.0 && inter >= 0.2 * area).then_some(((x0 + x1) / 2.0, (y0 + y1) / 2.0))
            })
            .collect();
        if !hidden.is_empty() {
            let under_mark = hidden.iter().any(|c| marks.iter().any(|m| m.page == f.page && (m.whole_page || m.quads.iter().any(|q| in_quad(q, *c)))));
            theirs.push(!under_mark);
            real.push(f);
        }
    }
    // Text hidden under a box but under no mark was hidden in the input already.
    if !real.is_empty() && theirs.iter().all(|t| *t) {
        let pages: BTreeSet<usize> = real.iter().map(|f| f.page + 1).collect();
        let list = pages.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ");
        let page = if pages.len() == 1 { "page" } else { "pages" };
        return Err(OpdfError::Engine(format!("This PDF already hides text under a box on {page} {list}. Mark that area too to remove it.")));
    }
    if let Some(f) = real.first() {
        return Err(fail(format!(
            "{} problem{}, first on page {} ({}{})",
            real.len(),
            if real.len() == 1 { "" } else { "s" },
            f.page + 1,
            f.kind.clone().id(),
            if f.covered_text.is_empty() { String::new() } else { format!(": \"{}\"", f.covered_text) }
        )));
    }
    drop(r);
    drop(doc);
    text_under_marks(path, marks).map_err(|e| match e {
        OpdfError::Conversion(_) => e,
        e => fail(format!("the output can't be read by pdfium ({e})")),
    })
}

/// Whether point `p` lies inside quadrilateral `q` (PDF /QuadPoints order: top-left,
/// top-right, bottom-left, bottom-right; any orientation).
fn in_quad(q: &[f64; 8], p: (f64, f64)) -> bool {
    let corners = [(q[0], q[1]), (q[2], q[3]), (q[6], q[7]), (q[4], q[5])];
    let side = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
    let s: Vec<f64> = (0..4).map(|i| side(corners[i], corners[(i + 1) % 4])).collect();
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}

/// pdfium's text under each mark: any non-blank character whose centre is inside a mark (or
/// anywhere on a page marked whole) fails.
fn text_under_marks(path: &Path, marks: &[Mark]) -> Result<()> {
    if marks.is_empty() {
        return Ok(());
    }
    let _g = pdfium_lock();
    let doc = open_pdf(path, None)?;
    let pages: BTreeSet<usize> = marks.iter().map(|m| m.page).collect();
    for p in pages {
        let page = doc.pages().get(p as PdfPageIndex)?;
        let text = page.text()?;
        let here: Vec<&Mark> = marks.iter().filter(|m| m.page == p).collect();
        let whole = here.iter().any(|m| m.whole_page);
        let mut left = String::new();
        for c in text.chars().iter() {
            let Some(ch) = c.unicode_char() else { continue };
            if ch.is_whitespace() || ch == '\u{fffe}' || c.is_generated().unwrap_or(false) {
                continue;
            }
            let b = c.tight_bounds().or_else(|_| c.loose_bounds())?;
            let centre = (((b.left().value + b.right().value) / 2.0) as f64, ((b.bottom().value + b.top().value) / 2.0) as f64);
            if whole || here.iter().any(|m| m.quads.iter().any(|q| in_quad(q, centre))) {
                left.push(ch);
            }
        }
        if !left.is_empty() {
            return Err(OpdfError::Conversion(format!("redaction audit failed: text is still under a black box on page {} (\"{left}\")", p + 1)));
        }
    }
    Ok(())
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
        let n = ["secretword".to_string()];
        for inner in ["<dc:title>&#115;ecretword</dc:title>", "<dc:title>s&#x65;cretword</dc:title>", "<dc:title>secret<!-- x -->word</dc:title>", "<dc:title>secret<![CDATA[word]]></dc:title>", "<dc:title>SECRETWORD</dc:title>"] {
            let e = check_scrubbed(&with_xmp(inner), &n).unwrap_err().to_string();
            assert!(e.contains("XMP metadata"), "{inner}: {e}");
        }
        // The word in a tag or attribute name, or split by a tag, is not the text.
        for inner in ["<secretword:x/>", "<dc:title>secret</dc:title><dc:x>word</dc:x>", "<dc:title>Report</dc:title>"] {
            check_scrubbed(&with_xmp(inner), &n).unwrap_or_else(|e| panic!("{inner}: {e}"));
        }
    }

    #[test]
    fn xmp_text_keeps_values_and_drops_markup() {
        let t = xmp_text(b"<a b=\"x&amp;y\" xmlns:q=\"secret\"><!-- c -->v&lt;1<![CDATA[&#115;]]>&#115;</a>");
        assert_eq!(t.split('\n').filter(|l| !l.is_empty()).collect::<Vec<_>>(), ["x&y", "v<1&#115;s"]);
    }

    #[test]
    fn matching_ignores_case_spacing_and_unicode_form() {
        let hay = nfc("Mme E\u{301}LODIE  Martin");
        assert!(find_loose(&hay, "élodie martin").is_some());
        assert_eq!(scrub("Doc secretword", &["secretword".into()]).as_deref(), Some("Doc "));
        assert_eq!(scrub("nothing", &["secretword".into()]), None);
    }
}
