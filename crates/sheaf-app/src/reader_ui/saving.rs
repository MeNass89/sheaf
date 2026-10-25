//! Saving, Save As, and handing the document to a tool.

use super::*;

/// ⌘S: the edits are written into the file itself, as an incremental update.
pub fn save(ui: &AppWindow, tab_id: i32) {
    commit_open_paragraph(ui, tab_id);
    update(tab_id, |r, _| {
        if r.dirty() {
            r.send(Op::Save { to: None });
        }
    });
}

/// A paragraph being edited is saved as typed (PDF Expert does the same), not as it was; emptied,
/// it is removed. Its text starts as the paragraph's own, so one opened and left alone is unchanged.
fn commit_open_paragraph(ui: &AppWindow, tab_id: i32) {
    let typed = update(tab_id, |r, _| r.draft.as_ref().is_some_and(|d| d.kind == 2).then(|| r.preview_text.clone())).flatten();
    if let Some(text) = typed {
        super::note_commit(ui, tab_id, &text);
    }
}

/// ⇧⌘S: a full copy with the edits, under a new name; the tab then shows that file.
pub fn save_as(ui: &AppWindow, tab_id: i32) {
    commit_open_paragraph(ui, tab_id);
    let Some((dir, name)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.name.clone())) else { return };
    let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(name).add_filter("PDF", &["pdf"]);
    crate::dialog::save_file(ui, d, move |ui, target| {
        let canon = std::fs::canonicalize(&target).unwrap_or_else(|_| target.clone());
        if with(|a| a.tabs.iter().any(|t| t.id != tab_id && t.reader_file.as_ref() == Some(&canon))) {
            return crate::flow::toast(ui, "That file is open in another tab: close it first");
        }
        with(|a| {
            if let Some(t) = a.tab(tab_id) {
                t.reader_file = Some(canon);
            }
        });
        update(tab_id, |r, _| r.send(Op::Save { to: Some(target) }));
    });
}

/// The file name of tab `tab_id` when closing it would lose edits.
pub fn unsaved_name(tab_id: i32) -> Option<String> {
    with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).filter(|r| r.dirty()).map(|r| r.name.clone()))
}

/// The document goes to the Tools page, ready for the tool picked. Unsaved edits are saved first,
/// and the file goes to the tools only once that save has succeeded.
pub fn to_tools(ui: &AppWindow, tab_id: i32) {
    if unsaved_name(tab_id).is_some() {
        update(tab_id, |r, _| r.tools_after_save = true);
        save(ui, tab_id);
        return;
    }
    hand_to_tools(ui, tab_id);
}

/// Redact (the edit group's button): the document opens in the Redact tool, saved first as for
/// the Tools button.
pub fn to_redact(ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| r.tool_after_save = Some(sheaf_core::tools::ToolId::Redact));
    to_tools(ui, tab_id);
}

pub(super) fn tools_waiting(tab_id: i32) -> bool {
    with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).is_some_and(|r| r.tools_after_save))
}

pub(super) fn hand_to_tools(ui: &AppWindow, tab_id: i32) {
    let Some((path, tool)) = update(tab_id, |r, _| (r.path.clone(), r.tool_after_save.take())) else { return };
    if let Some(tool) = tool {
        crate::flow::open_tool(ui, tool, vec![path]);
        return;
    }
    with(|a| {
        a.home_files.retain(|p| p != &path);
        a.home_files.push(path);
    });
    ui.set_active_tab(-1);
    ui.set_page(sheaf_app::Page::Tools);
    crate::flow::toast(ui, "Choose a tool for your file");
}

/// Files panel: save attachment `i` where the user says (its own name proposed).
pub fn attachment_save(ui: &AppWindow, tab_id: i32, i: i32) {
    let Some((att, bytes, password, dir)) = update(tab_id, |r, _| {
        let a = r.snap.attachments.get(i.max(0) as usize)?.clone();
        Some((a, r.snap.bytes.clone(), r.password(), crate::dest::parent_dir(&r.path)))
    })
    .flatten() else {
        return;
    };
    let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(&att.name);
    crate::dialog::save_file(ui, d, move |ui, to| {
        let msg = match reader_doc::save_attachment(&bytes, password.as_deref(), &att, &to) {
            Ok(()) => format!("Saved {}", file_name(&to)),
            Err(e) => format!("Not saved: {e}"),
        };
        crate::flow::toast(ui, &msg);
    });
}

/// Files panel: attach a file to the page in view (a file attachment comment at its top left, as
/// Acrobat's Attach File).
pub fn attachment_add(ui: &AppWindow, tab_id: i32) {
    let d = crate::dialog::new(ui);
    crate::dialog::pick_file(ui, d, move |ui, path| {
        let Ok(data) = std::fs::read(&path) else {
            return crate::flow::toast(ui, "That file could not be read");
        };
        let file = file_name(&path);
        let edit = update(tab_id, |_, v| {
            let page = v.current();
            let b = &v.viewer.boxes[page];
            let at = viewer::view_to_pdf(b, b.view_size(), 24.0, 24.0);
            let shape = Shape::Attachment { at: [at.0, at.1], icon: sheaf_core::pdfcraft::engine::AttachIcon::Paperclip, file, data };
            let style = Style::default_for(&shape);
            (page, Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: String::new(), author: author() }))
        });
        if let Some((page, e)) = edit {
            apply(ui, tab_id, e, &[page], None);
        }
    });
}
