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
                Query::Literal { text: needle, case_insensitive } => text.find_opts(needle, !case_insensitive, false),
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
    let bytes = post_pass(&bytes, marks, &needles)?;
    let path = atomic_write(out, |part| {
        std::fs::write(part, &bytes)?;
        verify(part, marks)?;
        check_scrubbed(&bytes, &needles)
    })?;
    Ok(RedactReport { path, areas: marks.len(), was_encrypted })
}

/// The texts to remove outside the page content: the typed searches and every match's text.
fn needles(marks: &[Mark], searched: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in searched.iter().chain(marks.iter().map(|m| &m.text)) {
        let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&t)) {
            out.push(t);
        }
    }
    out
}

/// Where `needle` occurs in `hay` (byte range): case-insensitive, any run of white space in
/// `needle` matching any run in `hay`.
fn find_loose(hay: &str, needle: &str) -> Option<(usize, usize)> {
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

/// `text` without any of `needles` (removed until none is left: each removal shortens it).
fn scrub(text: &str, needles: &[String]) -> Option<String> {
    let mut t = text.to_string();
    let mut changed = false;
    while let Some((a, b)) = needles.iter().find_map(|n| find_loose(&t, n)) {
        t.replace_range(a..b, "");
        changed = true;
    }
    changed.then_some(t)
}

fn decode(s: &Object) -> String {
    lopdf::decode_text_string(s).unwrap_or_else(|_| s.as_str().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default())
}

/// XMP text with the five XML entities decoded, as a reader of the metadata sees it.
fn xmp_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn is_xmp(s: &lopdf::Stream) -> bool {
    s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"Metadata".as_slice())
}

/// Removes `needles` from every string outside page content; returns whether one changed.
fn scrub_object(o: &mut Object, needles: &[String]) -> bool {
    match o {
        Object::String(..) => match scrub(&decode(o), needles) {
            Some(t) => {
                *o = lopdf::text_string(&t);
                true
            }
            None => false,
        },
        Object::Array(a) => a.iter_mut().fold(false, |c, x| scrub_object(x, needles) | c),
        Object::Dictionary(d) => scrub_dict(d, needles),
        Object::Stream(s) => scrub_dict(&mut s.dict, needles),
        _ => false,
    }
}

fn scrub_dict(d: &mut Dictionary, needles: &[String]) -> bool {
    d.iter_mut().fold(false, |c, (_, v)| scrub_object(v, needles) | c)
}

/// The written bytes, after PdfCraft: whole-page marks emptied, `needles` removed from
/// metadata, bookmarks, comments and form fields. Unchanged bytes are returned as they are.
fn post_pass(bytes: &[u8], marks: &[Mark], needles: &[String]) -> Result<Vec<u8>> {
    let whole: BTreeSet<usize> = marks.iter().filter(|m| m.whole_page).map(|m| m.page).collect();
    if whole.is_empty() && needles.is_empty() {
        return Ok(bytes.to_vec());
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
    // Every string outside page content (document information, bookmarks, annotations, form
    // fields, structure), then XMP streams.
    let ids: Vec<ObjectId> = d.objects.keys().copied().collect();
    let mut fields_changed = false;
    for id in ids {
        let Some(o) = d.objects.get_mut(&id) else { continue };
        if !scrub_object(o, needles) {
            continue;
        }
        changed = true;
        // An annotation or a field whose text changed loses its appearance (it still shows the
        // old text); viewers draw it again from the new value.
        if let Ok(dict) = o.as_dict_mut() {
            let field = dict.has(b"FT") || dict.has(b"V");
            if dict.has(b"Rect") || field {
                dict.remove(b"AP");
            }
            if field {
                fields_changed = true;
                let kids: Vec<ObjectId> = dict.get(b"Kids").and_then(Object::as_array).map(|a| a.iter().filter_map(|x| x.as_reference().ok()).collect()).unwrap_or_default();
                for k in kids {
                    if let Ok(kd) = d.get_object_mut(k).and_then(Object::as_dict_mut) {
                        kd.remove(b"AP");
                    }
                }
            }
        }
    }
    if let Ok(v) = d.trailer.get_mut(b"Info") {
        changed |= scrub_object(v, needles);
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
    let mut gone_xmp: Vec<ObjectId> = Vec::new();
    for (id, o) in d.objects.iter_mut() {
        let Object::Stream(s) = o else { continue };
        if !is_xmp(s) {
            continue;
        }
        let raw = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
        let text = String::from_utf8_lossy(&raw).into_owned();
        if let Some(t) = scrub(&text, needles) {
            s.set_plain_content(t.into_bytes());
            changed = true;
        }
        let now = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
        // An occurrence written with XML entities: the stream goes.
        if needles.iter().any(|n| find_loose(&xmp_text(&now), n).is_some()) {
            gone_xmp.push(*id);
        }
    }
    if !gone_xmp.is_empty() {
        for o in d.objects.values_mut() {
            let dict = match o {
                Object::Dictionary(x) => x,
                Object::Stream(s) => &mut s.dict,
                _ => continue,
            };
            if dict.get(b"Metadata").and_then(Object::as_reference).is_ok_and(|r| gone_xmp.contains(&r)) {
                dict.remove(b"Metadata");
            }
        }
        changed = true;
    }
    if !changed {
        return Ok(bytes.to_vec());
    }
    d.prune_objects();
    let mut out = Vec::new();
    d.save_to(&mut out)?;
    Ok(out)
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

/// Fails when a searched text is still in a string outside page content (document
/// information, bookmarks, comments, form fields: PDFDocEncoding or UTF-16BE, decoded) or in
/// XMP metadata (UTF-8, entities decoded).
fn check_scrubbed(bytes: &[u8], needles: &[String]) -> Result<()> {
    if needles.is_empty() {
        return Ok(());
    }
    fn found(o: &Object, needles: &[String]) -> bool {
        match o {
            Object::String(..) => {
                let t = decode(o);
                needles.iter().any(|n| find_loose(&t, n).is_some())
            }
            Object::Array(a) => a.iter().any(|x| found(x, needles)),
            Object::Dictionary(d) => d.iter().any(|(_, v)| found(v, needles)),
            Object::Stream(s) => {
                s.dict.iter().any(|(_, v)| found(v, needles))
                    || (is_xmp(s) && {
                        let raw = s.decompressed_content().unwrap_or_else(|_| s.content.clone());
                        let t = xmp_text(&raw);
                        needles.iter().any(|n| find_loose(&t, n).is_some())
                    })
            }
            _ => false,
        }
    }
    let d = lopdf::Document::load_mem(bytes).map_err(|e| OpdfError::Conversion(format!("redaction audit failed: the output can't be re-read ({e})")))?;
    let info = d.trailer.get(b"Info").ok().map(|i| d.dereference(i).map(|x| x.1.clone()).unwrap_or(Object::Null));
    if d.objects.values().any(|o| found(o, needles)) || info.is_some_and(|i| found(&i, needles)) {
        return Err(OpdfError::Conversion("redaction audit failed: a redacted text is still in the document's metadata, bookmarks, comments or form fields".into()));
    }
    Ok(())
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
/// checked. Anything that can't be read fails.
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
