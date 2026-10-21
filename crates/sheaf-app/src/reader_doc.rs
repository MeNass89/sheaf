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
}

/// A paragraph of a page as the edit tool shows it (from PdfCraft's `text_blocks`).
#[derive(Clone)]
pub struct Block {
    /// User space.
    pub rect: [f64; 4],
    pub text: String,
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

fn snap(doc: &Doc) -> Snap {
    let d = doc.document();
    // Replies and status replies point at their comment's name (/IRT → /NM).
    let mut replies: std::collections::HashMap<&str, Vec<String>> = std::collections::HashMap::new();
    let mut status: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    for a in &d.info.annotations {
        let Some(parent) = a.in_reply_to.as_deref() else { continue };
        match a.state.as_deref() {
            Some(s) => {
                status.insert(parent, s.to_string());
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
            status: a.name.as_deref().and_then(|n| status.get(n)).cloned().unwrap_or_default(),
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
    /// Acrobat's Summarize Comments: a PDF listing every comment, by page, written to `to`.
    Summary { to: PathBuf },
}

pub enum Reply {
    Opened(Box<Snap>),
    OpenFailed(sheaf_core::OpdfError),
    /// An edit, undo or redo is done (or failed): the pages to render again (`None`: all).
    /// `applied`: the answer to an `Apply` (an undo step was added unless `error`).
    Changed { snap: Box<Snap>, pages: Option<Vec<usize>>, error: Option<String>, applied: bool },
    Saved { snap: Box<Snap>, to: PathBuf, error: Option<String> },
    /// A page's paragraphs and its images' boxes (user space).
    /// A page's paragraphs and its images: (visible box, full box), user space; a cropped image
    /// shows less than its full box, which is what a move translates.
    Blocks(usize, Vec<Block>, Vec<([f64; 4], [f64; 4])>),
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
            for op in rx {
                let t0 = std::time::Instant::now();
                match op {
                    Op::Apply { edit, pages } => {
                        let error = doc.apply(*edit).err().map(|e| e.to_string());
                        let t1 = std::time::Instant::now();
                        let s = snap(&doc);
                        debug(format_args!("apply {:?}, snapshot {:?}", t1 - t0, t1.elapsed()));
                        reply(Reply::Changed { snap: Box::new(s), pages: Some(pages), error, applied: true });
                    }
                    op @ (Op::Undo { .. } | Op::Redo { .. }) => {
                        let (redo, pages) = match op {
                            Op::Redo { pages } => (true, pages),
                            Op::Undo { pages } => (false, pages),
                            _ => unreachable!(),
                        };
                        let id = doc.id();
                        let r = if redo { doc.session_mut().redo(id) } else { doc.session_mut().undo(id) };
                        reply(Reply::Changed { snap: Box::new(snap(&doc)), pages, error: r.err().map(|e| e.to_string()), applied: false });
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
                            .map(|b| Block { rect: b.rect, text: b.editor_text })
                            .collect();
                        let images = doc.document().page_images(page).iter().map(|i| (i.visible_rect.unwrap_or(i.rect), i.rect)).collect();
                        reply(Reply::Blocks(page, b, images));
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
