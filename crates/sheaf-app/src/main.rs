//! Sheaf desktop app entry point: builds the window and wires its callbacks.

mod dest;
mod flow;
mod home;
mod password;
mod run;
mod state;
mod edit_ui;
mod sign_ui;
mod viewer_ui;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use sheaf_app::icons::{icon_image, tile_colour};
use sheaf_app::open_with::Inbox;
use sheaf_app::prefs::ThemePref;
use sheaf_app::{jobs, AppWindow, ToolCard, ToolSection};
use sheaf_core::tools::{self, Category, ToolId, ToolInfo};
use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, VecModel};

thread_local! {
    /// Files received from outside the window, and the window they go to.
    static INBOX: RefCell<(Inbox, Option<slint::Weak<AppWindow>>)> = RefCell::default();
}

/// Files handed to the app: opened now if the window is ready, after it otherwise.
fn received(paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let now = INBOX.with(|i| i.borrow_mut().0.receive(paths));
    if !now.is_empty() {
        // Out of the AppKit callback: handled like any other event.
        let _ = slint::invoke_from_event_loop(move || deliver(now));
    }
}

/// One file or several: they wait on Home for a tool, like PDFs dropped there.
fn deliver(paths: Vec<PathBuf>) {
    let Some(ui) = INBOX.with(|i| i.borrow().1.as_ref().and_then(|w| w.upgrade())) else { return };
    let plan = sheaf_app::open_with::plan(paths);
    if !plan.open.is_empty() {
        flow::home_files(&ui, plan.open);
    }
    if let Some(msg) = plan.notice {
        flow::toast(&ui, &msg);
    }
}

/// Called after each successful job: stores it in the task history.
pub fn record_history(tool: ToolId, inputs: &[PathBuf], outputs: &[PathBuf]) {
    home::record(tool, inputs, outputs);
}

fn category_index(c: Category) -> i32 {
    Category::ALL.iter().position(|x| *x == c).unwrap_or(0) as i32
}

fn card(t: &ToolInfo) -> ToolCard {
    ToolCard {
        id: t.id.as_str().into(),
        name: t.name.into(),
        category: category_index(t.category),
        icon: icon_image(t.icon),
        tile: tile_colour(t.id),
        enabled: t.wave == 1,
    }
}

fn install_catalogue(ui: &AppWindow) {
    let catalogue = tools::catalogue();
    let all: Vec<ToolCard> = catalogue.iter().map(card).collect();
    let sections: Vec<ToolSection> = Category::ALL
        .iter()
        .map(|c| ToolSection {
            title: c.label().into(),
            collapsed: false,
            tools: ModelRc::new(VecModel::from(
                catalogue.iter().filter(|t| t.category == *c).map(card).collect::<Vec<_>>(),
            )),
        })
        .collect();
    ui.set_all_tools(ModelRc::new(VecModel::from(all)));
    ui.set_sections(ModelRc::from(Rc::new(VecModel::from(sections))));
}

/// Wires a callback that needs the window: `wire!(ui, on_x, |ui, args..| body)`.
macro_rules! wire {
    ($ui:ident, $on:ident, |$w:ident $(, $a:ident)*| $body:expr) => {{
        let weak = $ui.as_weak();
        $ui.$on(move |$($a),*| {
            if let Some($w) = weak.upgrade() {
                $body
            }
        });
    }};
}

/// Page viewer and Redact panel events (the `ViewerBridge` global).
fn wire_viewer(ui: &AppWindow) {
    let vb = ui.global::<sheaf_app::ViewerBridge>();
    macro_rules! with_ui {
        ($on:ident, |$w:ident $(, $a:ident)*| $body:expr) => {{
            let weak = ui.as_weak();
            vb.$on(move |$($a),*| {
                if let Some($w) = weak.upgrade() {
                    $body
                }
            });
        }};
    }
    with_ui!(on_viewport, |ui, id, w, top, h| viewer_ui::viewport(&ui, id, w, top, h));
    with_ui!(on_strip_viewport, |ui, id, top, h| viewer_ui::strip_viewport(&ui, id, top, h));
    vb.on_pressed(viewer_ui::pressed);
    vb.on_released(viewer_ui::released);
    vb.on_thumb_clicked(viewer_ui::thumb_clicked);
    with_ui!(on_zoom_step, |ui, id, d| viewer_ui::zoom_step(&ui, id, d));
    with_ui!(on_search, |ui, id, text, patterns| viewer_ui::search(&ui, id, &text, &patterns));
    vb.on_toggle_result(viewer_ui::toggle_result);
    vb.on_show_result(viewer_ui::show_result);
    with_ui!(on_mark_page, |ui, id| viewer_ui::mark_page(&ui, id));
    vb.on_remove_mark(viewer_ui::remove_mark);
    vb.on_clear_marks(viewer_ui::clear_marks);

    let sb = ui.global::<sheaf_app::SignBridge>();
    sb.on_mode(sign_ui::mode);
    sb.on_typed(|id, name| sign_ui::typed(id, &name));
    sb.on_drawn(|id, c, w, h| sign_ui::drawn(id, &c, w, h));
    sb.on_size(sign_ui::size);
    sb.on_pick_p12(sign_ui::pick_p12);
    let weak = ui.as_weak();
    sb.on_pick_image(move |id| {
        if let Some(ui) = weak.upgrade() {
            sign_ui::pick_image(&ui, id);
        }
    });

    let eb = ui.global::<sheaf_app::EditBridge>();
    eb.on_mode(edit_ui::mode);
    eb.on_text(|id, t| edit_ui::text(id, &t));
    eb.on_size_step(edit_ui::size_step);
    eb.on_colour(edit_ui::colour_pick);
    eb.on_width(edit_ui::width);
    eb.on_remove(edit_ui::remove);
    let weak = ui.as_weak();
    eb.on_pick_image(move |id| {
        if let Some(ui) = weak.upgrade() {
            edit_ui::pick_image(&ui, id);
        }
    });
}

fn main() -> Result<(), slint::PlatformError> {
    use winit::window::Theme as WinitTheme;

    // SHEAF_THEME=dark|light forces the appearance (debug, screenshots) over the preference.
    let forced = match std::env::var("SHEAF_THEME").as_deref() {
        Ok("light") => Some(ThemePref::Light),
        Ok("dark") => Some(ThemePref::Dark),
        _ => None,
    };
    // The window opens in the saved appearance; `home::install` keeps it in sync afterwards.
    let initial = forced.unwrap_or(sheaf_app::prefs::Prefs::load(&sheaf_app::store_file::data_dir()).theme);
    let winit_theme = match initial {
        ThemePref::Light => Some(WinitTheme::Light),
        ThemePref::Dark => Some(WinitTheme::Dark),
        ThemePref::System => None,
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(move |attrs| attrs.with_theme(winit_theme))
        .select()?;

    let ui = AppWindow::new()?;
    home::install(&ui, forced);
    install_catalogue(&ui);
    state::install_tabs_model(&ui);

    jobs::set_handlers(
        |_, tab, p| run::on_progress(tab, p),
        run::on_done,
    );

    // Home
    wire!(ui, on_open_file, |ui| {
        if let Some(paths) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).pick_files() {
            flow::home_files(&ui, paths);
        }
    });
    wire!(ui, on_open_tool, |ui, id| {
        if let Some(tool) = flow::tool_from_str(&id) {
            let preload = state::with(|a| std::mem::take(&mut a.home_files));
            flow::open_tool(&ui, tool, preload);
        }
    });

    // Tabs
    wire!(ui, on_close_tab, |ui, id| flow::close_tab(&ui, id));
    wire!(ui, on_confirm_keep, |ui| {
        state::with(|a| a.pending_close = None);
        ui.set_confirm_visible(false);
    });
    wire!(ui, on_confirm_cancel_job, |ui| {
        ui.set_confirm_visible(false);
        if let Some(id) = state::with(|a| a.pending_close.take()) {
            jobs::cancel(id);
            flow::remove_tab(&ui, id);
        }
    });

    // Files
    wire!(ui, on_add_files, |ui, id| flow::pick_and_add(&ui, id));
    wire!(ui, on_open_slot, |ui, id, slot| flow::pick_for_slot(&ui, id, slot));
    ui.on_swap_slots(flow::swap_compare);
    wire!(ui, on_remove_file, |ui, id, uid| flow::remove_file(&ui, id, uid));
    ui.on_move_file(flow::move_file);
    ui.on_remove_page(|id, k| flow::edit_pages(id, |o| {
        if (k as usize) < o.len() {
            o.remove(k as usize);
        }
    }));
    ui.on_move_page(|id, k, d| flow::edit_pages(id, |o| {
        let j = k + d;
        if k >= 0 && j >= 0 && (j as usize) < o.len() && (k as usize) < o.len() {
            o.swap(k as usize, j as usize);
        }
    }));
    ui.on_insert_blank(|id| flow::edit_pages(id, |o| o.push(0)));
    wire!(ui, on_pw_unlock, |ui, pw| password::entered(&ui, pw.into()));
    wire!(ui, on_pw_skip, |ui| password::skipped(&ui));

    // Run and result
    wire!(ui, on_run_tool, |ui, id, opts| run::run_tool(&ui, id, opts));
    ui.on_cancel_job(run::cancel);
    ui.on_open_output(run::open_output);
    ui.on_reveal_output(run::reveal_output);
    ui.on_open_folder(run::open_folder);
    ui.on_start_over(run::start_over);
    wire!(ui, on_continue_to, |ui, id, tool| run::continue_to(&ui, id, &tool));
    // Recents, history, preferences
    ui.on_open_recent(home::open_recent);
    ui.on_reveal_recent(home::reveal_recent);
    ui.on_reveal_history(home::reveal_history);
    wire!(ui, on_clear_history, |ui| home::clear(&ui));
    wire!(ui, on_page_shown, |ui| home::refresh(&ui));
    wire!(ui, on_set_theme, |ui, m| home::set_theme(&ui, m));
    wire!(ui, on_set_output, |ui, m| home::set_output(&ui, m));
    wire!(ui, on_choose_folder, |ui| home::choose_folder(&ui));
    wire!(ui, on_set_open_after, |ui, v| home::set_open_after(&ui, v));
    let parse = ui.global::<sheaf_app::Parse>();
    parse.on_mm(|s| run::parse_mm(&s).unwrap_or(0.0) as f32);
    parse.on_custom_text_error(|s| sheaf_core::numbers::custom_text_error(&s).unwrap_or_default().into());
    parse.on_has_code(|set, code| run::has_code(&set, &code));
    parse.on_toggle_code(|set, code| run::toggle_code(&set, &code).into());
    wire_viewer(&ui);
    let ocr = ui.global::<sheaf_app::OcrInfo>();
    let langs: Vec<sheaf_app::OcrLanguage> =
        sheaf_core::ocr::languages().iter().map(|(code, name)| sheaf_app::OcrLanguage { code: (*code).into(), name: (*name).into() }).collect();
    ocr.set_languages(ModelRc::new(VecModel::from(langs)));
    ocr.set_available(sheaf_core::ocr::available());
    ui.on_pick_image(|| {
        rfd::FileDialog::new()
            .set_title("Choose the watermark image")
            .add_filter("Image", &["png", "jpg", "jpeg"])
            .pick_file()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into()
    });
    ui.on_get_libreoffice(|| {
        run::open_detached(&["https://www.libreoffice.org/download/"]);
    });

    // Drag and drop from Finder.
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_, ev| match ev {
        winit::event::WindowEvent::DroppedFile(path) => {
            if let Some(ui) = weak.upgrade() {
                flow::dropped(&ui, path.clone());
            }
            EventResult::PreventDefault
        }
        _ => EventResult::Propagate,
    });

    // Files from Finder ("Open With", Dock icon) and the command line: queued until the event
    // loop runs, then sent where a PDF dropped on Home goes.
    // Command-line entries that are not existing files are dropped silently: AppKit-style
    // switches (`-AppleLanguages (fr)`) carry values that are not files.
    let mut args = sheaf_app::open_with::launch_args(std::env::args_os().skip(1));
    args.retain(|p| p.is_file());
    received(sheaf_app::open_with::absolute(args));
    #[cfg(target_os = "macos")]
    sheaf_app::open_with::install_hook(|paths| received(sheaf_app::open_with::absolute(paths)));
    let weak = ui.as_weak();
    INBOX.with(|i| i.borrow_mut().1 = Some(weak));
    let _ = slint::invoke_from_event_loop(|| {
        let queued = INBOX.with(|i| i.borrow_mut().0.set_ready());
        deliver(queued);
    });

    ui.run()
}
