//! Portfolio composition is a reviewable file list before writing, like Acrobat's Create Portfolio.
use std::{cell::RefCell, path::PathBuf};
use slint::{ComponentHandle, ModelRc, VecModel};
use sheaf_app::{AppWindow, PortfolioBridge, PortfolioRow};

thread_local! { static FILES: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) }; }

fn rows(ui: &AppWindow) {
    let rows = FILES.with(|files| files.borrow().iter().map(|p| PortfolioRow {
        name: p.file_name().unwrap_or_default().to_string_lossy().as_ref().into(),
        detail: format!("{} bytes · {}", std::fs::metadata(p).map_or(0, |m| m.len()), p.display()).into(),
        ..Default::default()
    }).collect::<Vec<_>>());
    ui.global::<PortfolioBridge>().set_files(ModelRc::new(VecModel::from(rows)));
}

pub fn install(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<PortfolioBridge>().on_open(move || {
        if let Some(ui) = weak.upgrade() {
            let b = ui.global::<PortfolioBridge>();
            if b.get_busy() { return; }
            b.set_browsing(false); b.set_visible(true); b.set_status("".into());
            rows(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.global::<PortfolioBridge>().on_browse(move |id| {
        if let Some(ui) = weak.upgrade() { crate::reader_ui::portfolio::browse(&ui, id); }
    });
    let weak = ui.as_weak();
    ui.global::<PortfolioBridge>().on_command(move |what, index| {
        if let Some(ui) = weak.upgrade() { command(&ui, &what, index); }
    });
}

fn command(ui: &AppWindow, what: &str, index: i32) {
    let b = ui.global::<PortfolioBridge>();
    if b.get_busy() { return; }
    if b.get_browsing() {
        let id = b.get_tab();
        if what == "sort" { crate::reader_ui::portfolio::sort(ui, index); }
        if what == "preview-page" { crate::reader_ui::portfolio::preview_page(ui, id, index); }
        if what == "preview-item" { crate::reader_ui::portfolio::preview(ui, id, index); }
        if what == "open-item" { b.set_visible(false); crate::reader_ui::attachment_open(ui, id, index); }
        if what == "save-item" { crate::reader_ui::attachment_save(ui, id, index); }
        return;
    }
    match what {
        "add" => {
            let d = crate::dialog::new(ui).set_title("Add original files to the portfolio");
            crate::dialog::pick_files(ui, d, |ui, files| {
                FILES.with(|s| s.borrow_mut().extend(files)); rows(ui);
            });
        }
        "create" => {
            let title = b.get_title().to_string();
            let files = FILES.with(|s| s.borrow().clone());
            if files.is_empty() { return; }
            let d = crate::dialog::new(ui).set_file_name("Portfolio.pdf").add_filter("PDF Portfolio", &["pdf"]);
            crate::dialog::save_file(ui, d, move |ui, out| {
                ui.global::<PortfolioBridge>().set_busy(true);
                ui.global::<PortfolioBridge>().set_status("Creating portfolio…".into());
                let weak = ui.as_weak();
                std::thread::spawn(move || {
                    let result = sheaf_core::portfolio::create(&files, &title, &out);
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        let b = ui.global::<PortfolioBridge>(); b.set_busy(false);
                        match result {
                            Ok(path) => { b.set_visible(false); crate::reader_ui::open(&ui, path, None); }
                            Err(e) => b.set_status(format!("Portfolio not created: {e}").into()),
                        }
                    });
                });
            });
        }
        "up" | "down" | "remove" => {
            FILES.with(|s| {
                let mut files = s.borrow_mut();
                let Ok(at) = usize::try_from(index) else { return };
                if at >= files.len() { return; }
                if what == "remove" { files.remove(at); }
                else if what == "up" && at > 0 { files.swap(at, at - 1); }
                else if what == "down" && at + 1 < files.len() { files.swap(at, at + 1); }
            });
            rows(ui);
        }
        _ => {},
    }
}
