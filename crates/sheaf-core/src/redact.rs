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
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// PdfCraft's search patterns.
pub use crate::pdfcraft::engine::RedactPattern as Pattern;

/// What to look for.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    /// The text as typed (runs of spaces match any spacing, words may wrap to the next line);
    /// `whole_word` skips matches inside a longer word.
    Literal { text: String, case_insensitive: bool, whole_word: bool },
    Pattern(Pattern),
}

/// One match: its page (0-based), one quadrilateral per line it covers (user space, as PDF
/// `/QuadPoints`: top-left, top-right, bottom-left, bottom-right), and the text matched, with
/// how its search matched it (kept for the clean-up outside the page, see [`Needle`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TextMatch {
    pub page: usize,
    pub quads: Vec<[f64; 8]>,
    pub text: String,
    pub match_case: bool,
    pub whole_word: bool,
}

/// A text [`apply`] removes outside the page content (metadata, bookmarks, comments, fields),
/// matched as the search that found it did: by default any case and inside longer words.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Needle {
    pub text: String,
    pub match_case: bool,
    pub whole_word: bool,
}

impl From<&str> for Needle {
    fn from(text: &str) -> Needle {
        Needle { text: text.into(), ..Default::default() }
    }
}

impl From<String> for Needle {
    fn from(text: String) -> Needle {
        Needle { text, ..Default::default() }
    }
}

/// An area to redact on page `page` (0-based): one or more quadrilaterals in user space.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mark {
    pub page: usize,
    pub quads: Vec<[f64; 8]>,
    /// The text matched under this mark (empty for a drawn area): [`apply`] also removes it
    /// from the document information, XMP metadata, bookmarks, comments and form fields,
    /// matched with the search's options.
    pub text: String,
    pub match_case: bool,
    pub whole_word: bool,
    /// The mark is the whole page: its content is replaced by a black fill of the page and its
    /// annotations dropped, so nothing outside the media box survives either.
    pub whole_page: bool,
}

/// The line shown when [`apply`] succeeds (its audit passed).
pub const VERIFIED: &str = "No text remains under the black boxes. Matches also removed from metadata, bookmarks and comments.";

/// The result line for `areas` areas redacted (followed by [`VERIFIED`]), in Sheaf's CLI and
/// app alike.
pub fn summary(areas: usize) -> String {
    format!("{} redacted", crate::tools::count(areas, "area"))
}

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
            let (match_case, whole_word) = match q {
                Query::Literal { case_insensitive, whole_word, .. } => (!case_insensitive, *whole_word),
                Query::Pattern(_) => (false, false),
            };
            let ranges = match q {
                // Typed text in either Unicode form ("É" as one character or "E" + accent) finds
                // the page's text in either.
                Query::Literal { text: needle, case_insensitive, whole_word } => {
                    use unicode_normalization::UnicodeNormalization;
                    let (c, dcomp): (String, String) = (needle.nfc().collect(), needle.nfd().collect());
                    let mut r = text.find_opts(&c, !case_insensitive, *whole_word);
                    if dcomp != c {
                        for g in text.find_opts(&dcomp, !case_insensitive, *whole_word) {
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
                out.push(TextMatch { page, quads, text: text.text_of(g).replace('\n', " "), match_case, whole_word });
            }
        }
    }
    Ok(out)
}

/// Marks over the matches.
pub fn mark_text(matches: &[TextMatch]) -> Vec<Mark> {
    matches
        .iter()
        .map(|m| Mark { page: m.page, quads: m.quads.clone(), text: m.text.clone(), match_case: m.match_case, whole_word: m.whole_word, whole_page: false })
        .collect()
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
            Ok(Mark { page: *p, quads: vec![rect_quad(b[0])], whole_page: true, ..Default::default() })
        })
        .collect()
}

/// Applies `marks` and writes the result to `out` (never overwriting).
///
/// `searched` are the texts typed for the search, removed with the marks' own texts from the
/// document information, XMP metadata, bookmarks, comments and form fields (see [`Mark::text`]).
/// The bytes are audited ([`verify`]) under their temporary name; any finding fails the call
/// and nothing is written.
pub fn apply(input: &Input, marks: &[Mark], searched: &[Needle], out: &Path) -> Result<RedactReport> {
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
    let (bytes, mut warnings) = post_pass(&bytes, marks, &needles)?;
    let mut notes = Vec::new();
    let path = atomic_write(out, |part| {
        std::fs::write(part, &bytes)?;
        verify(part, marks)?;
        notes = check_scrubbed(&bytes, &needles)?;
        Ok(())
    })?;
    warnings.extend(notes);
    Ok(RedactReport { path, areas: marks.len(), was_encrypted, warnings })
}

/// The texts to remove outside the page content: the typed searches and every match's text.
fn needles(marks: &[Mark], searched: &[Needle]) -> Vec<Needle> {
    let mut out: Vec<Needle> = Vec::new();
    let from_marks = marks.iter().map(|m| Needle { text: m.text.clone(), match_case: m.match_case, whole_word: m.whole_word });
    for n in searched.iter().cloned().chain(from_marks) {
        let n = Needle { text: nfc(&n.text.split_whitespace().collect::<Vec<_>>().join(" ")), ..n };
        let same = |o: &Needle| o.match_case == n.match_case && o.whole_word == n.whole_word && o.text.to_lowercase() == n.text.to_lowercase();
        if !n.text.is_empty() && !out.iter().any(same) {
            out.push(n);
        }
    }
    out
}

/// `s` in Unicode normalisation form C ("E" + combining acute becomes "É").
fn nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
}

/// Where `needle` occurs in `hay` (byte range of `hay`): any run of white space in the text
/// matching any run in `hay`, both compared in Unicode form C; any case unless
/// `match_case`, and only between non-letters when `whole_word`. `hay` must already be in
/// form C (see [`nfc`]) for the range to be meaningful.
fn find_loose(hay: &str, needle: &Needle) -> Option<(usize, usize)> {
    let text = nfc(&needle.text);
    let words: Vec<Vec<char>> = text.split_whitespace().map(|w| w.chars().collect()).collect();
    if words.is_empty() {
        return None;
    }
    let same = |a: char, b: char| a == b || (!needle.match_case && a.to_lowercase().eq(b.to_lowercase()));
    let chars: Vec<(usize, char)> = hay.char_indices().collect();
    let wordy = |i: usize| chars.get(i).is_some_and(|c| c.1.is_alphanumeric());
    'start: for s in 0..chars.len() {
        if needle.whole_word && s > 0 && wordy(s - 1) {
            continue;
        }
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
        if needle.whole_word && wordy(i) {
            continue;
        }
        let end = chars.get(i).map_or(hay.len(), |c| c.0);
        return Some((chars[s].0, end));
    }
    None
}

fn contains_any(hay: &str, needles: &[Needle]) -> bool {
    let hay = nfc(hay);
    needles.iter().any(|n| find_loose(&hay, n).is_some())
}

/// `text` (in form C) without any of `needles`, or `None` when none occurs.
fn scrub(text: &str, needles: &[Needle]) -> Option<String> {
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
fn scrub_dict(d: &mut Dictionary, kinds: &[Kind], needles: &[Needle]) -> bool {
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
fn post_pass(bytes: &[u8], marks: &[Mark], needles: &[Needle]) -> Result<(Vec<u8>, Warnings)> {
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
        // Painted content keeps its drawing; what it says outside the drawing (comments,
        // marked-content properties, inline image dictionaries) is scrubbed.
        changed |= scrub_painted_streams(&mut d, &reach, needles);
        let reach = Reach::of(&d);
        changed |= remove_unused_assets(&mut d, &reach, needles, &mut warnings);
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
/// kids), so a blacked-out page keeps no field values. A field met twice (a malformed form
/// whose /Kids loop back) is kept as it is the second time.
fn prune_fields(d: &mut lopdf::Document, dropped: &HashSet<ObjectId>) -> Result<()> {
    fn keep(d: &mut lopdf::Document, list: Vec<Object>, dropped: &HashSet<ObjectId>, seen: &mut HashSet<ObjectId>) -> Vec<Object> {
        let mut out = Vec::new();
        for o in list {
            let Ok(id) = o.as_reference() else {
                out.push(o);
                continue;
            };
            if dropped.contains(&id) {
                continue;
            }
            if !seen.insert(id) {
                out.push(o);
                continue;
            }
            let kids = d.get_dictionary(id).ok().and_then(|f| f.get(b"Kids").ok()).and_then(|k| k.as_array().ok()).cloned();
            if let Some(kids) = kids {
                let had = !kids.is_empty();
                let left = keep(d, kids, dropped, seen);
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
    let fields = keep(d, fields, dropped, &mut HashSet::new());
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
    /// Font programs (/FontFile*) among the assets: their bytes are only noted, never refused.
    programs: HashSet<ObjectId>,
    /// ToUnicode CMaps among the assets: their mapped text is only noted.
    cmaps: HashSet<ObjectId>,
    /// Annotation appearance streams already walked (they are not exempt, so not in `painted`).
    appearances: HashSet<ObjectId>,
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
                        if r.appearances.insert(f) {
                            r.content(d, f, &res, false, 0);
                        }
                    }
                }
            }
        }
        // Output intents' ICC profiles describe the whole document's colour.
        let intents = d.catalog().ok().and_then(|c| c.get(b"OutputIntents").ok()).and_then(|o| resolve(d, o)).and_then(|o| o.as_array().ok()).cloned().unwrap_or_default();
        for i in intents {
            if let Some(p) = resolve(d, &i).and_then(|i| i.as_dict().ok()).and_then(|i| i.get(b"DestOutputProfile").and_then(Object::as_reference).ok()) {
                r.assets.insert(p);
            }
        }
        // The form's default resources: the fonts its fields are drawn with.
        let dr = d.catalog().ok().and_then(|c| sub_dict(d, c, b"AcroForm")).and_then(|a| sub_dict(d, a, b"DR")).cloned();
        if let Some(fonts) = dr.as_ref().and_then(|x| sub_dict(d, x, b"Font")).cloned() {
            for (_, f) in fonts.iter() {
                if let Ok(f) = f.as_reference() {
                    r.font(d, f, dr.as_ref().unwrap_or(&Dictionary::new()), 0);
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
                            let first = if painted { self.painted.insert(x) } else { self.appearances.insert(x) };
                            if first {
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
                        && (if painted { self.painted.insert(p) } else { self.appearances.insert(p) })
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
                b"FontDescriptor" | b"ToUnicode" | b"Encoding" | b"CIDSystemInfo" | b"CIDToGIDMap" => self.font_part(d, k, v),
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
    fn font_part(&mut self, d: &lopdf::Document, key: &[u8], v: &Object) {
        let Ok(r) = v.as_reference() else { return };
        if !self.assets.insert(r) {
            return;
        }
        if key == b"ToUnicode" {
            self.cmaps.insert(r);
        }
        let dict = match d.get_object(r) {
            Ok(Object::Dictionary(x)) => x.clone(),
            Ok(Object::Stream(s)) => s.dict.clone(),
            _ => return,
        };
        for (k, x) in dict.iter() {
            if let Ok(xr) = x.as_reference()
                && d.get_object(xr).is_ok_and(|o| o.as_stream().is_ok())
            {
                self.assets.insert(xr);
                if k.starts_with(b"FontFile") {
                    self.programs.insert(xr);
                }
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
    // PdfCraft's own content tags (/PCMark values) and procedure sets.
    "Redaction", "Added", "Wrap", "HeaderFooter", "PDF", "ImageB", "ImageC", "ImageI",
];

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

/// The custom structure roles and classes a document declares (its `/RoleMap` and `/ClassMap`
/// keys): names its producer chose for its styles ("Text body"), structure like PDF's own.
fn role_names(d: &lopdf::Document) -> HashSet<Vec<u8>> {
    let mut out = HashSet::new();
    let Some(root) = d.catalog().ok().and_then(|c| sub_dict(d, c, b"StructTreeRoot")) else { return out };
    for k in [&b"RoleMap"[..], b"ClassMap"] {
        if let Some(m) = sub_dict(d, root, k) {
            out.extend(m.iter().map(|(k, _)| k.clone()));
        }
    }
    out
}

fn is_standard_name(n: &[u8], roles: &HashSet<Vec<u8>>) -> bool {
    let s = String::from_utf8_lossy(n);
    PDF_NAMES.contains(&s.as_ref()) || STANDARD_KEYS.binary_search(&s.as_ref()).is_ok() || roles.contains(n)
}

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

/// One piece of a content stream: an operation (its operands and operator) or a comment.
struct Span {
    start: usize,
    end: usize,
    /// The operator, "%" for a comment, "BI" for a whole inline image.
    op: String,
    /// Inline image: where its `ID` keyword starts (its dictionary is `start + 2 .. id_at`).
    id_at: usize,
    /// Comments inside an operation (between its operands, or in an inline image dictionary),
    /// as byte ranges from their `%`. Comments outside operations are spans of their own, so
    /// spans never overlap.
    comments: Vec<(usize, usize)>,
}

/// Splits content `raw` into operations and comments by byte range, skipping strings, arrays,
/// dictionaries and inline image data, so each can be read or rewritten on its own. The spans
/// are in order and never overlap: a comment inside an operation belongs to that operation.
fn scan_content(raw: &[u8]) -> Vec<Span> {
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
fn inline_dict(raw: &[u8], s: &Span) -> Option<Dictionary> {
    let mut b = b"<<".to_vec();
    b.extend_from_slice(&uncommented(raw, s, s.start + 2, s.id_at, |_| true));
    b.extend_from_slice(b">> Zz");
    let c = lopdf::content::Content::decode(&b).ok()?;
    c.operations.into_iter().next()?.operands.into_iter().next()?.as_dict().ok().cloned()
}

/// A marked-content or marked-point operation (`BMC`, `BDC`, `MP`, `DP`) of `raw`, decoded
/// (its comments left out).
fn marked_op(raw: &[u8], s: &Span) -> Option<lopdf::content::Operation> {
    lopdf::content::Content::decode(&uncommented(raw, s, s.start, s.end, |_| true)).ok()?.operations.into_iter().next()
}

/// The text a ToUnicode CMap maps codes to (its UTF-16BE hex strings, decoded).
fn cmap_text(raw: &[u8]) -> String {
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

/// Text drawn by the text operators of `raw` read as content, the strings of one `BT … ET`
/// block joined with nothing between them, for streams that are not drawn and so are read here.
fn shown_strings(raw: &[u8]) -> String {
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
fn check_scrubbed(bytes: &[u8], needles: &[Needle]) -> Result<Vec<String>> {
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

/// Keys (and a few names) PDF and PdfCraft define, sorted: a key named so is structure, not the
/// user's text. Drawn from the PDF reference and PdfCraft's own vocabulary.
const STANDARD_KEYS: &[&str] = &[
    "A", "A85", "AA", "AC", "ADBE", "AESV2", "AESV3", "AF", "AFRelationship", "AHx", "AIMetaData", "AIS", "AP", "APRef", "AS", "ASCII85Decode",
    "ASCIIHexDecode", "AbsoluteColorimetric", "Access", "AcroForm", "Action", "ActualText", "Added", "Adobe", "After", "Align", "AllOff", "AllOn",
    "Alpha", "Alt", "Alternate", "Alternates", "Annot", "Annots", "AntiAlias", "AnyOff", "AnyOn", "App", "Approved", "Arabic", "Art", "ArtBox",
    "Artifact", "Ascent", "Attached", "Attribute", "AuthEvent", "Author", "AvgWidth", "B", "BBox", "BC", "BDC", "BE", "BEAD", "BG", "BG2", "BI", "BM",
    "BMC", "BPC", "BS", "BT", "BX", "Background", "BackgroundColor", "BaseEncoding", "BaseFont", "BaseState", "BaseVersion", "BaselineShift",
    "Before", "BibEntry", "BitsPerComponent", "BitsPerCoordinate", "BitsPerFlag", "BitsPerSample", "BlackIs1", "BlackPoint", "BleedBox", "BlockAlign",
    "BlockQuote", "Border", "BorderColor", "BorderStyle", "BorderThickness", "Bounds", "BoxColorInfo", "BrotliDecode", "Btn", "ByteRange", "C", "CA",
    "CCF", "CCITTFaxDecode", "CF", "CFM", "CI", "CIDFontType0", "CIDFontType0C", "CIDFontType2", "CIDSet", "CIDSystemInfo", "CIDToGIDMap", "CL",
    "CMYK", "CMap", "CMapName", "CMapType", "CNS1", "CO", "CP", "CRL", "CRLS", "CRLs", "CS", "CYX", "CalCMYK", "CalGray", "CalRGB", "CapHeight",
    "Caption", "Caret", "Catalog", "CenterWindow", "Cert", "Certs", "Ch", "CharProcs", "CharSet", "CheckSum", "Circle", "ClassMap", "ClosedArrow",
    "ClrF", "ClrFf", "Code", "ColSpan", "Collection", "CollectionField", "CollectionItem", "CollectionSchema", "CollectionSort", "CollectionSubitem",
    "Color", "ColorBurn", "ColorDodge", "ColorSpace", "ColorTransform", "Colorants", "Colors", "ColumnCount", "ColumnGap", "ColumnWidths", "Columns",
    "Compatible", "Components", "ContactInfo", "Contents", "Coords", "Count", "Courier", "CreationDate", "Creator", "Crop", "CropBox", "Crypt",
    "CryptFilter", "Custom", "CustomData", "D", "DA", "DCT", "DCTDecode", "DL", "DOS", "DP", "DR", "DS", "DSS", "DV", "DW", "DW2", "Darken", "Date",
    "Decode", "DecodeParms", "DefaultCMYK", "DefaultCryptFilter", "DefaultGray", "DefaultRGB", "Desc", "DescendantFonts", "Descent", "Dest",
    "DestOutputProfile", "Dests", "DeviceCMYK", "DeviceGray", "DeviceN", "DeviceRGB", "DeviceRGBA", "DeviceRgb", "Diamond", "Difference",
    "Differences", "DigestMethod", "Direction", "DisplayDocTitle", "Div", "Do", "Doc", "DocChecksum", "DocMDP", "DocTimeStamp", "Document",
    "DocumentFragment", "Duplex", "Dur", "E", "EF", "EFF", "EI", "EMC", "ET", "EX", "EarlyChange", "Em", "EmbeddedFDFs", "EmbeddedFile",
    "EmbeddedFiles", "Encode", "EncodedByteAlign", "Encoding", "Encrypt", "EncryptMetadata", "EncryptedPayload", "EndIndent", "EndOfBlock",
    "EndOfLine", "ExData", "Exclusion", "Exif", "Export", "ExportState", "ExtGState", "Extend", "Extends", "Extension", "ExtensionLevel",
    "Extensions", "Extra", "F", "FB", "FD", "FDF", "FDecodeParms", "FENote", "FFilter", "FL", "FRM", "FS", "FT", "False", "Ff", "Fields", "Figure",
    "FileAttachment", "Filespec", "Filter", "First", "FirstChar", "Fit", "FitB", "FitBH", "FitBV", "FitH", "FitR", "FitV", "FitWindow", "Fl", "Flags",
    "FlateDecode", "FlipH", "FlipV", "Folders", "Font", "FontBBox", "FontDescriptor", "FontFamily", "FontFile", "FontFile2", "FontFile3",
    "FontMatrix", "FontName", "FontStretch", "FontWeight", "Fonts", "Form", "FormType", "Formula", "FreeText", "FreeTextCallout",
    "FreeTextTypeWriter", "FullScreen", "Function", "FunctionType", "Functions", "G", "GB1", "GRAY", "GTS_PDFA1", "GTS_PDFX", "Gamma",
    "GlyphOrientationVertical", "GoTo", "GoTo3DView", "GoToE", "GoToR", "Gray", "Group", "H", "H1", "H2", "H3", "H4", "H5", "H6", "HT", "Halftone",
    "HardLight", "HeaderFooter", "Headers", "Height", "Helv", "HelvB", "Helvetica", "Hide", "HideMenubar", "HideToolbar", "HideWindowUI", "Highlight",
    "Hue", "I", "IC", "ICCBased", "ID", "IDTree", "IF", "IM", "IRT", "IT", "Icon", "Identity", "Illustrator", "Image", "ImageB", "ImageC", "ImageI",
    "ImageMask", "Images", "ImportData", "Index", "Indexed", "Info", "Ink", "InkList", "InlineAlign", "IntegerItem", "Intent", "Interpolate",
    "Issuer", "ItalicAngle", "J", "JBIG2Decode", "JBIG2Globals", "JPXDecode", "JS", "Japan1", "JavaScript", "K", "KR", "KeyUsage", "Keywords", "Kids",
    "Kind", "Korea1", "L", "LBody", "LC", "LE", "LI", "LJ", "LL", "LLE", "LLO", "LW", "LZW", "LZWDecode", "Lab", "Lang", "Last", "LastChar",
    "LastModified", "Launch", "Lbl", "Leading", "Legal", "LegalAttestation", "Length", "Length1", "Length2", "Length3", "Lighten", "Limits", "Line",
    "LineDimension", "LineHeight", "Linearized", "Link", "ListNumbering", "Lock", "Locked", "Luminosity", "M", "MCID", "MCR", "MDP", "MK", "ML",
    "MMType1", "MP", "Mac", "MacExpertEncoding", "MacRomanEncoding", "MarkInfo", "Marked", "Marker", "Mask", "Matrix", "Matte", "MaxLen", "MaxWidth",
    "Measure", "MediaBox", "Metadata", "MissingWidth", "ModDate", "MoveOrigin", "Movie", "Multiply", "MyFont", "N", "NM", "Name", "Named", "Names",
    "NeedAppearances", "NeedsRendering", "NewWindow", "Next", "Nil", "NoSuchFilter", "NonEFontNoWarn", "NonFullScreenPageMode", "NonStruct", "None",
    "Normal", "NumCopies", "Nums", "O", "OBJR", "OC", "OCG", "OCGs", "OCMD", "OCProperties", "OCSP", "OCSPs", "OE", "OFF", "OID", "ON", "OP", "OPI",
    "OPM", "OS", "ObjStm", "Off", "OneColumn", "Opaque", "Open", "OpenAction", "OpenArrow", "OpenType", "Opt", "Order", "Ordering", "Outlines",
    "OutputCondition", "OutputConditionIdentifier", "OutputIntent", "OutputIntents", "Overlay", "OverlayText", "P", "PA", "PC", "PCAdded", "PCAr",
    "PCByLine", "PCCheck", "PCCour", "PCCross", "PCCustomInitials", "PCCustomSignature", "PCDot", "PCFillSign", "PCFl0", "PCFl1", "PCFull",
    "PCFullFont", "PCHe", "PCHelv", "PCItems", "PCJp", "PCLine", "PCMark", "PCMeasureLabel", "PCMeasureValue", "PCOutline", "PCPicture",
    "PCPictureImage", "PCPictureRotate", "PCPictureSize", "PCRedacted", "PCSom", "PCStamp", "PCStampRotate", "PCTimes", "PCTypedSignature",
    "PCUndrawn", "PCXfaClick", "PCXfaImage", "PCXfaImages", "PCXfaLayout", "PCXfaOverrides", "PCXfaSHA256", "PDF", "PDFDocEncoding", "PI", "PK",
    "PNG", "PO", "PS", "PV", "PZ", "Padding", "Page", "PageLabels", "PageLayout", "PageMode", "Pages", "PaintType", "Panose", "Paperclip", "Params",
    "Parent", "ParentTree", "ParentTreeNextKey", "Part", "Path", "Pattern", "PatternType", "Perceptual", "Perms", "Pg", "Pic", "PickTrayByPDFSize",
    "PieceInfo", "Placement", "PolyLine", "PolyLineDimension", "Polygon", "PolygonCloud", "PolygonDimension", "Popup", "PreRelease", "Predictor",
    "PresSteps", "PreserveRB", "Prev", "Print", "PrintArea", "PrintClip", "PrintPageRange", "PrintScaling", "PrintState", "PrinterMark", "Private",
    "PrivateChain", "PrivateData", "ProcSet", "Producer", "ProjectFont", "Properties", "PtData", "PubSec", "Q", "QuadPoints", "Quote", "R", "R2L",
    "RB", "RBGroups", "RC", "RClosedArrow", "RD", "RG", "RGB", "RGBA", "RI", "RIPEMD160", "RL", "ROpenArrow", "RP", "RT", "RV", "Range", "Reason",
    "Reasons", "Recipients", "Rect", "RectFont", "Redact", "Redaction", "Ref", "Reference", "Registry", "RegistryName", "RelativeColorimetric",
    "Rendition", "Requirements", "ResFork", "ResetForm", "Resources", "RichMedia", "Role", "RoleMap", "Root", "Rotate", "RowSpan", "Rows", "Ruby",
    "RubyAlign", "RubyPosition", "RunLengthDecode", "S", "SA", "SC", "SCN", "SE", "SHA1", "SHA256", "SHA384", "SHA512", "SM", "SMask", "SMaskInData",
    "SS", "SV", "SVCert", "SW", "Saturation", "Schema", "Scope", "Screen", "Sect", "Separation", "SeparationInfo", "SetF", "SetFf", "SetOCGState",
    "Shading", "ShadingType", "Short", "Sig", "SigFlags", "SigRef", "SinglePage", "Size", "Slash", "SoftLight", "Sort", "Sound", "SpaceAfter",
    "SpaceBefore", "Span", "SpiderInfo", "Square", "Squiggly", "Stamp", "Standard", "StandardEncoding", "StartIndent", "State", "StateModel",
    "Status", "StdCF", "StemH", "StemV", "Stm", "StmF", "StmOwn", "StrF", "StrikeOut", "StrikeOutTextEdit", "StringItem", "Strong", "StructElem",
    "StructParent", "StructParents", "StructTreeRoot", "Style", "Sub", "SubFilter", "Subdictionary", "Subj", "Subject", "SubjectDN", "SubmitForm",
    "Subtype", "Summary", "Supplement", "Suspects", "Symbol", "Synchronous", "T", "TBody", "TBorderStyle", "TD", "TFoot", "TH", "THead", "TI", "TJ",
    "TK", "TL", "TM", "TOC", "TOCI", "TP", "TPadding", "TR", "TR2", "TRef", "TS", "TU", "Table", "Tabs", "Target", "Tc", "Td", "TemplateInstantiated",
    "Templates", "Text", "TextAlign", "TextDecorationColor", "TextDecorationThickness", "TextDecorationType", "TextIndent", "Tf", "Thread", "Threads",
    "Thumb", "TilingType", "TimeStamp", "Title", "Tj", "Tm", "ToUnicode", "Toggle", "Tr", "Trans", "TransformMethod", "TransformParams",
    "Transparency", "Trapped", "TrimBox", "True", "TrueType", "TrustedMode", "Ts", "Tw", "TwoColumnLeft", "TwoColumnRight", "TwoPageLeft",
    "TwoPageRight", "Tx", "Type", "Type0", "Type1", "Type1C", "Type3", "Tz", "U", "UCR", "UCR2", "UE", "UF", "URI", "URL", "URLType", "Underline",
    "Unix", "Usage", "UseAttachments", "UseCMap", "UseOC", "UseOutlines", "UseThumbs", "UserProperties", "UserUnit", "V", "V2", "VE", "VP", "VRI",
    "Value", "VendorKey", "VendorNote", "VendorState", "Version", "Vertices", "VerticesPerRow", "View", "ViewArea", "ViewClip", "ViewState",
    "ViewerPreferences", "Volume", "W", "W2", "WC", "WMode", "WP", "WS", "WT", "Warichu", "Watermark", "WhitePoint", "Widget", "Width", "Widths",
    "WinAnsiEncoding", "Wrap", "WritingMode", "X", "XFA", "XHeight", "XML", "XObject", "XRef", "XRefStm", "XStep", "XYZ", "Y", "YStep", "Yes", "ZaDb",
    "ZapfDingbats", "Zoom",
];

/// One object written as PDF syntax (through lopdf's content writer).
fn object_bytes(o: &Object) -> Option<Vec<u8>> {
    let c = lopdf::content::Content { operations: vec![lopdf::content::Operation::new("Zz", vec![o.clone()])] };
    let mut b = c.encode().ok()?;
    b.truncate(b.len().checked_sub(3)?);
    Some(b)
}

/// `raw` (painted content) with `needles` removed from what [`Audit::painted`] reads: comments
/// holding one are dropped; marked-content property lists and inline image dictionaries are
/// scrubbed like any value; a marked-content tag holding one becomes `/Span`. `None` when
/// nothing changed.
fn scrub_painted(raw: &[u8], needles: &[Needle]) -> Option<Vec<u8>> {
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

/// Scrubs every painted stream of `d` (see [`scrub_painted`]); returns whether one changed.
fn scrub_painted_streams(d: &mut lopdf::Document, reach: &Reach, needles: &[Needle]) -> bool {
    let mut changed = false;
    for id in &reach.painted {
        let Some(Object::Stream(s)) = d.objects.get_mut(id) else { continue };
        if let Some(b) = scrub_painted(&decoded(s), needles) {
            s.set_plain_content(b);
            changed = true;
        }
    }
    changed
}

/// Fonts and colour spaces listed in a `/Resources` that no painted content or appearance uses,
/// whose dictionary, program or character map holds one of `needles`: removed from every
/// resource dictionary, each named in a warning.
fn remove_unused_assets(d: &mut lopdf::Document, reach: &Reach, needles: &[Needle], warnings: &mut Warnings) -> bool {
    let mut gone: Vec<(Vec<u8>, ObjectId, &'static str)> = Vec::new();
    for o in d.objects.values() {
        let dict = match o {
            Object::Dictionary(x) => x,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        let Some(res) = sub_dict(d, dict, b"Resources") else { continue };
        for (cat, label) in [(&b"Font"[..], "font"), (b"ColorSpace", "colour profile")] {
            let Some(list) = sub_dict(d, res, cat) else { continue };
            for (name, v) in list.iter() {
                let refs: Vec<ObjectId> = match resolve(d, v) {
                    _ if v.as_reference().is_ok() => vec![v.as_reference().unwrap_or((0, 0))],
                    Some(Object::Array(a)) => a.iter().filter_map(|x| x.as_reference().ok()).collect(),
                    _ => Vec::new(),
                };
                for r in refs {
                    if reach.assets.contains(&r) || gone.iter().any(|g| g.1 == r) {
                        continue;
                    }
                    let mut seen = HashSet::new();
                    let hit = d.get_object(r).is_ok_and(|o| holds(d, o, needles, &mut seen)) || seen.iter().any(|x| d.get_object(*x).is_ok_and(|o| o.as_stream().is_ok_and(|s| contains_any(&cmap_text(&decoded(s)), needles))));
                    if hit {
                        gone.push((name.clone(), r, label));
                    }
                }
            }
        }
    }
    if gone.is_empty() {
        return false;
    }
    for (name, _, label) in &gone {
        let n = String::from_utf8_lossy(name);
        if *label == "font" {
            warn(warnings, &format!("Unused font {n} removed: it contained the redacted text"));
        } else {
            warn(warnings, &format!("Unused colour profile {n} removed: it contained the redacted text"));
        }
    }
    let ids: HashSet<ObjectId> = gone.iter().map(|g| g.1).collect();
    remove_objects(d, &ids);
    true
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
fn scrub_any(o: &mut Object, needles: &[Needle]) -> Scrubbed {
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
fn scrub_dict_all(d: &mut Dictionary, needles: &[Needle]) -> bool {
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
fn holds(d: &lopdf::Document, o: &Object, needles: &[Needle], seen: &mut HashSet<ObjectId>) -> bool {
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
        let hidden: Vec<((f64, f64), &str)> = text
            .glyphs
            .iter()
            .filter(|g| !g.text.trim().is_empty())
            .filter_map(|g| {
                let [x0, y0, x1, y1] = info.view_rect_to_user(g.rect).map(f64::from);
                let area = (x1 - x0) * (y1 - y0);
                let inter = (x1.min(b[2]) - x0.max(b[0])).max(0.0) * (y1.min(b[3]) - y0.max(b[1])).max(0.0);
                (area > 0.0 && inter >= 0.2 * area).then_some((((x0 + x1) / 2.0, (y0 + y1) / 2.0), g.text.as_str()))
            })
            .collect();
        if hidden.is_empty() {
            continue;
        }
        let under_mark = hidden.iter().any(|(c, _)| marks.iter().any(|m| m.page == f.page && (m.whole_page || m.quads.iter().any(|q| in_quad(q, *c)))));
        // A box the input already had hides text only over a letter or digit: a frame over
        // private-use glyphs, spaces or punctuation is decoration.
        if under_mark || hidden.iter().any(|(_, t)| hides_text(t)) {
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

/// Whether covered text `t` is text a box can hide: it has a Unicode letter or digit.
fn hides_text(t: &str) -> bool {
    t.chars().any(char::is_alphanumeric)
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

    #[test]
    fn xmp_text_keeps_values_and_drops_markup() {
        let t = xmp_text(b"<a b=\"x&amp;y\" xmlns:q=\"secret\"><!-- c -->v&lt;1<![CDATA[&#115;]]>&#115;</a>");
        assert_eq!(t.split('\n').filter(|l| !l.is_empty()).collect::<Vec<_>>(), ["x&y", "v<1&#115;s"]);
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

    #[test]
    fn only_letters_and_digits_count_as_hidden_text() {
        assert!(!hides_text("\u{f001}") && !hides_text(" ") && !hides_text("•–.") && !hides_text(""));
        assert!(hides_text("ab") && hides_text("7") && hides_text("é"));
    }

    #[test]
    fn matching_ignores_case_spacing_and_unicode_form() {
        let hay = nfc("Mme E\u{301}LODIE  Martin");
        assert!(find_loose(&hay, &"élodie martin".into()).is_some());
        assert_eq!(scrub("Doc secretword", &["secretword".into()]).as_deref(), Some("Doc "));
        assert_eq!(scrub("nothing", &["secretword".into()]), None);
        // The search's options hold outside the page too: "Ann" as a whole word, case kept.
        let ann = Needle { text: "Ann".into(), match_case: true, whole_word: true };
        assert_eq!(scrub("Annual planning", std::slice::from_ref(&ann)), None);
        assert_eq!(scrub("ann notes", std::slice::from_ref(&ann)), None);
        assert_eq!(scrub("By Ann, 2026", std::slice::from_ref(&ann)).as_deref(), Some("By , 2026"));
    }
}
