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
//!
//! This file holds the public API. The parts behind it:
//! - `matching`: how a searched text is found in a string (case, spacing, Unicode form);
//! - `clean`: the clean-up outside the page content, after PdfCraft (`post_pass`);
//! - `audit`: the deny-by-default audit of every object of the final bytes (`check_scrubbed`);
//! - `verify`: the check of the pages themselves ([`verify`]);
//! - `reach`: what the pages paint and draw with, which the audit exempts;
//! - `content`: content streams read and rewritten by byte range (comments, marked content,
//!   inline images);
//! - `pdf`: PDF strings, streams, XMP and dictionaries decoded;
//! - `names`: the names and keys PDF defines, structure rather than text.

mod audit;
mod clean;
mod content;
mod matching;
mod names;
mod pdf;
mod reach;
mod verify;

use crate::pdfcraft::engine::{rect_quad, Edit, NewAnnotation, Shape, Style};
use crate::pdfcraft::{render, Doc};
use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use audit::check_scrubbed;
use clean::post_pass;
use matching::nfc;
use std::path::{Path, PathBuf};
pub use verify::{verify, verify_with};


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

/// What the marks applied were: text passages (selected or found), whole pages, drawn areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MarkCounts {
    pub texts: usize,
    pub pages: usize,
    pub areas: usize,
}

impl MarkCounts {
    pub fn of(marks: &[Mark]) -> MarkCounts {
        let pages = marks.iter().filter(|m| m.whole_page).count();
        let texts = marks.iter().filter(|m| !m.whole_page && !m.text.is_empty()).count();
        MarkCounts { texts, pages, areas: marks.len() - pages - texts }
    }
}

/// The result line for what was redacted (followed by [`VERIFIED`]), in Sheaf's CLI and app
/// alike: "2 text passages, 1 page and 1 area redacted".
pub fn summary(c: MarkCounts) -> String {
    use crate::tools::count;
    let parts: Vec<String> = [(c.texts, "text passage"), (c.pages, "page"), (c.areas, "area")].iter().filter(|(n, _)| *n > 0).map(|(n, w)| count(*n, w)).collect();
    match parts.as_slice() {
        [] => "Nothing redacted".into(),
        [one] => format!("{one} redacted"),
        [rest @ .., last] => format!("{} and {last} redacted", rest.join(", ")),
    }
}

/// What [`apply`] wrote.
#[derive(Debug, Clone)]
pub struct RedactReport {
    pub path: PathBuf,
    /// Marks applied (a match over two lines is one area).
    pub areas: usize,
    /// Of what kinds.
    pub counts: MarkCounts,
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

/// How applied marks look (Acrobat's Redaction Properties ▸ Appearance): the boxes' colour, and a
/// text drawn over each (a reason, "Redacted"), once or repeated to fill it. Whole pages take the
/// colour only.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub fill: [f64; 3],
    pub overlay: String,
    pub repeat: bool,
    /// Points added round each mark (not whole pages), so letters' edges and accents go too.
    pub margin: f64,
}

impl Default for Appearance {
    fn default() -> Self {
        Self { fill: [0.0; 3], overlay: String::new(), repeat: false, margin: 0.0 }
    }
}

/// `marks` each grown by `margin` points on every side (its quads' box), whole pages as they are.
pub fn grown(marks: &[Mark], margin: f64) -> Vec<Mark> {
    if margin <= 0.0 {
        return marks.to_vec();
    }
    marks
        .iter()
        .map(|m| {
            if m.whole_page {
                return m.clone();
            }
            let quads = m
                .quads
                .iter()
                .map(|q| {
                    let (xs, ys) = ([q[0], q[2], q[4], q[6]], [q[1], q[3], q[5], q[7]]);
                    let lo = |v: [f64; 4]| v.into_iter().fold(f64::INFINITY, f64::min);
                    let hi = |v: [f64; 4]| v.into_iter().fold(f64::NEG_INFINITY, f64::max);
                    rect_quad([lo(xs) - margin, lo(ys) - margin, hi(xs) + margin, hi(ys) + margin])
                })
                .collect();
            Mark { quads, ..m.clone() }
        })
        .collect()
}

/// [`apply_with`] black boxes and no text over them.
pub fn apply(input: &Input, marks: &[Mark], searched: &[Needle], out: &Path) -> Result<RedactReport> {
    apply_with(input, marks, searched, &Appearance::default(), out)
}

/// Applies `marks` and writes the result to `out` (never overwriting).
///
/// `searched` are the texts typed for the search, removed with the marks' own texts from the
/// document information, XMP metadata, bookmarks, comments and form fields (see [`Mark::text`]).
/// The bytes are audited ([`verify`]) under their temporary name; any finding fails the call
/// and nothing is written.
pub fn apply_with(input: &Input, marks: &[Mark], searched: &[Needle], look: &Appearance, out: &Path) -> Result<RedactReport> {
    let marks = &grown(marks, look.margin.clamp(0.0, 36.0))[..];
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
            // The overlay in white on a dark box, black on a light one.
            let light = look.fill.iter().sum::<f64>() > 1.5;
            let text = crate::pdfcraft::engine::OverlayLook { color: if light { [0.0; 3] } else { [1.0; 3] }, repeat: look.repeat, ..Default::default() };
            let shape = Shape::Redact { quads: m.quads.clone(), overlay: look.overlay.trim().to_string(), look: text };
            let style = Style { fill: Some(look.fill), ..Style::default_for(&shape) };
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
    let (bytes, mut warnings) = post_pass(&bytes, marks, &needles, look.fill)?;
    let mut notes = Vec::new();
    let path = atomic_write(out, |part| {
        std::fs::write(part, &bytes)?;
        verify_with(part, marks, &look.overlay)?;
        notes = check_scrubbed(&bytes, &needles)?;
        Ok(())
    })?;
    warnings.extend(notes);
    Ok(RedactReport { path, areas: marks.len(), counts: MarkCounts::of(marks), was_encrypted, warnings })
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
