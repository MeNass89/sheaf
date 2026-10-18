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

fn is_xmp(s: &lopdf::Stream) -> bool {
    s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Metadata".as_slice())
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
}

/// The keys whose text values are scrubbed, by kind: what a reader shows as text. Names-tree
/// keys, destinations, URIs, default appearances, field names and anything else are left alone.
fn scrub_keys(k: Kind) -> &'static [&'static [u8]] {
    match k {
        Kind::Info => &[],
        Kind::Outline => &[b"Title"],
        Kind::Annot => &[b"Contents", b"T", b"Subj", b"RC"],
        Kind::Field => &[b"V", b"DV", b"TU"],
    }
}

/// The kinds of `d` (an annotation widget can be its own field, so possibly two).
fn kinds(d: &Dictionary, outline_items: &HashSet<ObjectId>, id: ObjectId) -> Vec<Kind> {
    let name = |k: &[u8]| d.get(k).and_then(Object::as_name).ok();
    let mut out = Vec::new();
    if outline_items.contains(&id) {
        out.push(Kind::Outline);
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

/// Scrubs one text value: `Some(None)` means "remove the key" (nothing is left of it).
fn scrub_value(v: &Object, needles: &[String]) -> Option<Option<Object>> {
    match v {
        Object::String(..) => scrub(&decode(v), needles).map(|t| (!t.trim().is_empty()).then(|| lopdf::text_string(&t))),
        _ => None,
    }
}

/// Scrubs the allow-listed keys of `d` (every text value for the document information).
fn scrub_dict(d: &mut Dictionary, kinds: &[Kind], needles: &[String]) -> bool {
    let keys: Vec<Vec<u8>> = d
        .iter()
        .filter(|(k, _)| kinds.iter().any(|kd| *kd == Kind::Info || scrub_keys(*kd).contains(&k.as_slice())))
        .map(|(k, _)| k.clone())
        .collect();
    let mut changed = false;
    for k in keys {
        let Some(v) = d.get(&k).ok().and_then(|v| scrub_value(v, needles)) else { continue };
        changed = true;
        match v {
            Some(v) => d.set(k, v),
            None => {
                d.remove(&k);
            }
        }
    }
    changed
}

/// What the post-pass did that the user must be told.
type Warnings = Vec<String>;

/// The written bytes, after PdfCraft: whole-page marks emptied; `needles` removed from the
/// document information, bookmark titles, comments and form field values; XMP metadata,
/// attachments and JavaScript that contain one removed whole (each named in the warnings).
/// Unchanged bytes are returned as they are.
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
    let ids: Vec<ObjectId> = d.objects.keys().copied().collect();
    let mut fields_changed = false;
    for id in ids {
        let Some(Object::Dictionary(dict)) = d.objects.get_mut(&id) else { continue };
        let ks = kinds(dict, &outlines, id);
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
    for (id, o) in &d.objects {
        let Object::Stream(s) = o else { continue };
        if is_xmp(s) && contains_any(&xmp_text(&s.decompressed_content().unwrap_or_else(|_| s.content.clone())), needles) {
            gone.insert(*id);
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
        for id in &gone {
            d.objects.remove(id);
        }
        // Every reference to a removed stream goes too (dangling ones would be null anyway):
        // /Metadata keys, /EF entries, /JS keys.
        for o in d.objects.values_mut() {
            let dict = match o {
                Object::Dictionary(x) => x,
                Object::Stream(s) => &mut s.dict,
                _ => continue,
            };
            let keys: Vec<Vec<u8>> = dict.iter().filter(|(_, v)| v.as_reference().is_ok_and(|r| gone.contains(&r))).map(|(k, _)| k.clone()).collect();
            for k in keys {
                dict.remove(&k);
            }
            if let Ok(Object::Dictionary(ef)) = dict.get_mut(b"EF") {
                let keys: Vec<Vec<u8>> = ef.iter().filter(|(_, v)| v.as_reference().is_ok_and(|r| gone.contains(&r))).map(|(k, _)| k.clone()).collect();
                for k in keys {
                    ef.remove(&k);
                }
            }
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

/// Fails when a searched text is still where [`post_pass`] removes it: a scrubbed text value
/// (document information, bookmark titles, comments, form field values: decoded from
/// PDFDocEncoding, UTF-16BE or UTF-8), XMP metadata (read as [`xmp_text`] does), or any
/// other stream that is not page content, a form, an image or a font program (attachments,
/// JavaScript, …). Comparison is in Unicode form C, case-insensitive.
fn check_scrubbed(bytes: &[u8], needles: &[String]) -> Result<()> {
    if needles.is_empty() {
        return Ok(());
    }
    let fail = |what: &str| OpdfError::Conversion(format!("redaction audit failed: a redacted text is still in the document's {what}"));
    let d = lopdf::Document::load_mem(bytes).map_err(|e| OpdfError::Conversion(format!("redaction audit failed: the output can't be re-read ({e})")))?;
    let hit = |o: &Object| matches!(o, Object::String(..)) && contains_any(&decode(o), needles);
    // Document information.
    let info = d.trailer.get(b"Info").ok().and_then(|i| d.dereference(i).ok()).and_then(|(_, i)| i.as_dict().ok().cloned());
    if info.is_some_and(|i| i.iter().any(|(_, v)| hit(v))) {
        return Err(fail("information (title, author, subject, keywords)"));
    }
    // Allow-listed values of bookmarks, comments and fields.
    let outlines = outline_items(&d);
    for (id, o) in &d.objects {
        let Object::Dictionary(dict) = o else { continue };
        for k in kinds(dict, &outlines, *id) {
            if scrub_keys(k).iter().any(|key| dict.get(key).is_ok_and(hit)) {
                return Err(fail(match k {
                    Kind::Outline => "bookmarks",
                    Kind::Annot => "comments",
                    _ => "form fields",
                }));
            }
        }
        if dict.get(b"S").and_then(Object::as_name).ok() == Some(b"JavaScript".as_slice()) && dict.get(b"JS").is_ok_and(hit) {
            return Err(fail("JavaScript"));
        }
    }
    // Streams that are not drawn: XMP, attachments, scripts and anything else.
    let drawn = drawn_streams(&d);
    for (id, o) in &d.objects {
        let Object::Stream(s) = o else { continue };
        // Object and cross-reference streams hold the objects checked one by one above.
        let ty = s.dict.get(b"Type").and_then(Object::as_name).ok();
        if drawn.contains(id) || matches!(ty, Some(b"ObjStm" | b"XRef")) {
            continue;
        }
        let raw = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
        let text = if is_xmp(s) { xmp_text(&raw) } else { String::from_utf8_lossy(&raw).into_owned() };
        if contains_any(&text, needles) {
            return Err(fail(if is_xmp(s) { "XMP metadata" } else { "attachments or other embedded data" }));
        }
    }
    Ok(())
}

/// Streams that draw: page contents, form and image XObjects, font programs (their text is
/// checked on the page by [`verify`], or is out of reach: see its blind spots).
fn drawn_streams(d: &lopdf::Document) -> HashSet<ObjectId> {
    let mut out = HashSet::new();
    let refs = |o: &Object| -> Vec<ObjectId> {
        match o {
            Object::Reference(r) => vec![*r],
            Object::Array(a) => a.iter().filter_map(|x| x.as_reference().ok()).collect(),
            _ => Vec::new(),
        }
    };
    for (id, o) in &d.objects {
        let dict = match o {
            Object::Dictionary(x) => x,
            Object::Stream(s) => {
                let sub = s.dict.get(b"Subtype").and_then(Object::as_name).ok();
                // Forms, images, font programs (Length1, or a FontFile3 /Subtype), ICC profiles.
                let font3 = matches!(sub, Some(b"Type1C" | b"CIDFontType0C" | b"OpenType"));
                if matches!(sub, Some(b"Form" | b"Image")) || font3 || s.dict.has(b"Length1") || (s.dict.has(b"N") && s.dict.has(b"Alternate")) || (s.dict.has(b"N") && !s.dict.has(b"Type")) {
                    out.insert(*id);
                }
                &s.dict
            }
            _ => continue,
        };
        if dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Page".as_slice())
            && let Ok(c) = dict.get(b"Contents")
        {
            out.extend(refs(c));
            if let Ok(Object::Reference(r)) = dict.get(b"Contents")
                && let Ok(Object::Array(a)) = d.get_object(*r)
            {
                out.extend(a.iter().filter_map(|x| x.as_reference().ok()));
            }
        }
        for k in [&b"FontFile"[..], b"FontFile2", b"FontFile3", b"CharProcs"] {
            match dict.get(k) {
                Ok(Object::Dictionary(cp)) => out.extend(cp.iter().filter_map(|(_, v)| v.as_reference().ok())),
                Ok(o) => out.extend(refs(o)),
                Err(_) => {}
            }
        }
    }
    out
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
/// checked. Outside the pages, [`apply`] also checks that no searched text is left in metadata,
/// bookmarks, comments, form values, attachments or scripts, but not inside font programs
/// (compressed or not: glyph names, a subset's name), images (text drawn as pixels), ICC
/// profiles, or text values under keys it does not scrub (names-tree keys, destinations,
/// URIs, field names). Anything that can't be read fails.
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
