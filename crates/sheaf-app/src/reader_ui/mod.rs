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
use sheaf_app::{AppWindow, ReaderFieldProps, ReaderFormField, ReaderGrid, ReaderMark, ReaderProp, ReaderResultRow, ReaderAnnotRow, ReaderReply, ReaderNote, ReaderOutlineRow, ReaderState, ToolTab, ViewerOverlay};
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
pub const TEXT_SIZES: [f64; 15] = [8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 48.0, 72.0];
/// Added text's alignment by the format row's index (left, centre, right).
const TEXT_ALIGNS: [sheaf_core::pdfcraft::engine::TextAlign; 3] =
    [sheaf_core::pdfcraft::engine::TextAlign::Left, sheaf_core::pdfcraft::engine::TextAlign::Center, sheaf_core::pdfcraft::engine::TextAlign::Right];

/// Fill & Sign: click to place the signature (made once, kept for every document).
pub const SIGN: i32 = 9;
/// Shapes: drag a rectangle, oval, line or arrow (`ReaderUi::shape_kind`).
pub const SHAPE: i32 = 10;
/// Eraser: drag across drawings to rub them out where the path goes.
pub const ERASER: i32 = 11;
/// Stamp: click to place the stamp chosen in the options bar (`ReaderUi::stamp`).
pub const STAMP: i32 = 12;
/// Edit ▸ Link: drag a box to link it to a page or a web address; click a link to change or remove it.
pub const LINK: i32 = 13;
/// Prepare form: drag or click to add a field of the kind chosen; a field clicked is selected to
/// move, delete or set its properties (Acrobat's Prepare a Form).
pub const FORM: i32 = 14;
/// Fill & Sign marks offered first in the stamp list (Acrobat's ✓ ✗ • —), with their label.
pub const MARKS: [(sheaf_core::pdfcraft::engine::FillMark, &str); 4] = [
    (sheaf_core::pdfcraft::engine::FillMark::Check, "✓ Checkmark"),
    (sheaf_core::pdfcraft::engine::FillMark::Cross, "✗ Cross"),
    (sheaf_core::pdfcraft::engine::FillMark::Dot, "• Dot"),
    (sheaf_core::pdfcraft::engine::FillMark::Line, "— Line"),
];

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

/// An object of a page as Edit a PDF selects it: its inventory target and box (user space).
type PageObject = (sheaf_core::pdfcraft::engine::ObjectTarget, [f64; 4]);
/// A drag on a page: the page, from and to (view points).
type PageDrag = (usize, (f64, f64), (f64, f64));
/// A corner pulled: which (0 top-left … 3 bottom-right), from and to (view points).
type CornerPull = (u8, (f64, f64), (f64, f64));

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
    /// A paragraph: index, text to edit (wraps as spaces), text as drawn (a move keeps its rows),
    /// box (user space), its top-left (view).
    Block(usize, String, String, [f64; 4], (f64, f64)),
    /// An image: index among the page's images, visible box and full box (user space).
    Image(usize, [f64; 4], [f64; 4]),
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

/// A field dragged with the Form tool: its page, from and to (view points).
type FormDrag = (usize, (f64, f64), (f64, f64));

pub struct ReaderUi {
    ops: std::sync::mpsc::Sender<Op>,
    snap: Snap,
    /// Operations sent and not answered yet.
    in_flight: usize,
    /// Of those, edits, undos and redos: what can change the indices of comments and objects.
    edits_in_flight: usize,
    provisional: Vec<Provisional>,
    pub path: PathBuf,
    pub name: String,
    texts: HashMap<usize, Arc<PageText>>,
    /// The Notes panel's highlighted words being read on their thread (`model::quote_texts`).
    quotes_reading: bool,
    /// The text layer's renderer, kept for the working file it was made from.
    text_renderer: Option<(Arc<Vec<u8>>, render::PageRenderer)>,
    blocks: HashMap<usize, Arc<Vec<Block>>>,
    /// The boxes of each page's images (user space), read with its paragraphs.
    images: HashMap<usize, Arc<Vec<ImageBoxes>>>,
    /// Edit tool: each page's objects as Edit a PDF selects them together (target, box; user space).
    objects: HashMap<usize, Arc<Vec<PageObject>>>,
    /// Edit tool: the objects picked together (⇧-click, a marquee, a pressed image) on a page,
    /// moved by a drag or the arrow keys; `repick` their boxes where a move put them, to find
    /// them again in the page read again.
    picked: Option<(usize, Vec<PageObject>)>,
    repick: Option<(usize, Vec<[f64; 4]>)>,
    /// A drag of the picked objects (view points, from and to), and a marquee being drawn.
    picked_drag: Option<((f64, f64), (f64, f64))>,
    marquee: Option<PageDrag>,
    /// The one picked paragraph's side being pulled (corner 0–3, from, to; view points): it
    /// rewraps to the new width.
    para_resize: Option<CornerPull>,
    /// Edit tool: the drawing under the pointer (outlined), its page and box.
    hover: Option<(usize, [f64; 4])>,
    /// The offset (user space) of the last move preview sent: picked outlines follow what the
    /// page shows, not the pointer ahead of it.
    live_offset: Option<[f64; 2]>,
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
    /// The paragraph shown on its page as typed (Edit text): its page and index, while previewed.
    preview: Option<(usize, usize)>,
    /// The text typed last and the pause after which it is previewed.
    preview_text: String,
    preview_timer: Option<slint::Timer>,
    /// Add image: where the click was, while the file is chosen.
    image_at: Option<(usize, (f64, f64))>,
    /// Edit tool: the page image selected (page, index among the page's images); its corners
    /// resize it, the options bar turns, flips, replaces or deletes it.
    selected_image: Option<(usize, usize)>,
    /// A corner of the selected image being pulled: which (0 top-left, 1 top-right, 2 bottom-left,
    /// 3 bottom-right), where the pull started and where it is (view points).
    image_resize: Option<ImagePull>,
    /// An image just added to a page and how many the page had: it is selected once listed.
    select_added: Option<(usize, usize)>,
    /// A pull or drag shown live on its page (Edit tool): the page, whether a preview is on its
    /// way, and the latest one waiting for it.
    live: Option<usize>,
    live_in_flight: bool,
    live_pending: Option<(usize, Edit)>,
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
    /// Shapes' fill: a highlight colour (index, the sixth too), or None a tint of the stroke.
    fill_colour: Option<usize>,
    shape_end: Option<(f64, f64)>,
    /// ⇧ held when the shape was started: squares, circles, 45° lines.
    shape_shift: bool,
    /// Polygon, Cloud and Polygonal line: the vertices clicked so far (view points of the page).
    poly: Option<(usize, Vec<(f64, f64)>)>,
    /// A picture chosen as the stamp (Acrobat's custom stamp): its file name, bytes and
    /// height / width; each click places it.
    custom_stamp: Option<(String, Arc<Vec<u8>>, f64)>,
    /// Notes panel: filter (`reader::notes_filter`) and order (0 page, 1 newest, 2 author, 3 type).
    notes_filter: i32,
    notes_sort: i32,
    /// Outline panel: the row being renamed.
    outline_edit: Option<usize>,
    /// View ▸ Highlight links: every link tinted, as fields are.
    show_links: bool,
    /// Fillable fields tinted in Select mode (More ▸ Highlight form fields).
    show_fields: bool,
    /// The Document Properties sheet is open.
    props_open: bool,
    /// Document Properties: what is read from the file when the sheet opens.
    facts: Option<sheaf_core::facts::Facts>,
    /// Hide all comments: not drawn, not selectable.
    comments_hidden: bool,
    /// Stamp tool: one of `MARKS`, then `StampKind::ALL` (index past the marks).
    stamp: usize,
    /// Shapes and pen: index in `reader::OPACITIES`.
    opacity: usize,
    /// Added text and text boxes: size in points (the presets of TEXT_SIZES, or typed), 0 Helvetica
    /// / 1 Times / 2 Courier, bold, italic, alignment (0 left, 1 centre, 2 right).
    text_pt: f64,
    text_family: i32,
    /// What is typed in a text box or added text edited in place (committed by a click elsewhere).
    inplace_text: String,
    text_bold: bool,
    text_italic: bool,
    text_align: i32,
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
    /// Pages to delete once their extract is written (Extract's "Delete pages after extracting").
    delete_after_extract: Vec<usize>,
    /// The page grid is shown: the pages' layout to go back to when it closes.
    grid_prev: Option<u8>,
    /// ⌘C on a comment: the page it is on, until its copy comes back.
    copy_from: Option<usize>,
    /// The tool it goes to then (Redact from the edit group), else the Tools page.
    tool_after_save: Option<sheaf_core::tools::ToolId>,
    /// The Link editor's complaint about what was typed (not a page or an address).
    link_error: String,
    /// New links' look (the Link tool's options): a visible border, dashed; its colour and width
    /// are the ink colour and pen width.
    link_visible: bool,
    link_dashed: bool,
    /// A link made from selected text: one box per line (user space); the draft holds their union.
    link_rects: Vec<[f64; 4]>,
    /// Replace Text from selected words: their quads (PDF space), struck out once the new words are typed.
    replace_quads: Vec<[f64; 8]>,
    /// Prepare form: the kind of field to add (`prepare::FIELD_KINDS`), the radio group being
    /// placed, the selected field (its widget's page and box: found again after each change) and
    /// a selected field being dragged (page, from, to; view points).
    field_kind: i32,
    radio_group: Option<String>,
    /// Shapes ▸ Arrow: its head (`reader::ARROW_HEADS`) and whether both ends carry one.
    arrow_head: usize,
    arrow_both: bool,
    /// The Text box tool makes callouts (Acrobat's Callout).
    callout: bool,
    /// The Measuring tools' scale (`reader::MEASURE_SCALES`).
    measure_scale: usize,
    /// Calibrate: the next distance measured asks for its real length; then the scale it gives
    /// (real units per user-space unit, the unit) replaces the preset one.
    calibrating: bool,
    measure_custom: Option<(f64, &'static str)>,
    calibration_len: f64,
    /// Edit text: what of the format row was changed while a paragraph is open (1 colour, 2 size,
    /// 4 font, 8 alignment): those apply to it, the rest of its style stays its own.
    para_format: u8,
    form_sel: Option<(usize, [f64; 4])>,
    /// Other fields selected with it (⇧-click), on its page: their rects.
    form_also: Vec<[f64; 4]>,
    /// Prepare form's Grid: shown, and fields snap to it.
    form_grid: bool,
    /// More ▸ Page background… is open.
    background_open: bool,
    /// More ▸ Versions… is open.
    versions_open: bool,
    /// Zoom to area (Z): the next box dragged fills the view.
    zoom_area: bool,
    /// Copy an area (More ▸ Copy an area as a picture): the next box dragged is copied.
    snap_area: bool,
    /// More ▸ Scroll automatically is on (the view moves itself, reader.slint).
    auto_scroll: bool,
    /// More ▸ One page at a time: the page fits the view, the wheel and arrows turn pages.
    page_at_a_time: bool,
    /// The Signature tool places the initials (its options bar's Initials).
    initials: bool,
    /// The Signature tool places on every page at once (its options bar's Every page).
    sign_every_page: bool,
    form_drag: Option<FormDrag>,
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
    /// Places (page, and point in it) and their zoom left by a link or the outline (⌘[ goes back),
    /// and those gone back from.
    back: Vec<((usize, f64, f64), u32)>,
    forward: Vec<((usize, f64, f64), u32)>,
    /// Bumped when the outline rows or the annotation list change: the panel models are rebuilt
    /// only then, not on every scroll.
    version: u64,
    outline_cache: Cache<(OutlineKey, ModelRc<ReaderOutlineRow>)>,
    annots_cache: Cache<(AnnotsKey, ModelRc<ReaderAnnotRow>)>,
}

/// An annotation pulled by the pointer: its index, where the pull started, where it is (view).
type AnnotPull = (usize, (f64, f64), (f64, f64));
/// An image on a page: its visible box and its full box (user space).
type ImageBoxes = ([f64; 4], [f64; 4]);
/// A corner of an image pulled: which, from where, to where (view points).
type ImagePull = (u8, (f64, f64), (f64, f64));
/// A panel model kept until what it shows changes.
type Cache<T> = std::cell::RefCell<Option<T>>;
/// What the outline rows depend on: the version, the current section, the row being renamed.
type OutlineKey = (u64, Option<usize>, Option<usize>);
/// What the annotation rows depend on: the version, the selected one, how many pages have text read,
/// the filter and the order.
type AnnotsKey = (u64, Option<usize>, usize, i32, i32);

impl ReaderUi {
    fn new(ops: std::sync::mpsc::Sender<Op>, snap: Snap, path: PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Document".into());
        let has_outline = !snap.outline.is_empty();
        let mut r = ReaderUi {
            ops,
            snap,
            in_flight: 0,
            edits_in_flight: 0,
            provisional: Vec::new(),
            path,
            name,
            texts: HashMap::new(),
            quotes_reading: false,
            text_renderer: None,
            blocks: HashMap::new(),
            images: HashMap::new(),
            objects: HashMap::new(),
            picked: None,
            repick: None,
            picked_drag: None,
            marquee: None,
            para_resize: None,
            hover: None,
            live_offset: None,
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
            preview: None,
            preview_text: String::new(),
            preview_timer: None,
            image_at: None,
            selected_image: None,
            image_resize: None,
            select_added: None,
            live: None,
            live_in_flight: false,
            live_pending: None,
            sign_at: None,
            sign_open: false,
            print_open: false,
            printers: Vec::new(),
            printer: 0,
            note_seq: 0,
            pen_width: 1,
            shape_kind: 0,
            shape_fill: false,
            fill_colour: None,
            shape_end: None,
            shape_shift: false,
            poly: None,
            custom_stamp: None,
            opacity: 0,
            comments_hidden: false,
            props_open: false,
            facts: None,
            show_links: false,
            show_fields: !crate::home::prefs().plain_fields,
            outline_edit: None,
            notes_filter: 0,
            notes_sort: 0,
            stamp: MARKS.len() + StampKind::ALL.iter().position(|k| *k == StampKind::Approved).unwrap_or(0),
            text_pt: 12.0,
            text_family: 0,
            inplace_text: String::new(),
            text_bold: false,
            text_italic: false,
            text_align: 0,
            annot_candidate: None,
            annot_drag: None,
            annot_resize: None,
            pending_field: None,
            page_sel: Default::default(),
            page_anchor: None,
            page_drag: None,
            structure_pending: false,
            tools_after_save: false,
            delete_after_extract: Vec::new(),
            grid_prev: None,
            copy_from: None,
            tool_after_save: None,
            link_error: String::new(),
            link_visible: false,
            link_dashed: false,
            link_rects: Vec::new(),
            replace_quads: Vec::new(),
            field_kind: 0,
            radio_group: None,
            arrow_head: 0,
            arrow_both: false,
            callout: false,
            measure_scale: 0,
            calibrating: false,
            measure_custom: None,
            calibration_len: 0.0,
            para_format: 0,
            form_sel: None,
            form_also: Vec::new(),
            form_grid: false,
            background_open: false,
            versions_open: false,
            zoom_area: false,
            snap_area: false,
            auto_scroll: false,
            page_at_a_time: false,
            initials: false,
            sign_every_page: false,
            form_drag: None,
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

    pub(crate) fn dirty(&self) -> bool {
        self.snap.dirty || self.in_flight > 0
    }

    fn send(&mut self, op: Op) {
        // A preview is not counted: it never names something another edit is waiting on.
        let edit = matches!(op, Op::Apply { .. } | Op::LinkUrls { .. } | Op::Undo { .. } | Op::Redo { .. } | Op::ReplaceAll { .. } | Op::Revert | Op::DetectFields);
        if self.ops.send(op).is_ok() {
            self.in_flight += 1;
            self.edits_in_flight += usize::from(edit);
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

thread_local! {
    /// Documents to open on the Notes panel (see [`open_on_notes`]).
    static NOTES_FIRST: std::cell::RefCell<HashSet<PathBuf>> = Default::default();
}

/// Opens `path` with its comments listed: Compare's marked copy, each difference a comment to
/// step through (Acrobat shows its comparison in the app, its changes in a list).
pub fn open_on_notes(ui: &AppWindow, path: PathBuf) {
    NOTES_FIRST.with(|n| n.borrow_mut().insert(std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone())));
    open(ui, path, None);
}

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
        crate::state::push_tab_row(&t);
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
                restore_view(&mut v, &path, t.viewport.0);
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
        let _ = weak.upgrade_in_event_loop(move |ui| {
            on_reply(&ui, id, reply);
            update(id, |r, _| model::quote_texts(r, id));
        });
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
                // A damaged PDF goes to Repair PDF, ready to rebuild it (Acrobat repairs as it opens;
                // Sol 25/10: the tab just closed).
                sheaf_core::OpdfError::Pdfium(_) | sheaf_core::OpdfError::Lopdf(_) | sheaf_core::OpdfError::Engine(_) if sheaf_app::files::looks_like_pdf(&input.path) => {
                    crate::flow::toast(ui, &format!("{} Repair PDF can try to rebuild it.", sheaf_app::files::open_error(&file_name(&input.path), &e)));
                    crate::flow::open_tool(ui, sheaf_core::tools::ToolId::Repair, vec![input.path]);
                }
                e => crate::flow::toast(ui, &sheaf_app::files::open_error(&file_name(&input.path), &e)),
            }
        }
        Reply::Changed { snap, pages, error, applied, note, preview } => {
            update(id, |r, v| {
                r.in_flight = r.in_flight.saturating_sub(1);
                if !preview {
                    r.edits_in_flight = r.edits_in_flight.saturating_sub(1);
                } else if std::mem::take(&mut r.live_in_flight)
                    && r.live.is_some()
                    && let Some((page, e)) = r.live_pending.take()
                {
                    r.live_in_flight = true;
                    r.live_offset = match &e {
                        Edit::MoveObjects { offset, .. } => Some(*offset),
                        _ => None,
                    };
                    r.send(Op::Preview { edit: Some(Box::new(e)), pages: vec![page] });
                }
                // A refused edit added no undo step: its pages leave the stack too.
                if applied && error.is_some() {
                    r.undo_pages.pop();
                    // An image that was not added is not selected later either.
                    r.select_added = None;
                }
                let structural = r.snap.boxes.len() != snap.boxes.len() || r.structure_pending;
                r.snap = *snap;
                r.structure_pending = false;
                if structural {
                    forget_page_state(r);
                }
                // A preview keeps the paragraph list of the page as it is (the previewed one could
                // number its paragraphs otherwise, and every edit names one by its number there).
                let kept: Vec<_> = if preview {
                    pages.iter().flatten().filter_map(|p| Some((*p, r.blocks.get(p)?.clone(), r.images.get(p).cloned()))).collect()
                } else {
                    Vec::new()
                };
                after_change(r, v, pages.as_deref().filter(|p| !p.is_empty()));
                for (p, b, i) in kept {
                    r.blocks.insert(p, b);
                    if let Some(i) = i {
                        r.images.insert(p, i);
                    }
                    r.blocks_asked.insert(p);
                }
                // The Edit tool's outlines (paragraphs, images, the selected one) are read again at
                // once for the pages in view (not a previewed page: kept above); an image just
                // added is selected then.
                tools::load_blocks(r, v);
                if let Some((p, _)) = r.select_added {
                    r.blocks(p);
                }
                if r.in_flight == 0 {
                    r.provisional.clear();
                }
            });
            if let Some(e) = error {
                crate::flow::toast(ui, &e);
            } else if let Some(note) = note {
                crate::flow::toast(ui, &note);
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
                    // Required fields left empty are said (Acrobat warns before a form goes out).
                    let empty = update(id, |r, _| prepare::required_empty(&r.snap.fields)).unwrap_or_default();
                    crate::flow::toast(ui, &format!("Saved {n}{}", prepare::empty_note(&empty)));
                    if update(id, |r, _| std::mem::take(&mut r.tools_after_save)).unwrap_or(false) {
                        hand_to_tools(ui, id);
                    }
                    if with(|a| a.closing_after_save.remove(&id)) {
                        crate::flow::remove_tab(ui, id);
                        if with(|a| a.quitting) {
                            crate::flow::quit_or_ask(ui);
                        } else {
                            crate::flow::next_close(ui);
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
        Reply::ImageCopied(made) => {
            let copied = made.and_then(|(width, height, rgba)| {
                let image = arboard::ImageData { width: width as usize, height: height as usize, bytes: rgba.into() };
                arboard::Clipboard::new().and_then(|mut c| c.set_image(image)).map_err(|e| e.to_string())
            });
            match copied {
                Ok(()) => crate::flow::toast(ui, "Picture copied: ⌘V pastes it on a page, or in another app"),
                Err(e) => crate::flow::toast(ui, &format!("The picture was not copied: {e}")),
            }
        }
        Reply::PagesCopied(made) => match made {
            Ok((bytes, n)) => {
                let Some(name) = update(id, |r, _| r.name.clone()) else { return };
                pages::pages_copied(ui, (name, bytes, (0..n).collect()));
            }
            Err(e) => crate::flow::toast(ui, &format!("Pages not copied: {e}")),
        },
        Reply::Copied(element, picture) => {
            let Some((from, page)) = update(id, |r, _| Some((r.path.clone(), r.copy_from.take()?))).flatten() else { return };
            match element {
                Some(element) => {
                    with(|a| a.comment_clip = Some(crate::state::CommentClip { element, picture, from, page }));
                    // The system clipboard says a comment is copied: anything copied after it wins.
                    let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(COMMENT_ON_CLIPBOARD));
                    crate::flow::toast(ui, "Comment copied: ⌘V pastes it on the page shown");
                }
                None => crate::flow::toast(ui, "This comment could not be copied"),
            }
        }
        Reply::Exported { to, error } => {
            update(id, |r, _| r.in_flight = r.in_flight.saturating_sub(1));
            match error {
                None => crate::flow::toast(ui, &format!("Comments exported to {}", file_name(&to))),
                Some(e) => crate::flow::toast(ui, &format!("Not exported: {e}")),
            }
        }
        Reply::VersionSaved(r) => {
            update(id, |r, _| r.in_flight = r.in_flight.saturating_sub(1));
            match r {
                Ok(to) => {
                    crate::flow::toast(ui, &format!("{} opened: the file as it was saved then", file_name(&to)));
                    open(ui, to, None);
                }
                Err(e) => crate::flow::toast(ui, &format!("Version not opened: {e}")),
            }
        }
        Reply::Extracted { to, error } => {
            update(id, |r, _| r.in_flight = r.in_flight.saturating_sub(1));
            let delete = update(id, |r, _| std::mem::take(&mut r.delete_after_extract)).unwrap_or_default();
            match error {
                None if to.is_dir() => crate::flow::toast(ui, &format!("Extracted one file per page to {}", file_name(&to))),
                None => {
                    crate::flow::toast(ui, &format!("Extracted to {}", file_name(&to)));
                    open(ui, to, None);
                }
                Some(e) => return crate::flow::toast(ui, &format!("Not extracted: {e}")),
            }
            if !delete.is_empty() {
                pages::apply_structure(ui, id, Edit::DeletePages { pages: delete });
            }
        }
        Reply::Blocks(page, b, images, objects) => {
            update(id, |r, _| {
                r.blocks.insert(page, Arc::new(b));
                // Objects just moved: picked again where they went.
                if let Some((p, moved)) = r.repick.take_if(|(p, _)| *p == page) {
                    let rects: Vec<[f64; 4]> = objects.iter().map(|o| o.1).collect();
                    let again: Vec<_> = reader::picked_again(&moved, &rects).into_iter().map(|i| objects[i]).collect();
                    r.picked = (!again.is_empty()).then_some((p, again));
                }
                r.objects.insert(page, Arc::new(objects));
                if let Some((p, before)) = r.select_added
                    && p == page
                    && images.len() > before
                {
                    r.selected_image = Some((page, images.len() - 1));
                    r.select_added = None;
                }
                r.images.insert(page, Arc::new(images));
            });
        }
    }
}

/// The document is shown as it was left, as PDF Expert reopens it: pages side by side, the zoom
/// when one was chosen (Fit width and Fit page refit to the window), the page.
/// `width`: the view's width when known (0 before the widget reports it).
fn restore_view(v: &mut ViewerUi, path: &std::path::Path, width: f32) {
    if let Some((fit, zoom, spread)) = crate::home::last_view(path) {
        v.viewer.set_spread(spread & 3);
        v.viewer.set_rtl(spread & 0x10 != 0);
        match fit {
            // Before 25/10: a share of the window's width.
            2 => {
                v.viewer.set_zoom(zoom);
            }
            // Its actual size, whatever the window: set as the width comes, as Actual size is.
            3 => {
                let pct = zoom as f64 / 10.0;
                if width > 0.0 {
                    let z = v.viewer.zoom_for_actual(0, pct);
                    v.viewer.set_zoom(z);
                }
                v.pending_actual = Some(pct);
            }
            _ => {}
        }
    } else {
        // A document never opened: Preferences ▸ Documents, its pages side by side or not and its
        // zoom. Actual size: the zoom is a share of the view's width, which may still be a first
        // guess: it is set again as the width comes, until a zoom of the user's own.
        let p = crate::home::prefs();
        v.viewer.set_spread(p.default_spread.min(2));
        if p.default_zoom == 2 {
            if width > 0.0 {
                let z = v.viewer.zoom_for_actual(0, 100.0);
                v.viewer.set_zoom(z);
            }
            v.pending_actual = Some(100.0);
        } else if p.default_zoom == 1 {
            // Fit page: the height decides, set as the view reports it (kept on later resizes by keep_fit).
            v.pending_fit = true;
        }
    }
    if let Some(p) = crate::home::last_page(path).filter(|p| *p > 0 && !crate::home::prefs().open_first_page) {
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
                restore_view(&mut v, &path, t.viewport.0);
                v
            }
        };
        let mut r = ReaderUi::new(ops, snap, path.clone());
        // A document never opened here opens as it asks (Acrobat honours its Initial View): its
        // layout, zoom, page and side panel, over Preferences ▸ Documents.
        if crate::home::last_view(&path).is_none() {
            properties::open_as_asked(&mut r, &mut v);
        }
        // Compare's marked copy opens on its list of differences.
        if NOTES_FIRST.with(|n| n.borrow_mut().remove(&std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone()))) && !r.snap.annots.is_empty() {
            (r.panel, r.panel_open) = (2, true);
        }
        r.zoom_mode = crate::home::last_view(&path).map_or(crate::home::prefs().default_zoom.clamp(0, 2), |(fit, ..)| fit.clamp(0, 2));
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

/// The reader's side panel width until one is dragged, PDF Expert's.
pub const PANEL_WIDTH: f32 = 248.0;

/// Thumbnails in the reader's side panel: its width less the margins (150 px at the default
/// width, PDF Expert's size; the tools' strip uses 84 px).
pub fn reader_thumb(panel: f32) -> f64 {
    (panel as f64 - 98.0).max(100.0)
}

fn reader_viewer(boxes: Vec<viewer::PageBox>) -> Viewer {
    let mut v = Viewer::new(boxes);
    v.thumb_width = reader_thumb(crate::home::panel_width());
    v
}

/// The sixth colour of the row (index [`CUSTOM`]): the one picked, else a grey.
fn custom_colour(light: bool) -> [f64; 3] {
    let p = crate::home::prefs();
    (if light { p.custom_light } else { p.custom_ink }).and_then(|h| sheaf_core::numbers::parse_hex_colour(&h)).unwrap_or([0.5, 0.5, 0.5])
}

/// Colour row index `k` for strokes and text: the five presets, then the custom one.
pub(super) fn ink_at(k: usize) -> [f64; 3] {
    if k >= INK.len() { custom_colour(false) } else { INK[k] }
}

/// Colour row index `k` for highlights and notes.
pub(super) fn light_at(k: usize) -> [f64; 3] {
    if k >= COLOURS.len() { custom_colour(true) } else { COLOURS[k] }
}

/// The colour row's sixth place: the colour picked from the palette or typed.
pub const CUSTOM: usize = 5;

fn file_name(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Runs `f` on the reader and viewer of tab `tab_id`, then pushes the tab to the UI.
fn update<R>(tab_id: i32, f: impl FnOnce(&mut ReaderUi, &mut ViewerUi) -> R) -> Option<R> {
    with(|a| {
        let t = a.tab(tab_id)?;
        let (Some(r), Some(v)) = (t.reader.as_deref_mut(), t.viewer.as_deref_mut()) else { return None };
        let out = f(r, v);
        // A paragraph previewed whose editor has gone (closed, another paragraph, another tool):
        // the page shows it as it was.
        if let Some((page, block)) = r.preview
            && !r.draft.as_ref().is_some_and(|d| d.kind == 2 && d.page == page && d.block.as_ref().is_some_and(|b| b.0 == block))
        {
            r.preview = None;
            r.preview_timer = None;
            r.send(reader_doc::Op::Preview { edit: None, pages: vec![page] });
        }
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
    if r.comments_hidden {
        return None;
    }
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
mod properties;
mod prepare;
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
pub use properties::*;
pub use prepare::*;
pub use nav::*;
pub use edits::*;
pub use saving::*;
pub use pages::*;
pub use tools::*;
pub use search::*;
pub use model::*;
