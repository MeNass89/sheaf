//! UI-thread state of the tool tabs and how it is pushed into the Slint `tabs` model.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;

use sheaf_app::files::{self, Kind, Thumb};
use sheaf_app::icons::{icon_image, tile_colour};
use sheaf_app::jobs::JobOutcome;
use sheaf_app::{AppWindow, ContinueTool, FileItem, PageItem, ResultFile, ToolTab};
use sheaf_core::pages::Input;
use sheaf_core::tools::{self, ToolId};
use slint::{Image, Model as _, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};


pub struct FileEntry {
    pub uid: i32,
    pub input: Input,
    pub kind: Kind,
    pub size: u64,
    pub pages: Option<u32>,
    pub thumb: Option<Image>,
    /// The preview could not be rendered (the file is still processed).
    pub thumb_failed: bool,
    /// Repair: pdfium cannot open the file; the job will try lopdf's recovery.
    pub unreadable: bool,
    pub locked: bool,
    /// Compare: 0 Original, 1 Changed (0 for every other tool).
    pub slot: usize,
    /// Its first pages carry next to no text: a scan (PDF to Office suggests OCR).
    pub scanned: bool,
    /// Merge: the pages taken from it ("1-3, 5"; empty = all).
    pub pages_spec: String,
}

pub enum Phase {
    Edit,
    Running,
    Done(Box<Done>),
}

/// A finished job, as the result page shows it.
pub struct Done {
    pub outcome: JobOutcome,
    pub page_counts: Vec<Option<u32>>,
    /// Size of each input, in job order (Compress per-file percentages).
    pub input_sizes: Vec<u64>,
}

pub struct TabState {
    pub id: i32,
    pub tool: ToolId,
    pub icon: Image,
    pub files: Vec<FileEntry>,
    /// Organize: output order, 1-based source pages, 0 = blank page.
    pub order: Vec<u32>,
    pub page_thumbs: HashMap<u32, Image>,
    pub phase: Phase,
    pub progress: f32,
    /// When the running job started (for the time left).
    pub started: Option<std::time::Instant>,
    /// Cancel pressed; the job has not returned yet.
    pub cancelling: bool,
    /// Organize: page previews could not be rendered.
    pub pages_failed: bool,
    pub office_missing: bool,
    /// Crop: page 1 of the file; Add watermark: the first page it marks, marked (wm_preview);
    /// with its displayed size in points.
    pub preview: Option<(Image, f32, f32)>,
    /// Add watermark: why the preview shows no mark ("" when it does, or nothing is asked yet).
    pub preview_note: String,
    /// Edit: the text and images placed.
    pub edit: crate::edit_ui::EditState,
    /// Sign: the signature being made.
    pub sign: crate::sign_ui::SignState,
    /// Redact, Sign and Edit: the page viewer of the file, with its marks or the signature placed.
    pub viewer: Option<Box<crate::viewer_ui::ViewerUi>>,
    /// Redact: why the viewer could not open the file.
    pub viewer_error: Option<String>,
    /// The viewer widget's last viewport (width, top, height) and strip (top, height), kept
    /// for a document that finishes loading after the widget reported them.
    pub viewport: (f32, f32, f32),
    pub strip: (f32, f32),
    /// A document tab (the reader) instead of a tool.
    pub reader: Option<Box<crate::reader_ui::ReaderUi>>,
    /// A document tab whose file is still being read: its name.
    pub reader_loading: Option<String>,
    /// A document tab's file, canonical: one tab per file.
    pub reader_file: Option<PathBuf>,
    /// While the document opens: the channel to its thread and the file.
    pub reader_ops: Option<(std::sync::mpsc::Sender<crate::reader_doc::Op>, sheaf_core::pages::Input)>,
}

impl TabState {
    pub fn new(id: i32, tool: ToolId) -> Self {
        let info = tools::find(tool);
        let office = matches!(tool, ToolId::WordToPdf | ToolId::PowerPointToPdf | ToolId::ExcelToPdf | ToolId::HtmlToPdf);
        Self {
            id,
            tool,
            icon: icon_image(info.icon),
            files: Vec::new(),
            order: Vec::new(),
            page_thumbs: HashMap::new(),
            phase: Phase::Edit,
            progress: 0.0,
            started: None,
            cancelling: false,
            pages_failed: false,
            preview: None,
            preview_note: String::new(),
            edit: crate::edit_ui::EditState::new(),
            sign: crate::sign_ui::SignState::new(),
            viewer: None,
            viewer_error: None,
            viewport: (600.0, 0.0, 800.0),
            strip: (0.0, 800.0),
            reader: None,
            reader_loading: None,
            reader_file: None,
            reader_ops: None,
            office_missing: office && (sheaf_core::office::find_soffice().is_none() || (cfg!(debug_assertions) && std::env::var_os("SHEAF_NO_SOFFICE").is_some())),
        }
    }

    /// The viewer's file changed (Redact, Sign, Edit): the viewer reloads, and items placed on
    /// the old file's pages are dropped, so they never go into the new file's job.
    pub fn new_viewer_file(&mut self) {
        self.viewer = None;
        self.viewer_error = None;
        self.edit = crate::edit_ui::EditState::new();
    }

    /// Removes file `uid`; with no file left, everything made for it goes too.
    pub fn remove_file(&mut self, uid: i32) {
        self.files.retain(|f| f.uid != uid);
        if self.files.is_empty() {
            self.order.clear();
            self.page_thumbs.clear();
            self.preview = None;
            self.new_viewer_file();
        }
    }
}

/// Compare: the slot a new file goes in (0 Original, 1 Changed). `wanted` is the per-slot
/// "Open file"; otherwise the first empty slot, and with both full the Changed one.
pub fn compare_slot(taken: &[usize], wanted: Option<usize>) -> usize {
    wanted.map(|s| s.min(1)).unwrap_or_else(|| if taken.contains(&0) { 1 } else { 0 })
}

impl TabState {
    /// Puts `f` in Compare's slot `slot` (0 Original, 1 Changed), replacing what was there.
    /// `files` stays ordered by slot.
    pub fn put_compare(&mut self, slot: usize, mut f: FileEntry) {
        f.slot = slot;
        self.files.retain(|x| x.slot != slot);
        self.files.push(f);
        self.files.sort_by_key(|x| x.slot);
    }

    /// Compare: Original and Changed trade places.
    pub fn swap_compare(&mut self) {
        for f in &mut self.files {
            f.slot = 1 - f.slot.min(1);
        }
        self.files.sort_by_key(|x| x.slot);
    }

    /// The action button was pressed: a fresh run, never inheriting an earlier Cancel.
    pub fn start_job(&mut self) {
        self.phase = Phase::Running;
        self.progress = 0.0;
        self.started = Some(std::time::Instant::now());
        self.cancelling = false;
    }

    /// The job returned: the result page (`Some`) or back to the options (`None`).
    pub fn finish(&mut self, done: Option<Done>) {
        self.progress = 0.0;
        self.cancelling = false;
        self.phase = match done {
            Some(d) => Phase::Done(Box::new(d)),
            None => Phase::Edit,
        };
    }
}

#[derive(Default)]
pub struct App {
    pub tabs: Vec<TabState>,
    pub next_id: i32,
    pub next_uid: i32,
    /// Files waiting for the password dialog: (tab id, file uid). Head is on screen.
    pub pw_queue: VecDeque<(i32, i32)>,
    /// Tab whose close is waiting for "Cancel job / Keep".
    pub pending_close: Option<i32>,
    /// PDFs opened or dropped on Home, preloaded into the next tool opened.
    pub home_files: Vec<PathBuf>,
    pub toast_timer: Option<slint::Timer>,
    /// Tabs closed while their job ran: tab id → (tool, inputs), for the history and toast.
    pub closed_jobs: HashMap<i32, (ToolId, Vec<PathBuf>)>,
    /// A document the reader is opening, waiting for its password.
    pub reader_pw: Option<PathBuf>,
    /// A file a document links to, waiting for "Open this file?" (Acrobat asks before launching one).
    pub pending_launch: Option<PathBuf>,
    /// The window is closing: each document with unsaved edits is asked about in turn.
    pub quitting: bool,
    /// Documents to close once their save is done (Save in the unsaved-changes dialog).
    pub closing_after_save: std::collections::HashSet<i32>,
    /// Close Other / Left / Right: documents with unsaved edits still to ask about, in turn.
    pub close_queue: Vec<i32>,
    /// Pages copied from a document's Pages panel (⌘C): its name, its bytes as they were, the
    /// pages; pasted into another document's Pages panel (⌘V).
    pub page_clip: Option<(String, std::sync::Arc<Vec<u8>>, Vec<usize>)>,
    /// A comment copied (⌘C): its XFDF element, its file and page; pasted while the system
    /// clipboard still holds [`crate::reader_ui::COMMENT_ON_CLIPBOARD`].
    pub comment_clip: Option<(String, PathBuf, usize)>,
}

impl App {
    pub fn tab_index(&self, id: i32) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }
    pub fn tab(&mut self, id: i32) -> Option<&mut TabState> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }
}

pub fn reader_icon() -> Image {
    icon_image("file-text")
}

pub fn to_image(t: Thumb) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&t.rgba, t.width, t.height))
}

fn file_item(f: &FileEntry) -> FileItem {
    FileItem {
        uid: f.uid,
        name: f.input.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into(),
        info: if f.unreadable { "Can't preview — will try to repair".into() } else { files::caption(f.size, f.pages).into() },
        thumb: f.thumb.clone().unwrap_or_default(),
        has_thumb: f.thumb.is_some(),
        preview_failed: f.thumb_failed,
        // A file pdfium cannot read gets the generic document icon.
        kind: if f.unreadable { Kind::Document.index() } else { f.kind.index() },
        locked: f.locked,
        slot: f.slot as i32,
        pages_label: files::pages_label(&f.pages_spec, f.pages).0.into(),
        pages_spec: f.pages_spec.clone().into(),
        pages_bad: !files::pages_label(&f.pages_spec, f.pages).1,
    }
}

fn verb(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Merge => "merged",
        ToolId::Split => "split",
        ToolId::RemovePages => "edited",
        ToolId::ExtractPages => "extracted",
        ToolId::Organize => "organized",
        ToolId::Rotate => "rotated",
        ToolId::Compress => "compressed",
        ToolId::Protect => "protected",
        ToolId::Unlock => "unlocked",
        ToolId::Repair => "repaired",
        ToolId::PageNumbers => "numbered",
        ToolId::Watermark => "watermarked",
        ToolId::Crop => "cropped",
        ToolId::Ocr => "made searchable",
        ToolId::Redact => "redacted",
        ToolId::Sign => "signed",
        ToolId::Edit => "edited",
        _ => "converted",
    }
}

/// Result-page warning when a protected input gave an unprotected output.
pub const NOT_PROTECTED: &str = "The output is not password-protected.";

/// Redact: the written file was re-opened and audited (a failed audit is a failed job).
pub const REDACTION_VERIFIED: &str = sheaf_core::redact::VERIFIED;

/// Compress of protected input whose unprotected copy came out larger.
fn grew_text(many: bool, before: u64, after: u64) -> String {
    let pct = (-percent_saved(before, after)).max(1);
    let what = if many { "The unprotected copies are" } else { "The unprotected copy is" };
    let of = if many { "the protected originals" } else { "the protected original" };
    format!("{what} {pct}% larger than {of}.")
}

/// What PDF to Word / PowerPoint / Excel make, for the result title.
fn exported_kind(tool: ToolId) -> Option<&'static str> {
    match tool {
        ToolId::PdfToWord => Some("Word document"),
        ToolId::PdfToPowerPoint => Some("presentation"),
        ToolId::PdfToExcel => Some("spreadsheet"),
        _ => None,
    }
}

/// Heading above the result notes.
fn notes_title(tool: ToolId) -> &'static str {
    match tool {
        ToolId::PdfToPdfA => "Not fixed (clause of ISO 19005 and problem):",
        _ => "Notes:",
    }
}

/// A compressed file's line on the result page: the saving, or that there was none to make
/// (a saving that rounds to 0 % is none); nothing when it grew (the summary says why).
fn compress_note(before: u64, after: u64) -> String {
    match percent_saved(before, after) {
        _ if after > before => String::new(),
        0 => "Already optimised".into(),
        p => format!("PDF compressed (-{p}%)"),
    }
}

fn percent_saved(before: u64, after: u64) -> i64 {
    if before == 0 { 0 } else { ((before as f64 - after as f64) / before as f64 * 100.0).round() as i64 }
}

/// Tools offered under "Continue to…" for these outputs.
pub fn continue_tools(current: ToolId, outputs: &[PathBuf]) -> Vec<ToolId> {
    let all_pdf = outputs.iter().all(|p| files::kind_of(p) == Kind::Pdf);
    tools::catalogue()
        .iter()
        .filter(|t| t.wave == 1 && t.id != current)
        .filter(|t| outputs.iter().all(|p| files::accepts(t.id, p)))
        .filter(|t| all_pdf || t.id == ToolId::JpgToPdf)
        .map(|t| t.id)
        .collect()
}

pub fn build(t: &TabState) -> ToolTab {
    let info = tools::find(t.tool);
    let mut tab = ToolTab {
        id: t.id,
        tool: t.tool.as_str().into(),
        name: info.name.into(),
        icon: t.icon.clone(),
        tile: tile_colour(t.tool),
        multi: info.multi,
        any_scanned: t.files.iter().any(|f| f.scanned),
        any_locked: t.files.iter().any(|f| f.locked),
        phase: match t.phase { Phase::Edit => 0, Phase::Running => 1, Phase::Done(_) => 2 },
        progress: t.progress,
        progress_text: progress_text(t.progress, if info.multi { t.files.len() } else { 1 }, t.started.map(|s| s.elapsed())).into(),
        cancelling: t.cancelling,
        preview_failed: t.pages_failed,
        files: ModelRc::new(VecModel::from(t.files.iter().map(file_item).collect::<Vec<_>>())),
        pages: ModelRc::new(VecModel::from(
            t.order
                .iter()
                .map(|p| {
                    let th = t.page_thumbs.get(p).cloned();
                    PageItem { page: *p as i32, has_thumb: th.is_some(), thumb: th.unwrap_or_default() }
                })
                .collect::<Vec<_>>(),
        )),
        office_missing: t.office_missing,
        has_preview: t.preview.is_some(),
        preview_note: t.preview_note.clone().into(),
        preview: t.preview.as_ref().map(|p| p.0.clone()).unwrap_or_default(),
        preview_w: t.preview.as_ref().map(|p| p.1).unwrap_or_default(),
        preview_h: t.preview.as_ref().map(|p| p.2).unwrap_or_default(),
        // A Merge page choice that is not pages of its file holds the run back (its card says why).
        files_ready: t.files.iter().all(|f| (f.kind != Kind::Pdf || f.pages.is_some() || f.unreadable || f.thumb_failed) && files::pages_label(&f.pages_spec, f.pages).1),
        default_out: t.files.first().map(|f| sheaf_app::jobs::default_name(t.tool, &f.input.stem())).unwrap_or_default().into(),
        ..Default::default()
    };
    if let Some(name) = &t.reader_loading {
        tab.is_reader = true;
        tab.name = name.as_str().into();
        tab.tool = "read".into();
        tab.icon = crate::state::reader_icon();
        tab.reader.title = name.as_str().into();
        tab.reader.panel_open = true;
        match &t.viewer {
            // Pages on screen, the rest of the document still being read.
            Some(v) => {
                crate::viewer_ui::fill(&mut tab, v);
                tab.reader.page_count = v.viewer.page_count() as i32;
                tab.reader.page_text = (v.current() + 1).to_string().into();
                tab.reader.read_only = "Loading the document…".into();
            }
            None => tab.viewer_status = "Opening…".into(),
        }
        return tab;
    }
    if let (Some(r), Some(v)) = (&t.reader, &t.viewer) {
        crate::viewer_ui::fill(&mut tab, v);
        crate::reader_ui::fill(&mut tab, r, v);
        tab.tool = "read".into();
        tab.icon = crate::state::reader_icon();
        return tab;
    }
    if let Some(v) = &t.viewer {
        crate::viewer_ui::fill(&mut tab, v);
        if t.tool == ToolId::Sign {
            crate::viewer_ui::fill_sign(&mut tab, v, t.sign.width(), t.sign.preview.as_ref().map(|p| p.1));
        }
    }
    crate::sign_ui::fill(&mut tab, &t.sign);
    if t.tool == ToolId::Edit {
        crate::edit_ui::fill(&mut tab, &t.edit, t.viewer.as_ref().map(|v| &v.viewer));
    }
    tab.viewer_status = match (&t.viewer, &t.viewer_error, t.files.is_empty()) {
        (Some(_), _, _) | (None, None, true) => "".into(),
        (None, Some(e), _) => e.as_str().into(),
        (None, None, false) => "Opening…".into(),
    };
    if let Phase::Done(d) = &t.phase {
        fill_result(&mut tab, t, d);
    }
    tab
}

/// "File 2 of 5 · 40% · about 12 s left": the file being worked on (a batch shares the bar
/// evenly), how far, and the time left once enough is done to tell (iLovePDF's and Acrobat's).
fn progress_text(p: f32, files: usize, elapsed: Option<std::time::Duration>) -> String {
    let p = p.clamp(0.0, 1.0);
    let mut parts = Vec::new();
    if files > 1 {
        parts.push(format!("File {} of {files}", ((p * files as f32) as usize + 1).min(files)));
    }
    parts.push(format!("{:.0}%", p * 100.0));
    if let Some(e) = elapsed.filter(|e| (0.05..1.0).contains(&p) && e.as_secs_f32() >= 2.0) {
        let left = (e.as_secs_f32() * (1.0 - p) / p).round() as u64;
        parts.push(match left {
            0..=4 => "a few seconds left".to_string(),
            5..=59 => format!("about {left} s left"),
            _ => format!("about {} min left", left.div_ceil(60)),
        });
    }
    parts.join(" · ")
}

fn fill_result(tab: &mut ToolTab, t: &TabState, d: &Done) {
    let o = &d.outcome;
    let many = o.outputs.len() > 1;
    let is_compress = t.tool == ToolId::Compress;
    tab.result_title = if t.tool == ToolId::PdfToJpg {
        match o.outputs.len() {
            1 => "Your image has been created successfully.".into(),
            n => format!("Your {n} images have been created successfully.").into(),
        }
    } else if let Some(what) = exported_kind(t.tool) {
        let (it, have) = if many { (format!("{what}s"), "have") } else { (what.to_string(), "has") };
        format!("Your {it} {have} been created successfully.").into()
    } else if t.tool == ToolId::Compare {
        "Your comparison report has been created successfully.".into()
    } else if many {
        format!("Your PDFs have been {} successfully.", verb(t.tool)).into()
    } else {
        format!("Your PDF has been {} successfully.", verb(t.tool)).into()
    };
    if is_compress {
        let pct = percent_saved(o.before, o.after);
        let it = if many { "Your PDFs are" } else { "Your PDF is" };
        if pct > 0 {
            tab.result_sub = format!("{it} now").into();
            tab.result_pct = format!("{pct}% smaller!").into();
        } else if o.grew && o.after > o.before {
            tab.result_sub = grew_text(many, o.before, o.after).into();
        } else {
            tab.result_sub = format!("{it} already as small as it gets.").into();
        }
        tab.size_before = files::human_size(o.before).into();
        tab.size_after = files::human_size(o.after).into();
    }
    if !o.summary.is_empty() {
        tab.result_sub = o.summary.clone().into();
    }
    if o.was_encrypted {
        tab.warning = NOT_PROTECTED.into();
    }
    if t.tool == ToolId::Redact && !o.outputs.is_empty() {
        tab.result_verified = REDACTION_VERIFIED.into();
    }
    if let Some((page, th)) = &o.preview {
        tab.result_preview = Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&th.rgba, th.width, th.height));
        tab.result_preview_caption = format!("Page {} as redacted", page + 1).into();
    }
    if !o.notes.is_empty() {
        tab.notes_title = notes_title(t.tool).into();
        let lines: Vec<slint::SharedString> = o.notes.iter().map(|n| n.as_str().into()).collect();
        tab.notes = ModelRc::new(VecModel::from(lines));
    }
    if !o.failures.is_empty() {
        let n = o.failures.len();
        tab.failure_title = format!("{n} file{} failed:", if n == 1 { "" } else { "s" }).into();
        let lines: Vec<slint::SharedString> = o
            .failures
            .iter()
            .map(|(p, e)| format!("{} — {e}", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).into())
            .collect();
        tab.failures = ModelRc::new(VecModel::from(lines));
        // Not every file went through: the heading says how many of the files given did (iLovePDF's
        // partial result); a file may make several outputs (PDF to JPG), so the inputs are counted.
        let total = t.files.len().max(n);
        let done = total - n;
        tab.result_title = format!("{done} of {total} files {} {}; {n} failed.", if done == 1 { "was" } else { "were" }, verb(t.tool)).into();
    }
    let outs: Vec<ResultFile> = o
        .outputs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let size = files::file_size(p);
            let note = match (is_compress, d.input_sizes.get(i)) {
                (true, Some(&b)) => compress_note(b, size),
                _ => String::new(),
            };
            ResultFile {
                name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into(),
                info: files::caption(size, d.page_counts.get(i).copied().flatten()).into(),
                note: note.into(),
                is_pdf: files::kind_of(p) == Kind::Pdf,
                kind: match files::ext(p).as_str() {
                    e @ ("docx" | "pptx" | "xlsx") => e.into(),
                    _ => "image".into(),
                },
            }
        })
        .collect();
    tab.outputs = ModelRc::new(VecModel::from(outs));
    let cont: Vec<ContinueTool> = continue_tools(t.tool, &o.outputs)
        .into_iter()
        .map(|id| {
            let info = tools::find(id);
            ContinueTool { id: id.as_str().into(), name: info.name.into(), icon: icon_image(info.icon), tile: tile_colour(id) }
        })
        .collect();
    tab.continue_tools = ModelRc::new(VecModel::from(cont));
}

/// The persistent `tabs` model. Rows are updated in place so each tab's Slint instance
/// (and the options typed into it) survives updates.
pub fn tabs_model() -> Rc<VecModel<ToolTab>> {
    TABS.with(|t| t.borrow().clone().expect("tabs model installed at startup"))
}

thread_local! {
    static APP: RefCell<App> = RefCell::new(App::default());
}

/// Runs `f` with the app state. Never call a modal dialog (rfd) inside: its nested run loop
/// can deliver worker results that borrow the state again.
pub fn with<R>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with(|a| f(&mut a.borrow_mut()))
}

/// The tab bar's order: rows of [`tabs_model`] as the bar shows them. Dragging a tab reorders
/// only this, so no tab's page is rebuilt (its options and scroll stay).
pub fn tab_order() -> Rc<VecModel<i32>> {
    ORDER.with(|t| t.borrow().clone().expect("tab order installed at startup"))
}

thread_local! {
    static TABS: RefCell<Option<Rc<VecModel<ToolTab>>>> = const { RefCell::new(None) };
    static ORDER: RefCell<Option<Rc<VecModel<i32>>>> = const { RefCell::new(None) };
}

pub fn install_tabs_model(ui: &AppWindow) {
    let m = Rc::new(VecModel::<ToolTab>::default());
    TABS.with(|t| *t.borrow_mut() = Some(m.clone()));
    ui.set_tabs(ModelRc::from(m));
    let o = Rc::new(VecModel::<i32>::default());
    ORDER.with(|t| *t.borrow_mut() = Some(o.clone()));
    ui.set_tab_order(ModelRc::from(o));
}

/// Adds tab `t`'s row, last in the bar; returns its row.
pub fn push_tab_row(t: &TabState) -> i32 {
    let m = tabs_model();
    m.push(build(t));
    let row = m.row_count() as i32 - 1;
    tab_order().push(row);
    row
}

/// Takes out row `i` of the tabs model and its place in the bar.
pub fn remove_tab_row(i: usize) {
    tabs_model().remove(i);
    let order = tab_order();
    let rows: Vec<i32> = order.iter().filter(|r| *r != i as i32).map(|r| if r > i as i32 { r - 1 } else { r }).collect();
    order.set_vec(rows);
}

/// Moves the tab at place `from` of the bar to place `to`.
pub fn move_in_order(from: usize, to: usize) {
    let order = tab_order();
    if from < order.row_count() && to < order.row_count() && from != to {
        let r = order.remove(from);
        order.insert(to, r);
    }
}

/// Pushes the tab `id` into its model row.
pub fn refresh(app: &App, id: i32) {
    if let Some(i) = app.tab_index(id) {
        let m = tabs_model();
        if i < m.row_count() {
            m.set_row_data(i, build(&app.tabs[i]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_items_are_dropped_when_the_file_changes_or_is_removed() {
        let mut t = TabState::new(1, ToolId::Edit);
        let place = |t: &mut TabState| t.edit.release(0, (10.0, 10.0), (400.0, 500.0));
        place(&mut t);
        assert_eq!(t.edit.items.len(), 1);
        t.new_viewer_file();
        assert!(t.edit.items.is_empty() && t.edit.selected.is_none());
        place(&mut t);
        // No file: removing an unknown one still leaves none, so the items go.
        t.remove_file(42);
        assert!(t.edit.items.is_empty());
    }

    #[test]
    fn continue_to_offers_pdf_tools_for_pdfs_and_jpg_to_pdf_for_images() {
        let pdf = continue_tools(ToolId::Compress, &[PathBuf::from("a.pdf")]);
        assert!(pdf.contains(&ToolId::Merge) && pdf.contains(&ToolId::Protect));
        assert!(!pdf.contains(&ToolId::Compress) && !pdf.contains(&ToolId::JpgToPdf) && !pdf.contains(&ToolId::WordToPdf));
        assert_eq!(continue_tools(ToolId::PdfToJpg, &[PathBuf::from("a-1.jpg")]), vec![ToolId::JpgToPdf]);
    }

    #[test]
    fn a_late_cancel_never_leaks_into_the_next_job() {
        let mut t = TabState::new(1, ToolId::Split);
        t.start_job();
        // Cancel pressed after the job finished but before its result reached the UI.
        t.cancelling = true;
        t.finish(Some(Done { outcome: JobOutcome::default(), page_counts: vec![], input_sizes: vec![] }));
        assert!(!t.cancelling && matches!(t.phase, Phase::Done(_)));
        t.cancelling = true; // stale flag from any path
        t.start_job();
        assert!(!t.cancelling && matches!(t.phase, Phase::Running));
        t.cancelling = true;
        t.finish(None);
        assert!(!t.cancelling && matches!(t.phase, Phase::Edit));
        assert!(!build(&t).cancelling);
    }

    fn done_tab(tool: ToolId, outcome: JobOutcome) -> ToolTab {
        let mut t = TabState::new(1, tool);
        t.finish(Some(Done { outcome, page_counts: vec![], input_sizes: vec![] }));
        build(&t)
    }

    #[test]
    fn the_result_page_warns_whenever_protection_was_dropped() {
        let enc = JobOutcome { outputs: vec![PathBuf::from("/x/a-rotated.pdf")], was_encrypted: true, ..Default::default() };
        assert_eq!(done_tab(ToolId::Rotate, enc.clone()).warning, "The output is not password-protected.");
        assert_eq!(done_tab(ToolId::Compress, enc.clone()).warning, "The output is not password-protected.");
        let plain = JobOutcome { was_encrypted: false, ..enc };
        assert_eq!(done_tab(ToolId::Rotate, plain).warning, "");
    }

    #[test]
    fn compress_of_protected_input_that_grew_says_so() {
        let o = JobOutcome { outputs: vec![PathBuf::from("/x/a.pdf")], before: 1000, after: 1100, was_encrypted: true, grew: true, ..Default::default() };
        let tab = done_tab(ToolId::Compress, o);
        assert_eq!(tab.result_sub, "The unprotected copy is 10% larger than the protected original.");
        assert_eq!(tab.result_pct, "");
    }

    #[test]
    fn failed_batch_items_are_listed() {
        let o = JobOutcome {
            outputs: vec![PathBuf::from("/x/a.pdf")],
            failures: vec![(PathBuf::from("/x/b.docx"), "conversion failed: boom".into())],
            ..Default::default()
        };
        let mut t = TabState::new(1, ToolId::WordToPdf);
        t.files = vec![entry(1), entry(2)];
        t.finish(Some(Done { outcome: o.clone(), page_counts: vec![], input_sizes: vec![] }));
        let tab = build(&t);
        assert_eq!(tab.failure_title, "1 file failed:");
        assert_eq!(tab.result_title, "1 of 2 files was converted; 1 failed.");
        // A file that made several outputs (PDF to JPG) counts once.
        let mut t = TabState::new(1, ToolId::PdfToJpg);
        t.files = vec![entry(1), entry(2), entry(3)];
        let jpgs = JobOutcome { outputs: (1..=5).map(|k| PathBuf::from(format!("/x/a-{k}.jpg"))).collect(), ..o.clone() };
        t.finish(Some(Done { outcome: jpgs, page_counts: vec![], input_sizes: vec![] }));
        assert_eq!(build(&t).result_title, "2 of 3 files were converted; 1 failed.");
        assert_eq!(tab.failures.row_data(0).unwrap(), "b.docx — conversion failed: boom");
        assert_eq!(tab.outputs.row_count(), 1);
    }

    #[test]
    fn pdfa_result_shows_the_summary_and_the_rules_left() {
        let o = JobOutcome {
            outputs: vec![PathBuf::from("/x/a-pdfa.pdf")],
            summary: "1 file does not fully conform to PDF/A-2b".into(),
            notes: vec!["6.2.11.4: The font Helvetica isn't embedded".into()],
            ..Default::default()
        };
        let tab = done_tab(ToolId::PdfToPdfA, o);
        assert_eq!(tab.result_sub, "1 file does not fully conform to PDF/A-2b");
        assert_eq!(tab.notes.row_data(0).unwrap(), "6.2.11.4: The font Helvetica isn't embedded");
        assert!(tab.notes_title.starts_with("Not fixed"));
        let ok = JobOutcome { outputs: vec![PathBuf::from("/x/a-pdfa.pdf")], summary: "Conforms to PDF/A-2b".into(), ..Default::default() };
        let tab = done_tab(ToolId::PdfToPdfA, ok);
        assert_eq!((tab.result_sub.as_str(), tab.notes_title.as_str()), ("Conforms to PDF/A-2b", ""));
    }

    #[test]
    fn office_exports_get_their_own_title_and_no_pdf_tools() {
        let o = JobOutcome { outputs: vec![PathBuf::from("/x/a.docx")], ..Default::default() };
        let tab = done_tab(ToolId::PdfToWord, o);
        assert_eq!(tab.result_title, "Your Word document has been created successfully.");
        assert!(!tab.outputs.row_data(0).unwrap().is_pdf);
        assert_eq!(tab.outputs.row_data(0).unwrap().kind, "docx");
        assert_eq!(tab.continue_tools.row_count(), 0);
        let o = JobOutcome { outputs: vec![PathBuf::from("/x/a.xlsx"), PathBuf::from("/x/b.xlsx")], ..Default::default() };
        assert_eq!(done_tab(ToolId::PdfToExcel, o).result_title, "Your spreadsheets have been created successfully.");
        for out in ["a.docx", "a.pptx", "a.xlsx"] {
            assert!(continue_tools(ToolId::PdfToWord, &[PathBuf::from(out)]).is_empty(), "{out}");
        }
        let ocr = continue_tools(ToolId::Ocr, &[PathBuf::from("a-ocr.pdf")]);
        assert!(ocr.contains(&ToolId::PdfToWord) && ocr.contains(&ToolId::Compress) && !ocr.contains(&ToolId::Ocr));
    }

    fn entry(uid: i32) -> FileEntry {
        FileEntry { uid, input: Input::new(format!("/x/{uid}.pdf")), kind: Kind::Pdf, size: 0, pages: None, thumb: None, thumb_failed: false, unreadable: false, locked: false, slot: 0, scanned: false, pages_spec: String::new() }
    }

    #[test]
    fn compare_keeps_an_original_and_a_changed_file() {
        let mut t = TabState::new(1, ToolId::Compare);
        let uids = |t: &TabState| t.files.iter().map(|f| (f.slot, f.uid)).collect::<Vec<_>>();
        let taken = |t: &TabState| t.files.iter().map(|f| f.slot).collect::<Vec<_>>();
        // Two files added at once: the first is the Original.
        t.put_compare(compare_slot(&taken(&t), None), entry(1));
        t.put_compare(compare_slot(&taken(&t), None), entry(2));
        assert_eq!(uids(&t), [(0, 1), (1, 2)]);
        // A third replaces the Changed one; a per-slot open replaces that slot.
        t.put_compare(compare_slot(&taken(&t), None), entry(3));
        assert_eq!(uids(&t), [(0, 1), (1, 3)]);
        t.put_compare(compare_slot(&taken(&t), Some(0)), entry(4));
        assert_eq!(uids(&t), [(0, 4), (1, 3)]);
        t.swap_compare();
        assert_eq!(uids(&t), [(0, 3), (1, 4)]);
        // Changed chosen first: the Original slot stays empty until a file arrives.
        let mut t = TabState::new(2, ToolId::Compare);
        t.put_compare(compare_slot(&taken(&t), Some(1)), entry(7));
        assert_eq!(uids(&t), [(1, 7)]);
        t.put_compare(compare_slot(&taken(&t), None), entry(8));
        assert_eq!(uids(&t), [(0, 8), (1, 7)]);
    }

    #[test]
    fn progress_says_the_file_how_far_and_the_time_left() {
        use std::time::Duration;
        assert_eq!(progress_text(0.0, 1, None), "0%");
        assert_eq!(progress_text(0.4, 5, Some(Duration::from_secs(1))), "File 3 of 5 · 40%");
        assert_eq!(progress_text(0.25, 1, Some(Duration::from_secs(10))), "25% · about 30 s left");
        assert_eq!(progress_text(0.1, 1, Some(Duration::from_secs(30))), "10% · about 5 min left");
        assert_eq!(progress_text(0.9, 1, Some(Duration::from_secs(18))), "90% · a few seconds left");
        assert_eq!(progress_text(1.0, 2, Some(Duration::from_secs(9))), "File 2 of 2 · 100%");
    }

    #[test]
    fn redact_result_says_how_many_areas_and_that_the_audit_passed() {
        let o = JobOutcome { outputs: vec![PathBuf::from("/x/a-redacted.pdf")], summary: "3 areas redacted".into(), ..Default::default() };
        let tab = done_tab(ToolId::Redact, o);
        assert_eq!(tab.result_title, "Your PDF has been redacted successfully.");
        assert_eq!((tab.result_sub.as_str(), tab.result_verified.as_str()), ("3 areas redacted", REDACTION_VERIFIED));
        assert_eq!(done_tab(ToolId::Rotate, JobOutcome { outputs: vec![PathBuf::from("/x/a.pdf")], ..Default::default() }).result_verified, "");
        // The first redacted page is shown as written.
        let th = Thumb { width: 2, height: 3, rgba: vec![0; 24] };
        let o = JobOutcome { outputs: vec![PathBuf::from("/x/a-redacted.pdf")], preview: Some((2, th)), ..Default::default() };
        let tab = done_tab(ToolId::Redact, o);
        assert_eq!((tab.result_preview_caption.as_str(), tab.result_preview.size().width), ("Page 3 as redacted", 2));
    }

    #[test]
    fn saved_percentages() {
        assert_eq!(percent_saved(3_322_740, 2_339_000), 30);
        assert_eq!(percent_saved(0, 10), 0);
    }

    #[test]
    fn a_file_compress_could_not_shrink_says_so() {
        assert_eq!(compress_note(3_322_740, 2_339_000), "PDF compressed (-30%)");
        assert_eq!(compress_note(1_000_000, 998_000), "Already optimised");
        assert_eq!(compress_note(1_000_000, 1_000_000), "Already optimised");
        assert_eq!(compress_note(1_000_000, 1_200_000), "");
    }
}
