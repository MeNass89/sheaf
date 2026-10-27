//! Preferences ▸ Printing, Security and Startup on the window (the SettingsBridge global): the
//! print sheet's defaults (iLovePDF's Printer preferences), Acrobat's Enhanced Security with its
//! trusted locations and the web link question, and opening at login (iLovePDF's). The rules
//! themselves are in `sheaf_app::settings`.

use crate::home;
use sheaf_app::settings::{SettingsPrefs, Verdict};
use sheaf_app::{AppWindow, SettingsBridge};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;

thread_local! {
    /// The web link waiting for the question's answer.
    static ASKED: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn strings(v: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(v.into_iter().map(SharedString::from).collect::<Vec<_>>()))
}

/// The sites remembered, as Preferences lists them: allowed first, then blocked.
fn site_rows(s: &SettingsPrefs) -> Vec<String> {
    let t = &s.trust;
    t.allowed_sites.iter().map(|x| format!("{x} (allowed)")).chain(t.blocked_sites.iter().map(|x| format!("{x} (blocked)"))).collect()
}

pub fn push(ui: &AppWindow) {
    let s = home::prefs().settings;
    let g = ui.global::<SettingsBridge>();
    let p = s.print.clamped();
    g.set_print_two_sided(p.two_sided);
    g.set_print_paper(p.paper as i32);
    g.set_print_gray(p.gray);
    g.set_print_comments(!p.no_comments);
    g.set_print_fit(!p.actual_size);
    g.set_print_orientation(p.orientation as i32);
    g.set_print_layout(p.layout as i32);
    g.set_enhanced(s.trust.enhanced);
    g.set_web(s.trust.web as i32);
    g.set_other_apps(s.trust.other_apps);
    g.set_locations(strings(s.trust.locations.iter().map(|l| l.display().to_string())));
    g.set_sites(strings(site_rows(&s)));
    g.set_open_at_login(s.open_at_login);
    g.set_quit_warning(!s.no_quit_warning);
}

fn update(ui: &AppWindow, f: impl FnOnce(&mut SettingsPrefs)) {
    home::update_settings(ui, f);
    push(ui);
}

/// Quitting while a task runs asks first (Preferences ▸ Startup).
pub fn quit_warning() -> bool {
    !home::prefs().settings.no_quit_warning
}

/// A document's web or mail link clicked: opened, asked about, or refused as Preferences ▸
/// Security says (Acrobat's Trust Manager).
pub fn open_web_link(ui: &AppWindow, doc: &std::path::Path, url: &str) {
    match home::prefs().settings.trust.web_link(doc, url) {
        Verdict::Allow => crate::run::open_detached(url),
        Verdict::Block => crate::flow::toast(ui, "Blocked by Preferences ▸ Security: the link was not opened"),
        Verdict::Ask => {
            let g = ui.global::<SettingsBridge>();
            g.set_ask_site(sheaf_app::settings::site(url).unwrap_or_default().into());
            g.set_ask_url(url.into());
            ASKED.with(|a| *a.borrow_mut() = Some(url.to_string()));
        }
    }
}

/// A linked or attached file to open in its own app: true when it may open after the usual
/// question, false when it was opened (trusted document) or refused.
pub fn ask_other_app(ui: &AppWindow, doc: &std::path::Path, file: &std::path::Path) -> bool {
    match home::prefs().settings.trust.other_app(doc) {
        Verdict::Ask => true,
        Verdict::Allow => {
            crate::run::open_detached(file);
            false
        }
        Verdict::Block => {
            crate::flow::toast(ui, "Preferences ▸ Security keeps files that are not PDFs from opening in other apps");
            false
        }
    }
}

pub fn install(ui: &AppWindow) {
    let g = ui.global::<SettingsBridge>();
    let w = ui.as_weak();
    g.on_set_print_defaults(move |two_sided, paper, gray, comments, fit, orientation, layout| {
        let Some(ui) = w.upgrade() else { return };
        update(&ui, |s| {
            s.print = sheaf_app::settings::PrintDefaults {
                two_sided,
                paper: paper.clamp(0, 255) as u8,
                gray,
                no_comments: !comments,
                actual_size: !fit,
                orientation: orientation.clamp(0, 255) as u8,
                layout: layout.clamp(0, 255) as u8,
            }
            .clamped();
        });
    });
    let w = ui.as_weak();
    g.on_set_trust(move |enhanced, web, other_apps| {
        let Some(ui) = w.upgrade() else { return };
        update(&ui, |s| {
            s.trust.enhanced = enhanced;
            s.trust.web = web.clamp(0, 2) as u8;
            s.trust.other_apps = other_apps;
        });
    });
    let w = ui.as_weak();
    g.on_trust_list(move |what, k| {
        let Some(ui) = w.upgrade() else { return };
        let k = usize::try_from(k).unwrap_or(usize::MAX);
        match what {
            0 => crate::dialog::pick_folder(&ui, crate::dialog::new(&ui).set_title("A trusted folder"), |ui, p| update(ui, |s| s.trust.add_location(p))),
            1 => crate::dialog::pick_file(&ui, crate::dialog::new(&ui).set_title("A trusted file").add_filter("PDF", &["pdf"]), |ui, p| update(ui, |s| s.trust.add_location(p))),
            2 => update(&ui, |s| {
                if k < s.trust.locations.len() {
                    s.trust.locations.remove(k);
                }
            }),
            3 => update(&ui, |s| {
                let allowed = s.trust.allowed_sites.len();
                if k < allowed {
                    s.trust.allowed_sites.remove(k);
                } else if k - allowed < s.trust.blocked_sites.len() {
                    s.trust.blocked_sites.remove(k - allowed);
                }
            }),
            _ => {}
        }
    });
    let w = ui.as_weak();
    g.on_link_answer(move |allow, remember| {
        let Some(ui) = w.upgrade() else { return };
        let g = ui.global::<SettingsBridge>();
        g.set_ask_url("".into());
        g.set_ask_site("".into());
        let Some(url) = ASKED.with(|a| a.borrow_mut().take()) else { return };
        if remember {
            update(&ui, |s| s.trust.remember(&url, allow));
        }
        if allow {
            crate::run::open_detached(&url);
        }
    });
    let w = ui.as_weak();
    g.on_set_open_at_login(move |on| {
        let Some(ui) = w.upgrade() else { return };
        let g = ui.global::<SettingsBridge>();
        let done = match (std::env::current_exe(), dirs_home()) {
            (Ok(exe), Some(home_dir)) => sheaf_app::settings::set_open_at_login(&home_dir, &exe, on).map_err(|e| e.to_string()),
            _ => Err("Sheaf could not find itself or your home folder".to_string()),
        };
        match done {
            Ok(()) => {
                g.set_login_error("".into());
                update(&ui, |s| s.open_at_login = on);
            }
            Err(e) => g.set_login_error(format!("The login item could not be changed: {e}").into()),
        }
    });
    let w = ui.as_weak();
    g.on_set_quit_warning(move |on| {
        let Some(ui) = w.upgrade() else { return };
        update(&ui, |s| s.no_quit_warning = !on);
    });
    push(ui);
}

/// The user's home folder (HOME, or USERPROFILE on Windows).
fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from).filter(|p| p.is_absolute())
}
