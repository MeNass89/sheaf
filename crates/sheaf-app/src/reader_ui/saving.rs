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

impl ReaderUi {
    /// A paragraph open in Edit text whose text is no longer the page's.
    pub(super) fn retyping(&self) -> bool {
        self.draft.as_ref().is_some_and(|d| d.kind == 2 && d.block.as_ref().is_some_and(|b| !self.preview_text.is_empty() && self.preview_text != b.1.trim()))
    }
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

/// The file name of tab `tab_id` when closing it would lose edits, a paragraph being retyped
/// included (Sol, 25/10: it was lost without a word; Save applies it first).
pub fn unsaved_name(tab_id: i32) -> Option<String> {
    with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).filter(|r| r.dirty() || r.retyping()).map(|r| r.name.clone()))
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

/// Files panel: open attachment `i` (Acrobat's Open attachment). It is written to a private temporary
/// folder; a PDF opens in its own tab, any other file in its app once the user agrees.
pub fn attachment_open(ui: &AppWindow, tab_id: i32, i: i32) {
    let Some((att, bytes, password)) = update(tab_id, |r, _| Some((r.snap.attachments.get(i.max(0) as usize)?.clone(), r.snap.bytes.clone(), r.password()))).flatten() else {
        return;
    };
    // The name stays the file's own (its app shows it); the folder is ours alone.
    let dir = std::env::temp_dir().join(format!("sheaf-attachments-{}", std::process::id()));
    let safe: String = att.name.chars().map(|c| if matches!(c, '/' | '\\' | '\0') { '_' } else { c }).collect();
    // A folder per document tab and attachment: two attached files of one name stay apart, in one
    // document or in two (the second opened the first's tab, Sol 25/10).
    let to = dir.join(format!("{tab_id}-{i}")).join(if safe.trim_matches('.').is_empty() { "attachment".into() } else { safe });
    let written = std::fs::create_dir_all(to.parent().unwrap_or(&dir)).map_err(|e| e.to_string()).and_then(|_| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
        reader_doc::save_attachment(&bytes, password.as_deref(), &att, &to)
    });
    match written {
        Err(e) => crate::flow::toast(ui, &format!("Not opened: {e}")),
        Ok(()) if to.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) => crate::flow::home_files(ui, vec![to]),
        Ok(()) => {
            ui.set_launch_name(file_name(&to).into());
            with(|a| a.pending_launch = Some(to));
        }
    }
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

/// More ▸ Sanitize document (Acrobat's): metadata, scripts and actions, attachments, links, hidden
/// text and layers removed in one step, which ⌘Z undoes until the file is saved.
pub fn sanitize(ui: &AppWindow, tab_id: i32) {
    super::apply(ui, tab_id, Edit::Sanitize, &[], None);
    crate::flow::toast(ui, "Hidden information removed: metadata, scripts, attachments, links, layers (⌘Z undoes it)");
}
