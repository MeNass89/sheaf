//! The reader's document lives on a thread of its own: PdfCraft's work on a large file is
//! noticeable (a 70 MB book: 0.2 s per annotation, 1.2 s per save, 4.5 s to open), and the
//! window must never wait for it.
//! The UI sends operations in order and reads a snapshot of what it needs, refreshed after each.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use sheaf_app::viewer::{self, PageBox};
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::engine::Edit;
use sheaf_core::pdfcraft::render;
use sheaf_core::pdfcraft::Doc;

/// An annotation of the document, as the panel and hit tests need it.
#[derive(Clone)]
pub struct AnnotEntry {
    pub page: usize,
    pub index: usize,
    pub subtype: String,
    pub color: Option<[f32; 3]>,
    pub contents: String,
    /// Bounds in user space, and the markup quads (user space) if any.
    pub rect: [f64; 4],
    pub quads: Vec<[f64; 8]>,
    /// A Fill & Sign mark: a placed signature, initials, a typed text or a check.
    pub fill_sign: bool,
    /// Who made it and when it last changed, as shown ("" when the file does not say).
    pub author: String,
    pub modified: String,
    /// Replies to it, oldest first: "author: text"; and its latest review status ("Accepted").
    pub replies: Vec<String>,
    pub status: String,
    /// Checked with Acrobat's check mark (its last Marked/Unmarked reply is Marked).
    pub checked: bool,
    /// A text box's format: (standard font resource name, "TiBo"; size in points; alignment 0
    /// left, 1 centre, 2 right).
    pub font: Option<(&'static str, f64, u8)>,
}

/// A paragraph of a page as the edit tool shows it (from PdfCraft's `text_blocks`).
#[derive(Clone)]
pub struct Block {
    /// User space.
    pub rect: [f64; 4],
    /// The text to edit: wraps as spaces ([`reflowed`]).
    pub text: String,
    /// The text as drawn, a newline between rows.
    pub drawn: String,
}

/// A paragraph's editor text with its wraps as spaces (a hyphen at a wrap is kept, joined), so
/// an edit rewraps the whole paragraph as PDF Expert's does; line breaks the author typed stay.
/// `soft` has one entry per row break; a text that does not match it is left as it is.
fn reflowed(editor_text: &str, soft: &[bool]) -> String {
    let rows: Vec<&str> = editor_text.split('\n').collect();
    if rows.len() != soft.len() + 1 {
        return editor_text.to_string();
    }
    let mut out = String::from(rows[0]);
    for (row, wrap) in rows[1..].iter().zip(soft) {
        if !wrap {
            out.push('\n');
        } else if !out.ends_with(char::is_whitespace) && !out.ends_with('-') {
            out.push(' ');
        }
        out.push_str(row);
    }
    out
}

/// What the last edit drew otherwise than typed, as Acrobat says it: characters the paragraph's
/// font lacks drawn in another, or characters no font has drawn as `?` (a comment, a field).
fn edit_note(d: &sheaf_core::pdfcraft::engine::Document) -> Option<String> {
    if let Some(font) = &d.font_substituted {
        return Some(format!("Some characters aren't in this document's font: shown in {font}"));
    }
    let n = d.edit_notices.first()?.to_string();
    let mut c = n.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect())
}

/// A form field as the reader fills it.
#[derive(Clone)]
pub struct FieldEntry {
    pub name: String,
    /// 0 text, 1 check box, 2 radio, 3 choice (combo or list), 4 other (button, signature).
    pub kind: i32,
    pub value: Vec<String>,
    /// Choice: (export value, display text).
    pub options: Vec<(String, String)>,
    pub read_only: bool,
    /// (page, rect in user space, the widget's "on" state for check boxes and radios, tab order).
    pub widgets: Vec<(usize, [f64; 4], Option<String>, usize)>,
}

/// What the window knows of the document, as of the last operation.
#[derive(Clone)]
pub struct Snap {
    pub boxes: Vec<PageBox>,
    pub labels: Vec<String>,
    pub links: Arc<Vec<render::Link>>,
    pub outline: Arc<Vec<render::OutlineItem>>,
    pub annots: Arc<Vec<AnnotEntry>>,
    /// The working file (text layer, search) and what is displayed (renders).
    pub bytes: Arc<Vec<u8>>,
    pub display: Arc<Vec<u8>>,
    pub password: Option<String>,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub read_only: String,
    pub fields: Arc<Vec<FieldEntry>>,
    pub props: Arc<Props>,
    /// Files attached to the document or to its pages.
    pub attachments: Arc<Vec<sheaf_core::pdfcraft::render::Attachment>>,
}

/// Writes attachment `att` of the PDF `bytes` (opened with `password`) to `to`.
pub fn save_attachment(bytes: &[u8], password: Option<&str>, att: &sheaf_core::pdfcraft::render::Attachment, to: &std::path::Path) -> Result<(), String> {
    let data = sheaf_core::pdfcraft::render::attachment_data(bytes, password, att)?;
    replace(to, &data).map_err(|e| e.to_string())
}

/// Document Properties: the four entries one can edit, and what is only shown (name, value).
#[derive(Clone, Default, Debug)]
pub struct Props {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
    pub about: Vec<(String, String)>,
}

fn props(d: &sheaf_core::pdfcraft::engine::Document) -> Props {
    let i = &d.info;
    let or_none = |v: &Option<String>| v.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "—".into());
    let page = i.pages.first().map(|p| {
        let (w, h) = (p.width as f64 / 72.0 * 25.4, p.height as f64 / 72.0 * 25.4);
        format!("{w:.0} × {h:.0} mm")
    });
    let mb = i.file_size as f64 / 1_048_576.0;
    let about = vec![
        ("Pages".into(), i.pages.len().to_string()),
        ("Page size".into(), page.unwrap_or_else(|| "—".into())),
        ("File size".into(), if mb >= 1.0 { format!("{mb:.1} MB") } else { format!("{:.0} KB", i.file_size as f64 / 1024.0) }),
        ("PDF version".into(), i.pdf_version.clone()),
        ("Created with".into(), or_none(&i.creator)),
        ("PDF producer".into(), or_none(&i.producer)),
        ("Security".into(), if i.encrypted { "Encrypted".into() } else { "None".into() }),
        ("Tagged (accessible)".into(), if i.tagged { "Yes".into() } else { "No".into() }),
        ("Fonts".into(), i.fonts.len().to_string()),
    ];
    let get = |v: &Option<String>| v.clone().unwrap_or_default();
    Props { title: get(&i.title), author: get(&i.author), subject: get(&i.subject), keywords: get(&i.keywords), about }
}

/// The review status a status reply sets: Acrobat's check mark (state model Marked) is none and
/// leaves the status as it was; "None" clears it.
fn review_status(state: &str) -> Option<&str> {
    match state {
        "Marked" | "Unmarked" => None,
        "None" => Some(""),
        s => Some(s),
    }
}

fn snap(doc: &Doc) -> Snap {
    // Text boxes' fonts, read from the document as edited (the format row shows them): only the
    // pages holding one are read.
    let mut pages: Vec<usize> = doc.document().info.annotations.iter().filter(|a| a.subtype == "FreeText").map(|a| a.page).collect();
    pages.sort_unstable();
    pages.dedup();
    let fonts: std::collections::HashMap<(usize, usize), (&'static str, f64, u8)> =
        if pages.is_empty() { Default::default() } else { doc.session().text_box_fonts(doc.id(), &pages).into_iter().collect() };
    let d = doc.document();
    // Replies and status replies point at their comment's name (/IRT → /NM).
    let mut replies: std::collections::HashMap<&str, Vec<String>> = std::collections::HashMap::new();
    let mut status: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    let mut checked: std::collections::HashMap<&str, bool> = std::collections::HashMap::new();
    for a in &d.info.annotations {
        let Some(parent) = a.in_reply_to.as_deref() else { continue };
        match a.state.as_deref() {
            Some(s) => {
                if matches!(s, "Marked" | "Unmarked") {
                    checked.insert(parent, s == "Marked");
                }
                if let Some(s) = review_status(s) {
                    status.insert(parent, s.to_string());
                }
            }
            None => replies.entry(parent).or_default().push(format!("{}: {}", a.author.as_deref().unwrap_or("—"), a.contents.as_deref().unwrap_or("").trim())),
        }
    }
    let mut annots: Vec<AnnotEntry> = d
        .info
        .annotations
        .iter()
        .filter(|a| sheaf_app::reader::listed(&a.subtype) && a.in_reply_to.is_none())
        .map(|a| AnnotEntry {
            page: a.page,
            index: a.index,
            subtype: a.subtype.clone(),
            color: a.color,
            contents: a.contents.clone().unwrap_or_default(),
            rect: a.rect.map(f64::from),
            quads: a.quads.iter().map(|q| q.map(f64::from)).collect(),
            fill_sign: a.fill_sign,
            author: a.author.clone().unwrap_or_default(),
            modified: a.modified.clone().unwrap_or_default(),
            replies: a.name.as_deref().and_then(|n| replies.get(n)).cloned().unwrap_or_default(),
            checked: a.name.as_deref().and_then(|n| checked.get(n)).copied().unwrap_or(false),
            status: a.name.as_deref().and_then(|n| status.get(n)).cloned().unwrap_or_default(),
            font: fonts.get(&(a.page, a.index)).copied(),
        })
        .collect();
    annots.sort_by_key(|a| (a.page, a.index));
    Snap {
        boxes: viewer::boxes_of(&d.info.pages),
        labels: d.info.pages.iter().map(|p| p.label.clone()).collect(),
        links: Arc::new(d.info.links.clone()),
        outline: Arc::new(d.info.outline.clone()),
        annots: Arc::new(annots),
        bytes: d.bytes.clone(),
        display: d.display.clone(),
        password: d.password.clone(),
        dirty: d.dirty,
        can_undo: d.can_undo().is_some(),
        can_redo: d.can_redo().is_some(),
        read_only: d.read_only_reason.as_ref().map(|r| format!("Read only: {r}")).unwrap_or_default(),
        fields: Arc::new(fields(d)),
        props: Arc::new(props(d)),
        attachments: Arc::new(d.info.attachments.clone()),
    }
}

fn fields(d: &sheaf_core::pdfcraft::engine::Document) -> Vec<FieldEntry> {
    use sheaf_core::pdfcraft::engine::{field_flags, FormFieldKind as K};
    d.form
        .iter()
        .map(|f| FieldEntry {
            name: f.name.clone(),
            kind: match f.kind {
                K::Text => 0,
                K::CheckBox => 1,
                K::Radio => 2,
                K::Combo | K::List => 3,
                _ => 4,
            },
            value: f.value.clone(),
            options: f.options.clone(),
            read_only: f.flags & field_flags::READ_ONLY != 0,
            widgets: f.widgets.iter().filter_map(|w| Some((w.page?, w.rect, w.on_state.clone(), w.tab))).collect(),
        })
        .collect()
}

/// The web addresses `found` (page, line boxes, address; user space) whose boxes no link covers yet
/// (more than half of a box under a link of its page): linking twice adds nothing.
pub fn unlinked(found: Vec<(usize, Vec<[f64; 4]>, String)>, links: &[render::Link]) -> Vec<(usize, Vec<[f64; 4]>, String)> {
    let covered = |page: usize, r: &[f64; 4]| {
        let area = ((r[2] - r[0]) * (r[3] - r[1])).max(1e-9);
        links.iter().filter(|l| l.page == page).any(|l| {
            let l = l.rect.map(f64::from);
            let (w, h) = (r[2].min(l[2]) - r[0].max(l[0]), r[3].min(l[3]) - r[1].max(l[1]));
            w > 0.0 && h > 0.0 && w * h > 0.5 * area
        })
    };
    found
        .into_iter()
        .filter_map(|(page, rects, uri)| {
            let rects: Vec<[f64; 4]> = rects.into_iter().filter(|r| !covered(page, r)).collect();
            (!rects.is_empty()).then_some((page, rects, uri))
        })
        .collect()
}

pub enum Op {
    /// An edit; `pages` are the pages it changes (rendered again).
    Apply { edit: Box<Edit>, pages: Vec<usize> },
    /// `pages`: the pages the edit undone touched (`None`: unknown, all render again).
    Undo { pages: Option<Vec<usize>> },
    Redo { pages: Option<Vec<usize>> },
    /// Writes the edits: into the file itself as an incremental update, or (`to`) a full copy
    /// that becomes the document's file.
    Save { to: Option<PathBuf> },
    /// The paragraphs of a page (Edit text).
    Blocks(usize),
    /// `pages` written to a new PDF at `to`.
    Extract { pages: Vec<usize>, to: PathBuf },
    /// Acrobat's Create Links from URLs: every web address in the text not linked yet, in `style`.
    LinkUrls { style: sheaf_core::pdfcraft::engine::LinkStyle },
    /// Acrobat's Summarize Comments: a PDF listing every comment, by page, written to `to`.
    Summary { to: PathBuf },
    /// Every comment exported to `to`: XFDF, or FDF when `to` ends in .fdf.
    ExportComments { to: PathBuf },
    /// A paragraph being edited, shown on its page as typed: the last preview is taken back,
    /// then `edit` (if any) applied in its place, on `pages`. A preview is not an undo step; any
    /// other edit, undo, redo or save takes it back first. Answered by `Changed`, not applied.
    Preview { edit: Option<Box<Edit>>, pages: Vec<usize> },
}

pub enum Reply {
    Opened(Box<Snap>),
    OpenFailed(sheaf_core::OpdfError),
    /// An edit, undo or redo is done (or failed): the pages to render again (`None`: all).
    /// `applied`: the answer to an `Apply` (an undo step was added unless `error`); `note`: what
    /// the edit could not draw as typed ([`edit_note`]); `preview`: the answer to a `Preview`.
    Changed { snap: Box<Snap>, pages: Option<Vec<usize>>, error: Option<String>, applied: bool, note: Option<String>, preview: bool },
    Saved { snap: Box<Snap>, to: PathBuf, error: Option<String> },
    /// Comments exported to `to` (or why not).
    Exported { to: PathBuf, error: Option<String> },
    /// A page's paragraphs and its images' boxes (user space).
    /// A page's paragraphs and its images: (visible box, full box), user space; a cropped image
    /// shows less than its full box, which is what a move translates.
    /// A page's paragraphs, its images (visible and full box) and its objects as Edit a PDF
    /// selects them together (inventory target, box; user space).
    Blocks(usize, Vec<Block>, Vec<([f64; 4], [f64; 4])>, Vec<(sheaf_core::pdfcraft::engine::ObjectTarget, [f64; 4])>),
    Extracted { to: PathBuf, error: Option<String> },
}

/// Starts the document's thread: it opens `input`, then runs the operations sent on the
/// returned channel, answering each through `reply` (called on that thread).
pub fn start(input: Input, reply: impl Fn(Reply) + Send + 'static) -> Sender<Op> {
    let (tx, rx) = channel::<Op>();
    std::thread::Builder::new()
        .name("reader-doc".into())
        .spawn(move || {
            let t0 = std::time::Instant::now();
            let mut doc = match sheaf_app::jobs::catching(|| Doc::open(&input)) {
                Ok(d) => d,
                Err(e) => return reply(Reply::OpenFailed(e)),
            };
            debug(format_args!("document opened in {:?}", t0.elapsed()));
            reply(Reply::Opened(Box::new(snap(&doc))));
            let mut path = input.path.clone();
            // A preview on the document: the dirty, undo and redo state before it, which the
            // window is shown (a preview is not a change to save or undo).
            let mut preview: Option<(bool, bool, bool)> = None;
            for op in rx {
                let t0 = std::time::Instant::now();
                // Reading the paragraphs (asked after each change, the preview's too) keeps it.
                if !matches!(op, Op::Blocks(_)) && preview.take().is_some() {
                    let id = doc.id();
                    let _ = doc.session_mut().withdraw(id);
                    if !matches!(op, Op::Preview { .. }) {
                        debug(format_args!("a preview was left on the document: taken back"));
                    }
                }
                match op {
                    Op::Preview { edit, pages } => {
                        let d = doc.document();
                        let before = (d.dirty, d.can_undo().is_some(), d.can_redo().is_some());
                        let error = match edit {
                            Some(e) => {
                                let id = doc.id();
                                let r = doc.session_mut().preview(id, *e).err().map(|e| e.to_string());
                                if r.is_none() {
                                    preview = Some(before);
                                }
                                r
                            }
                            None => None,
                        };
                        let note = if preview.is_some() { edit_note(doc.document()) } else { None };
                        let mut s = snap(&doc);
                        (s.dirty, s.can_undo, s.can_redo) = before;
                        reply(Reply::Changed { snap: Box::new(s), pages: Some(pages), error, applied: false, note, preview: true });
                    }
                    Op::Apply { edit, pages } => {
                        let error = doc.apply(*edit).err().map(|e| e.to_string());
                        let note = if error.is_none() { edit_note(doc.document()) } else { None };
                        let t1 = std::time::Instant::now();
                        let s = snap(&doc);
                        debug(format_args!("apply {:?}, snapshot {:?}", t1 - t0, t1.elapsed()));
                        reply(Reply::Changed { snap: Box::new(s), pages: Some(pages), error, applied: true, note, preview: false });
                    }
                    op @ (Op::Undo { .. } | Op::Redo { .. }) => {
                        let (redo, pages) = match op {
                            Op::Redo { pages } => (true, pages),
                            Op::Undo { pages } => (false, pages),
                            _ => unreachable!(),
                        };
                        let id = doc.id();
                        let r = if redo { doc.session_mut().redo(id) } else { doc.session_mut().undo(id) };
                        reply(Reply::Changed { snap: Box::new(snap(&doc)), pages, error: r.err().map(|e| e.to_string()), applied: false, note: None, preview: false });
                    }
                    Op::LinkUrls { style } => {
                        let id = doc.id();
                        let found = unlinked(doc.session().find_urls(id), &doc.document().info.links);
                        let pages: Vec<usize> = found.iter().map(|f| f.0).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                        let n: usize = found.iter().map(|f| f.1.len()).sum();
                        let error = if n == 0 { Some("No web addresses to link: none in the text, or all linked already".to_string()) } else { doc.apply(Edit::AddLinks { links: found, style }).err().map(|e| e.to_string()) };
                        let note = (error.is_none()).then(|| format!("{} added", sheaf_core::tools::count(n, "link")));
                        // An undo step was counted for it by the window: `applied` even when refused, so it is taken back.
                        reply(Reply::Changed { snap: Box::new(snap(&doc)), pages: Some(pages), error, applied: true, note, preview: false });
                    }
                    Op::Save { to } => {
                        let target = to.clone().unwrap_or_else(|| path.clone());
                        let error = save(&mut doc, &target).err();
                        if error.is_none() {
                            path = target.clone();
                        }
                        reply(Reply::Saved { snap: Box::new(snap(&doc)), to: target, error });
                    }
                    Op::Extract { pages, to } => {
                        let id = doc.id();
                        let r = doc.session().extract(id, &pages).map_err(|e| e.to_string()).and_then(|bytes| replace(&to, &bytes).map_err(|e| e.to_string()));
                        reply(Reply::Extracted { to, error: r.err() });
                    }
                    Op::ExportComments { to } => {
                        let id = doc.id();
                        let fdf = to.extension().is_some_and(|e| e.eq_ignore_ascii_case("fdf"));
                        let format = if fdf { sheaf_core::pdfcraft::engine::DataFormat::Fdf } else { sheaf_core::pdfcraft::engine::DataFormat::Xfdf };
                        let r = doc.session().export_data(id, format, true, false).map_err(|e| e.to_string()).and_then(|bytes| replace(&to, &bytes).map_err(|e| e.to_string()));
                        reply(Reply::Exported { to, error: r.err() });
                    }
                    Op::Summary { to } => {
                        let id = doc.id();
                        let r = doc.session().summarize_comments(id, Default::default()).map_err(|e| e.to_string()).and_then(|bytes| replace(&to, &bytes).map_err(|e| e.to_string()));
                        reply(Reply::Extracted { to, error: r.err() });
                    }
                    Op::Blocks(page) => {
                        let b = doc
                            .document()
                            .text_blocks(page)
                            .into_iter()
                            .filter(|b| !b.text.trim().is_empty())
                            .map(|b| Block { rect: b.rect, text: reflowed(&b.editor_text, &b.soft_breaks), drawn: b.editor_text })
                            .collect();
                        let images = doc.document().page_images(page).iter().map(|i| (i.visible_rect.unwrap_or(i.rect), i.rect)).collect();
                        let objects = doc.document().editable_objects(page).unwrap_or_default().into_iter().map(|o| (o.target, o.rect)).collect();
                        reply(Reply::Blocks(page, b, images, objects));
                    }
                }
                debug(format_args!("operation done in {:?}", t0.elapsed()));
            }
        })
        .expect("spawn the reader's document thread");
    tx
}

/// Save and Save As both write an incremental update (the file as opened or last saved, then the
/// changes): it keeps signatures valid, and on a 1,982-page book Save As takes 1 s instead of 13 s
/// for a full rewrite (which also refuses signed files). A compact rewrite is Compress's job.
fn save(doc: &mut Doc, target: &std::path::Path) -> Result<(), String> {
    let id = doc.id();
    let t = std::time::Instant::now();
    let bytes = doc.session().save_bytes(id).map_err(|e| e.to_string())?;
    let t1 = std::time::Instant::now();
    // The bytes must reopen with every page before they replace a file; otherwise the file on
    // disk stays as it was.
    doc.session().check_save(id, &bytes).map_err(|e| e.to_string())?;
    let t2 = std::time::Instant::now();
    replace(target, &bytes).map_err(|e| e.to_string())?;
    let t3 = std::time::Instant::now();
    let r = doc.session_mut().mark_saved(id, bytes, Some(target.to_string_lossy().into_owned())).map_err(|e| e.to_string());
    debug(format_args!("save: bytes {:?}, check {:?}, write {:?}, mark saved {:?}", t1 - t, t2 - t1, t3 - t2, t3.elapsed()));
    r
}

/// Writes `bytes` to `target` in place: a temporary file next to it, flushed, then renamed over
/// it, so a crash leaves either the old file or the new one, never half of one. (Tools use
/// `paths::atomic_write`, which never overwrites; saving a document must.)
pub fn replace(target: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "document.pdf".into());
    let tmp = target.with_file_name(format!(".{name}.{}.saving", std::process::id()));
    let r = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        // Keep the file's permissions.
        if let Ok(m) = std::fs::metadata(target) {
            let _ = std::fs::set_permissions(&tmp, m.permissions());
        }
        std::fs::rename(&tmp, target)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

pub fn debug(args: std::fmt::Arguments) {
    if std::env::var_os("SHEAF_READER_DEBUG").is_some() {
        eprintln!("reader: {args}");
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_check_mark_is_not_a_review_status() {
        assert_eq!(review_status("Accepted"), Some("Accepted"));
        assert_eq!(review_status("Marked"), None);
        assert_eq!(review_status("Unmarked"), None);
        assert_eq!(review_status("None"), Some(""));
    }
    #[test]
    fn wraps_become_spaces_and_typed_breaks_stay() {
        let soft = [true, false, true];
        assert_eq!(super::reflowed("one two\nthree\n- item pseudo-\nmarisques", &soft), "one two three\n- item pseudo-marisques");
        assert_eq!(super::reflowed("ends with space \nnext", &[true]), "ends with space next");
        // Rows and flags that disagree: left as drawn.
        assert_eq!(super::reflowed("a\nb", &[]), "a\nb");
    }

    use super::*;
    use pdfium_render::prelude::*;
    use sheaf_core::pdfcraft::engine::{Markup, NewAnnotation, NoteIcon, Shape, Style};
    use std::path::Path;
    use std::sync::mpsc::{channel, Receiver};
    use std::time::Duration;

    /// A PDF whose page i (1-based) says "Page i".
    fn fixture(dir: &Path, pages: u32) -> PathBuf {
        let _g = sheaf_core::engine::pdfium_lock();
        let mut doc = sheaf_core::engine::new_document();
        let font = doc.fonts_mut().helvetica();
        for i in 1..=pages {
            let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
            page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(720.0), format!("Page {i}"), font, PdfPoints::new(24.0)).unwrap();
        }
        let path = dir.join("doc.pdf");
        doc.save_to_file(&path).unwrap();
        path
    }

    fn started(path: &Path) -> (Sender<Op>, Receiver<Reply>, Snap) {
        let (tx, rx) = channel();
        let ops = start(Input::new(path), move |r| {
            let _ = tx.send(r);
        });
        let snap = match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            Reply::Opened(s) => *s,
            _ => panic!("expected Opened"),
        };
        (ops, rx, snap)
    }

    /// The next Changed or Saved reply: its snapshot and error.
    fn next(rx: &Receiver<Reply>) -> (Snap, Option<String>) {
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            Reply::Changed { snap, error, .. } | Reply::Saved { snap, error, .. } => (*snap, error),
            Reply::Extracted { error, .. } => panic!("unexpected Extracted ({error:?})"),
            _ => panic!("unexpected reply"),
        }
    }

    fn highlight(page: usize) -> Edit {
        Edit::AddAnnotation(NewAnnotation {
            page,
            shape: Shape::TextMarkup { kind: Markup::Highlight, quads: vec![[72.0, 744.0, 160.0, 744.0, 72.0, 718.0, 160.0, 718.0]] },
            style: Style::default(),
            contents: "Page 1".into(),
            author: String::new(),
        })
    }

    fn qpdf_clean(path: &Path) {
        let o = std::process::Command::new("qpdf").arg("--check").arg(path).output().unwrap();
        let s = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        assert!(o.status.success() && !s.contains("WARNING"), "qpdf --check: {s}");
    }

    fn texts(path: &Path) -> Vec<String> {
        let d = lopdf::Document::load(path).unwrap();
        (1..=d.get_pages().len() as u32).map(|i| d.extract_text(&[i]).unwrap().trim().to_string()).collect()
    }

    /// What an edit costs on a large file (`SHEAF_BENCH_PDF`): run with --ignored --nocapture,
    /// and SHEAF_READER_DEBUG=1 for the engine / snapshot split.
    #[test]
    #[ignore]
    fn edit_cost_on_a_big_file() {
        let Some(path) = std::env::var_os("SHEAF_BENCH_PDF") else { return };
        let t = std::time::Instant::now();
        let (ops, rx, snap) = started(Path::new(&path));
        println!("open: {:?} ({} pages)", t.elapsed(), snap.boxes.len());
        // SHEAF_BENCH_BOXES=n: n text boxes first, one a page (each snapshot reads their fonts).
        let boxes = std::env::var("SHEAF_BENCH_BOXES").ok().and_then(|n| n.parse().ok()).unwrap_or(0).min(snap.boxes.len());
        for page in 0..boxes {
            let shape = Shape::TextBox { rect: [72.0, 600.0, 272.0, 640.0], font_size: 12.0 };
            let style = Style::default_for(&shape);
            let annotation = NewAnnotation { page, shape, style, contents: "Bench".into(), author: String::new() };
            ops.send(Op::Apply { edit: Box::new(Edit::AddTextBox { annotation, font: "TiRo".into(), align: 0 }), pages: vec![page] }).unwrap();
            next(&rx);
        }
        let edits = std::env::var("SHEAF_BENCH_EDITS").ok().and_then(|n| n.parse().ok()).unwrap_or(3);
        for i in 0..edits {
            let t = std::time::Instant::now();
            ops.send(Op::Apply { edit: Box::new(highlight(i)), pages: vec![i] }).unwrap();
            let (_, err) = next(&rx);
            println!("highlight {i}: {:?} {err:?}", t.elapsed());
        }
        let t = std::time::Instant::now();
        ops.send(Op::Undo { pages: Some(vec![2]) }).unwrap();
        next(&rx);
        println!("undo: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let copy = std::env::temp_dir().join("sheaf-bench-save.pdf");
        ops.send(Op::Save { to: Some(copy.clone()) }).unwrap();
        let (_, err) = next(&rx);
        println!("save as: {:?} {err:?}", t.elapsed());
        ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
        next(&rx);
        let t = std::time::Instant::now();
        ops.send(Op::Save { to: None }).unwrap();
        let (_, err) = next(&rx);
        println!("save (incremental): {:?} {err:?}", t.elapsed());
        let _ = std::fs::remove_file(copy);
    }

    /// What a search reads on a large file (`SHEAF_BENCH_PDF`): the text layer of every page,
    /// as the reader's search thread does. Run with --ignored --nocapture.
    #[test]
    #[ignore]
    fn search_cost_on_a_big_file() {
        use sheaf_core::pdfcraft::render;
        let Some(path) = std::env::var_os("SHEAF_BENCH_PDF") else { return };
        let bytes = Arc::new(std::fs::read(path).unwrap());
        let n = render::PageRenderer::new(bytes.clone(), render::RenderConfig::default()).page_count();
        let texts = std::sync::Mutex::new(std::collections::HashMap::new());
        let t = std::time::Instant::now();
        sheaf_app::reader::read_texts(&bytes, &render::RenderConfig::default(), n, &Default::default(), &|| false, |p, x| {
            texts.lock().unwrap().insert(p, x);
            true
        });
        let first = t.elapsed();
        let known: std::collections::HashMap<usize, Arc<render::PageText>> = texts.into_inner().unwrap().into_iter().filter_map(|(p, x)| Some((p, x?))).collect();
        let hits: usize = known.values().map(|x| x.find_opts("tumor", false, false).len()).sum();
        let t = std::time::Instant::now();
        sheaf_app::reader::read_texts(&bytes, &render::RenderConfig::default(), n, &known, &|| false, |_, _| true);
        println!("{n} pages, {hits} hits: first search {first:?}, again {:?}", t.elapsed());
    }

    #[test]
    fn a_summary_of_comments_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 2);
        let (ops, rx, _) = started(&path);
        ops.send(Op::Apply { edit: Box::new(highlight(1)), pages: vec![1] }).unwrap();
        next(&rx);
        let out = dir.path().join("summary.pdf");
        ops.send(Op::Summary { to: out.clone() }).unwrap();
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            Reply::Extracted { to, error } => assert!(error.is_none() && to == out, "{error:?}"),
            _ => panic!("expected Extracted"),
        }
        let text = texts(&out).join("\n");
        assert!(text.contains("Summary of Comments") && text.contains("Highlight"), "{text}");
    }

    #[test]
    fn a_reply_is_listed_under_its_comment() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, _) = started(&path);
        ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
        let (snap, _) = next(&rx);
        let index = snap.annots[0].index;
        ops.send(Op::Apply { edit: Box::new(Edit::ReplyToAnnotation { page: 0, index, text: "Agreed".into(), author: "Ada".into() }), pages: vec![0] }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(snap.annots.len(), 1, "a reply is not a comment of its own");
        assert_eq!(snap.annots[0].replies, ["Ada: Agreed"]);
    }

    #[test]
    fn a_review_status_shows_on_its_comment() {
        use sheaf_core::pdfcraft::engine::ReviewState;
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, _) = started(&path);
        ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
        let (snap, _) = next(&rx);
        let index = snap.annots[0].index;
        ops.send(Op::Apply { edit: Box::new(Edit::SetAnnotationStatus { page: 0, index, state: ReviewState::Accepted, author: "Ada".into() }), pages: vec![0] }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        assert_eq!((snap.annots.len(), snap.annots[0].status.as_str()), (1, "Accepted"));
    }

    #[test]
    fn bookmarks_are_added_renamed_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 3);
        let (ops, rx, snap) = started(&path);
        assert!(snap.outline.is_empty());
        let send = |e: Edit| ops.send(Op::Apply { edit: Box::new(e), pages: vec![] }).unwrap();
        send(Edit::AddBookmark { parent: vec![], index: 0, title: "Page 2".into(), page: 1 });
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        assert_eq!((snap.outline[0].title.as_str(), snap.outline[0].page), ("Page 2", Some(1)));
        send(Edit::RenameBookmark { path: vec![0], title: "Methods".into() });
        let (snap, _) = next(&rx);
        assert_eq!(snap.outline[0].title, "Methods");
        send(Edit::DeleteBookmark { path: vec![0] });
        let (snap, _) = next(&rx);
        assert!(snap.outline.is_empty());
    }

    #[test]
    fn a_text_box_takes_a_font_grows_to_fit_and_undoes_in_one_step() {
        let dir = tempfile::tempdir().unwrap();
        let (ops, rx, _) = started(&fixture(dir.path(), 1));
        let send = |e: Edit| ops.send(Op::Apply { edit: Box::new(e), pages: vec![0] }).unwrap();
        // 100 × 20 pt is one line of 24 pt text: the box grows down to keep all of it.
        let shape = Shape::TextBox { rect: [72.0, 700.0, 172.0, 720.0], font_size: 24.0 };
        let style = Style { color: [0.0; 3], width: 0.0, ..Style::default_for(&shape) };
        let annotation = NewAnnotation { page: 0, shape, style, contents: "A text box that wraps".into(), author: String::new() };
        send(Edit::AddTextBox { annotation, font: "TiBo".into(), align: 1 });
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        let boxes = |s: &Snap| s.annots.iter().filter(|a| a.subtype == "FreeText").cloned().collect::<Vec<_>>();
        let b = &boxes(&snap)[0];
        assert_eq!(b.font, Some(("TiBo", 24.0, 1)));
        assert!(b.rect[3] - b.rect[1] > 60.0 && (b.rect[3] - 720.0).abs() < 0.01, "grew down from its top: {:?}", b.rect);
        send(Edit::SetTextFont { page: 0, index: b.index, font: "CoBO".into(), size: Some(10.0), align: Some(2) });
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(boxes(&snap).len(), 1, "restyled, not added");
        assert_eq!(boxes(&snap)[0].font, Some(("CoBO", 10.0, 2)));
        // One undo per step: back to Times Bold, then no box at all.
        ops.send(Op::Undo { pages: None }).unwrap();
        assert_eq!(boxes(&next(&rx).0)[0].font, Some(("TiBo", 24.0, 1)));
        ops.send(Op::Undo { pages: None }).unwrap();
        assert!(boxes(&next(&rx).0).is_empty());
    }

    #[test]
    fn a_file_is_attached_listed_and_saved_back() {
        use sheaf_core::pdfcraft::engine::AttachIcon;
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, snap) = started(&path);
        assert!(snap.attachments.is_empty());
        let shape = Shape::Attachment { at: [72.0, 700.0], icon: AttachIcon::Paperclip, file: "notes.txt".into(), data: b"hello attachment".to_vec() };
        let style = Style::default_for(&shape);
        ops.send(Op::Apply { edit: Box::new(Edit::AddAnnotation(NewAnnotation { page: 0, shape, style, contents: String::new(), author: String::new() })), pages: vec![0] }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(snap.attachments.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["notes.txt"]);
        let out = dir.path().join("out.txt");
        save_attachment(&snap.bytes, None, &snap.attachments[0], &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"hello attachment");
    }

    #[test]
    fn comments_are_exported_and_imported() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, _) = started(&path);
        ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
        next(&rx);
        let xfdf = dir.path().join("c.xfdf");
        ops.send(Op::ExportComments { to: xfdf.clone() }).unwrap();
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            Reply::Exported { error, .. } => assert!(error.is_none(), "{error:?}"),
            _ => panic!("expected Exported"),
        }
        let text = std::fs::read_to_string(&xfdf).unwrap();
        assert!(text.contains("<xfdf") && text.contains("highlight"), "{text}");
        // Into another copy of the document, without comments: the highlight comes in.
        let other = dir.path().join("other");
        std::fs::create_dir(&other).unwrap();
        let path2 = fixture(&other, 1);
        let (ops2, rx2, snap2) = started(&path2);
        assert!(snap2.annots.is_empty());
        ops2.send(Op::Apply { edit: Box::new(Edit::ImportData { name: "c.xfdf".into(), bytes: Arc::new(std::fs::read(&xfdf).unwrap()) }), pages: vec![0] }).unwrap();
        let (snap2, err) = next(&rx2);
        assert!(err.is_none(), "{err:?}");
        assert_eq!(snap2.annots.iter().map(|a| a.subtype.as_str()).collect::<Vec<_>>(), ["Highlight"]);
    }

    /// ⌘S twice with an edit between: each save appends, and the light refresh after an
    /// appended save (patched engine) keeps the comment list and undo in step with the file.
    #[test]
    fn incremental_saves_in_a_row_keep_every_edit() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 3);
        let (ops, rx, _) = started(&path);
        ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
        next(&rx);
        ops.send(Op::Save { to: None }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none() && !snap.dirty, "{err:?}");
        let first = std::fs::metadata(&path).unwrap().len();
        ops.send(Op::Apply { edit: Box::new(highlight(2)), pages: vec![2] }).unwrap();
        let (snap, _) = next(&rx);
        assert_eq!(snap.annots.iter().map(|a| a.page).collect::<Vec<_>>(), [0, 2]);
        ops.send(Op::Save { to: None }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none() && !snap.dirty && snap.annots.len() == 2, "{err:?}");
        assert!(std::fs::metadata(&path).unwrap().len() > first);
        // Undo after the save still works on the saved state.
        ops.send(Op::Undo { pages: Some(vec![2]) }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none() && snap.dirty && snap.annots.len() == 1, "{err:?}");
        qpdf_clean(&path);
        let (_, _, reopened) = started(&path);
        assert_eq!(reopened.annots.iter().map(|a| a.page).collect::<Vec<_>>(), [0, 2]);
    }

    #[test]
    fn an_annotation_is_saved_into_the_file_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 2);
        let before = std::fs::metadata(&path).unwrap().len();
        let (ops, rx, snap) = started(&path);
        assert!(snap.annots.is_empty() && !snap.dirty && snap.boxes.len() == 2);
        ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        assert!(snap.dirty && snap.can_undo);
        assert_eq!(snap.annots.iter().map(|a| a.subtype.as_str()).collect::<Vec<_>>(), ["Highlight"]);
        ops.send(Op::Save { to: None }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none() && !snap.dirty, "{err:?}");
        // An incremental update: the original bytes, then the change.
        assert!(std::fs::metadata(&path).unwrap().len() > before);
        qpdf_clean(&path);
        let (_, _, again) = started(&path);
        assert_eq!(again.annots.len(), 1);
        assert_eq!(again.annots[0].subtype, "Highlight");
    }

    #[test]
    fn a_link_is_added_changed_and_removed_and_saved() {
        use sheaf_core::pdfcraft::engine::{LinkAction, LinkStyle};
        use sheaf_core::pdfcraft::render::LinkTarget;
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 3);
        let (ops, rx, _) = started(&path);
        let add = Edit::AddLink { page: 0, rect: [72.0, 700.0, 200.0, 740.0], action: LinkAction::Page(2), style: LinkStyle::default() };
        ops.send(Op::Apply { edit: Box::new(add), pages: vec![0] }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        let l = snap.links.iter().find(|l| l.page == 0).expect("the link is read back");
        assert!(matches!(l.target, LinkTarget::Page(2, _)), "{:?}", l.target);
        let index = l.index;
        let set = Edit::SetLink { page: 0, index, rect: None, action: Some(LinkAction::Uri("https://example.org".into())), style: None };
        ops.send(Op::Apply { edit: Box::new(set), pages: vec![0] }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        assert!(snap.links.iter().any(|l| matches!(&l.target, LinkTarget::Uri(u) if u == "https://example.org")), "{:?}", snap.links);
        ops.send(Op::Save { to: None }).unwrap();
        let (_, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        qpdf_clean(&path);
        let (ops, rx, again) = started(&path);
        assert_eq!(again.links.len(), 1);
        ops.send(Op::Apply { edit: Box::new(Edit::DeleteLink { page: 0, index: again.links[0].index }), pages: vec![0] }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none() && snap.links.is_empty(), "{err:?}");
    }

    #[test]
    fn a_visible_link_is_drawn_by_its_appearance() {
        use sheaf_core::pdfcraft::engine::{LinkAction, LinkHighlight, LinkStyle};
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, _) = started(&path);
        let style = LinkStyle { visible: true, color: [0.85, 0.2, 0.15], width: 2.0, dashed: true, underline: false, highlight: LinkHighlight::Invert };
        ops.send(Op::Apply { edit: Box::new(Edit::AddLink { page: 0, rect: [72.0, 700.0, 200.0, 740.0], action: LinkAction::Page(0), style }), pages: vec![0] }).unwrap();
        assert!(next(&rx).1.is_none());
        ops.send(Op::Save { to: None }).unwrap();
        assert!(next(&rx).1.is_none());
        qpdf_clean(&path);
        let qdf = std::process::Command::new("qpdf").args(["--qdf", "--object-streams=disable"]).arg(&path).arg("-").output().unwrap().stdout;
        let qdf = String::from_utf8_lossy(&qdf);
        // Its border as an appearance: dashed, 2 pt, in its colour (readers that draw only
        // appearances showed nothing).
        assert!(qdf.contains("/AP") && qdf.contains("0.850 0.200 0.150 RG 2.00 w [3] 0 d"), "{qdf}");
    }

    #[test]
    fn web_addresses_in_the_text_are_linked_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("urls.pdf");
        {
            let _g = sheaf_core::engine::pdfium_lock();
            let mut doc = sheaf_core::engine::new_document();
            let font = doc.fonts_mut().helvetica();
            let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
            page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(720.0), "Visit https://example.org today", font, PdfPoints::new(18.0)).unwrap();
            doc.save_to_file(&path).unwrap();
        }
        let (ops, rx, snap) = started(&path);
        assert!(snap.links.is_empty());
        let style = sheaf_core::pdfcraft::engine::LinkStyle::default();
        ops.send(Op::LinkUrls { style }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_none(), "{err:?}");
        assert!(matches!(&snap.links[..], [l] if matches!(&l.target, render::LinkTarget::Uri(u) if u.starts_with("https://example.org"))), "{:?}", snap.links);
        // Again: the address is linked already.
        ops.send(Op::LinkUrls { style }).unwrap();
        let (snap, err) = next(&rx);
        assert!(err.is_some_and(|e| e.contains("No web addresses")) && snap.links.len() == 1);
    }

    #[test]
    fn an_address_under_a_link_is_not_linked_again() {
        let link = render::Link { page: 0, index: 0, rect: [100.0, 700.0, 200.0, 720.0], target: render::LinkTarget::Uri("x".into()) };
        let found = vec![(0, vec![[105.0, 702.0, 190.0, 718.0], [100.0, 600.0, 150.0, 620.0]], "https://a.b".to_string()), (1, vec![[105.0, 702.0, 190.0, 718.0]], "https://c.d".to_string())];
        let left = unlinked(found, &[link]);
        assert_eq!(left.iter().map(|(p, r, _)| (*p, r.len())).collect::<Vec<_>>(), [(0, 1), (1, 1)], "{left:?}");
    }

    #[test]
    fn an_edit_that_draws_a_character_as_a_question_mark_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, _) = started(&path);
        let boxed = |contents: &str| {
            Edit::AddAnnotation(NewAnnotation {
                page: 0,
                shape: Shape::TextBox { rect: [100.0, 400.0, 300.0, 440.0], font_size: 12.0 },
                style: Style::default(),
                contents: contents.into(),
                author: String::new(),
            })
        };
        ops.send(Op::Apply { edit: Box::new(boxed("Plain")), pages: vec![0] }).unwrap();
        let Reply::Changed { note, .. } = rx.recv_timeout(Duration::from_secs(30)).unwrap() else { panic!("expected Changed") };
        assert_eq!(note, None);
        ops.send(Op::Apply { edit: Box::new(boxed("Ωmega")), pages: vec![0] }).unwrap();
        let Reply::Changed { note, .. } = rx.recv_timeout(Duration::from_secs(30)).unwrap() else { panic!("expected Changed") };
        assert!(note.as_deref().is_some_and(|n| n.starts_with("A comment on page 1 has \"Ω\"")), "{note:?}");
    }

    #[test]
    fn a_preview_is_taken_back_without_a_trace_and_a_commit_after_it_is_one_undo_step() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, _) = started(&path);
        let edit = |text: &str| Box::new(Edit::EditTextBlock { page: 0, block: 0, text: text.into(), style: Default::default() });
        ops.send(Op::Preview { edit: Some(edit("Page one draft")), pages: vec![0] }).unwrap();
        let (s, e) = next(&rx);
        assert!(e.is_none() && !s.dirty && !s.can_undo, "a preview is not a change to save or undo");
        ops.send(Op::Preview { edit: Some(edit("Page one, second draft")), pages: vec![0] }).unwrap();
        next(&rx);
        ops.send(Op::Preview { edit: None, pages: vec![0] }).unwrap();
        let (s, _) = next(&rx);
        assert!(!s.dirty && !s.can_undo && !s.can_redo, "taken back: clean, nothing to undo or redo");
        // Previewed, then committed: the commit takes the preview back first, one undo step.
        ops.send(Op::Preview { edit: Some(edit("Page one draft")), pages: vec![0] }).unwrap();
        next(&rx);
        ops.send(Op::Apply { edit: edit("Page one final"), pages: vec![0] }).unwrap();
        let (s, e) = next(&rx);
        assert!(e.is_none() && s.dirty && s.can_undo && !s.can_redo);
        ops.send(Op::Undo { pages: Some(vec![0]) }).unwrap();
        let (s, _) = next(&rx);
        assert!(!s.can_undo && s.can_redo, "one step: the preview left none");
        // Something to redo, a preview, Escape: it can still be redone.
        ops.send(Op::Preview { edit: Some(edit("Page one draft")), pages: vec![0] }).unwrap();
        let (s, _) = next(&rx);
        assert!(s.can_redo, "shown as before the preview");
        ops.send(Op::Preview { edit: None, pages: vec![0] }).unwrap();
        let (s, _) = next(&rx);
        assert!(s.can_redo && !s.can_undo, "the redo list is as it was");
        ops.send(Op::Redo { pages: Some(vec![0]) }).unwrap();
        let (s, e) = next(&rx);
        assert!(e.is_none() && s.can_undo && !s.can_redo);
    }

    #[test]
    fn undo_and_redo_take_the_edit_away_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, _) = started(&path);
        let note = Edit::AddAnnotation(NewAnnotation {
            page: 0,
            shape: Shape::Note { at: [100.0, 500.0], icon: NoteIcon::Comment },
            style: Style::default(),
            contents: "Check".into(),
            author: String::new(),
        });
        ops.send(Op::Apply { edit: Box::new(note), pages: vec![0] }).unwrap();
        assert_eq!(next(&rx).0.annots.len(), 1);
        ops.send(Op::Undo { pages: Some(vec![0]) }).unwrap();
        let (s, _) = next(&rx);
        assert!(s.annots.is_empty() && s.can_redo);
        ops.send(Op::Redo { pages: Some(vec![0]) }).unwrap();
        let (s, _) = next(&rx);
        assert_eq!(s.annots.len(), 1);
        assert_eq!(s.annots[0].contents, "Check");
    }

    #[test]
    fn page_edits_reorder_turn_and_delete_pages_in_the_saved_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 4);
        let (ops, rx, _) = started(&path);
        // Page 1 moved after page 3: 2 3 1 4.
        ops.send(Op::Apply { edit: Box::new(Edit::MovePages { pages: vec![0], to: 2 }), pages: vec![] }).unwrap();
        assert!(next(&rx).1.is_none());
        ops.send(Op::Apply { edit: Box::new(Edit::RotatePages { pages: vec![0], degrees: 90 }), pages: vec![] }).unwrap();
        let (s, _) = next(&rx);
        assert_eq!(s.boxes[0].rotation, 90);
        ops.send(Op::Apply { edit: Box::new(Edit::DeletePages { pages: vec![3] }), pages: vec![] }).unwrap();
        assert_eq!(next(&rx).0.boxes.len(), 3);
        ops.send(Op::Save { to: None }).unwrap();
        assert!(next(&rx).1.is_none());
        qpdf_clean(&path);
        assert_eq!(texts(&path), ["Page 2", "Page 3", "Page 1"]);
    }

    #[test]
    fn extract_writes_the_chosen_pages_to_a_new_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 3);
        let (ops, rx, _) = started(&path);
        let out = dir.path().join("extract.pdf");
        ops.send(Op::Extract { pages: vec![1, 2], to: out.clone() }).unwrap();
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            Reply::Extracted { to, error } => assert!(error.is_none() && to == out, "{error:?}"),
            _ => panic!("expected Extracted"),
        }
        qpdf_clean(&out);
        assert_eq!(texts(&out), ["Page 2", "Page 3"]);
        // The document itself is untouched.
        assert_eq!(texts(&path).len(), 3);
    }

    #[test]
    fn save_as_writes_a_full_copy_and_follows_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), 1);
        let (ops, rx, _) = started(&path);
        ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
        next(&rx);
        let copy = dir.path().join("copy.pdf");
        ops.send(Op::Save { to: Some(copy.clone()) }).unwrap();
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            Reply::Saved { to, error, snap } => assert!(error.is_none() && to == copy && !snap.dirty, "{error:?}"),
            _ => panic!("expected Saved"),
        }
        qpdf_clean(&copy);
        let (_, _, s) = started(&copy);
        assert_eq!(s.annots.len(), 1);
        // The original keeps no annotation.
        let (_, _, orig) = started(&path);
        assert!(orig.annots.is_empty());
    }

    #[test]
    fn replace_keeps_the_old_file_until_the_new_one_is_complete() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.pdf");
        std::fs::write(&p, b"old").unwrap();
        replace(&p, b"new bytes").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"new bytes");
        // No temporary file left behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
