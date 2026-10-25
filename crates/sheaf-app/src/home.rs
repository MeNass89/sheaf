//! Recents, Tasks History and Preferences: the stores, their Slint models, and what the
//! preferences change (theme, where outputs go, what happens after a job).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sheaf_app::files::{self, Kind};
use sheaf_app::history::{self, Entry, Store};
use sheaf_app::icons::{icon_image, tile_colour};
use sheaf_app::prefs::{OutputMode, Prefs, ThemePref};
use sheaf_app::{store_file, AppWindow, HistoryRow, RecentFile, Theme, ToolCard};
use sheaf_core::pages::Input;
use sheaf_core::tools::{self, ToolId};
use slint::winit_030::{winit, WinitWindowAccessor};
use slint::{ComponentHandle, Image, ModelRc, VecModel};

/// Recent output files shown on Recents.
const MAX_RECENTS: usize = 12;
const THUMB_PX: i32 = 120;

/// A rendered preview, keyed by path and modification time.
type ThumbKey = (PathBuf, Option<SystemTime>);

struct Home {
    dir: PathBuf,
    store: Store,
    prefs: Prefs,
    /// SHEAF_THEME: a debug override that wins over the preference.
    forced_theme: Option<ThemePref>,
    ui: slint::Weak<AppWindow>,
    /// Rendered previews with the page count; `None` = rendering or unreadable.
    thumbs: HashMap<ThumbKey, Option<(Image, Option<u32>)>>,
    /// Recents currently on screen, in model order.
    recents: Vec<PathBuf>,
}

thread_local! {
    static HOME: RefCell<Option<Home>> = const { RefCell::new(None) };
}

/// Runs `f` with the home state. Never open a modal dialog inside.
fn with<R>(f: impl FnOnce(&mut Home) -> R) -> R {
    HOME.with(|h| f(h.borrow_mut().as_mut().expect("home::install called at startup")))
}

/// Loads history and preferences from the data folder and fills the pages.
pub fn install(ui: &AppWindow, forced_theme: Option<ThemePref>) {
    let dir = store_file::data_dir();
    let h = Home {
        store: Store::open(&dir),
        prefs: Prefs::load(&dir),
        dir,
        forced_theme,
        ui: ui.as_weak(),
        thumbs: HashMap::new(),
        recents: Vec::new(),
    };
    HOME.with(|s| *s.borrow_mut() = Some(h));
    push_prefs(ui);
    apply_theme(ui);
    refresh(ui);
}

pub fn prefs() -> Prefs {
    with(|h| h.prefs.clone())
}

/// The reader's page theme, remembered for the next document opened.
pub fn set_reader_theme(theme: i32) {
    with(|h| {
        if h.prefs.reader_theme != theme {
            h.prefs.reader_theme = theme;
            if let Err(e) = h.prefs.save(&h.dir) {
                eprintln!("sheaf: cannot save preferences: {e}");
            }
        }
    });
}

/// More ▸ Highlight form fields, remembered (Acrobat's "Highlight existing fields").
pub fn toggle_field_tint() -> bool {
    with(|h| {
        h.prefs.plain_fields = !h.prefs.plain_fields;
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
        !h.prefs.plain_fields
    })
}

/// Remembers that `path` was left on `page` (0-based).
pub fn remember_page(path: &Path, page: usize) {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    with(|h| {
        h.prefs.last_pages.retain(|(q, _)| q != &p);
        h.prefs.last_pages.insert(0, (p, page as u32));
        h.prefs.last_pages.truncate(200);
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

/// How `path` was viewed when left (see `Prefs::last_views`).
pub fn remember_view(path: &Path, fit: i32, zoom: u32, spread: u8) {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    with(|h| {
        h.prefs.last_views.retain(|(q, ..)| q != &p);
        h.prefs.last_views.insert(0, (p, fit, zoom, spread));
        h.prefs.last_views.truncate(200);
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

pub fn last_view(path: &Path) -> Option<(i32, u32, u8)> {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    with(|h| h.prefs.last_views.iter().find(|(q, ..)| q == &p).map(|(_, f, z, s)| (*f, *z, *s)))
}

/// Starred tools (Tools ▸ star), in the order starred.
pub fn favourites() -> Vec<String> {
    with(|h| h.prefs.favourite_tools.clone())
}

pub fn is_favourite(id: &str) -> bool {
    with(|h| h.prefs.favourite_tools.iter().any(|f| f == id))
}

pub fn toggle_favourite(id: &str) {
    with(|h| {
        let list = &mut h.prefs.favourite_tools;
        match list.iter().position(|f| f == id) {
            Some(i) => {
                list.remove(i);
            }
            None => list.push(id.to_string()),
        }
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

/// The style last used with reader tool `tool` (colour, width, opacity), if any.
pub fn tool_style(tool: i32) -> Option<(u8, u8, u8)> {
    with(|h| tool_style_of(&h.prefs, tool))
}

pub fn set_tool_style(tool: i32, style: (u8, u8, u8)) {
    with(|h| {
        if tool_style_of(&h.prefs, tool) == Some(style) {
            return;
        }
        h.prefs.tool_styles.retain(|s| s.0 != tool);
        h.prefs.tool_styles.push((tool, style.0, style.1, style.2));
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

fn tool_style_of(p: &Prefs, tool: i32) -> Option<(u8, u8, u8)> {
    p.tool_styles.iter().find(|s| s.0 == tool).map(|s| (s.1, s.2, s.3))
}

/// The page `path` was left on, if any.
pub fn last_page(path: &Path) -> Option<usize> {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    with(|h| h.prefs.last_pages.iter().find(|(q, _)| q == &p).map(|(_, n)| *n as usize))
}

pub fn set_signature_pen(pen: (u8, u8)) {
    with(|h| {
        h.prefs.signature_pen = pen;
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

/// The reader's signature, kept for every document: drawn strokes, or a typed name. A new one goes
/// first in the list of kept signatures (four at most, as PDF Expert's); none clears the current one only.
pub fn set_signature(strokes: Vec<Vec<[f64; 2]>>, name: Option<String>, font: u8) {
    with(|h| {
        if !strokes.is_empty() || name.is_some() {
            let sig = (strokes.clone(), name.clone(), h.prefs.signature_pen, font);
            h.prefs.signatures.retain(|s| (&s.0, &s.1, s.3) != (&sig.0, &sig.1, sig.3));
            h.prefs.signatures.insert(0, sig);
            h.prefs.signatures.truncate(4);
        }
        h.prefs.signature_strokes = strokes;
        h.prefs.signature_name = name;
        h.prefs.signature_font = font;
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

/// Kept signatures, newest first (an older profile's single signature counts as one).
pub fn signatures() -> Vec<sheaf_app::prefs::KeptSignature> {
    with(|h| {
        if h.prefs.signatures.is_empty() && (!h.prefs.signature_strokes.is_empty() || h.prefs.signature_name.is_some()) {
            return vec![(h.prefs.signature_strokes.clone(), h.prefs.signature_name.clone(), h.prefs.signature_pen, h.prefs.signature_font)];
        }
        h.prefs.signatures.clone()
    })
}

/// Kept signature `i` becomes the one placed (`remove`: it is forgotten instead).
pub fn pick_signature(i: usize, remove: bool) {
    let list = signatures();
    let Some(sig) = list.get(i).cloned() else { return };
    with(|h| {
        h.prefs.signatures = list;
        h.prefs.signatures.remove(i);
        if remove {
            if (&h.prefs.signature_strokes, &h.prefs.signature_name, h.prefs.signature_font) == (&sig.0, &sig.1, sig.3) {
                let next = h.prefs.signatures.first().cloned().unwrap_or_default();
                h.prefs.signature_strokes = next.0;
                h.prefs.signature_name = next.1;
                h.prefs.signature_pen = next.2;
                h.prefs.signature_font = next.3;
            }
        } else {
            h.prefs.signatures.insert(0, sig.clone());
            h.prefs.signature_strokes = sig.0;
            h.prefs.signature_name = sig.1;
            // Placed in the pen and font it was made with.
            h.prefs.signature_pen = sig.2;
            h.prefs.signature_font = sig.3;
        }
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

/// Key of history entries that only record a document opened in the reader (for Recents):
/// they are not tasks and stay out of Tasks History.
pub const READ: &str = "read";

/// A document opened in the reader joins Recents.
pub fn record_open(path: &Path) {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let e = Entry { tool: READ.into(), inputs: vec![], outputs: vec![p], at_unix: history::now_unix() };
    let ui = with(|h| {
        if let Err(err) = h.store.push(e) {
            eprintln!("sheaf: cannot save history: {err}");
        }
        h.ui.upgrade()
    });
    if let Some(ui) = ui {
        refresh(&ui);
    }
}

fn update_prefs(ui: &AppWindow, f: impl FnOnce(&mut Prefs)) {
    with(|h| {
        f(&mut h.prefs);
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
    push_prefs(ui);
}

fn push_prefs(ui: &AppWindow) {
    let p = prefs();
    ui.set_theme_pref(p.theme.mode());
    ui.set_output_mode(p.output.index());
    ui.set_fixed_folder(p.fixed_folder.map(|f| f.display().to_string()).unwrap_or_default().into());
    ui.set_open_after(p.open_folder_after);
    ui.set_author_name(p.author.clone().unwrap_or_default().into());
    ui.set_author_default(login_name().into());
    ui.set_default_zoom(p.default_zoom);
    ui.global::<Theme>().set_accent_choice(p.accent.min(5) as i32);
    ui.set_backdrop_pref(p.backdrop.min(4) as i32);
    let rb = ui.global::<sheaf_app::ReaderBridge>();
    rb.set_panel_width(panel_width());
    rb.set_thumb_width(crate::reader_ui::reader_thumb(panel_width()) as f32);
}

/// Preferences ▸ Appearance ▸ Accent colour.
pub fn set_accent(ui: &AppWindow, i: i32) {
    update_prefs(ui, |p| p.accent = i.clamp(0, 5) as u8);
}

/// Preferences ▸ Appearance ▸ Background behind pages (Preview's window background); open
/// documents take it at once.
pub fn set_backdrop(ui: &AppWindow, i: i32) {
    update_prefs(ui, |p| p.backdrop = i.clamp(0, 4) as u8);
    crate::reader_ui::refresh_all();
}

/// The colour behind the pages chosen in Preferences, `None` for the theme's own.
pub fn backdrop() -> Option<[u8; 3]> {
    [None, Some([228, 228, 231]), Some([154, 154, 160]), Some([43, 43, 46]), Some([0, 0, 0])][prefs().backdrop.min(4) as usize]
}

/// The reader's side panel width, as last dragged (PDF Expert keeps it).
pub fn panel_width() -> f32 {
    match prefs().panel_width {
        0 => crate::reader_ui::PANEL_WIDTH,
        w => w as f32,
    }
}

/// The side panel let go at `w` px: kept within 200–480 and remembered. Returns the width kept.
pub fn set_panel_width(ui: &AppWindow, w: f32) -> f32 {
    let w = w.clamp(200.0, 480.0);
    update_prefs(ui, |p| p.panel_width = w.round() as u16);
    w
}

/// Theme.mode and the window's own appearance (title bar, scroll bars).
fn apply_theme(ui: &AppWindow) {
    let t = with(|h| h.forced_theme.unwrap_or(h.prefs.theme));
    ui.global::<Theme>().set_mode(t.mode());
    let wt = match t {
        ThemePref::System => None,
        ThemePref::Light => Some(winit::window::Theme::Light),
        ThemePref::Dark => Some(winit::window::Theme::Dark),
    };
    ui.window().with_winit_window(|w| w.set_theme(wt));
}

pub fn set_theme(ui: &AppWindow, m: i32) {
    update_prefs(ui, |p| p.theme = ThemePref::from_mode(m));
    apply_theme(ui);
}

pub fn set_output(ui: &AppWindow, m: i32) {
    let mode = OutputMode::from_index(m);
    if mode == OutputMode::Fixed && prefs().fixed_folder.is_none() {
        return choose_folder(ui);
    }
    update_prefs(ui, |p| p.output = mode);
}

pub fn choose_folder(ui: &AppWindow) {
    let d = crate::dialog::new(ui).set_title("Always save to");
    crate::dialog::pick_folder(ui, d, |ui, dir| {
        update_prefs(ui, |p| {
            p.fixed_folder = Some(dir);
            p.output = OutputMode::Fixed;
        });
    });
}

pub fn set_open_after(ui: &AppWindow, v: bool) {
    update_prefs(ui, |p| p.open_folder_after = v);
}

/// Preferences ▸ Open documents at (0 fit width, 1 fit page, 2 actual size).
pub fn set_default_zoom(ui: &AppWindow, z: i32) {
    update_prefs(ui, |p| p.default_zoom = z.clamp(0, 2));
}

pub fn set_author(ui: &AppWindow, name: &str) {
    let name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
    if prefs().author != name {
        update_prefs(ui, |p| p.author = name);
    }
}

/// The author written on annotations: the name set in Preferences, else the login name (as
/// Acrobat and PDF Expert do).
pub fn author() -> String {
    prefs().author.unwrap_or_else(login_name)
}

fn login_name() -> String {
    std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default()
}

/// A job succeeded: store it, refresh Recents and History, then "open the output folder".
pub fn record(tool: ToolId, inputs: &[PathBuf], outputs: &[PathBuf]) {
    let e = Entry { tool: tool.as_str().into(), inputs: inputs.to_vec(), outputs: outputs.to_vec(), at_unix: history::now_unix() };
    let ui = with(|h| {
        if let Err(err) = h.store.push(e) {
            eprintln!("sheaf: cannot save history: {err}");
        }
        h.ui.upgrade()
    });
    if let Some(ui) = ui {
        refresh(&ui);
    }
    after_processing(outputs);
}

/// "Open the output folder when done".
fn after_processing(outputs: &[PathBuf]) {
    if let Some(first) = outputs.first().filter(|_| prefs().open_folder_after) {
        crate::run::reveal(first);
    }
}

pub fn clear(ui: &AppWindow) {
    with(|h| {
        if let Err(e) = h.store.clear() {
            eprintln!("sheaf: cannot clear history: {e}");
        }
    });
    refresh(ui);
}

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn names(ps: &[PathBuf]) -> String {
    ps.iter().map(|p| name(p)).collect::<Vec<_>>().join(", ")
}

fn row(e: &Entry, now: u64) -> HistoryRow {
    let tool = ToolId::from_str(&e.tool);
    let info = tool.map(tools::find);
    HistoryRow {
        tool_name: info.map(|i| i.name).unwrap_or(e.tool.as_str()).into(),
        icon: info.map(|i| icon_image(i.icon)).unwrap_or_default(),
        tile: tool.map(tile_colour).unwrap_or_default(),
        files: format!("{} → {}", names(&e.inputs), names(&e.outputs)).into(),
        when: history::relative_time(e.at_unix, now).into(),
        can_reveal: e.outputs.first().is_some_and(|p| p.exists()),
        can_rerun: rerun_inputs(e).is_some(),
    }
}

/// The newest existing output files, without duplicates, with their entry time.
fn recent_outputs(store: &Store) -> Vec<(PathBuf, u64)> {
    let mut out: Vec<(PathBuf, u64)> = Vec::new();
    for e in store.list() {
        for p in &e.outputs {
            if store.visible_in_recents(p) && out.len() < MAX_RECENTS && !out.iter().any(|(q, _)| q == p) && p.is_file() {
                out.push((p.clone(), e.at_unix));
            }
        }
    }
    out
}

fn thumb_key(p: &Path) -> ThumbKey {
    (p.to_path_buf(), std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// Rebuilds the Recents and History models (relative times included). Missing previews are
/// rendered on the probe pool and pushed when ready.
pub fn refresh(ui: &AppWindow) {
    let now = history::now_unix();
    let (rows, recents, missing) = with(|h| {
        let rows: Vec<HistoryRow> = h.store.list().iter().filter(|e| e.tool != READ).map(|e| row(e, now)).collect();
        let outs = recent_outputs(&h.store);
        let mut missing = Vec::new();
        let recents: Vec<RecentFile> = outs
            .iter()
            .map(|(p, at)| {
                let kind = files::kind_of(p);
                let key = thumb_key(p);
                let cached = h.thumbs.get(&key).cloned().flatten();
                if !h.thumbs.contains_key(&key) && kind != Kind::Document {
                    h.thumbs.insert(key, None);
                    missing.push((p.clone(), kind));
                }
                let pages = cached.as_ref().and_then(|c| c.1);
                RecentFile {
                    name: name(p).into(),
                    info: files::caption(files::file_size(p), pages).into(),
                    when: history::relative_time(*at, now).into(),
                    has_thumb: cached.is_some(),
                    thumb: cached.map(|c| c.0).unwrap_or_default(),
                    kind: kind.index(),
                }
            })
            .collect();
        h.recents = outs.into_iter().map(|(p, _)| p).collect();
        (rows, recents, missing)
    });
    ui.set_history(ModelRc::new(VecModel::from(rows)));
    ui.set_recents(ModelRc::new(VecModel::from(recents)));
    // Ids read first: a card asks whether its tool is starred, which borrows the same state.
    let ids: Vec<sheaf_core::tools::ToolId> = with(|h| history::recent_tools(h.store.list(), READ, 6).into_iter().filter_map(sheaf_core::tools::ToolId::from_str).collect());
    let tools: Vec<ToolCard> = ids.into_iter().filter_map(|id| sheaf_core::tools::catalogue().iter().find(|t| t.id == id)).map(crate::tool_card).collect();
    ui.set_recent_tools(ModelRc::new(VecModel::from(tools)));
    for (p, kind) in missing {
        render_thumb(ui, p, kind);
    }
}

fn render_thumb(ui: &AppWindow, p: PathBuf, kind: Kind) {
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let r = match kind {
            Kind::Pdf => files::probe_pdf(&Input::new(&p), THUMB_PX).map(|(n, t)| (t, Some(n))),
            _ => files::image_thumb(&p, files::IMAGE_THUMB_PX).map(|t| (t, None)),
        };
        // An unreadable file (e.g. password-protected output) keeps its placeholder.
        let Ok((thumb, pages)) = r else { return };
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let key = thumb_key(&p);
            with(|h| h.thumbs.insert(key, Some((crate::state::to_image(thumb), pages))));
            refresh(&ui);
        });
    });
}

fn recent(i: i32) -> Option<PathBuf> {
    with(|h| h.recents.get(i as usize).cloned())
}

/// A recent PDF opens in the reader; other files in the app the system picks for them.
pub fn open_recent(ui: &AppWindow, i: i32) {
    if let Some(p) = recent(i) {
        if files::kind_of(&p) == Kind::Pdf {
            crate::reader_ui::open(ui, p, None);
        } else {
            crate::run::open_detached(&p);
        }
    }
}

pub fn remove_recent(ui: &AppWindow, i: i32) {
    if let Some(p) = recent(i) {
        let result = with(|h| h.store.remove_recent(&p));
        if let Err(e) = result {
            eprintln!("sheaf: cannot remove recent file: {e}");
            crate::flow::toast(ui, "Could not remove the recent file. Please try again.");
        }
        refresh(ui);
    }
}

pub fn clear_recents(ui: &AppWindow) {
    let result = with(|h| h.store.clear_recents());
    if let Err(e) = result {
        eprintln!("sheaf: cannot clear recents: {e}");
        crate::flow::toast(ui, "Could not clear recent files. Please try again.");
    }
    refresh(ui);
}

pub fn reveal_recent(i: i32) {
    if let Some(p) = recent(i) {
        crate::run::reveal(&p);
    }
}

/// The tool and files of a history entry, when it can run again: the tool is known and every
/// input still exists.
fn rerun_inputs(e: &Entry) -> Option<(ToolId, Vec<PathBuf>)> {
    let tool = ToolId::from_str(&e.tool)?;
    (!e.inputs.is_empty() && e.inputs.iter().all(|p| p.exists())).then(|| (tool, e.inputs.clone()))
}

/// "Run again": a new tab of the entry's tool with its files (options start from the defaults).
pub fn rerun_history(ui: &AppWindow, i: i32) {
    let entry = with(|h| h.store.list().iter().filter(|e| e.tool != READ).nth(i.max(0) as usize).and_then(rerun_inputs));
    match entry {
        Some((tool, inputs)) => crate::flow::open_tool(ui, tool, inputs),
        None => {
            crate::flow::toast(ui, "These files are no longer where they were.");
            refresh(ui);
        }
    }
}

pub fn reveal_history(i: i32) {
    let first = with(|h| h.store.list().iter().filter(|e| e.tool != READ).nth(i.max(0) as usize).and_then(|e| e.outputs.first().cloned()));
    if let Some(p) = first.filter(|p| p.exists()) {
        crate::run::reveal(&p);
    }
}
