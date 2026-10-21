//! The reader's document lives on a thread of its own: PdfCraft's edits on a large file take
//! seconds (an annotation on a 70 MB book: 1.5 s), and the window must never wait for them.
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
}

/// A paragraph of a page as the edit tool shows it (from PdfCraft's `text_blocks`).
#[derive(Clone)]
pub struct Block {
    /// User space.
    pub rect: [f64; 4],
    pub text: String,
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
}

fn snap(doc: &Doc) -> Snap {
    let d = doc.document();
    let mut annots: Vec<AnnotEntry> = d
        .info
        .annotations
        .iter()
        .filter(|a| sheaf_app::reader::listed(&a.subtype) && a.in_reply_to.is_none() && !a.fill_sign)
        .map(|a| AnnotEntry {
            page: a.page,
            index: a.index,
            subtype: a.subtype.clone(),
            color: a.color,
            contents: a.contents.clone().unwrap_or_default(),
            rect: a.rect.map(f64::from),
            quads: a.quads.iter().map(|q| q.map(f64::from)).collect(),
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
    }
}

pub enum Op {
    /// An edit; `pages` are the pages it changes (rendered again).
    Apply { edit: Edit, pages: Vec<usize> },
    Undo,
    Redo,
    /// Writes the edits: into the file itself as an incremental update, or (`to`) a full copy
    /// that becomes the document's file.
    Save { to: Option<PathBuf> },
    /// The paragraphs of a page (Edit text).
    Blocks(usize),
}

pub enum Reply {
    Opened(Box<Snap>),
    OpenFailed(sheaf_core::OpdfError),
    /// An edit, undo or redo is done (or failed): the pages to render again (`None`: all).
    Changed { snap: Box<Snap>, pages: Option<Vec<usize>>, error: Option<String> },
    Saved { snap: Box<Snap>, to: PathBuf, error: Option<String> },
    Blocks(usize, Vec<Block>),
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
                        let error = doc.apply(edit).err().map(|e| e.to_string());
                        reply(Reply::Changed { snap: Box::new(snap(&doc)), pages: Some(pages), error });
                    }
                    Op::Undo | Op::Redo => {
                        let id = doc.id();
                        let r = if matches!(op, Op::Redo) { doc.session_mut().redo(id) } else { doc.session_mut().undo(id) };
                        reply(Reply::Changed { snap: Box::new(snap(&doc)), pages: None, error: r.err().map(|e| e.to_string()) });
                    }
                    Op::Save { to } => {
                        let target = to.clone().unwrap_or_else(|| path.clone());
                        let error = save(&mut doc, &target, to.is_none()).err();
                        if error.is_none() {
                            path = target.clone();
                        }
                        reply(Reply::Saved { snap: Box::new(snap(&doc)), to: target, error });
                    }
                    Op::Blocks(page) => {
                        let b = doc
                            .document()
                            .text_blocks(page)
                            .into_iter()
                            .filter(|b| !b.text.trim().is_empty())
                            .map(|b| Block { rect: b.rect, text: b.editor_text })
                            .collect();
                        reply(Reply::Blocks(page, b));
                    }
                }
                debug(format_args!("operation done in {:?}", t0.elapsed()));
            }
        })
        .expect("spawn the reader's document thread");
    tx
}

fn save(doc: &mut Doc, target: &std::path::Path, incremental: bool) -> Result<(), String> {
    let id = doc.id();
    let bytes = if incremental { doc.session().save_bytes(id) } else { doc.session().save_full_bytes(id) }.map_err(|e| e.to_string())?;
    replace(target, &bytes).map_err(|e| e.to_string())?;
    doc.session_mut().mark_saved(id, bytes, Some(target.to_string_lossy().into_owned())).map_err(|e| e.to_string())
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
