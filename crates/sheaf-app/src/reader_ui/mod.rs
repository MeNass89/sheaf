//! The reader: one document open in its own tab to read and annotate, PDF Expert's reading
//! window as the model. The pages are the tab's viewer (`viewer_ui`), rendered from the working
//! copy held by a PdfCraft session, so an annotation shows the moment it is made; ⌘S writes it
//! back to the file as an incremental update.
//!
//! Points called "view" are a page's view space (points, y down, as displayed): the space of
//! PdfCraft's text layer. "Content" coordinates are the page scroll's.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use sheaf_app::reader::{self, PageTheme, COLOURS, INK};
use sheaf_app::viewer::{self, Viewer};
use sheaf_app::{AppWindow, ReaderMark, ReaderResultRow, ReaderAnnotRow, ReaderNote, ReaderOutlineRow, ReaderState, ToolTab, ViewerOverlay};
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::engine::{AddedText, Edit, FieldValue, LineEnding, Markup, NewAnnotation, NoteIcon, Shape, StampKind, Style};
use sheaf_core::pdfcraft::render::{self, PageText};
use std::rc::Rc;

use slint::{Color, ComponentHandle, Model, ModelRc, VecModel};

use crate::state::{refresh, tabs_model, with, TabState};
use crate::viewer_ui::ViewerUi;

pub const SELECT: i32 = 0;
pub const HIGHLIGHT: i32 = 1;
pub const UNDERLINE: i32 = 2;
pub const STRIKE: i32 = 3;
pub const NOTE: i32 = 4;
pub const PEN: i32 = 5;
pub const TEXTBOX: i32 = 6;
/// Edit mode: click a paragraph to edit its text, or empty space to add text to the page.
pub const EDIT_TEXT: i32 = 7;
/// Edit mode: click to place an image.
pub const ADD_IMAGE: i32 = 8;
/// Text sizes offered for added text, in points.
pub const TEXT_SIZES: [f64; 6] = [9.0, 10.0, 12.0, 14.0, 18.0, 24.0];

/// Fill & Sign: click to place the signature (made once, kept for every document).
pub const SIGN: i32 = 9;
/// Shapes: drag a rectangle, oval, line or arrow (`ReaderUi::shape_kind`).
pub const SHAPE: i32 = 10;
/// Eraser: drag across drawings to rub them out where the path goes.
pub const ERASER: i32 = 11;
/// Stamp: click to place the stamp chosen in the options bar (`ReaderUi::stamp`).
pub const STAMP: i32 = 12;

use crate::reader_doc::{self, AnnotEntry, Block, Op, Reply, Snap};

struct Search {
    query: String,
    /// (page, rects in view points) per match, in reading order.
    hits: Vec<(usize, Vec<[f32; 4]>)>,
    /// The text around each hit, for the results list: before, the match, after.
    snippets: Vec<(String, String, String)>,
    case_sensitive: bool,
    whole_words: bool,
    current: Option<usize>,
    busy: bool,
    /// Bumped by every new search: late results of an old one are dropped.
    generation: u64,
}

/// What is being typed: 0 a note, 1 a text box (annotation), 2 an existing paragraph, 3 new
/// text on the page, 4 a form text field, 5 a form choice field (a list to pick from).
struct Draft {
    page: usize,
    at: (f64, f64),
    kind: i32,
    /// Kind 2: the paragraph's index, its text and its box (user space).
    block: Option<(usize, String, [f64; 4])>,
    /// Kinds 4 and 5: the field's index in the snapshot and its widget's box (user space).
    field: Option<(usize, [f64; 4])>,
}

/// What the Edit tool's press is on.
#[derive(Clone)]
enum EditTarget {
    /// A paragraph: index, text, box (user space), its top-left (view).
    Block(usize, String, [f64; 4], (f64, f64)),
    /// An image: index among the page's images, box (user space).
    Image(usize, [f64; 4]),
    Empty,
}

struct EditPress {
    page: usize,
    target: EditTarget,
    from: (f64, f64),
    to: (f64, f64),
    dragging: bool,
}

/// An annotation sent to the document thread, drawn at once until the page renders with it.
struct Provisional {
    page: usize,
    /// Content rectangles of a markup, or the points of a pen stroke (view points).
    rects: Vec<[f32; 4]>,
    stroke: Vec<(f64, f64)>,
    colour: [f64; 3],
    kind: i32,
}

pub struct ReaderUi {
    ops: std::sync::mpsc::Sender<Op>,
    snap: Snap,
    /// Operations sent and not answered yet.
    in_flight: usize,
    provisional: Vec<Provisional>,
    pub path: PathBuf,
    pub name: String,
    texts: HashMap<usize, Arc<PageText>>,
    /// The text layer's renderer, kept for the working file it was made from.
    text_renderer: Option<(Arc<Vec<u8>>, render::PageRenderer)>,
    blocks: HashMap<usize, Arc<Vec<Block>>>,
    /// The boxes of each page's images (user space), read with its paragraphs.
    images: HashMap<usize, Arc<Vec<[f64; 4]>>>,
    /// Edit tool: what the press is on, where it started and where the pointer is (view).
    edit_press: Option<EditPress>,
    blocks_asked: HashSet<usize>,
    /// Text selection: page and glyph range.
    sel: Option<(usize, Range<usize>)>,
    /// The press of a drag: page and view point.
    press: Option<(usize, (f64, f64))>,
    pub tool: i32,
    /// Colour index per palette: highlights (light) and strokes (ink).
    light: usize,
    ink_colour: usize,
    pub theme: PageTheme,
    panel_open: bool,
    panel: i32,
    toggled: HashSet<Vec<usize>>,
    rows: Vec<reader::OutlineRow>,
    selected_annot: Option<usize>,
    search: Search,
    stroke: Option<(usize, Vec<(f64, f64)>)>,
    draft: Option<Draft>,
    /// Add image: where the click was, while the file is chosen.
    image_at: Option<(usize, (f64, f64))>,
    /// Signature: where the click was, and whether the pad is open to make one.
    sign_at: Option<(usize, (f64, f64))>,
    sign_open: bool,
    /// Print sheet: open, the system's printers and the chosen one.
    print_open: bool,
    printers: Vec<String>,
    printer: usize,
    note_seq: i32,
    pen_width: i32,
    /// Shapes tool: 0 rectangle, 1 oval, 2 line, 3 arrow; filled (rectangle, oval); where the drag is.
    shape_kind: i32,
    shape_fill: bool,
    shape_end: Option<(f64, f64)>,
    /// Stamp tool: index in `StampKind::ALL`.
    stamp: usize,
    /// Shapes and pen: index in `reader::OPACITIES`.
    opacity: usize,
    /// Added text: index in TEXT_SIZES, 0 Helvetica / 1 Times / 2 Courier, bold.
    text_size: i32,
    text_family: i32,
    text_bold: bool,
    /// Pages selected in the Pages panel (empty: the current page is meant).
    page_sel: std::collections::BTreeSet<usize>,
    /// The last page clicked in the panel, the anchor of a ⇧-click range.
    page_anchor: Option<usize>,
    /// Pages being dragged in the panel.
    page_drag: Option<Vec<usize>>,
    /// A page edit is on its way: edits that name pages wait until it is done (their page
    /// numbers may be about to change).
    structure_pending: bool,
    /// The Tools button saved first: the file goes to the tools when that save succeeds.
    tools_after_save: bool,
    /// Generation of the running search, shared with its thread so it stops when replaced.
    search_live: Arc<std::sync::atomic::AtomicU64>,
    /// A plain press on a page already selected: the selection shrinks to it only if no drag
    /// follows.
    pending_reduce: Option<usize>,
    /// Pages of each edit sent, for undo and redo.
    undo_pages: Vec<Vec<usize>>,
    redo_pages: Vec<Vec<usize>>,
    /// A check box or radio button clicked: its edit, sent once the click is handled.
    pending_field: Option<Edit>,
    /// An annotation being dragged: its index, where the drag started and where it is (view).
    annot_drag: Option<AnnotPull>,
    /// An annotation being resized by its corner handle, the same way.
    annot_resize: Option<AnnotPull>,
    /// The annotation under the press, selected if the press ends without a drag.
    annot_candidate: Option<usize>,
    /// The panel shown before a search took it over (results, panel 3).
    panel_before_search: Option<i32>,
    results_cache: Cache<(Rc<VecModel<ReaderResultRow>>, Option<usize>)>,
    /// 0 fit width, 1 fit page, 2 a zoom the user chose.
    zoom_mode: i32,
    /// Pages left by following a link or the outline (⌘[ goes back), and those gone back from.
    back: Vec<usize>,
    forward: Vec<usize>,
    /// Bumped when the outline rows or the annotation list change: the panel models are rebuilt
    /// only then, not on every scroll.
    version: u64,
    outline_cache: Cache<(OutlineKey, ModelRc<ReaderOutlineRow>)>,
    annots_cache: Cache<(AnnotsKey, ModelRc<ReaderAnnotRow>)>,
}

/// An annotation pulled by the pointer: its index, where the pull started, where it is (view).
type AnnotPull = (usize, (f64, f64), (f64, f64));
/// A panel model kept until what it shows changes.
type Cache<T> = std::cell::RefCell<Option<T>>;
/// What the outline rows depend on: the version and the current section.
type OutlineKey = (u64, Option<usize>);
/// What the annotation rows depend on: the version, the selected one, how many pages have text read.
type AnnotsKey = (u64, Option<usize>, usize);

impl ReaderUi {
    fn new(ops: std::sync::mpsc::Sender<Op>, snap: Snap, path: PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Document".into());
        let has_outline = !snap.outline.is_empty();
        let mut r = ReaderUi {
            ops,
            snap,
            in_flight: 0,
            provisional: Vec::new(),
            path,
            name,
            texts: HashMap::new(),
            text_renderer: None,
            blocks: HashMap::new(),
            images: HashMap::new(),
            edit_press: None,
            blocks_asked: HashSet::new(),
            sel: None,
            press: None,
            tool: SELECT,
            light: 0,
            ink_colour: 0,
            theme: PageTheme::Day,
            // The outline is open on documents that have one, as PDF Expert does for books.
            panel_open: true,
            panel: if has_outline { 1 } else { 0 },
            toggled: HashSet::new(),
            rows: Vec::new(),
            selected_annot: None,
            search: Search { query: String::new(), hits: Vec::new(), snippets: Vec::new(), case_sensitive: false, whole_words: false, current: None, busy: false, generation: 0 },
            stroke: None,
            draft: None,
            image_at: None,
            sign_at: None,
            sign_open: false,
            print_open: false,
            printers: Vec::new(),
            printer: 0,
            note_seq: 0,
            pen_width: 1,
            shape_kind: 0,
            shape_fill: false,
            shape_end: None,
            opacity: 0,
            stamp: StampKind::ALL.iter().position(|k| *k == StampKind::Approved).unwrap_or(0),
            text_size: 2,
            text_family: 0,
            text_bold: false,
            annot_candidate: None,
            annot_drag: None,
            annot_resize: None,
            pending_field: None,
            page_sel: Default::default(),
            page_anchor: None,
            page_drag: None,
            structure_pending: false,
            tools_after_save: false,
            search_live: Default::default(),
            pending_reduce: None,
            undo_pages: Vec::new(),
            redo_pages: Vec::new(),
            panel_before_search: None,
            results_cache: Default::default(),
            zoom_mode: 0,
            back: Vec::new(),
            forward: Vec::new(),
            version: 0,
            outline_cache: Default::default(),
            annots_cache: Default::default(),
        };
        r.rows = reader::outline_rows(&r.snap.outline, &r.toggled);
        r
    }

    fn annots(&self) -> &[AnnotEntry] {
        &self.snap.annots
    }

    fn bytes(&self) -> Arc<Vec<u8>> {
        self.snap.display.clone()
    }

    fn password(&self) -> Option<String> {
        self.snap.password.clone()
    }

    fn dirty(&self) -> bool {
        self.snap.dirty || self.in_flight > 0
    }

    fn send(&mut self, op: Op) {
        if self.ops.send(op).is_ok() {
            self.in_flight += 1;
        }
    }

    /// The paragraphs of `page`, asked of the document thread once per change of the page
    /// (`None` until they arrive).
    fn blocks(&mut self, page: usize) -> Option<Arc<Vec<Block>>> {
        if let Some(b) = self.blocks.get(&page) {
            return Some(b.clone());
        }
        if self.blocks_asked.insert(page) {
            let _ = self.ops.send(Op::Blocks(page));
        }
        None
    }

    /// The text layer of `page`, read once.
    fn text(&mut self, page: usize) -> Option<Arc<PageText>> {
        if let Some(t) = self.texts.get(&page) {
            return Some(t.clone());
        }
        let bytes = self.snap.bytes.clone();
        if !self.text_renderer.as_ref().is_some_and(|(b, _)| Arc::ptr_eq(b, &bytes)) {
            let config = render::RenderConfig { password: self.snap.password.as_deref().map(Arc::from), ..Default::default() };
            self.text_renderer = Some((bytes.clone(), render::PageRenderer::new(bytes, config)));
        }
        let (_, r) = self.text_renderer.as_mut()?;
        let res = r.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
        let t = res.text?;
        self.texts.insert(page, t.clone());
        Some(t)
    }
}

// ---- opening ---------------------------------------------------------------------------------

/// Opens `path` in a new reader tab and shows it. A file already open is brought forward.
pub fn open(ui: &AppWindow, path: PathBuf, password: Option<String>) {
    let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    // One tab per file, a tab still loading included: two tabs saving one file would overwrite
    // each other's edits.
    let existing = with(|a| a.tabs.iter().position(|t| t.reader_file.as_ref() == Some(&canon)));
    if let Some(i) = existing {
        ui.set_active_tab(i as i32);
        return;
    }
    // The tab shows at once ("Opening…"); the document is read and parsed on a worker.
    let id = with(|a| {
        a.next_id += 1;
        let mut t = TabState::new(a.next_id, sheaf_core::tools::ToolId::Edit);
        t.reader_loading = Some(file_name(&path));
        t.reader_file = Some(canon.clone());
        tabs_model().push(crate::state::build(&t));
        a.tabs.push(t);
        a.next_id
    });
    ui.set_active_tab(slint::Model::row_count(&*tabs_model()) as i32 - 1);
    let input = Input { path: path.clone(), password };
    // Pages first: pdfium reads the page boxes fast and the pages render from the file while
    // PdfCraft parses the document (outline, annotations, edits) on the document thread.
    let weak = ui.as_weak();
    let fast = input.clone();
    sheaf_app::pool::probes().submit(move || {
        let t0 = std::time::Instant::now();
        let Ok(boxes) = viewer::fast_boxes(&fast) else { return };
        reader_doc::debug(format_args!("pages shown after {:?}", t0.elapsed()));
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let scale = ui.window().scale_factor() as f64;
            let shown = with(|a| {
                let Some(t) = a.tab(id) else { return false };
                if t.viewer.is_some() {
                    return false;
                }
                let path = fast.path.clone();
                let mut v = ViewerUi::new(fast, reader_viewer(boxes), scale, t.viewport, t.strip);
                v.theme = PageTheme::from_index(crate::home::prefs().reader_theme);
                v.viewer.set_width(t.viewport.0 as f64);
                restore_view(&mut v, &path);
                t.viewer = Some(Box::new(v));
                refresh(a, id);
                true
            });
            if shown {
                crate::viewer_ui::request(&ui, id);
            }
        });
    });
    let weak = ui.as_weak();
    let ops = reader_doc::start(input.clone(), move |reply| {
        let _ = weak.upgrade_in_event_loop(move |ui| on_reply(&ui, id, reply));
    });
    with(|a| {
        if let Some(t) = a.tab(id) {
            t.reader_ops = Some((ops, input));
        }
    });
}

/// An answer of tab `id`'s document thread.
fn on_reply(ui: &AppWindow, id: i32, reply: Reply) {
    match reply {
        Reply::Opened(snap) => opened(ui, id, *snap),
        Reply::OpenFailed(e) => {
            let Some((_, input)) = with(|a| a.tab(id).and_then(|t| t.reader_ops.take())) else { return };
            crate::flow::remove_tab(ui, id);
            match e {
                sheaf_core::OpdfError::PasswordRequired(_) | sheaf_core::OpdfError::WrongPassword(_) => crate::password::ask_reader(ui, input.path, input.password.is_some()),
                e => crate::flow::toast(ui, &format!("Can't open {}: {e}", file_name(&input.path))),
            }
        }
        Reply::Changed { snap, pages, error, applied } => {
            update(id, |r, v| {
                r.in_flight = r.in_flight.saturating_sub(1);
                // A refused edit added no undo step: its pages leave the stack too.
                if applied && error.is_some() {
                    r.undo_pages.pop();
                }
                let structural = r.snap.boxes.len() != snap.boxes.len() || r.structure_pending;
                r.snap = *snap;
                r.structure_pending = false;
                if structural {
                    forget_page_state(r);
                }
                after_change(r, v, pages.as_deref().filter(|p| !p.is_empty()));
                if r.in_flight == 0 {
                    r.provisional.clear();
                }
            });
            if let Some(e) = error {
                crate::flow::toast(ui, &e);
            }
            crate::viewer_ui::request(ui, id);
        }
        Reply::Saved { snap, to, error } => {
            let name = update(id, |r, _| {
                r.in_flight = r.in_flight.saturating_sub(1);
                r.snap = *snap;
                if error.is_none() {
                    r.path = to.clone();
                    r.name = file_name(&to);
                }
                r.name.clone()
            });
            match (error, name) {
                (None, Some(n)) => {
                    // Edits made while this save ran are not in it: save again before closing or
                    // handing the file to a tool.
                    if unsaved_name(id).is_some() && (with(|a| a.closing_after_save.contains(&id)) || tools_waiting(id)) {
                        save(ui, id);
                        return;
                    }
                    crate::flow::toast(ui, &format!("Saved {n}"));
                    if update(id, |r, _| std::mem::take(&mut r.tools_after_save)).unwrap_or(false) {
                        hand_to_tools(ui, id);
                    }
                    if with(|a| a.closing_after_save.remove(&id)) {
                        crate::flow::remove_tab(ui, id);
                        if with(|a| a.quitting) {
                            crate::flow::quit_or_ask(ui);
                        }
                    }
                }
                (Some(e), _) => {
                    with(|a| {
                        a.closing_after_save.remove(&id);
                        a.quitting = false;
                    });
                    update(id, |r, _| r.tools_after_save = false);
                    crate::flow::toast(ui, &format!("Not saved: {e}"));
                }
                _ => {}
            }
        }
        Reply::Extracted { to, error } => {
            update(id, |r, _| r.in_flight = r.in_flight.saturating_sub(1));
            match error {
                None => {
                    crate::flow::toast(ui, &format!("Extracted to {}", file_name(&to)));
                    open(ui, to, None);
                }
                Some(e) => crate::flow::toast(ui, &format!("Not extracted: {e}")),
            }
        }
        Reply::Blocks(page, b, images) => {
            update(id, |r, _| {
                r.blocks.insert(page, Arc::new(b));
                r.images.insert(page, Arc::new(images));
            });
        }
    }
}

/// The document is shown as it was left, as PDF Expert reopens it: pages side by side, the zoom
/// when one was chosen (Fit width and Fit page refit to the window), the page.
fn restore_view(v: &mut ViewerUi, path: &std::path::Path) {
    if let Some((fit, zoom, spread)) = crate::home::last_view(path) {
        v.viewer.set_spread(spread);
        if fit == 2 {
            v.viewer.set_zoom(zoom);
        }
    }
    if let Some(p) = crate::home::last_page(path).filter(|p| *p > 0) {
        v.show_page(p, None);
    }
}

fn opened(ui: &AppWindow, id: i32, snap: Snap) {
    let Some((ops, input)) = with(|a| a.tab(id).and_then(|t| t.reader_ops.take())) else { return };
    let path = input.path.clone();
    crate::home::record_open(&path);
    let scale = ui.window().scale_factor() as f64;
    let ok = with(|a| {
        let Some(t) = a.tab(id) else { return false };
        // The pages may already be on screen (fast path): keep the viewer, its scroll and renders.
        let mut v = match t.viewer.take() {
            Some(mut v) if v.viewer.boxes.len() == snap.boxes.len() => {
                // Same displayed sizes, now with their real crop boxes and rotations.
                v.viewer.boxes = snap.boxes.clone();
                v
            }
            _ => {
                let mut v = Box::new(ViewerUi::new(input, reader_viewer(snap.boxes.clone()), scale, t.viewport, t.strip));
                v.viewer.set_width(t.viewport.0 as f64);
                restore_view(&mut v, &path);
                v
            }
        };
        let mut r = ReaderUi::new(ops, snap, path.clone());
        r.zoom_mode = crate::home::last_view(&path).map_or(0, |(fit, ..)| fit.clamp(0, 2));
        r.theme = PageTheme::from_index(crate::home::prefs().reader_theme);
        v.theme = r.theme;
        // Same file, same pixels: what is rendered stays; later renders come from the working copy.
        v.source = Some(r.bytes());
        t.viewer = Some(v);
        t.reader = Some(Box::new(r));
        t.reader_loading = None;
        refresh(a, id);
        true
    });
    if ok {
        crate::viewer_ui::request(ui, id);
    }
}

/// Thumbnails in the reader's side panel, PDF Expert's size (the tools' strip uses 84 px).
pub const READER_THUMB: f64 = 150.0;

fn reader_viewer(boxes: Vec<viewer::PageBox>) -> Viewer {
    let mut v = Viewer::new(boxes);
    v.thumb_width = READER_THUMB;
    v
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Runs `f` on the reader and viewer of tab `tab_id`, then pushes the tab to the UI.
fn update<R>(tab_id: i32, f: impl FnOnce(&mut ReaderUi, &mut ViewerUi) -> R) -> Option<R> {
    with(|a| {
        let t = a.tab(tab_id)?;
        let (Some(r), Some(v)) = (t.reader.as_deref_mut(), t.viewer.as_deref_mut()) else { return None };
        let out = f(r, v);
        refresh(a, tab_id);
        Some(out)
    })
}

pub fn is_reader(tab_id: i32) -> bool {
    with(|a| a.tab(tab_id).is_some_and(|t| t.reader.is_some()))
}

// ---- geometry ----------------------------------------------------------------------------------

/// Page `page`'s view points per content pixel.
fn k(v: &Viewer, page: usize) -> f64 {
    v.boxes[page].view_size().0 / v.page_rect(page)[2].max(1e-9)
}

/// A content point as page `page`'s view point, held to the page.
fn to_view(v: &Viewer, page: usize, x: f64, y: f64) -> (f64, f64) {
    let [px, py, w, h] = v.page_rect(page);
    let s = k(v, page);
    ((x - px).clamp(0.0, w) * s, (y - py).clamp(0.0, h) * s)
}

/// A view rectangle of `page` as content `[x, y, w, h]`.
fn view_rect_to_content(v: &Viewer, page: usize, r: [f32; 4]) -> [f64; 4] {
    let [px, py, _, _] = v.page_rect(page);
    let s = 1.0 / k(v, page);
    [px + r[0] as f64 * s, py + r[1] as f64 * s, (r[2] - r[0]) as f64 * s, (r[3] - r[1]) as f64 * s]
}

fn view_to_content(v: &Viewer, page: usize, p: (f64, f64)) -> (f64, f64) {
    let [px, py, _, _] = v.page_rect(page);
    let s = 1.0 / k(v, page);
    (px + p.0 * s, py + p.1 * s)
}

/// The page under a content point, or the nearest one vertically (a press in a gap).
fn page_at(v: &Viewer, x: f64, y: f64) -> Option<usize> {
    if let Some((p, ..)) = v.hit(x, y) {
        return Some(p);
    }
    (0..v.page_count()).min_by(|&a, &b| {
        let d = |i: usize| {
            let [_, py, _, h] = v.page_rect(i);
            if y < py { py - y } else if y > py + h { y - py - h } else { 0.0 }
        };
        d(a).total_cmp(&d(b))
    })
}

/// The annotation under view point `p` of `page`, topmost first.
fn annot_at(r: &ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) -> Option<usize> {
    let size = v.boxes[page].view_size();
    r.annots().iter().enumerate().rev().find_map(|(i, a)| {
        if a.page != page {
            return None;
        }
        let boxes: Vec<[f64; 4]> = if a.quads.is_empty() { vec![a.rect] } else { a.quads.iter().map(|q| sheaf_core::redact::bounds(std::slice::from_ref(q))).collect() };
        boxes
            .iter()
            .any(|b| {
                let [x, y, w, h] = viewer::pdf_rect_to_view(&v.boxes[page], size, *b);
                p.0 >= x - 2.0 && p.0 <= x + w + 2.0 && p.1 >= y - 2.0 && p.1 <= y + h + 2.0
            })
            .then_some(i)
    })
}

// ---- the parts -------------------------------------------------------------------------------

mod pointer;
mod forms;
mod sign;
mod print_sheet;
mod nav;
mod edits;
mod saving;
mod pages;
mod tools;
mod search;
mod model;
pub use pointer::*;
pub use forms::*;
pub use sign::*;
pub use print_sheet::*;
pub use nav::*;
pub use edits::*;
pub use saving::*;
pub use pages::*;
pub use tools::*;
pub use search::*;
pub use model::*;
