//! Acrobat's JavaScript console on the active reader document. Execution stays on its document
//! thread; the result, console lines, alerts and errors come back alongside the refreshed form.

use super::*;
use sheaf_app::{JavaScriptBridge, DocumentScriptRow};
use sheaf_core::pdfcraft::engine::js::Request;

pub fn install(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<JavaScriptBridge>().on_open(move |id| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(name) = update(id, |r, _| r.name.clone()) else { return };
        let b = ui.global::<JavaScriptBridge>();
        if b.get_busy() { return; }
        if b.get_tab() != id { b.set_output("".into()); }
        b.set_tab(id); b.set_document(name.into()); b.set_visible(true); b.set_scripts_open(false);
    });
    let weak = ui.as_weak();
    ui.global::<JavaScriptBridge>().on_run(move |selected| {
        let Some(ui) = weak.upgrade() else { return };
        let b = ui.global::<JavaScriptBridge>();
        if b.get_busy() { return; }
        let source = b.get_source().to_string();
        let source = if selected {
            match sheaf_app::javascript_console::selection(&source, b.get_selection_anchor(), b.get_selection_cursor()) { Ok(s) => s.into(), Err(e) => { b.set_output(e.into()); return; } }
        } else { source };
        if source.len() > 1_000_000 { b.set_output("The script exceeds 1 MB. Shorten it before running.".into()); return; }
        let sent = update(b.get_tab(), |r, v| {
            if r.edits_in_flight > 0 || r.structure_pending { return false; }
            r.send(Op::RunJavaScript { source, page: v.current() });
            true
        }).unwrap_or(false);
        b.set_busy(sent);
        if !sent { b.set_output("The document is busy or closed. Try again when it is ready.".into()); }
    });
    let weak = ui.as_weak();
    ui.global::<JavaScriptBridge>().on_script_command(move |what, index| {
        let Some(ui) = weak.upgrade() else { return };
        let b = ui.global::<JavaScriptBridge>();
        if b.get_busy() { return; }
        let id = b.get_tab();
        if what == "list" { b.set_busy(true); if update(id, |r, _| r.send(Op::EditableScripts)).is_none() { b.set_busy(false); } return; }
        if what == "select" {
            use slint::Model;
            if let Some(row) = b.get_scripts().row_data(index as usize) { b.set_script_name(row.name); b.set_script_source(row.source); b.set_script_field(row.field); b.set_script_event(row.event); }
            return;
        }
        if what == "new" { b.set_script_name("".into()); b.set_script_source("".into()); return; }
        let name = b.get_script_name().trim().to_string();
        let source = b.get_script_source().to_string();
        if name.is_empty() || name.len() > 256 || source.len() > 1_000_000 {
            b.set_output("Enter a script name (up to 256 bytes) and source no larger than 1 MB.".into()); return;
        }
        if what == "save" || what == "delete" {
            apply_structure(&ui, id, if b.get_script_field() { Edit::SetFieldScript { name, event: b.get_script_event().to_string(), script: (what == "save").then_some(source) } } else { Edit::SetDocumentScript { name, script: (what == "save").then_some(source) } });
            b.set_busy(true);
            if update(id, |r, _| r.send(Op::EditableScripts)).is_none() { b.set_busy(false); }
            b.set_output("Script change requested. Undo reverses it; save the PDF to keep it.".into());
        }
    });

}

pub(super) fn finished(ui: &AppWindow, id: i32, output: String, changed: bool, requests: Vec<Request>) {
    if changed {
        update(id, |r, _| {
            r.undo_pages.push(Vec::new());
            r.redo_pages.clear();
        });
    }
    let notices = dispatch(ui, id, requests);
    let output = if notices.is_empty() { output } else { format!("{output}\n{}", notices.join("\n")) };
    let b = ui.global::<JavaScriptBridge>();
    if b.get_tab() != id { return; }
    let previous = b.get_output();
    b.set_output(if previous.is_empty() { output } else { format!("{previous}\n{output}") }.into());
    b.set_busy(false);
}

pub(super) fn scripts(ui: &AppWindow, id: i32, scripts: Vec<sheaf_core::pdfcraft::engine::js::ConsoleScript>) {
    let b = ui.global::<JavaScriptBridge>();
    if b.get_tab() != id { return; }
    b.set_busy(false);
    b.set_scripts(ModelRc::new(VecModel::from(scripts.into_iter().map(|script| DocumentScriptRow {
        label: script.event.as_ref().map(|e| format!("Field · {} · {e}", script.name)).unwrap_or_else(|| format!("Document · {}", script.name)).into(),
        name: script.name.into(), source: script.source.into(), field: script.event.is_some(), event: script.event.unwrap_or_else(|| "keystroke".into()).into(),
    }).collect::<Vec<_>>())));
}

fn dispatch(ui: &AppWindow, id: i32, requests: Vec<Request>) -> Vec<String> {
    let mut dialog_open = false;
    let mut notices = Vec::new();
    for request in requests {
        if let Some(notice) = sheaf_app::javascript_console::request_notice(&request) { notices.push(notice); continue; }
        match request {
            Request::GoToPage(page) => {
                update(id, |r, v| {
                    if page < v.viewer.page_count() { jump(r, v, page, None); }
                });
                ui.global::<JavaScriptBridge>().set_visible(false);
                crate::viewer_ui::request(ui, id);
            }
            Request::Focus(name) => {
                update(id, |r, v| {
                    let field = r.snap.fields.iter().enumerate().find(|(_, f)| f.name == name && !f.read_only && (f.kind == 0 || f.kind == 3))
                        .and_then(|(i, f)| f.widgets.first().map(|w| (i, w.0, w.1)));
                    if let Some((i, page, rect)) = field { jump(r, v, page, Some(rect)); forms::open_field(r, v, page, i, rect); }
                });
                ui.global::<JavaScriptBridge>().set_visible(false);
                crate::viewer_ui::request(ui, id);
            }
            Request::Print | Request::SaveAs if !dialog_open => {
                dialog_open = true;
                ui.global::<JavaScriptBridge>().set_visible(false);
                if matches!(request, Request::Print) { print_open(id); } else { save_as(ui, id); }
            }
            Request::Beep => crate::flow::toast(ui, "JavaScript: beep"),
            Request::Print | Request::SaveAs => notices.push("A second dialog request was not opened. Run it separately after closing the first dialog.".into()),
            Request::LaunchUrl(_) | Request::Submit(_) | Request::Reset(_) => {},
        }
    }
    notices
}
