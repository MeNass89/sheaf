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
use sheaf_core::pdfcraft::engine::{AddedText, Edit, FieldValue, Markup, NewAnnotation, NoteIcon, Shape, Style};
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
/// Fill & Sign: click to place the signature (made once, kept for every document).
pub const SIGN: i32 = 9;

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
    note_seq: i32,
    pen_width: i32,
    /// Pages selected in the Pages panel (empty: the current page is meant).
    page_sel: std::collections::BTreeSet<usize>,
    /// The last page clicked in the panel, the anchor of a ⇧-click range.
    page_anchor: Option<usize>,
    /// Pages being dragged in the panel.
    page_drag: Option<Vec<usize>>,
    /// A page edit is on its way: the outline and the thumbnails follow when it is done.
    structure_pending: bool,
    /// Pages of each edit sent, for undo and redo.
    undo_pages: Vec<Vec<usize>>,
    redo_pages: Vec<Vec<usize>>,
    /// A check box or radio button clicked: its edit, sent once the click is handled.
    pending_field: Option<(usize, Edit)>,
    /// The annotation under the press, selected if the press ends without a drag.
    annot_candidate: Option<usize>,
    /// The panel shown before a search took it over (results, panel 3).
    panel_before_search: Option<i32>,
    results_cache: std::cell::RefCell<Option<(Rc<VecModel<ReaderResultRow>>, Option<usize>)>>,
    /// 0 fit width, 1 fit page, 2 a zoom the user chose.
    zoom_mode: i32,
    /// Pages left by following a link or the outline (⌘[ goes back), and those gone back from.
    back: Vec<usize>,
    forward: Vec<usize>,
    /// Bumped when the outline rows or the annotation list change: the panel models are rebuilt
    /// only then, not on every scroll.
    version: u64,
    outline_cache: std::cell::RefCell<Option<((u64, Option<usize>), ModelRc<ReaderOutlineRow>)>>,
    annots_cache: std::cell::RefCell<Option<((u64, Option<usize>, usize), ModelRc<ReaderAnnotRow>)>>,
}

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
            note_seq: 0,
            pen_width: 1,
            annot_candidate: None,
            pending_field: None,
            page_sel: Default::default(),
            page_anchor: None,
            page_drag: None,
            structure_pending: false,
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
    let existing = with(|a| a.tabs.iter().position(|t| t.reader.as_ref().is_some_and(|r| std::fs::canonicalize(&r.path).unwrap_or_else(|_| r.path.clone()) == canon)));
    if let Some(i) = existing {
        ui.set_active_tab(i as i32);
        return;
    }
    // The tab shows at once ("Opening…"); the document is read and parsed on a worker.
    let id = with(|a| {
        a.next_id += 1;
        let mut t = TabState::new(a.next_id, sheaf_core::tools::ToolId::Edit);
        t.reader_loading = Some(file_name(&path));
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
                let last = crate::home::last_page(&fast.path);
                let mut v = ViewerUi::new(fast, reader_viewer(boxes), scale, t.viewport, t.strip);
                v.theme = PageTheme::from_index(crate::home::prefs().reader_theme);
                v.viewer.set_width(t.viewport.0 as f64);
                // Back where the document was left, as PDF Expert reopens it.
                if let Some(p) = last.filter(|p| *p > 0) {
                    v.show_page(p, None);
                }
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
        Reply::Changed { snap, pages, error } => {
            update(id, |r, v| {
                r.in_flight = r.in_flight.saturating_sub(1);
                r.snap = *snap;
                r.structure_pending = false;
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
                    crate::flow::toast(ui, &format!("Saved {n}"));
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
        Reply::Blocks(page, b) => {
            update(id, |r, _| {
                r.blocks.insert(page, Arc::new(b));
            });
        }
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
                v
            }
        };
        let mut r = ReaderUi::new(ops, snap, path);
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

// ---- pointer -----------------------------------------------------------------------------------

pub fn pressed(ui: &AppWindow, tab_id: i32, x: f32, y: f32, clicks: i32) {
    let (x, y) = (x as f64, y as f64);
    let opened = update(tab_id, |r, v| {
        let Some(page) = page_at(&v.viewer, x, y) else { return false };
        let p = to_view(&v.viewer, page, x, y);
        r.press = Some((page, p));
        match r.tool {
            PEN => {
                r.stroke = Some((page, vec![p]));
                false
            }
            NOTE | TEXTBOX => {
                r.draft = Some(Draft { page, at: p, kind: if r.tool == NOTE { 0 } else { 1 }, block: None, field: None });
                r.note_seq += 1;
                r.press = None;
                true
            }
            EDIT_TEXT => {
                // A paragraph under the pointer is edited in place; elsewhere new text is added.
                let size = v.viewer.boxes[page].view_size();
                // Paragraphs not read yet (just switched tool): the click adds text instead.
                let blocks = r.blocks(page).unwrap_or_default();
                let hit = blocks.iter().enumerate().find_map(|(i, b)| {
                    let [x, y, w, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, b.rect);
                    (p.0 >= x - 2.0 && p.0 <= x + w + 2.0 && p.1 >= y - 2.0 && p.1 <= y + h + 2.0).then(|| (i, b.text.clone(), b.rect, (x, y)))
                });
                r.draft = Some(match hit {
                    Some((i, text, rect, at)) => Draft { page, at, kind: 2, block: Some((i, text, rect)), field: None },
                    None => Draft { page, at: p, kind: 3, block: None, field: None },
                });
                r.note_seq += 1;
                r.press = None;
                true
            }
            ADD_IMAGE => {
                r.press = None;
                r.image_at = Some((page, p));
                true
            }
            SIGN => {
                // Placed at once when a signature exists; otherwise it is made first.
                r.press = None;
                r.sign_at = Some((page, p));
                if saved_signature().is_none() {
                    r.sign_open = true;
                }
                false
            }
            _ => {
                if clicks == 2 {
                    // Double-click: the word.
                    r.sel = r.text(page).and_then(|t| reader::word(&t, (p.0 as f32, p.1 as f32))).map(|w| (page, w));
                    r.press = None;
                    return false;
                }
                // A form field under the pointer is filled: text and choices open an editor,
                // check boxes and radio buttons change at once.
                if r.tool == SELECT
                    && let Some((fi, rect)) = field_at(r, &v.viewer, page, p)
                {
                    r.press = None;
                    r.sel = None;
                    return open_field(r, v, page, fi, rect);
                }
                // A click (no drag) on an annotation with the select tool selects it on release;
                // a drag that starts on one selects the text under it, as in PDF Expert.
                r.annot_candidate = if r.tool == SELECT { annot_at(r, &v.viewer, page, p) } else { None };
                r.selected_annot = None;
                r.sel = None;
                false
            }
        }
    });
    if with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).is_some_and(|r| r.sign_at.is_some() && !r.sign_open)) {
        place_signature(ui, tab_id);
    }
    if opened == Some(true) && with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).is_some_and(|r| r.image_at.is_some())) {
        pick_image(ui, tab_id);
    }
    apply_pending_field(ui, tab_id);
}

/// Add image: the file is chosen, then placed 200 pt wide (less on a small page) at the click.
fn pick_image(ui: &AppWindow, tab_id: i32) {
    let d = crate::dialog::new(ui).set_title("Choose an image").add_filter("Image", &["png", "jpg", "jpeg"]);
    crate::dialog::pick_file(ui, d, move |ui, path| {
        let pic = match sheaf_core::edit::Picture::read(&path) {
            Ok(p) => p,
            Err(e) => return crate::flow::toast(ui, &e.to_string()),
        };
        let edit = update(tab_id, |r, v| {
            let (page, at) = r.image_at.take()?;
            let size = v.viewer.boxes[page].view_size();
            let w = 200.0_f64.min(size.0 * 0.8);
            let h = w * pic.aspect();
            let x0 = at.0.clamp(0.0, (size.0 - w).max(0.0));
            let y0 = at.1.clamp(0.0, (size.1 - h).max(0.0));
            // Display space: points from the displayed page's bottom-left, y up.
            let rect = [x0, size.1 - (y0 + h), x0 + w, size.1 - y0];
            let name = file_name(&path);
            Some((page, Edit::AddImage { page, rect: Some(rect), name, bytes: pic.bytes.clone() }))
        })
        .flatten();
        if let Some((page, e)) = edit {
            apply(ui, tab_id, e, &[page], None);
        }
    });
}

pub fn moved(tab_id: i32, x: f32, y: f32) {
    let (x, y) = (x as f64, y as f64);
    update(tab_id, |r, v| {
        let Some((page, a)) = r.press else { return };
        let b = to_view(&v.viewer, page, x, y);
        if r.tool == PEN {
            if let Some((_, pts)) = r.stroke.as_mut() {
                let last = *pts.last().unwrap_or(&b);
                if (last.0 - b.0).hypot(last.1 - b.1) >= 0.75 {
                    pts.push(b);
                }
            }
            return;
        }
        if (a.0 - b.0).hypot(a.1 - b.1) < 1.5 {
            return;
        }
        r.annot_candidate = None;
        r.sel = r.text(page).and_then(|t| reader::selection(&t, (a.0 as f32, a.1 as f32), (b.0 as f32, b.1 as f32))).map(|s| (page, s));
    });
}

pub fn released(ui: &AppWindow, tab_id: i32, x: f32, y: f32) {
    moved(tab_id, x, y);
    let action = update(tab_id, |r, v| {
        let press = r.press.take();
        if let Some(i) = r.annot_candidate.take()
            && r.sel.is_none()
        {
            r.selected_annot = Some(i);
            // A note or text box opens to read and edit its text, as PDF Expert's pop-ups do.
            if let Some(a) = r.annots().get(i)
                && matches!(a.subtype.as_str(), "Text" | "FreeText")
            {
                let (page, index, contents, rect) = (a.page, a.index, a.contents.clone(), a.rect);
                let size = v.viewer.boxes[page].view_size();
                let [x, y, _, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
                reader_doc::debug(format_args!("open comment {index}: {contents:?}"));
                r.draft = Some(Draft { page, at: (x, y + h + 4.0), kind: 6, block: Some((index, contents, rect)), field: None });
                r.note_seq += 1;
            }
            return None;
        }
        // A click (no drag) on a link with the select tool follows it.
        if r.tool == SELECT
            && r.sel.is_none()
            && let Some((page, p)) = press
            && let Some(target) = link_at(r, &v.viewer, page, p)
        {
            return Some(Pending::Link(target));
        }
        match r.tool {
            PEN => r.stroke.take().filter(|(_, pts)| pts.len() >= 2).map(|(page, pts)| Pending::Ink(page, pts)),
            HIGHLIGHT | UNDERLINE | STRIKE if press.is_some() && r.sel.is_some() => Some(Pending::Markup(r.tool)),
            _ => None,
        }
    })
    .flatten();
    match action {
        Some(Pending::Ink(page, pts)) => add_ink(ui, tab_id, page, pts),
        Some(Pending::Markup(kind)) => markup(ui, tab_id, kind),
        Some(Pending::Link(render::LinkTarget::Page(page, view))) => {
            update(tab_id, |r, v| {
                // An /XYZ destination's top edge, when it has one, is brought near the top.
                let focus = match view {
                    render::DestView::Xyz { top: Some(t), .. } => {
                        let b = &v.viewer.boxes[page.min(v.viewer.page_count().saturating_sub(1))];
                        Some([b.crop[0], t as f64 - 1.0, b.crop[2], t as f64])
                    }
                    _ => None,
                };
                jump(r, v, page, focus);
            });
        }
        Some(Pending::Link(render::LinkTarget::Uri(u))) if u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:") => crate::run::open_detached(&u),
        Some(Pending::Link(_)) | None => {}
    }
}

/// The fillable form field under view point `p` of `page`, with its widget's box.
fn field_at(r: &ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) -> Option<(usize, [f64; 4])> {
    let size = v.boxes[page].view_size();
    r.snap.fields.iter().enumerate().filter(|(_, f)| !f.read_only && f.kind < 4).find_map(|(i, f)| {
        f.widgets.iter().filter(|w| w.0 == page).find_map(|w| {
            let [x, y, ww, hh] = viewer::pdf_rect_to_view(&v.boxes[page], size, w.1);
            (p.0 >= x && p.0 <= x + ww && p.1 >= y && p.1 <= y + hh).then_some((i, w.1))
        })
    })
}

/// Opens field `fi` (its widget at `rect` on `page`): an editor for text and choices; a check
/// box or radio button is set at once. Returns whether an editor opened.
fn open_field(r: &mut ReaderUi, v: &ViewerUi, page: usize, fi: usize, rect: [f64; 4]) -> bool {
    let f = &r.snap.fields[fi];
    match f.kind {
        1 => {
            let on = f.value.first().is_some_and(|x| x != "Off");
            let edit = Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Check(!on) };
            r.pending_field = Some((page, edit));
            false
        }
        2 => {
            let state = f.widgets.iter().find(|w| w.0 == page && w.1 == rect).and_then(|w| w.2.clone());
            let edit = Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Radio(state) };
            r.pending_field = Some((page, edit));
            false
        }
        0 | 3 => {
            let size = v.viewer.boxes[page].view_size();
            let [x, y, _, _] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
            let text = if f.kind == 0 { f.value.join("") } else { String::new() };
            r.draft = Some(Draft { page, at: (x, y), kind: if f.kind == 0 { 4 } else { 5 }, block: Some((0, text, rect)), field: Some((fi, rect)) });
            r.note_seq += 1;
            true
        }
        _ => false,
    }
}

/// Field editor: Tab saves the field and opens the next one in the form's tab order.
pub fn field_tab(ui: &AppWindow, tab_id: i32, text: &str, back: bool) {
    let next = with(|a| {
        let t = a.tab(tab_id)?;
        let r = t.reader.as_ref()?;
        let (fi, rect) = r.draft.as_ref()?.field?;
        let order = |i: usize, rc: [f64; 4]| r.snap.fields[i].widgets.iter().find(|w| w.1 == rc).map(|w| w.3).unwrap_or(usize::MAX);
        let here = order(fi, rect);
        let mut all: Vec<(usize, usize, usize, [f64; 4])> = r
            .snap
            .fields
            .iter()
            .enumerate()
            // Text fields only: a check box reached by Tab must not change.
            .filter(|(_, f)| !f.read_only && f.kind == 0)
            .flat_map(|(i, f)| f.widgets.iter().map(move |w| (w.3, i, w.0, w.1)))
            .collect();
        all.sort_by_key(|w| w.0);
        let pos = all.iter().position(|w| w.0 == here && w.1 == fi)?;
        let n = all.len();
        let k = if back { (pos + n - 1) % n } else { (pos + 1) % n };
        Some((all[k].1, all[k].2, all[k].3))
    });
    note_commit(ui, tab_id, text);
    if let Some((fi, page, rect)) = next {
        update(tab_id, |r, v| {
            let size = v.viewer.boxes[page].view_size();
            let [_, y, _, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
            let top = v.viewer.page_rect(page)[1] + y / k(&v.viewer, page);
            if top < v.top || top + h > v.top + v.height {
                v.show_page(page, Some(rect));
            }
            open_field(r, v, page, fi, rect);
        });
        apply_pending_field(ui, tab_id);
    }
}

/// Clear the form (back to its defaults) or flatten it into the pages.
pub fn form_op(ui: &AppWindow, tab_id: i32, op: i32) {
    let edit = match op {
        0 => Edit::ResetForm { names: None },
        1 => Edit::Flatten { comments: false, fields: true },
        _ => Edit::Flatten { comments: true, fields: true },
    };
    apply_structure(ui, tab_id, edit);
}

/// A choice picked in a field's list.
pub fn choice_picked(ui: &AppWindow, tab_id: i32, k: i32) {
    let edit = update(tab_id, |r, _| {
        let d = r.draft.take()?;
        let (fi, _) = d.field?;
        let f = r.snap.fields.get(fi)?;
        let (export, _) = f.options.get(k.max(0) as usize)?;
        Some((d.page, Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Choice(vec![export.clone()]) }))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

fn apply_pending_field(ui: &AppWindow, tab_id: i32) {
    if let Some((page, e)) = update(tab_id, |r, _| r.pending_field.take()).flatten() {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// The signature kept in the preferences.
fn saved_signature() -> Option<sheaf_core::sign::Graphic> {
    let p = crate::home::prefs();
    if !p.signature_strokes.is_empty() {
        return Some(sheaf_core::sign::Graphic::Drawn(p.signature_strokes));
    }
    p.signature_name.filter(|n| !n.trim().is_empty()).map(sheaf_core::sign::Graphic::Typed)
}

/// Places the signature where the Signature tool was clicked, 150 pt wide as PDF Expert's default.
fn place_signature(ui: &AppWindow, tab_id: i32) {
    let Some(graphic) = saved_signature() else { return };
    let made = update(tab_id, |r, v| {
        let (page, at) = r.sign_at.take()?;
        let b = &v.viewer.boxes[page];
        let (w, h) = b.view_size();
        let info = render::PageInfo { width: w as f32, height: h as f32, label: String::new(), crop: b.crop.map(|c| c as f32), rotation: b.rotation };
        Some((page, sheaf_core::sign::signature_edit(&info, page, [at.0, at.1], 150.0_f64.min(w * 0.6), &graphic)))
    })
    .flatten();
    match made {
        Some((page, Ok(e))) => apply(ui, tab_id, e, &[page], None),
        Some((_, Err(e))) => crate::flow::toast(ui, &e.to_string()),
        None => {}
    }
}

/// The signature pad's answer: strokes drawn (pad commands and size) or a name typed; empty
/// cancels. The signature is kept and placed where the tool was clicked.
pub fn signature_made(ui: &AppWindow, tab_id: i32, commands: &str, w: f32, h: f32, name: &str) {
    let strokes = crate::sign_ui::pad_strokes(commands, w, h);
    let name = name.trim().to_string();
    update(tab_id, |r, _| r.sign_open = false);
    if strokes.is_empty() && name.is_empty() {
        update(tab_id, |r, _| r.sign_at = None);
        return;
    }
    crate::home::set_signature(strokes, (!name.is_empty()).then_some(name));
    place_signature(ui, tab_id);
}

/// Forget the kept signature and draw a new one at the next click.
pub fn signature_reset(tab_id: i32) {
    crate::home::set_signature(Vec::new(), None);
    update(tab_id, |r, _| r.sign_open = r.sign_at.is_some());
}

enum Pending {
    Ink(usize, Vec<(f64, f64)>),
    Markup(i32),
    Link(render::LinkTarget),
}

/// The link under view point `p` of `page`.
fn link_at(r: &ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) -> Option<render::LinkTarget> {
    let size = v.boxes[page].view_size();
    r.snap.links.iter().filter(|l| l.page == page).find_map(|l| {
        let [x, y, w, h] = viewer::pdf_rect_to_view(&v.boxes[page], size, l.rect.map(f64::from));
        (p.0 >= x && p.0 <= x + w && p.1 >= y && p.1 <= y + h).then(|| l.target.clone())
    })
}

/// Keeps the page a document tab shows, for its next opening.
pub fn remember_position(tab_id: i32) {
    if let Some((path, page)) = with(|a| {
        let t = a.tab(tab_id)?;
        Some((t.reader.as_ref()?.path.clone(), t.viewer.as_ref()?.current()))
    }) {
        crate::home::remember_page(&path, page);
    }
}

/// Whether the content point (`x`, `y`) is over a link (the hand cursor).
pub fn over_link(tab_id: i32, x: f32, y: f32) -> bool {
    with(|a| {
        let t = a.tab(tab_id)?;
        let (r, v) = (t.reader.as_ref()?, t.viewer.as_ref()?);
        let page = v.viewer.hit(x as f64, y as f64)?.0;
        let p = to_view(&v.viewer, page, x as f64, y as f64);
        link_at(r, &v.viewer, page, p).map(|_| ())
    })
    .is_some()
}

/// Goes to `page`, remembering where the reader was for ⌘[.
fn jump(r: &mut ReaderUi, v: &mut ViewerUi, page: usize, focus: Option<[f64; 4]>) {
    let cur = v.current();
    if r.back.last() != Some(&cur) {
        r.back.push(cur);
    }
    r.forward.clear();
    v.show_page(page, focus);
}

/// ⌘[ (back, `dir` −1) and ⌘] (forward, +1).
pub fn history(tab_id: i32, dir: i32) {
    update(tab_id, |r, v| {
        let cur = v.current();
        let to = if dir < 0 { r.back.pop() } else { r.forward.pop() };
        if let Some(p) = to {
            if dir < 0 { r.forward.push(cur) } else { r.back.push(cur) }
            v.show_page(p, None);
        }
    });
}

// ---- edits -------------------------------------------------------------------------------------

/// Sends `edit` to the document thread; the pages it touches render again when it is done.
/// `provisional` is drawn meanwhile.
fn apply(ui: &AppWindow, tab_id: i32, edit: Edit, pages: &[usize], provisional: Option<Provisional>) {
    let err = update(tab_id, |r, _| {
        if !r.snap.read_only.is_empty() {
            return Some(r.snap.read_only.clone());
        }
        r.send(Op::Apply { edit, pages: pages.to_vec() });
        r.undo_pages.push(pages.to_vec());
        r.redo_pages.clear();
        r.provisional.extend(provisional);
        None
    })
    .flatten();
    if let Some(e) = err {
        crate::flow::toast(ui, &e);
    }
}

/// The working copy changed: new bytes to render, annotations to list.
fn after_change(r: &mut ReaderUi, v: &mut ViewerUi, pages: Option<&[usize]>) {
    v.source = Some(r.bytes());
    if v.viewer.boxes != r.snap.boxes {
        // A page was added, removed or turned.
        let width = v.viewer.content_width();
        v.viewer = reader_viewer(r.snap.boxes.clone());
        v.viewer.set_width(width);
    }
    match pages {
        Some(ps) => ps.iter().for_each(|&p| {
            v.page_changed(p);
            r.blocks.remove(&p);
            r.blocks_asked.remove(&p);
        }),
        None => {
            v.all_changed();
            r.blocks.clear();
            r.blocks_asked.clear();
        }
    }
    // Annotations don't change the text layer; edits of the page content do.
    let stale_text = r.text_renderer.as_ref().is_some_and(|(b, _)| !Arc::ptr_eq(b, &r.snap.bytes));
    if stale_text {
        match pages {
            Some(ps) => ps.iter().for_each(|p| {
                r.texts.remove(p);
            }),
            None => r.texts.clear(),
        }
    }
    r.version += 1;
    if r.selected_annot.is_some_and(|i| i >= r.snap.annots.len()) {
        r.selected_annot = None;
    }
    r.rows = reader::outline_rows(&r.snap.outline, &r.toggled);
}

fn style(colour: [f64; 3], width: f64) -> Style {
    Style { color: colour, opacity: 1.0, width, fill: None }
}

/// Highlight, underline or strike the selection (`kind` 1, 2, 3).
pub fn markup(ui: &AppWindow, tab_id: i32, kind: i32) {
    let made = update(tab_id, |r, v| {
        let (page, range) = r.sel.clone()?;
        let text = r.text(page)?;
        let rects = text.line_rects(range.clone());
        let quads = reader::quads(&v.viewer.boxes[page], &rects);
        let contents = reader::selected_text(&text, range);
        let (m, colour) = match kind {
            UNDERLINE => (Markup::Underline, INK[r.ink_colour]),
            STRIKE => (Markup::StrikeOut, INK[r.ink_colour]),
            _ => (Markup::Highlight, COLOURS[r.light]),
        };
        r.sel = None;
        let prov = Provisional { page, rects, stroke: Vec::new(), colour, kind };
        Some((page, Edit::AddAnnotation(NewAnnotation { page, shape: Shape::TextMarkup { kind: m, quads }, style: style(colour, 1.0), contents, author: author() }), prov))
    })
    .flatten();
    if let Some((page, edit, prov)) = made {
        apply(ui, tab_id, edit, &[page], Some(prov));
    }
}

fn author() -> String {
    crate::home::prefs().author.unwrap_or_default()
}

/// Chaikin corner cutting, twice: a hand-drawn stroke loses the jitter of the pointer (PDF Expert
/// smooths its pen the same way). The end points stay where they were.
pub fn smooth(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut cur = pts.to_vec();
    for _ in 0..2 {
        if cur.len() < 3 {
            break;
        }
        let mut next = vec![cur[0]];
        for w in cur.windows(2) {
            let (a, b) = (w[0], w[1]);
            next.push((0.75 * a.0 + 0.25 * b.0, 0.75 * a.1 + 0.25 * b.1));
            next.push((0.25 * a.0 + 0.75 * b.0, 0.25 * a.1 + 0.75 * b.1));
        }
        next.push(*cur.last().expect("at least 3 points"));
        cur = next;
    }
    cur
}

fn add_ink(ui: &AppWindow, tab_id: i32, page: usize, pts: Vec<(f64, f64)>) {
    let pts = smooth(&pts);
    let edit = update(tab_id, |r, v| {
        let b = &v.viewer.boxes[page];
        let size = b.view_size();
        let stroke: Vec<[f64; 2]> = pts.iter().map(|p| { let (x, y) = viewer::view_to_pdf(b, size, p.0, p.1); [x, y] }).collect();
        let prov = Provisional { page, rects: Vec::new(), stroke: pts.clone(), colour: INK[r.ink_colour], kind: PEN };
        (Edit::AddAnnotation(NewAnnotation { page, shape: Shape::Ink { strokes: vec![stroke] }, style: style(INK[r.ink_colour], pen_points(r.pen_width)), contents: String::new(), author: author() }), prov)
    });
    if let Some((e, prov)) = edit {
        apply(ui, tab_id, e, &[page], Some(prov));
    }
}

pub fn note_commit(ui: &AppWindow, tab_id: i32, text: &str) {
    let text = text.trim().to_string();
    let edit = update(tab_id, |r, v| {
        let d = r.draft.take()?;
        let b = &v.viewer.boxes[d.page];
        let size = b.view_size();
        if d.kind == 6 {
            // An existing note or text box: its new text.
            let (index, old, _) = d.block?;
            if text == old.trim() {
                return None;
            }
            return Some((d.page, Edit::SetAnnotationContents { page: d.page, index, text: text.clone() }));
        }
        if d.kind == 4 {
            // A form text field: its new value (empty clears it).
            let (fi, _) = d.field?;
            let f = r.snap.fields.get(fi)?;
            if text == f.value.join("") {
                return None;
            }
            return Some((d.page, Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Text(text.clone()) }));
        }
        if d.kind == 2 {
            // The paragraph's new text (empty deletes it), rewrapped to its box by PdfCraft.
            let (block, old, _) = d.block?;
            if text == old.trim() {
                return None;
            }
            // A standard font in place of the embedded one (for characters its subset lacks) was
            // tried: PdfCraft then draws every row of the paragraph on one line. Kept as it is.
            let style = sheaf_core::pdfcraft::edit::BlockStyle::default();
            return Some((d.page, Edit::EditTextBlock { page: d.page, block, text: text.clone(), style }));
        }
        if text.is_empty() {
            return None;
        }
        if d.kind == 3 {
            let (w, h) = sheaf_core::edit::text_extent(&text, 12.0);
            let x0 = d.at.0.min((size.0 - w - 12.0).max(0.0));
            let rect = [x0, size.1 - (d.at.1 + h), x0 + w + 12.0, size.1 - d.at.1];
            return Some((d.page, Edit::AddText { page: d.page, text: AddedText { rect, text: text.clone(), size: 12.0, color: INK[r.ink_colour], ..Default::default() } }));
        }
        let (x, y) = viewer::view_to_pdf(b, size, d.at.0, d.at.1);
        let shape = if d.kind == 0 {
            Shape::Note { at: [x, y], icon: NoteIcon::Comment }
        } else {
            // A box 200 pt wide, tall enough for the lines typed (12 pt text), kept on the page.
            let lines = text.lines().count().max(1) as f64 + (text.len() as f64 / 38.0).floor();
            // 200 pt wide, moved left or up so the whole box is on the page (PDF Expert does the same).
            let (w, h) = (200.0_f64.min(size.0), (lines * 15.0 + 8.0).min(size.1));
            let at = (d.at.0.min(size.0 - w).max(0.0), d.at.1.min(size.1 - h).max(0.0));
            let r = viewer::view_rect_to_pdf(b, size, at, (at.0 + w, at.1 + h));
            Shape::TextBox { rect: r, font_size: 12.0 }
        };
        let colour = if d.kind == 0 { COLOURS[r.light] } else { INK[r.ink_colour] };
        Some((d.page, Edit::AddAnnotation(NewAnnotation { page: d.page, shape, style: style(colour, if d.kind == 0 { 1.0 } else { 0.0 }), contents: text, author: author() })))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

pub fn note_cancel(tab_id: i32) {
    update(tab_id, |r, _| r.draft = None);
}

pub fn delete_selected(ui: &AppWindow, tab_id: i32) {
    let edit = update(tab_id, |r, _| {
        let i = r.selected_annot.take()?;
        let a = r.annots().get(i)?;
        Some((a.page, Edit::DeleteAnnotation { page: a.page, index: a.index }))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

pub fn annot_delete(ui: &AppWindow, tab_id: i32, k: i32) {
    update(tab_id, |r, _| r.selected_annot = Some(k.max(0) as usize));
    delete_selected(ui, tab_id);
}

pub fn undo(_ui: &AppWindow, tab_id: i32, redo: bool) {
    update(tab_id, |r, _| {
        r.selected_annot = None;
        // The pages the undone edit touched, so only they render again.
        let (from, to) = if redo { (&mut r.redo_pages, &mut r.undo_pages) } else { (&mut r.undo_pages, &mut r.redo_pages) };
        let pages = from.pop();
        if let Some(p) = &pages {
            to.push(p.clone());
        }
        // A page edit (no pages listed) changes them all.
        let pages = pages.filter(|p| !p.is_empty());
        r.send(if redo { Op::Redo { pages } } else { Op::Undo { pages } });
    });
}

// ---- saving ------------------------------------------------------------------------------------

/// ⌘S: the edits are written into the file itself, as an incremental update.
pub fn save(_ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| {
        if r.dirty() {
            r.send(Op::Save { to: None });
        }
    });
}

/// ⇧⌘S: a full copy with the edits, under a new name; the tab then shows that file.
pub fn save_as(ui: &AppWindow, tab_id: i32) {
    let Some((dir, name)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.name.clone())) else { return };
    let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(name).add_filter("PDF", &["pdf"]);
    crate::dialog::save_file(ui, d, move |_, target| {
        update(tab_id, |r, _| r.send(Op::Save { to: Some(target) }));
    });
}

/// The file name of tab `tab_id` when closing it would lose edits.
pub fn unsaved_name(tab_id: i32) -> Option<String> {
    with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).filter(|r| r.dirty()).map(|r| r.name.clone()))
}

/// The document goes to the Tools page, ready for the tool picked (unsaved edits are saved first).
pub fn to_tools(ui: &AppWindow, tab_id: i32) {
    if unsaved_name(tab_id).is_some() {
        save(ui, tab_id);
    }
    let Some(path) = with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).map(|r| r.path.clone())) else { return };
    with(|a| {
        a.home_files.retain(|p| p != &path);
        a.home_files.push(path);
    });
    ui.set_active_tab(-1);
    ui.set_page(sheaf_app::Page::Tools);
    crate::flow::toast(ui, "Choose a tool for your file");
}

// ---- pages (the Pages panel) --------------------------------------------------------------------

/// A thumbnail clicked: it is shown; ⇧ extends the selection from the last one clicked, ⌘ adds or
/// removes it, a plain click selects it alone (PDF Expert's thumbnails).
pub fn thumb_pressed(tab_id: i32, page: i32, extend: bool, toggle: bool) {
    update(tab_id, |r, v| {
        let p = page.max(0) as usize;
        match (extend, toggle, r.page_anchor) {
            (true, _, Some(a)) => {
                let (lo, hi) = (a.min(p), a.max(p));
                r.page_sel = (lo..=hi).collect();
            }
            (_, true, _) => {
                if !r.page_sel.remove(&p) {
                    r.page_sel.insert(p);
                }
                r.page_anchor = Some(p);
            }
            _ => {
                r.page_sel = [p].into_iter().collect();
                r.page_anchor = Some(p);
            }
        }
        v.show_page(p, None);
    });
}

/// The pages an operation acts on: the selection, else the page shown.
fn op_pages(r: &ReaderUi, v: &ViewerUi) -> Vec<usize> {
    if r.page_sel.is_empty() { vec![v.current()] } else { r.page_sel.iter().copied().collect() }
}

pub fn page_op(ui: &AppWindow, tab_id: i32, op: i32) {
    if op == 4 {
        return extract(ui, tab_id);
    }
    let made = update(tab_id, |r, v| {
        let pages = op_pages(r, v);
        let n = v.viewer.page_count();
        let edit = match op {
            0 | 1 => Edit::RotatePages { pages: pages.clone(), degrees: if op == 0 { -90 } else { 90 } },
            2 => {
                if pages.len() >= n {
                    return Err("A PDF needs at least one page".to_string());
                }
                Edit::DeletePages { pages: pages.clone() }
            }
            3 => Edit::DuplicatePages { pages: pages.clone() },
            5 => {
                let after = *pages.last().unwrap_or(&0);
                let (w, h) = v.viewer.boxes[after].view_size();
                Edit::InsertBlankPage { at: after + 1, width: w, height: h }
            }
            _ => return Err(String::new()),
        };
        // Page numbers shift: the selection follows what is left.
        r.page_sel.clear();
        Ok(edit)
    });
    match made {
        Some(Ok(e)) => apply_structure(ui, tab_id, e),
        Some(Err(m)) if !m.is_empty() => crate::flow::toast(ui, &m),
        _ => {}
    }
}

/// An edit that adds, removes, moves or turns pages: every page renders again.
fn apply_structure(ui: &AppWindow, tab_id: i32, edit: Edit) {
    let err = update(tab_id, |r, _| {
        if !r.snap.read_only.is_empty() {
            return Some(r.snap.read_only.clone());
        }
        r.send(Op::Apply { edit, pages: Vec::new() });
        r.undo_pages.push(Vec::new());
        r.redo_pages.clear();
        r.structure_pending = true;
        None
    })
    .flatten();
    if let Some(e) = err {
        crate::flow::toast(ui, &e);
    }
}

/// The selected pages (or the one shown) written to a new PDF chosen in a Save dialog, which then
/// opens in its own tab.
fn extract(ui: &AppWindow, tab_id: i32) {
    let Some((pages, dir, stem)) = update(tab_id, |r, v| {
        (op_pages(r, v), crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    }) else {
        return;
    };
    let name = if pages.len() == 1 { format!("{stem} - page {}.pdf", pages[0] + 1) } else { format!("{stem} - {} pages.pdf", pages.len()) };
    let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(name).add_filter("PDF", &["pdf"]);
    crate::dialog::save_file(ui, d, move |_, target| {
        update(tab_id, |r, _| r.send(Op::Extract { pages, to: target }));
    });
}

/// A thumbnail dragged to content y `y` of the panel: the drop line's y (−1: none).
pub fn thumb_drag(tab_id: i32, page: i32, y: f32) -> f32 {
    update(tab_id, |r, v| {
        if r.page_drag.is_none() {
            let p = page.max(0) as usize;
            if !r.page_sel.contains(&p) {
                r.page_sel = [p].into_iter().collect();
            }
            r.page_drag = Some(r.page_sel.iter().copied().collect());
        }
        let (_, line) = drop_gap(&v.viewer, y as f64);
        line as f32
    })
    .unwrap_or(-1.0)
}

/// The page whose thumbnail is at content y of the panel (−1: a gap).
pub fn thumb_at(tab_id: i32, y: f32) -> i32 {
    with(|a| {
        let v = a.tab(tab_id)?.viewer.as_ref()?;
        let y = y as f64;
        v.viewer.thumb_layout().iter().position(|(t, h)| y >= *t && y <= t + h + 18.0).map(|p| p as i32)
    })
    .unwrap_or(-1)
}

/// The gap a drop at content y lands in (0 = before the first page) and the y of its line.
fn drop_gap(v: &Viewer, y: f64) -> (usize, f64) {
    let tl = v.thumb_layout();
    let gap = tl.iter().position(|(top, h)| y < top + h / 2.0).unwrap_or(tl.len());
    let line = match tl.get(gap) {
        Some((top, _)) => top - viewer::PAGE_GAP / 2.0,
        None => tl.last().map(|(t, h)| t + h + viewer::PAGE_GAP / 2.0).unwrap_or(0.0),
    };
    (gap, line)
}

pub fn thumb_drop(ui: &AppWindow, tab_id: i32, y: f32) {
    let edit = update(tab_id, |r, v| {
        let pages = r.page_drag.take()?;
        let (gap, _) = drop_gap(&v.viewer, y as f64);
        // `to` is the position in the result: the gap less the moved pages before it.
        let to = gap - pages.iter().filter(|p| **p < gap).count();
        if pages.iter().enumerate().all(|(k, p)| *p == to + k) {
            return None;
        }
        r.page_sel = (to..to + pages.len()).collect();
        Some(Edit::MovePages { pages, to })
    })
    .flatten();
    if let Some(e) = edit {
        apply_structure(ui, tab_id, e);
    }
}

// ---- tools, panel, theme -----------------------------------------------------------------------

pub fn set_tool(tab_id: i32, tool: i32) {
    update(tab_id, |r, v| {
        r.tool = tool.clamp(SELECT, SIGN);
        r.draft = None;
        r.stroke = None;
        load_blocks(r, v);
    });
}

/// Edit text: the paragraphs of the pages in view are read so they can be outlined.
fn load_blocks(r: &mut ReaderUi, v: &ViewerUi) {
    if r.tool == EDIT_TEXT {
        for p in v.viewer.wanted(v.top, v.height) {
            r.blocks(p);
        }
    }
}

/// The view scrolled or resized.
pub fn viewport_changed(ui: &AppWindow, tab_id: i32) {
    keep_fit(ui, tab_id);
    let editing = with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).is_some_and(|r| r.tool == EDIT_TEXT));
    if editing {
        update(tab_id, |r, v| load_blocks(r, v));
    }
}

pub fn set_tool_and_mark(ui: &AppWindow, tab_id: i32, tool: i32) {
    let has_sel = update(tab_id, |r, _| r.sel.is_some()).unwrap_or(false);
    set_tool(tab_id, tool);
    if has_sel && (HIGHLIGHT..=STRIKE).contains(&tool) {
        markup(ui, tab_id, tool);
    }
}

/// The colour of the current tool: highlights and notes take the light palette, strokes the ink one.
/// With an annotation selected, it is recoloured (PDF Expert's colour row does the same).
pub fn set_colour(ui: &AppWindow, tab_id: i32, k: i32) {
    let k = k.clamp(0, 4) as usize;
    let restyle = update(tab_id, |r, _| {
        if let Some(a) = r.selected_annot.and_then(|i| r.annots().get(i)) {
            let light = matches!(a.subtype.as_str(), "Highlight" | "Text");
            let colour = if light { COLOURS[k] } else { INK[k] };
            return Some((a.page, Edit::StyleAnnotation { page: a.page, index: a.index, color: Some(colour), opacity: None, width: None, endings: None }));
        }
        if uses_light(r.tool) {
            r.light = k;
        } else {
            r.ink_colour = k;
        }
        None
    })
    .flatten();
    if let Some((page, e)) = restyle {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// Pen thickness in points: 0 fine, 1 medium, 2 thick.
pub fn set_pen_width(tab_id: i32, k: i32) {
    update(tab_id, |r, _| r.pen_width = k.clamp(0, 2));
}

fn pen_points(k: i32) -> f64 {
    [1.0, 2.0, 4.0][k.clamp(0, 2) as usize]
}

fn uses_light(tool: i32) -> bool {
    matches!(tool, SELECT | HIGHLIGHT | NOTE)
}

pub fn set_theme(ui: &AppWindow, tab_id: i32, theme: i32) {
    update(tab_id, |r, v| {
        let t = PageTheme::from_index(theme);
        if t != r.theme {
            r.theme = t;
            v.theme = t;
            v.all_changed();
        }
    });
    crate::home::set_reader_theme(theme);
    crate::viewer_ui::request(ui, tab_id);
}

pub fn set_panel(tab_id: i32, panel: i32) {
    update(tab_id, |r, _| {
        r.panel = panel.clamp(0, 3);
        r.panel_open = true;
    });
}

pub fn toggle_panel(tab_id: i32) {
    update(tab_id, |r, _| r.panel_open = !r.panel_open);
}

pub fn outline_clicked(tab_id: i32, row: i32) {
    update(tab_id, |r, v| {
        if let Some(p) = r.rows.get(row.max(0) as usize).and_then(|x| x.page) {
            jump(r, v, p, None);
        }
    });
}

pub fn outline_toggled(tab_id: i32, row: i32) {
    update(tab_id, |r, _| {
        if let Some(path) = r.rows.get(row.max(0) as usize).map(|x| x.path.clone()) {
            if !r.toggled.remove(&path) {
                r.toggled.insert(path);
            }
            r.rows = reader::outline_rows(&r.snap.outline, &r.toggled);
            r.version += 1;
        }
    });
}

pub fn annot_clicked(tab_id: i32, k: i32) {
    update(tab_id, |r, v| {
        let k = k.max(0) as usize;
        let Some(a) = r.annots().get(k) else { return };
        let (page, rect) = (a.page, a.rect);
        r.selected_annot = Some(k);
        r.sel = None;
        v.show_page(page, Some(rect));
    });
}

// ---- navigation --------------------------------------------------------------------------------

pub fn go_to_page(tab_id: i32, text: &str) {
    update(tab_id, |r, v| {
        let t = text.trim();
        let n = v.viewer.page_count();
        // A number, or a page label ("iv", "A-3").
        let page = t.parse::<usize>().ok().filter(|p| (1..=n).contains(p)).map(|p| p - 1).or_else(|| r.snap.labels.iter().position(|l| l.eq_ignore_ascii_case(t)));
        if let Some(p) = page {
            jump(r, v, p, None);
        }
    });
}

pub fn page_step(tab_id: i32, step: i32) {
    update(tab_id, |_, v| {
        let n = v.viewer.page_count();
        if n == 0 {
            return;
        }
        let cur = v.current();
        match step {
            10 => v.show_page(n - 1, None),
            -10 => v.show_page(0, None),
            2 | -2 => {
                let h = v.height * 0.9;
                v.scroll = (v.top + if step > 0 { h } else { -h }).clamp(0.0, (v.viewer.content_height() - v.height).max(0.0));
                v.scroll_seq += 1;
            }
            s => {
                // From the top of the current page: ← goes to its start when it is scrolled into.
                let top_of = v.viewer.page_rect(cur)[1] - viewer::PAGE_GAP;
                let target = if s < 0 && v.top > top_of + 4.0 { cur } else { (cur as i64 + s as i64).clamp(0, n as i64 - 1) as usize };
                v.show_page(target, None);
            }
        }
    });
}

pub fn scroll_by(tab_id: i32, dy: f32) {
    update(tab_id, |_, v| {
        v.scroll = (v.top + dy as f64).clamp(0.0, (v.viewer.content_height() - v.height).max(0.0));
        v.scroll_seq += 1;
    });
}

/// PDF Expert's View menu: fit width (0, ⌘9), fit page (1, ⌘8), 100 % actual size (2, ⌘0).
pub fn zoom_fit(ui: &AppWindow, tab_id: i32, mode: i32) {
    let z = update(tab_id, |r, v| {
        let cur = v.current();
        r.zoom_mode = if mode == 2 { 2 } else { mode.clamp(0, 1) };
        match mode {
            1 => v.viewer.fit_page_zoom(v.height, cur..cur + 1),
            2 => v.viewer.zoom_for_actual(cur, 100.0),
            _ => 100,
        }
    });
    if let Some(z) = z {
        crate::viewer_ui::zoom_to(ui, tab_id, z);
        update(tab_id, |_, v| {
            let cur = v.current();
            v.show_page(cur, None);
        });
    }
}

/// A zoom chosen with + / − or the wheel: the view no longer follows the window's size.
pub fn zoom_changed(tab_id: i32) {
    update(tab_id, |r, _| r.zoom_mode = 2);
}

/// Fit page is kept when the window or the panel changes size, as in PDF Expert.
fn keep_fit(ui: &AppWindow, tab_id: i32) {
    let z = with(|a| {
        let t = a.tab(tab_id)?;
        let (r, v) = (t.reader.as_ref()?, t.viewer.as_ref()?);
        if r.zoom_mode != 1 {
            return None;
        }
        let cur = v.current();
        let z = v.viewer.fit_page_zoom(v.height, cur..cur + 1);
        (z != v.viewer.zoom()).then_some(z)
    });
    if let Some(z) = z {
        crate::viewer_ui::zoom_to(ui, tab_id, z);
    }
}

pub fn escape(tab_id: i32) {
    update(tab_id, |r, _| {
        if r.draft.take().is_some() {
            return;
        }
        if r.sel.take().is_some() || r.selected_annot.take().is_some() {
            return;
        }
        r.tool = SELECT;
    });
}

pub fn select_all(tab_id: i32) {
    update(tab_id, |r, v| {
        let page = v.current();
        r.sel = r.text(page).filter(|t| !t.glyphs.is_empty()).map(|t| (page, 0..t.glyphs.len()));
    });
}

// ---- search ------------------------------------------------------------------------------------

pub fn search(ui: &AppWindow, tab_id: i32, query: &str) {
    let q = query.trim().to_string();
    let Some((bytes, password, n, generation, case, whole)) = update(tab_id, |r, v| {
        r.search.generation += 1;
        r.search.query = q.clone();
        r.search.hits.clear();
        r.search.snippets.clear();
        r.search.current = None;
        r.search.busy = !q.is_empty();
        if !q.is_empty() {
            // The results show in the side panel, as in PDF Expert (evidence 09).
            r.panel_before_search = Some(r.panel);
            r.panel = 3;
            r.panel_open = true;
        }
        (r.snap.bytes.clone(), r.password(), v.viewer.page_count(), r.search.generation, r.search.case_sensitive, r.search.whole_words)
    }) else {
        return;
    };
    if q.is_empty() {
        return;
    }
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let config = render::RenderConfig { password: password.as_deref().map(Arc::from), ..Default::default() };
        let mut rr = render::PageRenderer::new(bytes, config);
        let mut texts = Vec::new();
        let mut hits = Vec::new();
        let mut snippets = Vec::new();
        // Results come back in batches so the first pages show while the rest are read.
        let mut sent = 0;
        for page in 0..n {
            let res = rr.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
            if let Some(t) = res.text {
                for m in t.find_opts(&q, case, whole) {
                    snippets.push(snippet(&t, m.clone()));
                    hits.push((page, t.line_rects(m)));
                }
                texts.push((page, t));
            }
            let last = page + 1 == n;
            if last || (hits.len() > sent && (page % 16 == 15)) {
                let batch: Vec<_> = hits[sent..].to_vec();
                let snips: Vec<_> = snippets[sent..].to_vec();
                sent = hits.len();
                let tx = std::mem::take(&mut texts);
                let w = weak.clone();
                let _ = w.upgrade_in_event_loop(move |_| {
                    update(tab_id, |r, v| {
                        if r.search.generation != generation {
                            return;
                        }
                        for (p, t) in tx {
                            r.texts.entry(p).or_insert(t);
                        }
                        let first = r.search.hits.is_empty() && !batch.is_empty();
                        r.search.hits.extend(batch);
                        r.search.snippets.extend(snips);
                        r.version += 1;
                        r.search.busy = !last;
                        if first {
                            r.search.current = Some(0);
                            let (page, rects) = &r.search.hits[0];
                            let rect = union(rects);
                            show_view_rect(v, *page, rect);
                        }
                    });
                });
            }
        }
    });
}

fn union(rects: &[[f32; 4]]) -> [f32; 4] {
    rects.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, r| [a[0].min(r[0]), a[1].min(r[1]), a[2].max(r[2]), a[3].max(r[3])])
}

/// Up to 40 characters of context on each side of a match, on one line.
fn snippet(t: &PageText, m: Range<usize>) -> (String, String, String) {
    let n = t.glyphs.len();
    let flat = |s: String| s.replace('\n', " ");
    let before = flat(t.text_of(m.start.saturating_sub(40)..m.start));
    let after = flat(t.text_of(m.end..(m.end + 40).min(n)));
    let before = if m.start > 40 { format!("…{}", before.trim_start()) } else { before };
    let after = if m.end + 40 < n { format!("{}…", after.trim_end()) } else { after };
    (before, flat(t.text_of(m)), after)
}

pub fn search_options(ui: &AppWindow, tab_id: i32, case: bool, whole: bool, query: &str) {
    update(tab_id, |r, _| {
        r.search.case_sensitive = case;
        r.search.whole_words = whole;
    });
    if !query.trim().is_empty() {
        search(ui, tab_id, query);
    }
}

pub fn result_clicked(tab_id: i32, k: i32) {
    update(tab_id, |r, v| {
        let k = k.max(0) as usize;
        let Some((page, rects)) = r.search.hits.get(k) else { return };
        let (page, rect) = (*page, union(rects));
        r.search.current = Some(k);
        show_view_rect(v, page, rect);
    });
}

/// Scrolls so the view rectangle `r` of `page` is a third down the viewport.
fn show_view_rect(v: &mut ViewerUi, page: usize, r: [f32; 4]) {
    let c = view_rect_to_content(&v.viewer, page, r);
    v.scroll = (c[1] - v.height / 3.0).clamp(0.0, (v.viewer.content_height() - v.height).max(0.0));
    v.scroll_seq += 1;
}

pub fn search_step(tab_id: i32, dir: i32) {
    update(tab_id, |r, v| {
        let n = r.search.hits.len();
        if n == 0 {
            return;
        }
        let next = match r.search.current {
            None => 0,
            Some(c) => ((c as i64 + dir.signum() as i64).rem_euclid(n as i64)) as usize,
        };
        r.search.current = Some(next);
        let (page, rects) = &r.search.hits[next];
        let rect = union(rects);
        show_view_rect(v, *page, rect);
    });
}

pub fn search_clear(tab_id: i32) {
    update(tab_id, |r, _| {
        r.search.generation += 1;
        r.search.query.clear();
        r.search.hits.clear();
        r.search.snippets.clear();
        r.search.current = None;
        r.search.busy = false;
        r.version += 1;
        if r.panel == 3 {
            r.panel = r.panel_before_search.take().unwrap_or(0);
        }
    });
}

// ---- into the tab model ------------------------------------------------------------------------

/// The palette entry (light or ink) closest to `c`, if one is close.
fn palette_index(c: [f32; 3]) -> i32 {
    let d = |p: &[f64; 3]| (0..3).map(|i| (p[i] - c[i] as f64).powi(2)).sum::<f64>();
    COLOURS.iter().chain(INK.iter()).enumerate().map(|(i, p)| (i % 5, d(p))).filter(|(_, d)| *d < 0.01).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i as i32).unwrap_or(-1)
}

fn colour_of(c: [f64; 3]) -> Color {
    Color::from_rgb_u8((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
}

fn overlay(r: [f64; 4], kind: i32) -> ViewerOverlay {
    ViewerOverlay { x: r[0] as f32, y: r[1] as f32, w: r[2] as f32, h: r[3] as f32, kind }
}

pub fn fill(tab: &mut ToolTab, r: &ReaderUi, v: &ViewerUi) {
    let wanted = v.viewer.wanted(v.top, v.height);
    let cur = v.current();
    let mut marks = Vec::new();
    if let Some((page, range)) = &r.sel
        && let Some(t) = r.texts.get(page)
    {
        for rect in t.line_rects(range.clone()) {
            marks.push(overlay(view_rect_to_content(&v.viewer, *page, rect), 3));
        }
    }
    for (i, (page, rects)) in r.search.hits.iter().enumerate() {
        if wanted.contains(page) {
            let kind = if r.search.current == Some(i) { 5 } else { 4 };
            for rect in rects {
                marks.push(overlay(view_rect_to_content(&v.viewer, *page, *rect), kind));
            }
        }
    }
    // Annotations on their way to the document thread, drawn as they will look.
    let mut prov_ink = String::new();
    let mut provisional = Vec::new();
    for p in &r.provisional {
        if p.kind == PEN {
            for (i, pt) in p.stroke.iter().enumerate() {
                let (x, y) = view_to_content(&v.viewer, p.page, *pt);
                prov_ink.push_str(&format!("{}{x:.1} {y:.1} ", if i == 0 { "M" } else { "L" }));
            }
            continue;
        }
        for rect in &p.rects {
            let c = view_rect_to_content(&v.viewer, p.page, *rect);
            let c = match p.kind {
                UNDERLINE => [c[0], c[1] + c[3] - 1.5, c[2], 1.5],
                STRIKE => [c[0], c[1] + c[3] * 0.55, c[2], 1.5],
                _ => c,
            };
            let (col, alpha) = (p.colour, if p.kind == HIGHLIGHT { 110 } else { 255 });
            provisional.push(ReaderMark {
                x: c[0] as f32,
                y: c[1] as f32,
                w: c[2] as f32,
                h: c[3] as f32,
                colour: Color::from_argb_u8(alpha, (col[0] * 255.0) as u8, (col[1] * 255.0) as u8, (col[2] * 255.0) as u8),
            });
        }
    }
    if r.tool == SELECT {
        // Fillable fields, tinted as PDF Expert and Acrobat highlight them.
        for f in r.snap.fields.iter().filter(|f| !f.read_only && f.kind < 4) {
            for (page, rect, _, _) in f.widgets.iter().filter(|w| wanted.contains(&w.0)) {
                marks.push(overlay(v.viewer.overlay(*page, *rect), 10));
            }
        }
    }
    if r.tool == EDIT_TEXT {
        // The paragraphs that can be edited, outlined on the pages in view (read when first shown).
        for page in wanted.clone() {
            if let Some(blocks) = r.blocks.get(&page) {
                for b in blocks.iter() {
                    let c = v.viewer.overlay(page, b.rect);
                    marks.push(overlay([c[0] - 2.0, c[1] - 1.0, c[2] + 4.0, c[3] + 2.0], 7));
                }
            }
        }
    }
    if let Some(a) = r.selected_annot.and_then(|i| r.annots().get(i)) {
        let c = v.viewer.overlay(a.page, a.rect);
        marks.push(overlay([c[0] - 2.0, c[1] - 2.0, c[2] + 4.0, c[3] + 4.0], 6));
    }
    let ink = match &r.stroke {
        Some((page, pts)) if pts.len() >= 2 => {
            let mut s = String::new();
            for (i, p) in pts.iter().enumerate() {
                let (x, y) = view_to_content(&v.viewer, *page, *p);
                s.push_str(&format!("{}{x:.1} {y:.1} ", if i == 0 { "M" } else { "L" }));
            }
            s
        }
        _ => prov_ink,
    };
    let current_row = reader::current_section(&r.rows, cur);
    let outline = {
        let key = (r.version, current_row);
        let mut c = r.outline_cache.borrow_mut();
        match c.as_ref() {
            Some((k, m)) if *k == key => m.clone(),
            _ => {
                let m = ModelRc::new(VecModel::from(outline_model(r, current_row)));
                *c = Some((key, m.clone()));
                m
            }
        }
    };
    let annots = {
        let key = (r.version, r.selected_annot, r.texts.len());
        let mut c = r.annots_cache.borrow_mut();
        match c.as_ref() {
            Some((k, m)) if *k == key => m.clone(),
            _ => {
                let m = ModelRc::new(VecModel::from(annots_model(r, v)));
                *c = Some((key, m.clone()));
                m
            }
        }
    };
    // One model for the whole search: rows are appended as batches arrive and the current one is
    // updated in place, so a click on a result is never lost to a model replaced under it.
    let results = {
        let mut c = r.results_cache.borrow_mut();
        let (model, shown_current) = c.get_or_insert_with(|| (Rc::new(VecModel::default()), None));
        let have = model.row_count();
        if have > r.search.hits.len() {
            *model = Rc::new(VecModel::default());
            *shown_current = None;
        }
        let have = model.row_count();
        for (i, ((page, _), (b, m, a))) in r.search.hits.iter().zip(&r.search.snippets).enumerate().skip(have).take(2000usize.saturating_sub(have)) {
            model.push(ReaderResultRow {
                before: b.clone().into(),
                hit: m.clone().into(),
                after: a.clone().into(),
                page_label: format!("Page {}", label(r, *page)).into(),
                current: r.search.current == Some(i),
            });
        }
        if *shown_current != r.search.current {
            for (i, on) in [(*shown_current, false), (r.search.current, true)] {
                if let Some(i) = i.filter(|i| *i < model.row_count())
                    && let Some(mut row) = model.row_data(i)
                {
                    row.current = on;
                    model.set_row_data(i, row);
                }
            }
            *shown_current = r.search.current;
        }
        ModelRc::from(model.clone())
    };
    let note = match &r.draft {
        Some(d) => {
            let (x, y) = view_to_content(&v.viewer, d.page, d.at);
            let (text, w, h) = match &d.block {
                // A form field: the editor sits on the field, its size.
                Some((_, t, rect)) if d.field.is_some() => {
                    let c = v.viewer.overlay(d.page, *rect);
                    {
                    let n = d.field.and_then(|(fi, _)| r.snap.fields.get(fi)).map(|f| f.options.len()).unwrap_or(0) as f64;
                    (t.clone(), c[2].max(160.0), if d.kind == 5 { 34.0 + 28.0 * n } else { c[3].max(28.0) + 34.0 })
                }
                }
                Some((_, t, rect)) => {
                    let c = v.viewer.overlay(d.page, *rect);
                    (t.clone(), c[2].max(260.0), c[3].max(60.0) + 70.0)
                }
                None => (String::new(), 260.0, 150.0),
            };
            ReaderNote { show: true, x: x as f32, y: y as f32, w: w as f32, h: h as f32, text: text.into(), kind: d.kind, seq: r.note_seq }
        }
        None => ReaderNote { seq: r.note_seq, ..Default::default() },
    };
    let n = r.search.hits.len();
    let search_count = if r.search.query.is_empty() {
        String::new()
    } else if n == 0 && r.search.busy {
        "…".into()
    } else if n == 0 {
        "None".into()
    } else {
        format!("{}/{}{}", r.search.current.map(|c| c + 1).unwrap_or(0), n, if r.search.busy { "+" } else { "" })
    };
    if !r.page_sel.is_empty() {
        let thumbs: Vec<sheaf_app::ViewerThumb> = slint::Model::iter(&tab.viewer_thumbs)
            .map(|mut t| {
                t.selected = r.page_sel.contains(&(t.page as usize));
                t
            })
            .collect();
        tab.viewer_thumbs = ModelRc::new(VecModel::from(thumbs));
    }
    tab.name = r.name.clone().into();
    tab.is_reader = true;
    tab.reader = ReaderState {
        title: r.name.clone().into(),
        dirty: r.dirty(),
        read_only: r.snap.read_only.clone().into(),
        page_count: v.viewer.page_count() as i32,
        page_text: (cur + 1).to_string().into(),
        page_label: label(r, cur).into(),
        panel_open: r.panel_open,
        panel: r.panel,
        outline_empty: r.rows.is_empty(),
        outline,
        annots,
        tool: r.tool,
        colour: if uses_light(r.tool) { r.light as i32 } else { r.ink_colour as i32 },
        theme: r.theme.index(),
        backdrop: reader::backdrop(r.theme).map(|c| Color::from_rgb_u8(c[0], c[1], c[2])).unwrap_or_default(),
        marks: ModelRc::new(VecModel::from(marks)),
        provisional: ModelRc::new(VecModel::from(provisional)),
        has_selection: r.sel.is_some(),
        selected_text: r.sel.as_ref().and_then(|(p, range)| r.texts.get(p).map(|t| reader::selected_text(t, range.clone()))).unwrap_or_default().into(),
        selected_annot: r.selected_annot.is_some(),
        annot_colour: r.selected_annot.and_then(|i| r.annots().get(i)).and_then(|a| a.color).map(palette_index).unwrap_or(-1),
        ink: ink.into(),
        ink_colour: colour_of(INK[r.ink_colour]),
        note,
        search_count: search_count.into(),
        search_busy: r.search.busy,
        can_undo: r.snap.can_undo && r.in_flight == 0,
        can_redo: r.snap.can_redo && r.in_flight == 0,
        zoom_mode: r.zoom_mode,
        pen_width: r.pen_width,
        pages_selected: r.page_sel.len() as i32,
        sign_open: r.sign_open,
        has_signature: r.tool == SIGN && saved_signature().is_some(),
        has_form: r.snap.fields.iter().any(|f| f.kind < 4),
        choices: ModelRc::new(VecModel::from(
            r.draft
                .as_ref()
                .filter(|d| d.kind == 5)
                .and_then(|d| d.field)
                .and_then(|(fi, _)| r.snap.fields.get(fi))
                .map(|f| f.options.iter().map(|(e, t)| slint::SharedString::from(if t.is_empty() { e } else { t })).collect::<Vec<_>>())
                .unwrap_or_default(),
        )),
        results,
        case_sensitive: r.search.case_sensitive,
        whole_words: r.search.whole_words,
    };
    if reader::backdrop(r.theme).is_none() {
        tab.reader.backdrop = Color::from_argb_u8(0, 0, 0, 0);
    }
}

fn outline_model(r: &ReaderUi, current_row: Option<usize>) -> Vec<ReaderOutlineRow> {
    let outline: Vec<ReaderOutlineRow> = r
        .rows
        .iter()
        .enumerate()
        .map(|(i, x)| ReaderOutlineRow {
            title: x.title.clone().into(),
            depth: x.depth as i32,
            page_label: x.page.map(|p| label(r, p)).unwrap_or_default().into(),
            has_children: x.has_children,
            expanded: x.expanded,
            current: current_row == Some(i),
        })
        .collect();
    outline
}

fn annots_model(r: &ReaderUi, v: &ViewerUi) -> Vec<ReaderAnnotRow> {
    let annots: Vec<ReaderAnnotRow> = r
        .annots()
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let text = if a.quads.is_empty() || !a.contents.is_empty() {
                a.contents.clone()
            } else {
                r.texts
                    .get(&a.page)
                    .map(|t| {
                        let size = v.viewer.boxes[a.page].view_size();
                        let rects: Vec<[f32; 4]> = a
                            .quads
                            .iter()
                            .map(|q| {
                                let [x, y, w, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[a.page], size, sheaf_core::redact::bounds(std::slice::from_ref(q)));
                                [x as f32, y as f32, (x + w) as f32, (y + h) as f32]
                            })
                            .collect();
                        reader::text_under(t, &rects)
                    })
                    .unwrap_or_default()
            };
            ReaderAnnotRow {
                kind: reader::kind_label(&a.subtype).into(),
                text: text.chars().take(300).collect::<String>().into(),
                page_label: format!("Page {}", label(r, a.page)).into(),
                colour: a.color.map(|c| colour_of([c[0] as f64, c[1] as f64, c[2] as f64])).unwrap_or(Color::from_rgb_u8(255, 214, 10)),
                selected: r.selected_annot == Some(i),
            }
        })
        .collect();
    annots
}

/// The page's label ("iv") or number, 1-based.
fn label(r: &ReaderUi, p: usize) -> String {
    r.snap.labels.get(p).cloned().filter(|l| !l.is_empty()).unwrap_or_else(|| (p + 1).to_string())
}
