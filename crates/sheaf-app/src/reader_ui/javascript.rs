//! Acrobat's JavaScript console on the active reader document. Execution stays on its document
//! thread; the result, console lines, alerts and errors come back alongside the refreshed form.

use super::*;
use sheaf_app::JavaScriptBridge;

pub fn install(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<JavaScriptBridge>().on_open(move |id| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(name) = update(id, |r, _| r.name.clone()) else { return };
        let b = ui.global::<JavaScriptBridge>();
        b.set_tab(id); b.set_document(name.into()); b.set_visible(true); b.set_output("".into());
    });
    let weak = ui.as_weak();
    ui.global::<JavaScriptBridge>().on_run(move || {
        let Some(ui) = weak.upgrade() else { return };
        let b = ui.global::<JavaScriptBridge>();
        if b.get_busy() { return; }
        let source = b.get_source().to_string();
        if source.len() > 1_000_000 { b.set_output("The script exceeds 1 MB. Shorten it before running.".into()); return; }
        let sent = update(b.get_tab(), |r, _| {
            if r.edits_in_flight > 0 || r.structure_pending { return false; }
            r.send(Op::RunJavaScript { source });
            true
        }).unwrap_or(false);
        b.set_busy(sent);
        if !sent { b.set_output("The document is busy or closed. Try again when it is ready.".into()); }
    });
}

pub(super) fn finished(ui: &AppWindow, id: i32, output: String, changed: bool) {
    if changed {
        update(id, |r, _| {
            r.undo_pages.push(Vec::new());
            r.redo_pages.clear();
        });
    }
    let b = ui.global::<JavaScriptBridge>();
    if b.get_tab() != id { return; }
    let previous = b.get_output();
    b.set_output(if previous.is_empty() { output } else { format!("{previous}\n{output}") }.into());
    b.set_busy(false);
}
