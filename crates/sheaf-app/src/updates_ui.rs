//! Check for Updates… on the window (the UpdateBridge global): the check on a worker thread, the
//! dialog's states (checking, up to date, a newer version with its notes, an error said plainly),
//! Download opening the file or the release page in the browser, Skip This Version, and the daily
//! check at launch with its quiet notice. When and what: `sheaf_app::updates`; the request:
//! `sheaf_core::updates`.

use crate::home;
use sheaf_app::updates::{size_text, UpdatePrefs};
use sheaf_app::{AppWindow, UpdateBridge};
use sheaf_core::updates::{self as check, Asset, Outcome, Release, UpdateError};
use slint::ComponentHandle;
use std::cell::{Cell, RefCell};

/// The running version, compared with GitHub's latest release.
const VERSION: &str = env!("CARGO_PKG_VERSION");

thread_local! {
    /// The newer release last found, and its download for this computer.
    static FOUND: RefCell<Option<(Release, Option<Asset>)>> = const { RefCell::new(None) };
    /// The check the dialog waits for: an older one's answer (cancelled, asked again) is dropped.
    static ASKED: Cell<u32> = const { Cell::new(0) };
}

fn update(ui: &AppWindow, f: impl FnOnce(&mut UpdatePrefs)) {
    home::update_settings(ui, |s| f(&mut s.updates));
    ui.global::<UpdateBridge>().set_auto(home::prefs().settings.updates.auto);
}

/// The plain sentence for a failed check.
fn error_text(e: &UpdateError) -> String {
    match e {
        UpdateError::Offline(_) => sheaf_app::i18n::tr("GitHub could not be reached. Check your internet connection, then try again."),
        UpdateError::RateLimited(Some(at)) => {
            let at = chrono::DateTime::from_timestamp(*at as i64, 0).map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default();
            sheaf_app::i18n::trf("GitHub limits how often one network may check. Try again after {}.", &[&at])
        }
        UpdateError::RateLimited(None) => sheaf_app::i18n::tr("GitHub limits how often one network may check. Try again in an hour."),
        UpdateError::Http(status) => sheaf_app::i18n::trf("GitHub answered with error {}. Try again later.", &[status]),
        UpdateError::BadResponse => sheaf_app::i18n::tr("GitHub's answer could not be read. Try again later."),
    }
}

/// The dialog showing a newer release.
fn show_found(ui: &AppWindow, r: &Release, asset: Option<&Asset>) {
    let g = ui.global::<UpdateBridge>();
    g.set_state(2);
    g.set_heading(sheaf_app::i18n::trf("Sheaf {} is available", &[&r.version]).into());
    g.set_message(sheaf_app::i18n::trf("You have version {}.", &[&VERSION]).into());
    g.set_notes(check::notes_text(&r.notes).into());
    g.set_download(
        asset
            .map(|a| match size_text(a.size) {
                s if s.is_empty() => a.name.clone(),
                s => format!("{} ({s})", a.name),
            })
            .unwrap_or_default()
            .into(),
    );
}

/// The dialog showing what a check found.
fn show(ui: &AppWindow, result: Result<Outcome, UpdateError>) {
    let g = ui.global::<UpdateBridge>();
    g.set_notes("".into());
    match result {
        Ok(Outcome::Available(r, asset)) => {
            show_found(ui, &r, asset.as_ref());
            FOUND.with_borrow_mut(|f| *f = Some((r, asset)));
        }
        Ok(Outcome::UpToDate(latest)) => {
            g.set_state(1);
            g.set_heading(sheaf_app::i18n::tr("Sheaf is up to date").into());
            g.set_message(match latest {
                Some(_) => sheaf_app::i18n::trf("Version {} is the latest version.", &[&VERSION]).into(),
                None => sheaf_app::i18n::trf("You have version {}. No release has been published yet.", &[&VERSION]).into(),
            });
        }
        Err(e) => {
            g.set_state(3);
            g.set_heading(sheaf_app::i18n::tr("Could not check for updates").into());
            g.set_message(error_text(&e).into());
        }
    }
}

/// Check for Updates…: the dialog opens at once and the answer comes from a worker thread.
fn check_now(ui: &AppWindow) {
    let seq = ASKED.with(|a| {
        a.set(a.get() + 1);
        a.get()
    });
    let g = ui.global::<UpdateBridge>();
    g.set_notice(false);
    g.set_state(0);
    g.set_heading(sheaf_app::i18n::tr("Checking for updates…").into());
    g.set_message("".into());
    g.set_notes("".into());
    g.set_open(true);
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let result = check::check(VERSION);
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if ASKED.with(Cell::get) == seq && ui.global::<UpdateBridge>().get_open() {
                show(&ui, result);
            }
        });
    });
}

/// The daily check, when it is turned on and due: quiet unless a newer version, not skipped, exists.
fn check_at_launch(ui: &AppWindow) {
    let now = sheaf_app::updates::now();
    if !home::prefs().settings.updates.due(now) {
        return;
    }
    // Counted when it starts, so a crash or no connection does not make it ask at every launch.
    update(ui, |u| u.last_check = Some(now));
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let Ok(outcome) = check::check(VERSION) else { return };
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let Some(r) = home::prefs().settings.updates.tell(&outcome).cloned() else { return };
            let g = ui.global::<UpdateBridge>();
            if g.get_open() {
                return;
            }
            g.set_notice_text(sheaf_app::i18n::trf("Sheaf {} is available.", &[&r.version]).into());
            g.set_notice(true);
            if let Outcome::Available(r, asset) = outcome {
                FOUND.with_borrow_mut(|f| *f = Some((r, asset)));
            }
        });
    });
}

pub fn install(ui: &AppWindow) {
    let g = ui.global::<UpdateBridge>();
    g.set_auto(home::prefs().settings.updates.auto);
    let w = ui.as_weak();
    g.on_check(move || {
        if let Some(ui) = w.upgrade() {
            check_now(&ui);
        }
    });
    let w = ui.as_weak();
    g.on_download_it(move || {
        let Some(ui) = w.upgrade() else { return };
        let Some((r, asset)) = FOUND.with_borrow(Clone::clone) else { return };
        crate::run::open_detached(asset.map_or(r.page, |a| a.url));
        ui.global::<UpdateBridge>().set_open(false);
    });
    let w = ui.as_weak();
    g.on_skip(move || {
        let Some(ui) = w.upgrade() else { return };
        let Some((r, _)) = FOUND.with_borrow(Clone::clone) else { return };
        update(&ui, |u| u.skipped = Some(r.version.to_string()));
        let g = ui.global::<UpdateBridge>();
        g.set_open(false);
        g.set_notice(false);
    });
    let w = ui.as_weak();
    g.on_set_auto(move |on| {
        if let Some(ui) = w.upgrade() {
            update(&ui, |u| u.auto = on);
        }
    });
    let w = ui.as_weak();
    g.on_see_notice(move || {
        let Some(ui) = w.upgrade() else { return };
        let g = ui.global::<UpdateBridge>();
        g.set_notice(false);
        if let Some((r, asset)) = FOUND.with_borrow(Clone::clone) {
            show_found(&ui, &r, asset.as_ref());
            g.set_open(true);
        }
    });
    let w = ui.as_weak();
    g.on_dismiss_notice(move || {
        if let Some(ui) = w.upgrade() {
            ui.global::<UpdateBridge>().set_notice(false);
        }
    });
    check_at_launch(ui);
}
