//! Sheaf desktop app entry point: builds the window and wires its callbacks.

// Windows: a GUI app, no console window behind it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod dest;
mod dialog;
mod flow;
mod home;
mod password;
mod run;
mod state;
mod edit_ui;
mod sign_ui;
mod viewer_ui;
mod reader_ui;
mod reader_doc;

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

/// One file or several: PDFs open in the reader, other files wait on Home for a tool.
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

/// Wires a callback of a global that needs the window: `wire_global!(ui, global, on_x, |ui, args..| body)`.
macro_rules! wire_global {
    ($ui:ident, $g:ident, $on:ident, |$w:ident $(, $a:ident)*| $body:expr) => {{
        let weak = $ui.as_weak();
        $g.$on(move |$($a),*| {
            if let Some($w) = weak.upgrade() {
                $body
            }
        });
    }};
}

/// Reader events (the `ReaderBridge` global).
fn wire_reader(ui: &AppWindow) {
    let rb = ui.global::<sheaf_app::ReaderBridge>();
    wire_global!(ui, rb, on_pressed, |ui, id, x, y, n| reader_ui::pressed(&ui, id, x, y, n));
    rb.on_moved(reader_ui::moved);
    wire_global!(ui, rb, on_released, |ui, id, x, y| reader_ui::released(&ui, id, x, y));
    wire_global!(ui, rb, on_set_tool, |ui, id, t| reader_ui::set_tool_and_mark(&ui, id, t));
    wire_global!(ui, rb, on_set_colour, |ui, id, c| reader_ui::set_colour(&ui, id, c));
    rb.on_set_pen_width(reader_ui::set_pen_width);
    rb.on_text_format(reader_ui::text_format);
    wire_global!(ui, rb, on_set_theme, |ui, id, t| reader_ui::set_theme(&ui, id, t));
    rb.on_set_panel(reader_ui::set_panel);
    rb.on_toggle_panel(reader_ui::toggle_panel);
    rb.on_outline_clicked(reader_ui::outline_clicked);
    rb.on_outline_toggled(reader_ui::outline_toggled);
    rb.on_annot_clicked(reader_ui::annot_clicked);
    wire_global!(ui, rb, on_annot_delete, |ui, id, k| reader_ui::annot_delete(&ui, id, k));
    rb.on_go_to_page(|id, t| reader_ui::go_to_page(id, &t));
    rb.on_page_step(reader_ui::page_step);
    rb.on_scroll_by(reader_ui::scroll_by);
    wire_global!(ui, rb, on_zoom_fit, |ui, id, m| reader_ui::zoom_fit(&ui, id, m));
    wire_global!(ui, rb, on_search, |ui, id, q| reader_ui::search(&ui, id, &q));
    rb.on_search_step(reader_ui::search_step);
    rb.on_search_clear(reader_ui::search_clear);
    wire_global!(ui, rb, on_search_options, |ui, id, c, w, q| reader_ui::search_options(&ui, id, c, w, &q));
    rb.on_result_clicked(reader_ui::result_clicked);
    wire_global!(ui, rb, on_markup, |ui, id, k| reader_ui::markup(&ui, id, k));
    wire_global!(ui, rb, on_note_commit, |ui, id, t| reader_ui::note_commit(&ui, id, &t));
    rb.on_note_cancel(reader_ui::note_cancel);
    wire_global!(ui, rb, on_save, |ui, id| reader_ui::save(&ui, id));
    wire_global!(ui, rb, on_save_as, |ui, id| reader_ui::save_as(&ui, id));
    wire_global!(ui, rb, on_undo, |ui, id| reader_ui::undo(&ui, id, false));
    wire_global!(ui, rb, on_redo, |ui, id| reader_ui::undo(&ui, id, true));
    wire_global!(ui, rb, on_delete_selected, |ui, id| reader_ui::delete_selected(&ui, id));
    rb.on_escape(reader_ui::escape);
    rb.on_select_all(reader_ui::select_all);
    rb.on_history(reader_ui::history);
    wire_global!(ui, rb, on_to_tools, |ui, id| reader_ui::to_tools(&ui, id));
    wire_global!(ui, rb, on_close, |ui, id| flow::close_tab(&ui, id));
    rb.on_over_link(reader_ui::over_link);
    rb.on_thumb_pressed(reader_ui::thumb_pressed);
    wire_global!(ui, rb, on_page_op, |ui, id, op| reader_ui::page_op(&ui, id, op));
    rb.on_thumb_drag(reader_ui::thumb_drag);
    rb.on_thumb_at(reader_ui::thumb_at);
    rb.on_thumb_released(reader_ui::thumb_released);
    wire_global!(ui, rb, on_field_tab, |ui, id, t, back| reader_ui::field_tab(&ui, id, &t, back));
    wire_global!(ui, rb, on_choice_picked, |ui, id, k| reader_ui::choice_picked(&ui, id, k));
    wire_global!(ui, rb, on_form_op, |ui, id, op| reader_ui::form_op(&ui, id, op));
    wire_global!(ui, rb, on_signature_made, |ui, id, c, w, h, n| reader_ui::signature_made(&ui, id, &c, w, h, &n));
    rb.on_signature_reset(reader_ui::signature_reset);
    rb.on_print_open(reader_ui::print_open);
    rb.on_shape_option(reader_ui::shape_option);
    wire_global!(ui, rb, on_cycle_spread, |ui, id| reader_ui::cycle_spread(&ui, id));
    wire_global!(ui, rb, on_notes_action, |ui, id, w| reader_ui::notes_action(&ui, id, w));
    rb.on_props_open(reader_ui::props_open);
    rb.on_props_cancel(reader_ui::props_cancel);
    wire_global!(ui, rb, on_props_save, |ui, id, t, a, s, k| reader_ui::props_save(&ui, id, &t, &a, &s, &k));
    rb.on_print_cancel(reader_ui::print_cancel);
    wire_global!(ui, rb, on_print, |ui, id, p, c, m, r, f, t| reader_ui::print(&ui, id, p, c, m, &r, f, t));
    wire_global!(ui, rb, on_thumb_drop, |ui, id, y| reader_ui::thumb_drop(&ui, id, y));
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
        let d = dialog::new(&ui).add_filter("PDF", &["pdf"]);
        dialog::pick_files(&ui, d, flow::home_files);
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
    wire!(ui, on_open_recent, |ui, i| home::open_recent(&ui, i));
    ui.on_reveal_recent(home::reveal_recent);
    ui.on_reveal_history(home::reveal_history);
    wire!(ui, on_clear_history, |ui| home::clear(&ui));
    wire!(ui, on_page_shown, |ui| home::refresh(&ui));
    wire!(ui, on_set_theme, |ui, m| home::set_theme(&ui, m));
    wire!(ui, on_set_output, |ui, m| home::set_output(&ui, m));
    wire!(ui, on_choose_folder, |ui| home::choose_folder(&ui));
    wire!(ui, on_set_open_after, |ui, v| home::set_open_after(&ui, v));
    wire!(ui, on_set_author, |ui, v| home::set_author(&ui, &v));
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
    wire_reader(&ui);
    wire!(ui, on_unsaved_answer, |ui, a| flow::unsaved_answer(&ui, a));
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || match weak.upgrade() {
        Some(ui) => flow::close_requested(&ui),
        None => slint::CloseRequestResponse::HideWindow,
    });
    let picked = ui.global::<sheaf_app::Picked>();
    wire_global!(ui, picked, on_pick_watermark, |ui, tab| {
        let d = dialog::new(&ui).set_title("Choose the watermark image").add_filter("Image", &["png", "jpg", "jpeg"]);
        dialog::pick_file(&ui, d, move |ui, p| {
            let g = ui.global::<sheaf_app::Picked>();
            g.set_tab(tab);
            g.set_watermark(p.to_string_lossy().into_owned().into());
            g.set_watermark_seq(g.get_watermark_seq() + 1);
        });
    });
    wire_global!(ui, picked, on_pick_p12, |ui, tab| {
        let d = dialog::new(&ui).set_title("Choose your certificate").add_filter("Digital ID", &["p12", "pfx"]);
        dialog::pick_file(&ui, d, move |ui, p| {
            let g = ui.global::<sheaf_app::Picked>();
            g.set_tab(tab);
            g.set_p12(p.to_string_lossy().into_owned().into());
            g.set_p12_seq(g.get_p12_seq() + 1);
        });
    });
    ui.on_get_libreoffice(|| {
        run::open_detached("https://www.libreoffice.org/download/");
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
