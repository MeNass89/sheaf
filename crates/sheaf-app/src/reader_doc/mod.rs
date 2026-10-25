//! The reader's document lives on a thread of its own: PdfCraft's work on a large file is
//! noticeable (a 70 MB book: 0.2 s per annotation, 1.2 s per save, 4.5 s to open), and the
//! window must never wait for it.
//! The UI sends operations in order and reads a snapshot of what it needs, refreshed after each.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use sheaf_app::viewer::{self, PageBox};
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::engine::{Edit, NewField};
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
    /// Replies to it, oldest first: ("author: text", its page, its index there); and its latest
    /// review status ("Accepted").
    pub replies: Vec<(String, usize, usize)>,
    pub status: String,
    /// Its /NM, what an XFDF export names it by ("" when the file gives none).
    pub nm: String,
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
    /// Prepare form's properties: required, its tooltip, several lines (text fields).
    pub required: bool,
    pub tooltip: String,
    pub multiline: bool,
    /// Which field exactly: 0 text, 1 check box, 2 radio button, 3 dropdown, 4 list, 5 button, 6 signature.
    pub exact: i32,
    /// Its text's alignment (0 left, 1 centre, 2 right) and size in points (0: auto).
    pub align: i32,
    pub font_size: f64,
    /// A button's click: 0 nothing (or another action), 1 clear the form, 2 print, 3 open `url`,
    /// 4 submit the form to `url`.
    pub action: i32,
    pub url: String,
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
    /// Each signature's signed version (Acrobat's View Signed Version): who signed, how many bytes
    /// of the file it covers, its place in the document's signatures, and whether its certificate
    /// is trusted already.
    pub signed: Vec<(String, usize, usize, bool)>,
}

fn props(d: &sheaf_core::pdfcraft::engine::Document) -> Props {
    let i = &d.info;
    let or_none = |v: &Option<String>| v.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "—".into());
    let page = i.pages.first().map(|p| {
        let (w, h) = (p.width as f64 / 72.0 * 25.4, p.height as f64 / 72.0 * 25.4);
        format!("{w:.0} × {h:.0} mm")
    });
    let mb = i.file_size as f64 / 1_048_576.0;
    let mut about = vec![
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
    // Acrobat's Signatures panel, in short: who signed, the verdict, when and why, and what the
    // verdict rests on (its sentences: changed since, certificate not trusted…).
    for (k, s) in d.signatures.iter().filter(|s| s.signed).enumerate() {
        let who = s.signer.clone().unwrap_or_else(|| "unknown signer".into());
        let when = s.date.as_deref().and_then(sheaf_core::facts::pdf_date).unwrap_or_default();
        let mut line = vec![s.summary().to_string()];
        line.extend([when, s.reason.clone().unwrap_or_default()].into_iter().filter(|v| !v.is_empty()));
        line.extend(s.details.iter().map(|x| x.message.clone()));
        about.push((format!("Signature {} · {who}", k + 1), line.join(". ")));
        // Its certificate, as Acrobat's Certificate Viewer summarises it (Sol, 25/10).
        if let Some(c) = &s.certificate {
            about.push((format!("Signature {} · certificate", k + 1), format!("Issued to {} by {}, valid {} to {}", c.subject.display_name(), c.issuer.display_name(), c.not_before, c.not_after)));
        }
    }
    let trusted = sheaf_core::pdfcraft::trusted_certificates();
    let signed = d
        .signatures
        .iter()
        .enumerate()
        .filter(|(_, s)| s.signed && s.signed_len > 0)
        .map(|(i, s)| (s.signer.clone().unwrap_or_else(|| "unknown signer".into()), s.signed_len, i, s.certificate.as_ref().is_none_or(|c| trusted.iter().any(|t| t.raw == c.raw))))
        .collect();
    let get = |v: &Option<String>| v.clone().unwrap_or_default();
    Props { title: get(&i.title), author: get(&i.author), subject: get(&i.subject), keywords: get(&i.keywords), about, signed }
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
    let mut replies: std::collections::HashMap<&str, Vec<(String, usize, usize)>> = std::collections::HashMap::new();
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
            None => replies.entry(parent).or_default().push((format!("{}: {}", a.author.as_deref().unwrap_or("—"), a.contents.as_deref().unwrap_or("").trim()), a.page, a.index)),
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
            nm: a.name.clone().unwrap_or_default(),
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
    use sheaf_core::pdfcraft::engine::{field_flags, form_scripts::ButtonAction as B, FormFieldKind as K};
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
            required: f.flags & field_flags::REQUIRED != 0,
            tooltip: f.tooltip.clone().unwrap_or_default(),
            multiline: f.flags & field_flags::MULTILINE != 0,
            exact: match f.kind {
                K::Text => 0,
                K::CheckBox => 1,
                K::Radio => 2,
                K::Combo => 3,
                K::List => 4,
                K::PushButton => 5,
                _ => 6,
            },
            align: f.quadding.clamp(0, 2) as i32,
            font_size: f.da.split_whitespace().collect::<Vec<_>>().windows(2).find(|w| w[1] == "Tf").and_then(|w| w[0].parse().ok()).unwrap_or(0.0),
            action: match &f.button {
                Some(B::Reset { .. }) => 1,
                Some(B::Named(n)) if n == "Print" => 2,
                Some(B::Uri(_)) => 3,
                Some(B::Submit(_)) => 4,
                _ => 0,
            },
            url: match &f.button {
                Some(B::Uri(u)) | Some(B::Submit(u)) => u.clone(),
                _ => String::new(),
            },
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
    /// Each group of pages written to a new PDF at its path (one file, or one per page).
    Extract { parts: Vec<(Vec<usize>, PathBuf)> },
    /// Acrobat's Create Links from URLs: every web address in the text not linked yet, in `style`.
    LinkUrls { style: sheaf_core::pdfcraft::engine::LinkStyle },
    /// Trust signature `index`'s certificate from now on (Acrobat's Add to Trusted Certificates):
    /// the document's signatures are validated again.
    TrustSignature { index: usize },
    /// ⌘C on a picture picked with the Edit tool: its pixels at its own resolution, for the clipboard.
    CopyImage { page: usize, index: usize },
    /// ⌘C in the Pages panel of a document opened with its password: its pages as a new PDF
    /// (a copy of the file's bytes would need the password to be pasted).
    CopyPages { pages: Vec<usize> },
    /// Prepare form ▸ Detect fields (Acrobat's automatic field detection), as one undo step.
    DetectFields,
    /// Acrobat's Summarize Comments: a PDF listing every comment, by page, written to `to`.
    Summary { to: PathBuf },
    /// Every comment exported to `to`: XFDF, or FDF when `to` ends in .fdf.
    ExportComments { to: PathBuf },
    /// The XFDF element of the comment named `nm` (⌘C on a selected comment).
    /// A comment by its name, or (none in the file) by its page and box.
    CopyComment { nm: String, page: usize, rect: [f64; 4] },
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
    /// The copied comment's XFDF element (`None`: it could not be written).
    Copied(Option<String>),
    /// A picture copied: (width, height, RGBA pixels), or why not.
    ImageCopied(Result<(u32, u32, Vec<u8>), String>),
    /// Pages copied from a document opened with its password: a new PDF of them, unprotected.
    PagesCopied(Result<(Arc<Vec<u8>>, usize), String>),
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
                    Op::DetectFields => {
                        let id = doc.id();
                        let found = doc.session().detect_fields(id, &[]);
                        let pages: Vec<usize> = found.iter().map(|f| f.0).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                        let n = found.len();
                        let edits: Vec<Edit> = found
                            .into_iter()
                            .map(|(page, c)| Edit::AddField {
                                page,
                                rect: c.rect,
                                kind: match c.kind {
                                    sheaf_core::pdfcraft::engine::detect::Kind::Text => NewField::Text { multiline: c.rect[3] - c.rect[1] > 30.0 },
                                    sheaf_core::pdfcraft::engine::detect::Kind::CheckBox => NewField::CheckBox,
                                },
                                name: Some(c.name),
                            })
                            .collect();
                        let error = if n == 0 { Some("No fields found: no lines, boxes or labelled blanks to fill".to_string()) } else { doc.apply(Edit::Batch { label: "Detect form fields".into(), edits }).err().map(|e| e.to_string()) };
                        let note = (error.is_none()).then(|| format!("{} found", sheaf_core::tools::count(n, "field")));
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
                    Op::Extract { parts } => {
                        let id = doc.id();
                        let r = parts.iter().try_for_each(|(pages, to)| {
                            doc.session().extract(id, pages).map_err(|e| e.to_string()).and_then(|bytes| replace(to, &bytes).map_err(|e| e.to_string()))
                        });
                        // One file is opened; several are announced by their folder.
                        let to = match &parts[..] {
                            [(_, one)] => one.clone(),
                            _ => parts.first().and_then(|(_, p)| p.parent()).map(PathBuf::from).unwrap_or_default(),
                        };
                        reply(Reply::Extracted { to, error: r.err() });
                    }
                    Op::ExportComments { to } => {
                        let id = doc.id();
                        let fdf = to.extension().is_some_and(|e| e.eq_ignore_ascii_case("fdf"));
                        let format = if fdf { sheaf_core::pdfcraft::engine::DataFormat::Fdf } else { sheaf_core::pdfcraft::engine::DataFormat::Xfdf };
                        let r = doc.session().export_data(id, format, true, false).map_err(|e| e.to_string()).and_then(|bytes| replace(&to, &bytes).map_err(|e| e.to_string()));
                        reply(Reply::Exported { to, error: r.err() });
                    }
                    Op::CopyComment { nm, page, rect } => {
                        let id = doc.id();
                        let xfdf = doc.session().export_data(id, sheaf_core::pdfcraft::engine::DataFormat::Xfdf, true, false).ok();
                        reply(Reply::Copied(xfdf.and_then(|b| {
                            let x = String::from_utf8_lossy(&b);
                            if nm.is_empty() { sheaf_app::reader::xfdf_element_at(&x, page, rect) } else { sheaf_app::reader::xfdf_element(&x, &nm) }
                        })));
                    }
                    Op::TrustSignature { index } => {
                        let id = doc.id();
                        let cert = doc.document().signatures.get(index).and_then(|s| s.certificate.as_ref().map(|c| c.raw.clone()));
                        let error = match cert {
                            Some(der) => sheaf_core::pdfcraft::trust_certificate(&der).err().map(|e| e.to_string()),
                            None => Some("This signature has no certificate to trust".into()),
                        };
                        doc.session_mut().set_trusted_certificates(sheaf_core::pdfcraft::trusted_certificates());
                        let _ = id;
                        reply(Reply::Changed { snap: Box::new(snap(&doc)), pages: Some(Vec::new()), error, applied: false, note: Some("Certificate trusted: its signatures validate".into()), preview: false });
                    }
                    Op::CopyImage { page, index } => {
                        reply(Reply::ImageCopied(doc.document().page_image_rgba(page, index)));
                    }
                    Op::CopyPages { pages } => {
                        let id = doc.id();
                        reply(Reply::PagesCopied(doc.session().extract(id, &pages).map(|b| (b, pages.len())).map_err(|e| e.to_string())));
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
mod tests;
