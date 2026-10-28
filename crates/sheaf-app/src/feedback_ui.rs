//! Help ▸ Report an Issue on the window (the FeedbackBridge global): files attached, the report
//! saved, copied or opened as a new issue in the browser (`sheaf_app::feedback`).

use sheaf_app::feedback::{self, Report};
use sheaf_app::{AppWindow, FeedbackBridge};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;

thread_local! {
    static FILES: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

fn push_files(ui: &AppWindow) {
    let names: Vec<SharedString> = FILES.with(|f| f.borrow().iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into()).collect());
    ui.global::<FeedbackBridge>().set_files(ModelRc::new(VecModel::from(names)));
}

/// The details sent with a report: the app's version and system, and what can run here.
fn details() -> String {
    let office = if sheaf_core::office::find_soffice().is_some() { "found" } else { "not found" };
    let ocr = if sheaf_core::ocr::available() { "available" } else { "not installed" };
    let gpu = if crate::home::prefs().no_gpu { "off" } else { "on" };
    feedback::diagnostics(&[("LibreOffice", office.into()), ("OCR", ocr.into()), ("GPU rendering", gpu.into())])
}

pub fn install(ui: &AppWindow) {
    let g = ui.global::<FeedbackBridge>();
    let w = ui.as_weak();
    g.on_opened(move || {
        let Some(ui) = w.upgrade() else { return };
        FILES.with(|f| f.borrow_mut().clear());
        push_files(&ui);
        let g = ui.global::<FeedbackBridge>();
        g.set_error("".into());
        g.set_diagnostics(details().trim_end().into());
    });
    let w = ui.as_weak();
    g.on_add_files(move || {
        let Some(ui) = w.upgrade() else { return };
        let d = crate::dialog::new(&ui).set_title(sheaf_app::i18n::tr("Files to attach"));
        crate::dialog::pick_files(&ui, d, |ui, picked| {
            FILES.with(|f| {
                let mut f = f.borrow_mut();
                for p in picked {
                    if f.len() < feedback::MAX_FILES && !f.contains(&p) {
                        f.push(p);
                    }
                }
            });
            push_files(ui);
        });
    });
    let w = ui.as_weak();
    g.on_remove_file(move |k| {
        let Some(ui) = w.upgrade() else { return };
        FILES.with(|f| {
            let mut f = f.borrow_mut();
            if let Some(k) = usize::try_from(k).ok().filter(|k| *k < f.len()) {
                f.remove(k);
            }
        });
        push_files(&ui);
    });
    let w = ui.as_weak();
    g.on_act(move |what, message, with_details| {
        let Some(ui) = w.upgrade() else { return };
        let g = ui.global::<FeedbackBridge>();
        let report = Report {
            subject: g.get_subject().clamp(0, 1) as usize,
            message: message.to_string(),
            diagnostics: with_details.then(|| g.get_diagnostics().to_string() + "\n"),
            files: FILES.with(|f| f.borrow().clone()),
        };
        if let Some(p) = report.problem() {
            g.set_error(p.into());
            return;
        }
        g.set_error("".into());
        match what {
            0 => {
                let d = crate::dialog::new(&ui).set_title(sheaf_app::i18n::tr("Save the report")).set_file_name("Sheaf report.zip");
                crate::dialog::save_file(&ui, d, move |ui, to| match report.save_zip(&to) {
                    Ok(()) => {
                        ui.global::<FeedbackBridge>().set_open(false);
                        crate::flow::toast(ui, &sheaf_app::i18n::tr("Report saved"));
                    }
                    Err(e) => ui.global::<FeedbackBridge>().set_error(format!("Not saved: {e}").into()),
                });
            }
            1 => {
                let copied = arboard::Clipboard::new().and_then(|mut c| c.set_text(report.text())).is_ok();
                crate::flow::toast(&ui, &if copied { sheaf_app::i18n::tr("Report copied") } else { sheaf_app::i18n::tr("The clipboard could not be reached") });
            }
            _ => {
                crate::run::open_detached(report.issue_url());
                g.set_open(false);
                if !report.files.is_empty() {
                    crate::flow::toast(&ui, &sheaf_app::i18n::tr("Attach your files on the issue page"));
                }
            }
        }
    });
}
