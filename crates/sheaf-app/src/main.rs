//! Sheaf desktop app entry point: builds the window and wires its callbacks.

mod dest;
mod flow;
mod home;
mod password;
mod run;
mod state;

use std::path::PathBuf;
use std::rc::Rc;

use sheaf_app::icons::{icon_image, tile_colour};
use sheaf_app::prefs::ThemePref;
use sheaf_app::{jobs, AppWindow, ToolCard, ToolSection};
use sheaf_core::tools::{self, Category, ToolId, ToolInfo};
use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, VecModel};

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

    // PDFs passed on the command line (`Sheaf file.pdf`) wait on Home for a tool.
    let launch: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.is_file() && sheaf_app::files::ext(p) == "pdf")
        .collect();
    if !launch.is_empty() {
        flow::home_files(&ui, launch);
    }

    ui.run()
}
