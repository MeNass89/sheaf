//! Acrobat's custom tool commands operate on the active unsaved document through reader edits.
use super::*;

pub fn apply(ui: &AppWindow, id: i32, step: &sheaf_app::actions::ActionStep) -> Result<(), String> {
    if step.prompt { return Err("This command pauses for settings. Edit it in Actions or use Run on PDFs.".into()); }
    let tool = step.engine().map_err(|e| e.to_string())?;
    let edit = update(id, |r, _| {
        if r.edits_in_flight > 0 || r.structure_pending { return Err("The open document is busy. Try again when it is ready.".into()); }
        sheaf_core::pdfcraft::engine::actions::command_edit(&tool, r.snap.boxes.len()).map_err(|e| e.to_string())
    }).ok_or("Open a PDF before using this command")??;
    if !step.instruction.is_empty() { crate::flow::toast(ui, &step.instruction); }
    apply_structure(ui, id, edit);
    Ok(())
}

/// Capture completed reader edits on the UI thread. Arc bytes remain valid if a tab closes or
/// changes during the batch; opening the file path again would lose those unsaved edits.
pub fn open_inputs() -> Result<Vec<sheaf_app::actions::ActionInput>, String> {
    with(|a| {
        let mut inputs = Vec::new();
        for tab in &a.tabs {
            let Some(reader) = &tab.reader else { continue };
            if reader.edits_in_flight > 0 || reader.structure_pending { return Err("An open PDF is busy. Finish its edit before running the action.".into()); }
            let path = tab.reader_file.clone().unwrap_or_else(|| PathBuf::from(&reader.name));
            inputs.push(sheaf_app::actions::ActionInput { path, bytes: reader.snap.bytes.clone(), password: reader.snap.password.clone() });
        }
        if inputs.is_empty() { return Err("Open at least one PDF in the reader first".into()); }
        Ok(inputs)
    })
}
