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
    /// A paragraph open in Edit text whose text is no longer the page's: retyped, or emptied (a
    /// deletion; Sol 25/10).
    pub(super) fn retyping(&self) -> bool {
        self.draft.as_ref().is_some_and(|d| d.kind == 2 && d.block.as_ref().is_some_and(|b| self.preview_text.trim() != b.1.trim()))
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

/// More ▸ Page background…'s colours (pale, as Acrobat's background presets) and opacities.
pub const BACKGROUNDS: [[f64; 3]; 6] = [[1.0, 0.973, 0.839], [0.890, 0.941, 1.0], [0.910, 0.969, 0.894], [0.992, 0.906, 0.925], [0.937, 0.937, 0.937], [1.0, 1.0, 1.0]];
pub const BACKGROUND_OPACITY: [f64; 3] = [1.0, 0.75, 0.5];

pub fn background_open(tab_id: i32) {
    update(tab_id, |r, _| r.background_open = true);
}

/// More ▸ Page background… (Acrobat's Edit a PDF ▸ Background): `action` 0 puts the colour behind
/// every page's content, 1 a picture chosen in a dialog, 2 removes the backgrounds the file has,
/// 3 closes the sheet. A new background replaces one Sheaf or Acrobat added before; one undo step.
pub fn background_apply(ui: &AppWindow, tab_id: i32, colour: i32, opacity: i32, action: i32) {
    use sheaf_core::pdfcraft::engine::{Background, MarkFile, MarkKind};
    let Some(pages) = update(tab_id, |r, v| {
        r.background_open = false;
        (0..v.viewer.page_count()).collect::<Vec<usize>>()
    }) else {
        return;
    };
    let settings = Background {
        color: BACKGROUNDS[colour.clamp(0, 5) as usize],
        opacity: BACKGROUND_OPACITY[opacity.clamp(0, 2) as usize],
        ..Background::default()
    };
    match action {
        0 => super::apply(ui, tab_id, Edit::AddBackground { pages, settings, replace: true, file: None }, &[], None),
        1 => {
            let d = crate::dialog::new(ui).set_title("A picture behind the pages").add_filter("Image or PDF", &["png", "jpg", "jpeg", "pdf"]);
            crate::dialog::pick_file(ui, d, move |ui, path| match std::fs::read(&path) {
                Ok(bytes) => {
                    let file = MarkFile { name: file_name(&path), bytes: Arc::new(bytes), page: 0 };
                    super::apply(ui, tab_id, Edit::AddBackground { pages, settings, replace: true, file: Some(file) }, &[], None);
                }
                Err(e) => crate::flow::toast(ui, &format!("{} could not be read: {e}", file_name(&path))),
            });
        }
        2 => super::apply(ui, tab_id, Edit::RemoveMarks { kind: MarkKind::Background }, &[], None),
        _ => {}
    }
}

pub fn versions_open(tab_id: i32) {
    update(tab_id, |r, _| r.versions_open = true);
}

/// More ▸ Versions…: `n` 0 reverts to the file as last saved (its unsaved edits and their undo
/// history gone, Acrobat's File ▸ Revert); `n` ≥ 1 opens version `n` of the saved file (each Save
/// of an incremental update adds one) in a tab of its own, from a copy beside the others in the
/// temporary folder; −1 closes the sheet.
pub fn version_action(ui: &AppWindow, tab_id: i32, n: i32) {
    let name = update(tab_id, |r, _| {
        r.versions_open = false;
        r.name.clone()
    });
    let Some(name) = name else { return };
    match n {
        0 => {
            update(tab_id, |r, _| {
                r.undo_pages.clear();
                r.redo_pages.clear();
                r.selected_annot = None;
                r.send(Op::Revert);
            });
        }
        n if n > 0 => {
            let dir = std::env::temp_dir().join("sheaf-versions");
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return crate::flow::toast(ui, &format!("Version not opened: {e}"));
            }
            let stem = name.trim_end_matches(".pdf").trim_end_matches(".PDF");
            let to = dir.join(format!("{stem} (version {n}).pdf"));
            update(tab_id, |r, _| r.send(Op::SaveVersion { n: n as usize, to }));
        }
        _ => {}
    }
}

/// More ▸ Sanitize document (Acrobat's): metadata, scripts and actions, attachments, links, hidden
/// text and layers removed in one step, which ⌘Z undoes until the file is saved.
pub fn sanitize(ui: &AppWindow, tab_id: i32) {
    // A document that cannot be changed says why (the edit's own refusal), and nothing else.
    let read_only = update(tab_id, |r, _| !r.snap.read_only.is_empty()).unwrap_or(true);
    super::apply(ui, tab_id, Edit::Sanitize, &[], None);
    if !read_only {
        crate::flow::toast(ui, "Hidden information removed: metadata, scripts, attachments, links, layers (⌘Z undoes it)");
    }
}
