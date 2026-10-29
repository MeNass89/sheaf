//! The Settings window (DESIGN §5.8): a second window, made the first time ⌘, asks for it and kept
//! hidden between openings, so it reopens on the tab last used.
//!
//! Slint gives each window its own copy of every global, while the rest of Sheaf reads and writes
//! the main window's. So this module mirrors: the main window's preferences and the globals the
//! Settings tabs read are copied into the Settings window (at once after each change made here,
//! and every 250 ms while it is shown, for changes made elsewhere: an AI test's status, a dialog's
//! answer), and each callback of the Settings window is handed to the main window's.

use std::cell::{Cell, RefCell};

use sheaf_app::{AiBridge, AppWindow, PlatformBridge, PluginBridge, Picked, Prefs, SettingsBridge, SettingsWindow, Theme, UpdateBridge};
use slint::ComponentHandle;

thread_local! {
    static WINDOW: RefCell<Option<SettingsWindow>> = const { RefCell::new(None) };
    static TIMER: slint::Timer = slint::Timer::default();
    // The tab shown last, for the next opening.
    static TAB: Cell<i32> = const { Cell::new(0) };
}

/// Copies `from`'s properties into `to`: `copy!(from, to; a, b)` is `to.set_a(from.get_a())`.
macro_rules! copy {
    ($from:expr, $to:expr; $($p:ident),* $(,)?) => {
        paste::paste! { $( $to.[<set_ $p>]($from.[<get_ $p>]()); )* }
    };
}

/// Hands each of the Settings window's callbacks of global `$g` to the main window's, then mirrors
/// the result back: `forward!(ui, sw, G; name(a, b))`.
macro_rules! forward {
    ($ui:expr, $sw:expr, $g:ident; $($cb:ident($($a:ident),*)),* $(,)?) => {
        paste::paste! { $(
            let weak = $ui.as_weak();
            $sw.global::<$g>().[<on_ $cb>](move |$($a),*| {
                let Some(ui) = weak.upgrade() else { return Default::default() };
                let r = ui.global::<$g>().[<invoke_ $cb>]($($a),*);
                sync(&ui);
                r
            });
        )* }
    };
}

/// The same for the preferences, which are the main window's own callbacks.
macro_rules! forward_root {
    ($ui:expr, $sw:expr; $($cb:ident($($a:ident),*)),* $(,)?) => {
        paste::paste! { $(
            let weak = $ui.as_weak();
            $sw.global::<Prefs>().[<on_ $cb>](move |$($a),*| {
                let Some(ui) = weak.upgrade() else { return };
                ui.[<invoke_ $cb>]($($a),*);
                sync(&ui);
            });
        )* }
    };
}

/// ⌘, and the menus: the Settings window shown on `tab` (-1: the last one used), in front.
pub fn install(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_open_settings(move |tab| {
        if let Some(ui) = weak.upgrade() {
            show(&ui, tab);
        }
    });
}

fn show(ui: &AppWindow, tab: i32) {
    let fresh = WINDOW.with(|w| w.borrow().is_none());
    if fresh {
        let Ok(sw) = make(ui) else { return };
        WINDOW.with(|w| *w.borrow_mut() = Some(sw));
    }
    sync(ui);
    with(|sw| {
        sw.set_tab(if tab >= 0 { tab } else { TAB.get() });
        sw.set_opened(sw.get_opened() + 1);
        let visible = sw.window().is_visible();
        let _ = sw.show();
        sw.window().set_size(slint::LogicalSize::new(520.0, sw.get_wanted_height().max(200.0)));
        // Over the main window, centred, a little under its top (no window manager places it on X11
        // without one; on macOS and Windows the system keeps it where it was moved).
        if !visible {
            let main = ui.window().position();
            let scale = ui.window().scale_factor();
            let width = ui.window().size().width as f32 / scale;
            let x = main.x as f32 / scale + (width - 520.0) / 2.0;
            sw.window().set_position(slint::LogicalPosition::new(x.max(0.0), main.y as f32 / scale + 60.0));
        }
    });
    // Changes made elsewhere while it is shown (an AI test's status, a dialog's answer).
    let weak = ui.as_weak();
    TIMER.with(|t| {
        t.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(250), move || {
            let shown = with(|sw| sw.window().is_visible()).unwrap_or(false);
            match weak.upgrade() {
                Some(ui) if shown => sync(&ui),
                _ => TIMER.with(|t| t.stop()),
            }
        });
    });
}

/// `f` on the Settings window, when it was made.
fn with<R>(f: impl FnOnce(&SettingsWindow) -> R) -> Option<R> {
    WINDOW.with(|w| w.borrow().as_ref().map(f))
}

/// The Settings window, its callbacks handed to the main window's.
fn make(ui: &AppWindow) -> Result<SettingsWindow, slint::PlatformError> {
    let sw = SettingsWindow::new()?;
    sw.on_fit(|h| {
        with(|sw| {
            if sw.window().is_visible() {
                sw.window().set_size(slint::LogicalSize::new(520.0, h.max(200.0)));
            }
        });
    });
    sw.on_dismiss(|| {
        with(|sw| {
            TAB.set(sw.get_tab());
            let _ = sw.hide();
        });
    });
    // The window's close button: hidden, remembering its tab.
    sw.window().on_close_requested(|| {
        with(|sw| TAB.set(sw.get_tab()));
        slint::CloseRequestResponse::HideWindow
    });
    forward_root!(ui, sw;
        set_theme(m), set_accent(i), set_backdrop(i), set_output(m), choose_folder(), set_open_after(v),
        set_author(n), set_default_zoom(z), set_default_spread(s), set_open_first_page(f), set_stamp_date(f),
        set_metric(m), set_guide_colour(k), set_gpu(on), set_page_display(w, on), set_smoothing(k, on),
        choose_display_profile(), reset_display_profile(), make_default(), signature_prefs(w, k),
        create_digital_id(n, u, o, e, c, p, q, x),
    );
    forward!(ui, sw, SettingsBridge;
        set_print_quality(q), set_print_defaults(a, b, c, d, e, f, g), set_trust(a, b, c), trust_list(a, b),
        set_open_at_login(on), use_system_display(), set_quit_warning(on),
    );
    forward!(ui, sw, UpdateBridge; check(), set_auto(on));
    forward!(ui, sw, AiBridge; pref(k, v));
    forward!(ui, sw, PluginBridge; set_enabled(id, on), remove(id), add(), open_folder());
    forward!(ui, sw, Picked; tsa_prefs(a, b, c));
    forward!(ui, sw, PlatformBridge;
        set_spell_on(on), set_grammar_on(on), forget_learned(), dictionary_change(l, d), shortcut_query(q),
        reset_shortcut(i), reset_shortcuts(), record_shortcut(i, t, c, a, s, m), set_hide_unavailable(on),
        set_direction(d), set_language(l),
    );
    // Recording a shortcut: the main window's menu bar lets keys through, and knows it records.
    let weak = ui.as_weak();
    sw.global::<PlatformBridge>().on_recording(move |on| {
        if let Some(ui) = weak.upgrade() {
            let g = ui.global::<PlatformBridge>();
            g.set_recording_keys(on);
            g.invoke_recording(on);
            sync(&ui);
        }
    });
    Ok(sw)
}

/// The main window's state copied into the Settings window.
fn sync(ui: &AppWindow) {
    with(|sw| {
        let p = sw.global::<Prefs>();
        copy!(ui, p;
            theme_pref, output_mode, fixed_folder, open_after, author_name, author_default, default_zoom,
            default_spread, open_first_page, stamp_date, metric, guide_colour, gpu, show_boxes,
            physical_numbers, transparency_grid, smooth, display_profile, backdrop_pref, digital_ids,
            trusted_certs,
        );
        let (from, to) = (ui.global::<Theme>(), sw.global::<Theme>());
        copy!(from, to; mode, mac, system_accent, accent_choice, reduce_motion, serif_family, mono_family);
        let (from, to) = (ui.global::<SettingsBridge>(), sw.global::<SettingsBridge>());
        copy!(from, to;
            print_two_sided, print_paper, print_gray, print_comments, print_fit, print_orientation,
            print_quality, print_layout, enhanced, web, other_apps, locations, sites, open_at_login,
            login_error, quit_warning, display_system,
        );
        let (from, to) = (ui.global::<UpdateBridge>(), sw.global::<UpdateBridge>());
        copy!(from, to; auto);
        let (from, to) = (ui.global::<AiBridge>(), sw.global::<AiBridge>());
        copy!(from, to;
            off, providers, pref_provider, pref_model, pref_model_hint, pref_base, pref_base_hint,
            pref_needs_key, pref_has_key, pref_auto_open, pref_status,
        );
        let (from, to) = (ui.global::<PluginBridge>(), sw.global::<PluginBridge>());
        copy!(from, to; rows, broken, folder);
        let (from, to) = (ui.global::<Picked>(), sw.global::<Picked>());
        copy!(from, to; tsa_servers);
        let (from, to) = (ui.global::<PlatformBridge>(), sw.global::<PlatformBridge>());
        copy!(from, to;
            mac, rtl, direction, language, spell_on, spell_dictionary, learned, dictionaries, grammar_on,
            shortcut_rows, shortcut_error, hide_unavailable, unavailable_tools,
        );
    });
}

/// The Settings window's frame, for the self-snapshot hook (debug builds).
pub fn snapshot() -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    with(|sw| sw.window().take_snapshot().ok()).flatten()
}
