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
    display_transform: Option<std::sync::Arc<sheaf_app::display_color::DisplayTransform>>,
    /// SHEAF_THEME: a debug override that wins over the preference.
    forced_theme: Option<ThemePref>,
    ui: slint::Weak<AppWindow>,
    /// Rendered previews with the page count; `None` = rendering or unreadable.
    thumbs: HashMap<ThumbKey, Option<(Image, Option<u32>)>>,
    /// Recents currently on screen, in model order.
    recents: Vec<PathBuf>,
    /// Home ▸ Recent files: the search typed and the order (0 newest, 1 name, 2 size).
    recent_query: String,
    recent_order: i32,
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
    // The certificates trusted for signing live with the rest of Sheaf's data.
    sheaf_core::pdfcraft::set_trust_dir(dir.join("trusted"));
    let prefs = Prefs::load(&dir);
    let display_transform = if prefs.display_system {
        system_display().map_err(|e| eprintln!("sheaf: {e}; using sRGB")).ok().map(std::sync::Arc::new)
    } else {
        prefs.display_profile.as_deref().and_then(|path| {
            sheaf_app::display_color::DisplayTransform::load(path).map_err(|e| eprintln!("sheaf: {e}; using sRGB")).ok().map(std::sync::Arc::new)
        })
    };
    let h = Home {
        store: Store::open(&dir),
        prefs,
        display_transform,
        dir,
        forced_theme,
        ui: ui.as_weak(),
        thumbs: HashMap::new(),
        recents: Vec::new(),
        recent_query: String::new(),
        recent_order: 0,
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

/// A value typed in a form text field, remembered for Auto-Complete (newest first, once).
pub fn remember_entry(value: &str) {
    let v = value.trim();
    if v.is_empty() || v.len() > 200 {
        return;
    }
    with(|h| {
        h.prefs.form_entries.retain(|e| e != v);
        h.prefs.form_entries.insert(0, v.to_string());
        h.prefs.form_entries.truncate(sheaf_app::prefs::ENTRIES);
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

/// More ▸ Reading speed: normal, then slow, then fast, remembered. Returns the new one.
/// Read aloud's voice, kept (None: the system's default).
pub fn set_reading_voice(voice: Option<String>) {
    with(|h| {
        h.prefs.reading_voice = voice;
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
}

pub fn cycle_reading_speed() -> u8 {
    with(|h| {
        h.prefs.reading_speed = (h.prefs.reading_speed + 1) % 3;
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
        h.prefs.reading_speed
    })
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

/// The reader's sixth colour, kept for next time (`light`: the highlights' one).
pub fn set_custom_colour(light: bool, hex: &str) {
    with(|h| {
        let v = Some(hex.trim().trim_start_matches('#').to_uppercase());
        if light { h.prefs.custom_light = v } else { h.prefs.custom_ink = v }
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
        }
    });
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
            if let Some(ui) = h.ui.upgrade() { crate::flow::toast(&ui, &e.to_string()); }
        }
    });
}

/// The initials kept (drawn strokes or typed letters), placed by the Signature tool's Initials.
pub fn set_initials(strokes: Vec<Vec<[f64; 2]>>, name: Option<String>, font: u8) {
    with(|h| {
        let pen = h.prefs.signature_pen;
        h.prefs.initials = (!strokes.is_empty() || name.is_some()).then_some((strokes, name, pen, font));
        if let Err(e) = h.prefs.save(&h.dir) {
            eprintln!("sheaf: cannot save preferences: {e}");
            if let Some(ui) = h.ui.upgrade() { crate::flow::toast(&ui, &e.to_string()); }
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

/// Manage Signatures: persist the complete ordered list, then publish it to the reader.
/// A failed credential write leaves the visible list intact, so a failed deletion is not hidden.
pub fn save_signature_list(list: Vec<sheaf_app::prefs::KeptSignature>) -> Result<(), String> {
    with(|h| {
        let before = h.prefs.clone();
        let first = list.first().cloned().unwrap_or_default();
        h.prefs.signatures = list;
        (h.prefs.signature_strokes, h.prefs.signature_name, h.prefs.signature_pen, h.prefs.signature_font) = first;
        if let Err(error) = h.prefs.save(&h.dir) {
            h.prefs = before;
            return Err(error.to_string());
        }
        Ok(())
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
            if let Some(ui) = h.ui.upgrade() { crate::flow::toast(&ui, &e.to_string()); }
        }
    });
}

/// Key of history entries that only record a document opened in the reader (for Recents):
/// they are not tasks and stay out of Tasks History.
pub const READ: &str = "read";

/// A document opened in the reader joins Recents.
pub fn record_open(path: &Path) {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let e = Entry { tool: READ.into(), inputs: vec![], outputs: vec![p], at_unix: history::now_unix(), options: Default::default() };
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

/// Spelling, shortcuts, tools shown and direction changed and saved (src/platform_ui.rs).
pub fn update_platform(ui: &AppWindow, f: impl FnOnce(&mut sheaf_app::platform::PlatformPrefs)) {
    update_prefs(ui, |p| f(&mut p.platform));
}

/// Preferences ▸ AI changed and saved (src/ai_ui.rs).
pub fn update_ai(ui: &AppWindow, f: impl FnOnce(&mut sheaf_app::ai_store::AiPrefs)) {
    update_prefs(ui, |p| f(&mut p.ai));
}

/// The interface's language changed: the texts made in Rust (tool names, sections) built again.
pub fn language_changed(ui: &AppWindow) {
    crate::install_catalogue(ui);
    refresh(ui);
}

/// Print defaults, Security and Startup changed and saved (src/settings_ui.rs).
pub fn update_settings(ui: &AppWindow, f: impl FnOnce(&mut sheaf_app::settings::SettingsPrefs)) {
    update_prefs(ui, |p| f(&mut p.settings));
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
    ui.set_default_spread(p.default_spread as i32);
    ui.set_open_first_page(p.open_first_page);
    ui.set_smooth(slint::ModelRc::new(slint::VecModel::from([0, 1, 2].map(|k| p.rough & (1 << k) == 0).to_vec())));
    ui.global::<Theme>().set_accent_choice(p.accent.min(5) as i32);
    ui.set_backdrop_pref(p.backdrop.min(4) as i32);
    let rb = ui.global::<sheaf_app::ReaderBridge>();
    rb.set_panel_width(panel_width());
    rb.set_thumb_width(crate::reader_ui::reader_thumb(panel_width()) as f32);
    ui.set_stamp_date(p.stamp_date);
    ui.set_metric(p.metric);
    ui.set_guide_colour(i32::from(p.guide_colour));
    ui.set_gpu(!p.no_gpu);
    let display_name = with(|h| match (&p.display_profile, &h.display_transform) {
        _ if p.display_system && h.display_transform.is_some() => sheaf_app::i18n::tr("The system's display profile"),
        _ if p.display_system => sheaf_app::i18n::tr("sRGB (the system sets no display profile)"),
        (Some(path), Some(_)) => path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        (Some(_), None) => "sRGB (saved ICC unavailable)".into(),
        _ => "sRGB".into(),
    });
    ui.set_display_profile(display_name.into());
    ui.global::<sheaf_app::SettingsBridge>().set_display_system(p.display_system);
    ui.set_show_boxes(p.show_boxes);
    ui.set_digital_ids(ModelRc::new(VecModel::from(p.digital_ids.iter().map(|d| slint::SharedString::from(d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())).collect::<Vec<_>>())));
    ui.set_trusted_certs(ModelRc::new(VecModel::from(
        sheaf_core::pdfcraft::trusted_certificates().iter().map(|c| slint::SharedString::from(format!("{} (until {}-{:02})", c.display_name(), c.not_after.year, c.not_after.month))).collect::<Vec<_>>(),
    )));
    ui.set_physical_numbers(p.physical_numbers);
    ui.set_transparency_grid(p.transparency_grid);
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
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Always save to"));
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

/// Preferences ▸ Dated stamps (0 Acrobat's, 1 day first, 2 ISO).
/// A digital ID signed with kept at the top of Preferences ▸ Signatures's list (at most 10).
pub fn remember_digital_id(ui: &AppWindow, p: &std::path::Path) {
    let p = p.to_path_buf();
    update_prefs(ui, |pr| {
        pr.digital_ids.retain(|q| *q != p);
        pr.digital_ids.insert(0, p);
        pr.digital_ids.truncate(10);
    });
}

/// Preferences ▸ Signatures ▸ Create a digital ID… (Acrobat's Add Digital ID ▸ A new digital
/// ID ▸ New PKCS#12 file): saved where the user chooses, with its certificate (.cer) beside it
/// to send to people who encrypt for them, and kept in the list of IDs.
pub fn create_digital_id(ui: &AppWindow, spec: sheaf_core::digital_id::NewId, password: String, confirm: &str) {
    if password != confirm { return crate::flow::toast(ui, &sheaf_app::i18n::tr("The two passwords differ")); }
    if spec.name.trim().is_empty() { return crate::flow::toast(ui, &sheaf_app::i18n::tr("Type the name the digital ID is issued to")); }
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Save the new digital ID")).set_file_name(format!("{}.p12", spec.name.trim())).add_filter("Digital ID", &["p12", "pfx"]);
    crate::dialog::save_file(ui, d, move |ui, out| {
        crate::flow::toast(ui, &sheaf_app::i18n::tr("Creating the digital ID…"));
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let made = sheaf_core::digital_id::create_file(&spec, &password, &out).and_then(|p12| {
                let id = sheaf_core::pdfcraft::engine::sign::pkcs12::open(&std::fs::read(&p12)?, &password).map_err(|e| sheaf_core::OpdfError::Engine(e.to_string()))?;
                let cer = sheaf_core::digital_id::export_certificate(&id, &p12.with_extension("cer"))?;
                Ok((p12, cer))
            });
            let _ = weak.upgrade_in_event_loop(move |ui| match made {
                Ok((p12, cer)) => {
                    remember_digital_id(&ui, &p12);
                    crate::flow::toast(&ui, &sheaf_app::i18n::trf("Digital ID created: {} (certificate to share: {})", &[&p12.display(), &cer.file_name().unwrap_or_default().to_string_lossy()]));
                }
                Err(e) => crate::flow::toast(&ui, &sheaf_app::i18n::trf("Digital ID not created: {}", &[&e])),
            });
        });
    });
}

/// Preferences ▸ Signatures: `what` 0 a digital ID added from a file, 1 digital ID `k` taken off
/// the list, 2 a trusted certificate added from a file, 3 trusted certificate `k` removed.
pub fn signature_prefs(ui: &AppWindow, what: i32, k: i32) {
    match what {
        0 => {
            let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("A digital ID")).add_filter("Digital ID", &["p12", "pfx"]);
            crate::dialog::pick_file(ui, d, |ui, p| remember_digital_id(ui, &p));
        }
        1 => update_prefs(ui, |pr| {
            if let Some(i) = usize::try_from(k).ok().filter(|i| *i < pr.digital_ids.len()) {
                pr.digital_ids.remove(i);
            }
        }),
        2 => {
            let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("A certificate to trust")).add_filter("Certificate", &["cer", "crt", "der", "pem"]);
            crate::dialog::pick_file(ui, d, |ui, p| {
                let msg = match std::fs::read(&p).ok().and_then(|b| sheaf_core::pdfcraft::certificate_der(&b)) {
                    Some(der) => match sheaf_core::pdfcraft::trust_certificate(&der) {
                        Ok(true) => "Certificate trusted: its signatures validate".to_string(),
                        Ok(false) => "That certificate is trusted already".to_string(),
                        Err(e) => format!("Not trusted: {e}"),
                    },
                    None => "That file holds no certificate".to_string(),
                };
                crate::flow::toast(ui, &msg);
                push_prefs(ui);
            });
        }
        3 => {
            if let Some(c) = usize::try_from(k).ok().and_then(|i| sheaf_core::pdfcraft::trusted_certificates().into_iter().nth(i)) {
                let _ = sheaf_core::pdfcraft::untrust_certificate(&c.raw);
            }
            push_prefs(ui);
        }
        _ => {}
    }
}

/// Preferences ▸ Page display, read on every refresh: boxes shown, pages by their place.
pub fn show_boxes() -> bool {
    with(|h| h.prefs.show_boxes)
}

pub fn physical_numbers() -> bool {
    with(|h| h.prefs.physical_numbers)
}

pub fn transparency_grid() -> bool {
    with(|h| h.prefs.transparency_grid)
}

/// Preferences ▸ Page display: `what` 0 the boxes shown, 1 pages called by their place, 2 the
/// transparency grid.
pub fn set_page_display(ui: &AppWindow, what: i32, on: bool) {
    update_prefs(ui, |p| match what {
        0 => p.show_boxes = on,
        1 => p.physical_numbers = on,
        _ => p.transparency_grid = on,
    });
    crate::viewer_ui::redraw_all(ui);
}

/// Preferences ▸ Rendering ▸ Use the graphics card: kept for the next start, said.
pub fn set_gpu(ui: &AppWindow, on: bool) {
    update_prefs(ui, |p| p.no_gpu = !on);
    crate::flow::toast(ui, &sheaf_app::i18n::tr("Takes effect when Sheaf starts again"));
}

/// Acrobat's display profile changes all open pages without changing PDF bytes.
pub fn display_transform() -> Option<std::sync::Arc<sheaf_app::display_color::DisplayTransform>> {
    with(|h| h.display_transform.clone())
}

pub fn choose_display_profile(ui: &AppWindow) {
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Choose RGB monitor ICC profile")).add_filter("ICC profile", &["icc", "icm"]);
    crate::dialog::pick_file(ui, d, move |ui, path| {
        match sheaf_app::display_color::DisplayTransform::load(&path) {
            Ok(transform) => {
                with(|h| h.display_transform = Some(std::sync::Arc::new(transform)));
                update_prefs(ui, |p| { p.display_profile = Some(path); p.display_system = false; });
                crate::viewer_ui::redraw_all(ui);
            }
            Err(error) => crate::flow::toast(ui, &error),
        }
    });
}

/// The system's display profile as a transform from sRGB.
fn system_display() -> Result<sheaf_app::display_color::DisplayTransform, String> {
    let icc = sheaf_app::display_color::system_profile().ok_or_else(|| "the system sets no display profile".to_string())?;
    sheaf_app::display_color::DisplayTransform::from_bytes(&icc)
}

/// Acrobat's Use display profile: the system's profile, read now and at each start.
pub fn use_system_display(ui: &AppWindow) {
    let t = system_display();
    if let Err(e) = &t {
        crate::flow::toast(ui, &sheaf_app::i18n::trf("No display profile from the system ({}): pages stay in sRGB", &[e]));
    }
    with(|h| h.display_transform = t.ok().map(std::sync::Arc::new));
    update_prefs(ui, |p| { p.display_system = true; p.display_profile = None; });
    crate::viewer_ui::redraw_all(ui);
}

pub fn reset_display_profile(ui: &AppWindow) {
    with(|h| h.display_transform = None);
    update_prefs(ui, |p| { p.display_profile = None; p.display_system = false; });
    crate::viewer_ui::redraw_all(ui);
}

/// Preferences ▸ Guides: their colour, of `reader::GUIDE_COLOURS`.
pub fn set_guide_colour(ui: &AppWindow, k: i32) {
    update_prefs(ui, |p| p.guide_colour = k.clamp(0, sheaf_app::reader::GUIDE_COLOURS.len() as i32 - 1) as u8);
    crate::reader_ui::refresh_all();
}

/// Preferences ▸ Units: the rulers in inches (false) or centimetres (true).
pub fn set_metric(ui: &AppWindow, metric: bool) {
    update_prefs(ui, |p| p.metric = metric);
}

pub fn set_stamp_date(ui: &AppWindow, f: i32) {
    update_prefs(ui, |p| p.stamp_date = f.clamp(0, 2));
}

/// Preferences ▸ Open documents at (0 fit width, 1 fit page, 2 actual size).
/// Preferences ▸ Documents ▸ reopen on the first page or where left.
pub fn set_open_first_page(ui: &AppWindow, first: bool) {
    update_prefs(ui, |p| p.open_first_page = first);
}

/// The pages drawn without smoothing (Preferences ▸ Rendering), read on every render request: no
/// copy of the preferences.
pub fn rough() -> u8 {
    with(|h| h.prefs.rough)
}

/// Preferences ▸ Rendering ▸ Smooth text (0), line art (1) or images (2) ticked or not; every
/// open document's pages are drawn again.
pub fn set_smoothing(ui: &AppWindow, which: i32, on: bool) {
    let bit = 1u8 << which.clamp(0, 2);
    update_prefs(ui, |p| p.rough = if on { p.rough & !bit } else { p.rough | bit });
    crate::viewer_ui::redraw_all(ui);
}

/// Preferences ▸ Documents ▸ pages: one column, two, two with the cover alone.
pub fn set_default_spread(ui: &AppWindow, s: i32) {
    update_prefs(ui, |p| p.default_spread = s.clamp(0, 2) as u8);
}

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
pub fn record(tool: ToolId, inputs: &[PathBuf], outputs: &[PathBuf], options: Option<&sheaf_app::ToolOptions>) {
    let options = options.map(sheaf_app::saved_options::to_json).unwrap_or_default();
    let e = Entry { tool: tool.as_str().into(), inputs: inputs.to_vec(), outputs: outputs.to_vec(), at_unix: history::now_unix(), options };
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

/// Home's Recent files as shown: the starred ones first (in the order starred, those still on disk),
/// then the others; only those whose name holds every word of `query` (any case); in `order` (0
/// newest first, 1 by name, 2 largest first), starred ones kept on top. `size` gives a file's bytes.
pub(crate) fn listed(recent: &[(PathBuf, u64)], pinned: &[PathBuf], query: &str, order: i32, size: impl Fn(&Path) -> u64) -> Vec<(PathBuf, u64, bool)> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let fits = |p: &Path| {
        let n = name(p).to_lowercase();
        words.iter().all(|w| n.contains(w.as_str()))
    };
    let at = |p: &Path| recent.iter().find(|(q, _)| q == p).map_or(0, |e| e.1);
    let mut stars: Vec<(PathBuf, u64, bool)> = pinned.iter().filter(|p| p.is_file() && fits(p)).map(|p| (p.clone(), at(p), true)).collect();
    let mut rest: Vec<(PathBuf, u64, bool)> = recent.iter().filter(|(p, _)| !pinned.contains(p) && fits(p)).map(|(p, t)| (p.clone(), *t, false)).collect();
    // Newest first is the history's own order (and the order starred for starred files).
    for list in [&mut stars, &mut rest] {
        match order {
            1 => list.sort_by_key(|e| name(&e.0).to_lowercase()),
            2 => list.sort_by_key(|e| std::cmp::Reverse(size(&e.0))),
            _ => {}
        }
    }
    stars.extend(rest);
    stars
}

/// A recent file starred or unstarred (Home ▸ Recent files' star).
pub fn pin_recent(ui: &AppWindow, i: i32) {
    if let Some(p) = recent(i) {
        with(|h| {
            match h.prefs.pinned_files.iter().position(|q| *q == p) {
                Some(k) => {
                    h.prefs.pinned_files.remove(k);
                }
                None => h.prefs.pinned_files.push(p),
            }
            if let Err(e) = h.prefs.save(&h.dir) {
                eprintln!("sheaf: cannot save preferences: {e}");
            }
        });
        refresh(ui);
    }
}

/// Home ▸ Recent files' search field and order.
pub fn filter_recents(ui: &AppWindow, query: &str, order: i32) {
    with(|h| {
        h.recent_query = query.to_string();
        h.recent_order = order.clamp(0, 2);
    });
    refresh(ui);
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
        let outs = listed(&recent_outputs(&h.store), &h.prefs.pinned_files, &h.recent_query, h.recent_order, files::file_size);
        let mut missing = Vec::new();
        let recents: Vec<RecentFile> = outs
            .iter()
            .map(|(p, at, pinned)| {
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
                    // A starred file no task or opening remembers any more says it is starred.
                    when: if *at > 0 { history::relative_time(*at, now).into() } else { "Starred".into() },
                    has_thumb: cached.is_some(),
                    thumb: cached.map(|c| c.0).unwrap_or_default(),
                    kind: kind.index(),
                    pinned: *pinned,
                }
            })
            .collect();
        h.recents = outs.into_iter().map(|(p, ..)| p).collect();
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

/// The user guide (`guide/sheaf-guide.md`, made into a PDF by `dev/scripts/make-guide.sh`).
const GUIDE: &[u8] = include_bytes!("../guide/Sheaf Guide.pdf");

/// Help ▸ Sheaf Guide: the guide written to the data folder (again when this version's differs)
/// and opened in a tab, as PDF Expert opens its Welcome document.
pub fn open_guide(ui: &AppWindow) {
    let p = store_file::data_dir().join("Sheaf Guide.pdf");
    if std::fs::read(&p).ok().as_deref() != Some(GUIDE)
        && let Err(e) = store_file::replace(&p, GUIDE)
    {
        crate::flow::toast(ui, &sheaf_app::i18n::trf("The guide could not be opened ({})", &[&e]));
        return;
    }
    crate::reader_ui::open(ui, p, None);
}

pub fn remove_recent(ui: &AppWindow, i: i32) {
    if let Some(p) = recent(i) {
        let result = with(|h| h.store.remove_recent(&p));
        if let Err(e) = result {
            eprintln!("sheaf: cannot remove recent file: {e}");
            crate::flow::toast(ui, &sheaf_app::i18n::tr("Could not remove the recent file. Please try again."));
        }
        refresh(ui);
    }
}

pub fn clear_recents(ui: &AppWindow) {
    let result = with(|h| h.store.clear_recents());
    if let Err(e) = result {
        eprintln!("sheaf: cannot clear recents: {e}");
        crate::flow::toast(ui, &sheaf_app::i18n::tr("Could not clear recent files. Please try again."));
    }
    refresh(ui);
}

pub fn reveal_recent(i: i32) {
    if let Some(p) = recent(i) {
        crate::run::reveal(&p);
    }
}

/// The settings `tool` last ran with, to start it from (iLovePDF's and Acrobat's remembered
/// settings): what belongs to the files it ran on (pages, ranges, order, names, crops, texts typed
/// for them, the certificate file) left at its defaults.
pub fn last_options(tool: ToolId) -> Option<sheaf_app::ToolOptions> {
    let m = with(|h| h.store.list().iter().find(|e| e.tool == tool.as_str() && !e.options.is_empty()).map(|e| e.options.clone()))?;
    let mut o = sheaf_app::saved_options::from_json(&m);
    let d = sheaf_app::ToolOptions::default();
    (o.pages, o.ranges, o.organize_order, o.compare_old_pages, o.compare_new_pages, o.out_name) = (d.pages.clone(), d.ranges.clone(), d.organize_order.clone(), d.compare_old_pages.clone(), d.compare_new_pages.clone(), d.out_name.clone());
    (o.from_page, o.to_page, o.crop_top, o.crop_bottom, o.crop_left, o.crop_right) = (d.from_page.clone(), d.to_page.clone(), d.crop_top.clone(), d.crop_bottom.clone(), d.crop_left.clone(), d.crop_right.clone());
    (o.sign_p12, o.redact_overlay) = (d.sign_p12.clone(), d.redact_overlay.clone());
    // A sheet layout (pages per sheet, booklet) makes another kind of document: it is chosen each
    // time, not carried over (Organize would put four pages on one sheet unasked).
    o.per_sheet = d.per_sheet;
    Some(o)
}

/// The tool, files and options of a history entry, when it can run again: the tool is known and
/// every input still exists (an entry from before options were kept has none).
fn rerun_inputs(e: &Entry) -> Option<(ToolId, Vec<PathBuf>, Option<sheaf_app::ToolOptions>)> {
    let tool = ToolId::from_str(&e.tool)?;
    let options = (!e.options.is_empty()).then(|| sheaf_app::saved_options::from_json(&e.options));
    (!e.inputs.is_empty() && e.inputs.iter().all(|p| p.exists())).then(|| (tool, e.inputs.clone(), options))
}

/// "Run again": a new tab of the entry's tool with its files and the options it ran with.
pub fn rerun_history(ui: &AppWindow, i: i32) {
    let entry = with(|h| h.store.list().iter().filter(|e| e.tool != READ).nth(i.max(0) as usize).and_then(rerun_inputs));
    match entry {
        Some((tool, inputs, options)) => crate::flow::open_tool_with(ui, tool, inputs, options),
        None => {
            crate::flow::toast(ui, &sheaf_app::i18n::tr("These files are no longer where they were."));
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

/// Picture stamps kept for next time (Acrobat's custom stamps), in the data folder's `stamps`:
/// their files, by name.
pub fn stamp_library() -> Vec<PathBuf> {
    let dir = with(|h| h.dir.join("stamps"));
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect()).unwrap_or_default();
    files.sort();
    files
}

/// Keeps a picture stamp in the library under `name` (a name taken gets " (1)"); its file.
pub fn keep_stamp(name: &str, bytes: &[u8]) -> Option<PathBuf> {
    let dir = with(|h| h.dir.join("stamps"));
    std::fs::create_dir_all(&dir).ok()?;
    let to = sheaf_core::paths::unique_path(&dir.join(name));
    std::fs::write(&to, bytes).ok()?;
    Some(to)
}

#[cfg(test)]
mod tests {

    /// The guide's PDF is made from its Markdown: every section of the Markdown is in it, so a
    /// guide edited without `dev/scripts/make-guide.sh` fails here.
    #[test]
    fn the_guide_pdf_holds_every_section_of_its_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("guide.pdf");
        std::fs::write(&pdf, GUIDE).unwrap();
        let text = sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&pdf), &[]).unwrap().join("\n");
        let squash = |s: &str| s.split_whitespace().collect::<String>();
        let text = squash(&text);
        let md = include_str!("../guide/sheaf-guide.md");
        let heads: Vec<&str> = md.lines().filter_map(|l| l.strip_prefix("## ")).collect();
        assert!(heads.len() >= 9, "{heads:?}");
        for h in heads.iter().chain(md.lines().filter(|l| l.starts_with("- **")).map(|l| l.trim_start_matches("- **").split("**").next().unwrap()).collect::<Vec<_>>().iter()) {
            assert!(text.contains(&squash(h)), "{h} is not in the guide's PDF");
        }
    }

    use super::*;

    #[test]
    fn starred_files_come_first_and_the_list_is_searched_and_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let f = |n: &str, bytes: usize| {
            let p = dir.path().join(n);
            std::fs::write(&p, vec![0u8; bytes]).unwrap();
            p
        };
        let (a, b, c) = (f("Report 2026.pdf", 30), f("budget.pdf", 10), f("Annex.pdf", 20));
        let recent = vec![(a.clone(), 30), (b.clone(), 20), (c.clone(), 10)];
        let names = |l: Vec<(PathBuf, u64, bool)>| l.into_iter().map(|e| (name(&e.0), e.2)).collect::<Vec<_>>();
        let size = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
        // Starred first, the rest newest first.
        assert_eq!(names(listed(&recent, std::slice::from_ref(&c), "", 0, size)), [("Annex.pdf".into(), true), ("Report 2026.pdf".into(), false), ("budget.pdf".into(), false)]);
        // By name, any case; by size, largest first.
        assert_eq!(names(listed(&recent, &[], "", 1, size)).iter().map(|e| e.0.as_str()).collect::<Vec<_>>(), ["Annex.pdf", "budget.pdf", "Report 2026.pdf"]);
        assert_eq!(names(listed(&recent, &[], "", 2, size)).iter().map(|e| e.0.as_str()).collect::<Vec<_>>(), ["Report 2026.pdf", "Annex.pdf", "budget.pdf"]);
        // Every word typed, any case.
        assert_eq!(names(listed(&recent, &[], "report 26", 0, size)).len(), 1);
        // A starred file gone from the disk is not listed.
        std::fs::remove_file(&c).unwrap();
        assert!(names(listed(&recent, std::slice::from_ref(&c), "", 0, size)).iter().all(|e| !e.1));
    }
}
