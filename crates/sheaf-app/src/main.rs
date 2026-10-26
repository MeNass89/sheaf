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
mod wm_preview;
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
    for (tool, files) in plan.convert {
        flow::open_tool(&ui, tool, files);
    }
    if let Some(msg) = plan.notice {
        flow::toast(&ui, &msg);
    }
}

/// Called after each successful job: stores it in the task history.
pub fn record_history(tool: ToolId, inputs: &[PathBuf], outputs: &[PathBuf], options: Option<&sheaf_app::ToolOptions>) {
    home::record(tool, inputs, outputs, options);
}

fn category_index(c: Category) -> i32 {
    Category::ALL.iter().position(|x| *x == c).unwrap_or(0) as i32
}

/// A tool as a card of the Tools page (and of Home's Recent tools).
pub(crate) fn tool_card(t: &ToolInfo) -> ToolCard {
    ToolCard {
        id: t.id.as_str().into(),
        name: t.name.into(),
        category: category_index(t.category),
        icon: icon_image(t.icon),
        tile: tile_colour(t.id),
        enabled: t.wave == 1,
        favourite: home::is_favourite(t.id.as_str()),
    }
}

/// The Tools page's grid, sections and favourites: built again when a tool is starred.
pub(crate) fn install_catalogue(ui: &AppWindow) {
    let catalogue = tools::catalogue();
    let all: Vec<ToolCard> = catalogue.iter().map(tool_card).collect();
    let sections: Vec<ToolSection> = Category::ALL
        .iter()
        .map(|c| ToolSection {
            title: c.label().into(),
            collapsed: false,
            tools: ModelRc::new(VecModel::from(
                catalogue.iter().filter(|t| t.category == *c).map(tool_card).collect::<Vec<_>>(),
            )),
        })
        .collect();
    let favourites: Vec<ToolCard> = home::favourites().iter().filter_map(|id| ToolId::from_str(id)).filter_map(|id| catalogue.iter().find(|t| t.id == id)).map(tool_card).collect();
    ui.set_favourite_tools(ModelRc::new(VecModel::from(favourites)));
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
    wire_global!(ui, rb, on_set_custom_colour, |ui, id, hex| reader_ui::set_custom_colour(&ui, id, &hex));
    wire_global!(ui, rb, on_set_pen_width, |ui, id, k| reader_ui::set_pen_width(&ui, id, k));
    wire_global!(ui, rb, on_text_format, |ui, id, what, v| reader_ui::text_format(&ui, id, what, v));
    wire_global!(ui, rb, on_text_size_typed, |ui, id, s| reader_ui::text_size_typed(&ui, id, &s));
    wire_global!(ui, rb, on_set_theme, |ui, id, t| reader_ui::set_theme(&ui, id, t));
    rb.on_set_panel(reader_ui::set_panel);
    rb.on_toggle_panel(reader_ui::toggle_panel);
    rb.on_outline_clicked(reader_ui::outline_clicked);
    rb.on_outline_toggled(reader_ui::outline_toggled);
    rb.on_annot_clicked(reader_ui::annot_clicked);
    wire_global!(ui, rb, on_annot_delete, |ui, id, k| reader_ui::annot_delete(&ui, id, k));
    wire_global!(ui, rb, on_reply_delete, |ui, id, page, index| reader_ui::reply_delete(&ui, id, page, index));
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
    wire_global!(ui, rb, on_note_edited, |ui, id, t| reader_ui::note_edited(&ui, id, &t));
    rb.on_note_cancel(reader_ui::note_cancel);
    wire_global!(ui, rb, on_save, |ui, id| reader_ui::save(&ui, id));
    wire_global!(ui, rb, on_save_as, |ui, id| reader_ui::save_as(&ui, id));
    wire_global!(ui, rb, on_undo, |ui, id| reader_ui::undo(&ui, id, false));
    wire_global!(ui, rb, on_redo, |ui, id| reader_ui::undo(&ui, id, true));
    wire_global!(ui, rb, on_delete_selected, |ui, id| reader_ui::delete_selected(&ui, id));
    wire_global!(ui, rb, on_image_action, |ui, id, a| reader_ui::image_action(&ui, id, a));
    wire_global!(ui, rb, on_paste, |ui, id| reader_ui::paste_image(&ui, id));
    wire_global!(ui, rb, on_copy_pages, |ui, id| reader_ui::copy_pages(&ui, id));
    wire_global!(ui, rb, on_stamp_image, |ui, id| reader_ui::stamp_image(&ui, id));
    wire_global!(ui, rb, on_copy_comment, |ui, id, cut| reader_ui::copy_comment(&ui, id, cut));
    rb.on_escape(reader_ui::escape);
    rb.on_select_all(reader_ui::select_all);
    rb.on_history(reader_ui::history);
    wire_global!(ui, rb, on_to_tools, |ui, id| reader_ui::to_tools(&ui, id));
    wire_global!(ui, rb, on_redact, |ui, id| reader_ui::to_redact(&ui, id));
    wire_global!(ui, rb, on_link_urls, |ui, id| reader_ui::link_urls(&ui, id));
    rb.on_field_kind(reader_ui::field_kind);
    rb.on_form_grid(reader_ui::form_grid);
    wire_global!(ui, rb, on_detect_fields, |ui, id| reader_ui::detect_fields(&ui, id));
    wire_global!(ui, rb, on_tab_order, |ui, id, o| reader_ui::tab_order(&ui, id, o));
    wire_global!(ui, rb, on_copy_image, |ui, id, cut| reader_ui::copy_image(&ui, id, cut));
    wire_global!(ui, rb, on_sanitize, |ui, id| reader_ui::sanitize(&ui, id));
    wire_global!(ui, rb, on_field_props, |ui, id, name, tip, req, ro, ml, opts, size, align, font, act, url| reader_ui::field_props(&ui, id, &name, &tip, req, ro, ml, &opts, (&size, align, font), (act, &url)));
    wire_global!(ui, rb, on_field_delete, |ui, id| reader_ui::field_delete(&ui, id));
    wire_global!(ui, rb, on_field_everywhere, |ui, id| reader_ui::field_everywhere(&ui, id));
    wire_global!(ui, rb, on_fields_arrange, |ui, id, how| reader_ui::fields_arrange(&ui, id, how));
    wire_global!(ui, rb, on_fields_nudge, |ui, id, dx, dy| reader_ui::fields_nudge(&ui, id, dx, dy));
    wire_global!(ui, rb, on_nudge, |ui, id, dx, dy| reader_ui::nudge(&ui, id, dx, dy));
    rb.on_link_selection(reader_ui::link_selection);
    rb.on_replace_selection(reader_ui::replace_selection);
    rb.on_insert_after_selection(reader_ui::insert_after_selection);
    rb.on_hover(reader_ui::hover);
    wire_global!(ui, rb, on_close, |ui, id| flow::close_tab(&ui, id));
    wire_global!(ui, rb, on_close_all, |ui, id| flow::close_tabs_around(&ui, id, 3));
    rb.on_over_link(reader_ui::over_link);
    rb.on_thumb_pressed(reader_ui::thumb_pressed);
    wire_global!(ui, rb, on_page_op, |ui, id, op| reader_ui::page_op(&ui, id, op));
    wire_global!(ui, rb, on_panel_resized, |ui, w| reader_ui::panel_resized(&ui, w));
    wire_global!(ui, rb, on_number_pages, |ui, id, style, prefix, start, through| reader_ui::number_pages(&ui, id, style, &prefix, &start, &through));
    wire_global!(ui, rb, on_extract, |ui, id, each, delete| reader_ui::extract(&ui, id, each, delete));
    rb.on_thumb_drag(reader_ui::thumb_drag);
    rb.on_thumb_at(reader_ui::thumb_at);
    rb.on_thumb_released(reader_ui::thumb_released);
    wire_global!(ui, rb, on_field_tab, |ui, id, t, back| reader_ui::field_tab(&ui, id, &t, back));
    wire_global!(ui, rb, on_choice_picked, |ui, id, k| reader_ui::choice_picked(&ui, id, k));
    wire_global!(ui, rb, on_form_op, |ui, id, op| reader_ui::form_op(&ui, id, op));
    wire_global!(ui, rb, on_signature_made, |ui, id, c, w, h, n, ink, pen, font, keep| reader_ui::signature_made(&ui, id, &c, (w, h), &n, (ink, pen), font, keep));
    rb.on_signature_reset(reader_ui::signature_reset);
    rb.on_signature_pick(reader_ui::signature_pick);
    rb.on_signature_preview(reader_ui::signature_preview);
    rb.on_print_open(reader_ui::print_open);
    wire_global!(ui, rb, on_shape_option, |ui, id, w, v| reader_ui::shape_option(&ui, id, w, v));
    wire_global!(ui, rb, on_cycle_spread, |ui, id| reader_ui::cycle_spread(&ui, id));
    wire_global!(ui, rb, on_toggle_rtl, |ui, id| reader_ui::toggle_rtl(&ui, id));
    wire_global!(ui, rb, on_toggle_grid, |ui, id| reader_ui::toggle_grid(&ui, id));
    wire_global!(ui, rb, on_set_spread, |ui, id, m| reader_ui::set_spread(&ui, id, m));
    wire_global!(ui, rb, on_notes_action, |ui, id, w| reader_ui::notes_action(&ui, id, w));
    rb.on_props_open(reader_ui::props_open);
    rb.on_background_open(reader_ui::background_open);
    rb.on_versions_open(reader_ui::versions_open);
    wire_global!(ui, rb, on_zoom_area, |ui, id| reader_ui::zoom_area(&ui, id));
    wire_global!(ui, rb, on_snap_area, |ui, id| reader_ui::snap_area(&ui, id));
    rb.on_auto_scroll(reader_ui::auto_scroll);
    wire_global!(ui, rb, on_version_action, |ui, id, n| reader_ui::version_action(&ui, id, n));
    wire_global!(ui, rb, on_background_apply, |ui, id, c, o, a| reader_ui::background_apply(&ui, id, c, o, a));
    rb.on_notes_view(reader_ui::notes_view);
    rb.on_toggle_links(reader_ui::toggle_links);
    rb.on_toggle_fields(reader_ui::toggle_fields);
    rb.on_annot_reply(reader_ui::annot_reply);
    wire_global!(ui, rb, on_attachment_save, |ui, id, i| reader_ui::attachment_save(&ui, id, i));
    wire_global!(ui, rb, on_attachment_add, |ui, id| reader_ui::attachment_add(&ui, id));
    wire_global!(ui, rb, on_attachment_change, |ui, id, i, to| reader_ui::attachment_change(&ui, id, i, &to));
    wire_global!(ui, rb, on_attachment_open, |ui, id, i| reader_ui::attachment_open(&ui, id, i));
    wire_global!(ui, rb, on_outline_op, |ui, id, row, op, t| reader_ui::outline_op(&ui, id, row, op, &t));
    wire_global!(ui, rb, on_zoom_typed, |ui, id, t| reader_ui::zoom_typed(&ui, id, &t));
    wire_global!(ui, rb, on_annot_status, |ui, id, k, s| reader_ui::annot_status(&ui, id, k, s));
    rb.on_props_cancel(reader_ui::props_cancel);
    wire_global!(ui, rb, on_props_save, |ui, id, t, a, s, k, c, page, layout, zoom, panel| reader_ui::props_save(&ui, id, &t, &a, &s, &k, &c, (&page, layout, zoom, panel)));
    wire_global!(ui, rb, on_open_signed, |ui, id, k| reader_ui::open_signed(&ui, id, k));
    rb.on_trust_signature(reader_ui::trust_signature);
    rb.on_print_cancel(reader_ui::print_cancel);
    wire_global!(ui, rb, on_print, |ui, id, p, c, m, r, f, t, paper, o| reader_ui::print(&ui, id, p, c, m, &r, f, t, paper, o));
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
    vb.on_view_left(viewer_ui::view_left);
    with_ui!(on_strip_viewport, |ui, id, top, h| viewer_ui::strip_viewport(&ui, id, top, h));
    vb.on_pressed(viewer_ui::pressed);
    vb.on_released(viewer_ui::released);
    vb.on_dragged(viewer_ui::dragged);
    vb.on_double_clicked(viewer_ui::double_clicked);
    vb.on_thumb_clicked(viewer_ui::thumb_clicked);
    with_ui!(on_zoom_step, |ui, id, d| viewer_ui::zoom_step(&ui, id, d));
    let weak = ui.as_weak();
    vb.on_zoom_at(move |id, pct, ax, ay, sx| weak.upgrade().map_or(sx, |ui| viewer_ui::zoom_at(&ui, id, pct, ax, ay, sx)));
    let weak = ui.as_weak();
    vb.on_zoom_by(move |id, f, ax, ay, sx| weak.upgrade().map_or(sx, |ui| viewer_ui::zoom_by(&ui, id, f, ax, ay, sx)));
    with_ui!(on_search, |ui, id, text, patterns| viewer_ui::search(&ui, id, &text, &patterns));
    vb.on_toggle_result(viewer_ui::toggle_result);
    vb.on_show_result(viewer_ui::show_result);
    with_ui!(on_mark_page, |ui, id, range| viewer_ui::mark_page(&ui, id, &range));
    vb.on_remove_mark(viewer_ui::remove_mark);
    vb.on_clear_marks(viewer_ui::clear_marks);

    let sb = ui.global::<sheaf_app::SignBridge>();
    sb.on_mode(sign_ui::mode);
    sb.on_typed(|id, name| sign_ui::typed(id, &name));
    sb.on_font(sign_ui::font);
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
    eb.on_colour_typed(|id, hex| edit_ui::colour_typed(id, &hex));
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
    // Installed serif and monospace families for labels shown in their font.
    let (serif, mono) = if cfg!(target_os = "linux") { ("Liberation Serif", "DejaVu Sans Mono") } else { ("Times New Roman", "Courier New") };
    ui.global::<sheaf_app::Theme>().set_serif_family(serif.into());
    ui.global::<sheaf_app::Theme>().set_mono_family(mono.into());
    home::install(&ui, forced);
    install_catalogue(&ui);
    state::install_tabs_model(&ui);

    jobs::set_handlers(
        |_, tab, p| run::on_progress(tab, p),
        run::on_done,
    );

    // Home
    // Open file: any file Sheaf reads, as a drop on the window: PDFs open in the reader, images,
    // Office and HTML files in the tool that makes them PDFs (Acrobat's Open / Create PDF).
    wire!(ui, on_open_file, |ui| {
        use sheaf_core::tools::{find, ToolId};
        let mut exts: Vec<&str> = vec!["pdf"];
        for t in [ToolId::JpgToPdf, ToolId::WordToPdf, ToolId::PowerPointToPdf, ToolId::ExcelToPdf, ToolId::HtmlToPdf] {
            exts.extend(find(t).accepts.iter().copied().filter(|e| !exts.contains(e)).collect::<Vec<_>>());
        }
        let d = dialog::new(&ui).add_filter("PDFs, images and documents", &exts).add_filter("PDF", &["pdf"]);
        dialog::pick_files(&ui, d, |_, paths| deliver(paths));
    });
    // Home ▸ New blank PDF: named and placed first (a document Sheaf opens is a file), then opened.
    wire!(ui, on_new_blank, |ui| {
        let d = dialog::new(&ui).set_file_name("Untitled.pdf").add_filter("PDF", &["pdf"]);
        dialog::save_file(&ui, d, |ui, path| match sheaf_core::pages::blank(sheaf_core::pages::letter_region(sys_locale::get_locale().as_deref()), &path) {
            Ok(p) => deliver(vec![p]),
            Err(e) => flow::toast(ui, &e.to_string()),
        });
    });
    // Home ▸ From clipboard: a picture, else text, becomes a new PDF, named and placed first.
    wire!(ui, on_new_from_clipboard, |ui| {
        use sheaf_core::pages::Pasted;
        let pasted = arboard::Clipboard::new().ok().and_then(|mut c| {
            c.get_image().ok().map(|i| Pasted::Image { width: i.width as u32, height: i.height as u32, rgba: i.bytes.into_owned() }).or_else(|| c.get_text().ok().map(Pasted::Text))
        });
        let Some(pasted) = pasted else { return flow::toast(&ui, "The clipboard holds no picture and no text") };
        let d = dialog::new(&ui).set_file_name("Clipboard.pdf").add_filter("PDF", &["pdf"]);
        dialog::save_file(&ui, d, move |ui, path| match sheaf_core::pages::from_pasted(&pasted, &path) {
            Ok(p) => deliver(vec![p]),
            Err(e) => flow::toast(ui, &e.to_string()),
        });
    });
    // Tools search: found tools, best first (sheaf_core::tools::search).
    let weak = ui.as_weak();
    ui.on_tool_query(move |q| {
        if let Some(ui) = weak.upgrade() {
            let found: Vec<ToolCard> = tools::search(&q).into_iter().map(tool_card).collect();
            ui.set_tool_results(ModelRc::new(VecModel::from(found)));
        }
        q.trim().into()
    });
    wire!(ui, on_star_tool, |ui, id| {
        home::toggle_favourite(&id);
        install_catalogue(&ui);
        home::refresh(&ui);
    });
    wire!(ui, on_open_tool, |ui, id| {
        if let Some(tool) = flow::tool_from_str(&id) {
            let preload = state::with(|a| std::mem::take(&mut a.home_files));
            flow::open_tool(&ui, tool, preload);
        }
    });

    // Tabs
    wire!(ui, on_close_tab, |ui, id| flow::close_tab(&ui, id));
    ui.on_move_tab(flow::move_tab);
    wire!(ui, on_step_tab, |ui, d| flow::step_tab(&ui, d));
    wire!(ui, on_close_tabs_around, |ui, id, which| flow::close_tabs_around(&ui, id, which));
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
    ui.on_place_file(flow::place_file);
    ui.on_sort_files(flow::sort_files);
    wire!(ui, on_launch_answer, |ui, yes| {
        ui.set_launch_name("".into());
        if let Some(p) = state::with(|a| a.pending_launch.take()).filter(|_| yes) {
            run::open_detached(&p);
        }
    });
    ui.on_set_pages(|id, uid, text| flow::set_pages(id, uid, text.as_str()));
    ui.on_open_doc_count(|id| flow::open_documents(id).len() as i32);
    ui.on_open_doc_names(|id| {
        let names: Vec<slint::SharedString> = flow::open_documents(id).iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into()).collect();
        slint::ModelRc::new(slint::VecModel::from(names))
    });
    wire!(ui, on_add_open_doc_to_slot, |ui, id, slot, i| flow::add_open_doc_to_slot(&ui, id, slot, i));
    wire!(ui, on_add_open_docs, |ui, id| flow::add_open_documents(&ui, id));
    ui.on_remove_page(|id, k| flow::page_op(id, k, 0));
    ui.on_move_page(|id, k, d| flow::page_op(id, k, d.signum()));
    wire!(ui, on_show_merge_pages, |ui, id, on| flow::show_merge_pages(&ui, id, on));
    wire!(ui, on_crop_preview_step, |ui, id, d| flow::crop_preview_step(&ui, id, d));
    ui.on_insert_blank(|id| flow::edit_pages(id, |o| o.push(0)));
    wire!(ui, on_pw_unlock, |ui, pw| password::entered(&ui, pw.into()));
    wire!(ui, on_pw_skip, |ui| password::skipped(&ui));

    // Run and result
    wire!(ui, on_run_tool, |ui, id, opts| run::run_tool(&ui, id, opts));
    ui.on_cancel_job(run::cancel);
    wire!(ui, on_open_output, |ui, id, k| run::open_output(&ui, id, k));
    ui.on_reveal_output(run::reveal_output);
    ui.on_open_folder(run::open_folder);
    ui.on_start_over(run::start_over);
    wire!(ui, on_continue_to, |ui, id, tool| run::continue_to(&ui, id, &tool));
    // Recents, history, preferences
    wire!(ui, on_open_recent, |ui, i| home::open_recent(&ui, i));
    ui.on_reveal_recent(home::reveal_recent);
    wire!(ui, on_remove_recent, |ui, i| home::remove_recent(&ui, i));
    wire!(ui, on_clear_recents, |ui| home::clear_recents(&ui));
    ui.on_reveal_history(home::reveal_history);
    wire!(ui, on_rerun_history, |ui, i| home::rerun_history(&ui, i));
    wire!(ui, on_clear_history, |ui| home::clear(&ui));
    wire!(ui, on_page_shown, |ui| home::refresh(&ui));
    wire!(ui, on_set_theme, |ui, m| home::set_theme(&ui, m));
    wire!(ui, on_set_accent, |ui, i| home::set_accent(&ui, i));
    wire!(ui, on_set_backdrop, |ui, i| home::set_backdrop(&ui, i));
    wire!(ui, on_set_output, |ui, m| home::set_output(&ui, m));
    wire!(ui, on_choose_folder, |ui| home::choose_folder(&ui));
    wire!(ui, on_set_open_after, |ui, v| home::set_open_after(&ui, v));
    wire!(ui, on_set_author, |ui, v| home::set_author(&ui, &v));
    wire!(ui, on_set_default_zoom, |ui, z| home::set_default_zoom(&ui, z));
    wire!(ui, on_set_default_spread, |ui, s| home::set_default_spread(&ui, s));
    wire!(ui, on_set_open_first_page, |ui, f| home::set_open_first_page(&ui, f));
    wire!(ui, on_set_stamp_date, |ui, f| home::set_stamp_date(&ui, f));
    let parse = ui.global::<sheaf_app::Parse>();
    parse.on_password_strength(|s| run::password_strength(&s));
    parse.on_mm(|s| run::parse_mm(&s).unwrap_or(0.0) as f32);
    parse.on_colour(|hex, k| {
        let [r, g, b] = run::text_colour(k, &hex).map(|c| (c * 255.0).round() as u8);
        slint::Color::from_rgb_u8(r, g, b)
    });
    parse.on_colour_error(|s| if s.trim().is_empty() || sheaf_core::numbers::parse_hex_colour(&s).is_some() { "".into() } else { "Type six hex digits, like 1F6FEB".into() });
    parse.on_split_cut(|s, after, n| sheaf_core::ranges::toggle_cut(&s, after.max(0) as u32, n.max(1) as u32).into());
    parse.on_split_range_of(|s, p, n| sheaf_core::ranges::range_of(&s, p.max(0) as u32, n.max(1) as u32).map_or(-1, |i| i as i32));
    parse.on_name_matches(|name, q| q.trim().is_empty() || name.to_lowercase().contains(&q.trim().to_lowercase()));
    parse.on_split_count(|s, n| sheaf_core::ranges::parse_ranges(&s, n.max(1) as u32).map_or(-1, |r| r.len() as i32));
    parse.on_file_name_error(|s| dest::file_name_error(&s).unwrap_or_default().into());
    parse.on_custom_text_error(|s| sheaf_core::numbers::custom_text_error(&s).unwrap_or_default().into());
    parse.on_has_code(|set, code| run::has_code(&set, &code));
    parse.on_toggle_code(|set, code| run::toggle_code(&set, &code).into());
    wire_viewer(&ui);
    let ocr = ui.global::<sheaf_app::OcrInfo>();
    let langs: Vec<sheaf_app::OcrLanguage> =
        sheaf_core::ocr::languages().iter().map(|(code, name)| sheaf_app::OcrLanguage { code: (*code).into(), name: (*name).into() }).collect();
    ocr.set_languages(ModelRc::new(VecModel::from(langs)));
    ocr.set_available(sheaf_core::ocr::available());
    ocr.set_default_language(sheaf_core::ocr::default_language(sys_locale::get_locale().as_deref()).into());
    wire_reader(&ui);
    wire!(ui, on_unsaved_answer, |ui, a| flow::unsaved_answer(&ui, a));
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || match weak.upgrade() {
        Some(ui) => flow::close_requested(&ui),
        None => slint::CloseRequestResponse::HideWindow,
    });
    let picked = ui.global::<sheaf_app::Picked>();
    wire_global!(ui, picked, on_preview_watermark, |ui, tab, opts| wm_preview::request(&ui, tab, opts));
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
        winit::event::WindowEvent::HoveredFile(path) => {
            if let Some(ui) = weak.upgrade() {
                let (hint, refused) = flow::drop_hint(&ui, path);
                ui.set_drop_hint(hint.into());
                ui.set_drop_refused(refused);
            }
            EventResult::PreventDefault
        }
        winit::event::WindowEvent::HoveredFileCancelled => {
            if let Some(ui) = weak.upgrade() {
                ui.set_drop_hint("".into());
            }
            EventResult::PreventDefault
        }
        winit::event::WindowEvent::DroppedFile(path) => {
            if let Some(ui) = weak.upgrade() {
                ui.set_drop_hint("".into());
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

    let r = ui.run();
    sheaf_app::files::clear_office_cache();
    r
}
