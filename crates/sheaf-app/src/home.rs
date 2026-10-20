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
use sheaf_app::{store_file, AppWindow, HistoryRow, RecentFile, Theme};
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
    // Modal dialog: no state borrow is held here.
    if let Some(dir) = rfd::FileDialog::new().set_title("Always save to").pick_folder() {
        update_prefs(ui, |p| {
            p.fixed_folder = Some(dir);
            p.output = OutputMode::Fixed;
        });
    }
}

pub fn set_open_after(ui: &AppWindow, v: bool) {
    update_prefs(ui, |p| p.open_folder_after = v);
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
    }
}

/// The newest existing output files, without duplicates, with their entry time.
fn recent_outputs(list: &[Entry]) -> Vec<(PathBuf, u64)> {
    let mut out: Vec<(PathBuf, u64)> = Vec::new();
    for e in list {
        for p in &e.outputs {
            if out.len() < MAX_RECENTS && !out.iter().any(|(q, _)| q == p) && p.is_file() {
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
        let rows: Vec<HistoryRow> = h.store.list().iter().map(|e| row(e, now)).collect();
        let outs = recent_outputs(h.store.list());
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

pub fn open_recent(i: i32) {
    if let Some(p) = recent(i) {
        crate::run::open_detached(&p);
    }
}

pub fn reveal_recent(i: i32) {
    if let Some(p) = recent(i) {
        crate::run::reveal(&p);
    }
}

pub fn reveal_history(i: i32) {
    let first = with(|h| h.store.list().get(i as usize).and_then(|e| e.outputs.first().cloned()));
    if let Some(p) = first.filter(|p| p.exists()) {
        crate::run::reveal(&p);
    }
}
