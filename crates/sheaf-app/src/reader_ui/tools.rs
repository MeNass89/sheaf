//! Tools, colours, text format, theme and the side panel.

use super::*;

pub fn set_tool(tab_id: i32, tool: i32) {
    update(tab_id, |r, v| {
        r.tool = tool.clamp(SELECT, STAMP);
        r.draft = None;
        r.stroke = None;
        load_blocks(r, v);
    });
}

/// Edit text: the paragraphs of the pages in view are read so they can be outlined.
pub(super) fn load_blocks(r: &mut ReaderUi, v: &ViewerUi) {
    if r.tool == EDIT_TEXT {
        for p in v.viewer.wanted(v.top, v.height) {
            r.blocks(p);
        }
    }
}

/// The view scrolled or resized.
pub fn viewport_changed(ui: &AppWindow, tab_id: i32) {
    keep_fit(ui, tab_id);
    let editing = with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).is_some_and(|r| r.tool == EDIT_TEXT));
    if editing {
        update(tab_id, |r, v| load_blocks(r, v));
    }
}

pub fn set_tool_and_mark(ui: &AppWindow, tab_id: i32, tool: i32) {
    let has_sel = update(tab_id, |r, _| r.sel.is_some()).unwrap_or(false);
    set_tool(tab_id, tool);
    if has_sel && (HIGHLIGHT..=STRIKE).contains(&tool) {
        markup(ui, tab_id, tool);
    }
}

/// The colour of the current tool: highlights and notes take the light palette, strokes the ink one.
/// With an annotation selected, it is recoloured (PDF Expert's colour row does the same).
pub fn set_colour(ui: &AppWindow, tab_id: i32, k: i32) {
    let k = k.clamp(0, 4) as usize;
    let restyle = update(tab_id, |r, _| {
        if let Some(a) = r.selected_annot.and_then(|i| r.annots().get(i)) {
            let light = matches!(a.subtype.as_str(), "Highlight" | "Text");
            let colour = if light { COLOURS[k] } else { INK[k] };
            return Some((a.page, Edit::StyleAnnotation { page: a.page, index: a.index, color: Some(colour), opacity: None, width: None, endings: None }));
        }
        if uses_light(r.tool) {
            r.light = k;
        } else {
            r.ink_colour = k;
        }
        None
    })
    .flatten();
    if let Some((page, e)) = restyle {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// Added text's format: `what` 0 size step (±1 in `v`), 1 family (`v`), 2 bold toggle.
pub fn text_format(tab_id: i32, what: i32, v: i32) {
    update(tab_id, |r, _| match what {
        0 => r.text_size = (r.text_size + v).clamp(0, TEXT_SIZES.len() as i32 - 1),
        1 => r.text_family = v.clamp(0, 2),
        _ => r.text_bold = !r.text_bold,
    });
}

/// Shapes, pen and stamp options: `what` 0 the shape (0 rectangle, 1 oval, 2 line, 3 arrow), 1
/// fill on or off, 2 the next opacity, 3 the stamp (index in `StampKind::ALL`).
pub fn shape_option(tab_id: i32, what: i32, value: i32) {
    update(tab_id, |r, _| match what {
        0 => r.shape_kind = value.clamp(0, 3),
        1 => r.shape_fill = !r.shape_fill,
        3 => r.stamp = (value.max(0) as usize).min(StampKind::ALL.len() - 1),
        _ => r.opacity = (r.opacity + 1) % reader::OPACITIES.len(),
    });
}

/// Pen thickness in points: 0 fine, 1 medium, 2 thick.
pub fn set_pen_width(tab_id: i32, k: i32) {
    update(tab_id, |r, _| r.pen_width = k.clamp(0, 2));
}

pub(super) fn pen_points(k: i32) -> f64 {
    [1.0, 2.0, 4.0][k.clamp(0, 2) as usize]
}

pub(super) fn uses_light(tool: i32) -> bool {
    matches!(tool, SELECT | HIGHLIGHT | NOTE)
}

pub fn set_theme(ui: &AppWindow, tab_id: i32, theme: i32) {
    update(tab_id, |r, v| {
        let t = PageTheme::from_index(theme);
        if t != r.theme {
            r.theme = t;
            v.theme = t;
            v.all_changed();
        }
    });
    crate::home::set_reader_theme(theme);
    crate::viewer_ui::request(ui, tab_id);
}

pub fn set_panel(tab_id: i32, panel: i32) {
    update(tab_id, |r, _| {
        r.panel = panel.clamp(0, 3);
        r.panel_open = true;
    });
}

pub fn toggle_panel(tab_id: i32) {
    update(tab_id, |r, _| r.panel_open = !r.panel_open);
}

/// Outline panel edits (PDF Expert's and Acrobat's bookmarks): `op` 0 bookmark the page in view,
/// 1 start renaming `row`, 2 rename it to `text`, 3 stop renaming, 4 delete `row` and what is under it.
pub fn outline_op(ui: &AppWindow, tab_id: i32, row: i32, op: i32, text: &str) {
    let edit = update(tab_id, |r, v| {
        let path = usize::try_from(row).ok().and_then(|i| r.rows.get(i)).map(|x| x.path.clone());
        match op {
            0 => {
                // At the top level, in page order.
                let page = v.current();
                let index = r.snap.outline.iter().filter(|o| o.page.is_some_and(|p| p <= page)).count();
                Some(Edit::AddBookmark { parent: Vec::new(), index, title: format!("Page {}", label(r, page)), page })
            }
            1 => {
                r.outline_edit = usize::try_from(row).ok();
                None
            }
            2 => {
                r.outline_edit = None;
                let title = text.trim();
                let old = usize::try_from(row).ok().and_then(|i| r.rows.get(i)).map(|x| x.title.clone()).unwrap_or_default();
                let path = path?;
                (!title.is_empty() && title != old).then(|| Edit::RenameBookmark { path, title: title.into() })
            }
            3 => {
                r.outline_edit = None;
                None
            }
            _ => Some(Edit::DeleteBookmark { path: path? }),
        }
    })
    .flatten();
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[], None);
    }
}

pub fn outline_clicked(tab_id: i32, row: i32) {
    update(tab_id, |r, v| {
        if let Some(p) = r.rows.get(row.max(0) as usize).and_then(|x| x.page) {
            jump(r, v, p, None);
        }
    });
}

pub fn outline_toggled(tab_id: i32, row: i32) {
    update(tab_id, |r, _| {
        if let Some(path) = r.rows.get(row.max(0) as usize).map(|x| x.path.clone()) {
            if !r.toggled.remove(&path) {
                r.toggled.insert(path);
            }
            r.rows = reader::outline_rows(&r.snap.outline, &r.toggled);
            r.version += 1;
        }
    });
}

/// Notes panel: `what` 0 the filter, 1 the order.
pub fn notes_view(tab_id: i32, what: i32, value: i32) {
    update(tab_id, |r, _| match what {
        0 => r.notes_filter = value.clamp(0, 4),
        _ => r.notes_sort = value.clamp(0, 3),
    });
}

/// Reply to comment `k` (Notes panel): the note editor opens under it; ↩ sends the reply.
pub fn annot_reply(tab_id: i32, k: i32) {
    update(tab_id, |r, v| {
        let Some(a) = r.annots().get(k.max(0) as usize) else { return };
        let (page, index, rect) = (a.page, a.index, a.rect);
        let size = v.viewer.boxes[page].view_size();
        let [x, y, _, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
        r.selected_annot = Some(k as usize);
        r.draft = Some(Draft { page, at: (x, y + h + 4.0), kind: 7, block: Some((index, String::new(), rect)), field: None });
        r.note_seq += 1;
        v.show_page(page, Some(rect));
    });
}

pub fn annot_clicked(tab_id: i32, k: i32) {
    update(tab_id, |r, v| {
        let k = k.max(0) as usize;
        let Some(a) = r.annots().get(k) else { return };
        let (page, rect) = (a.page, a.rect);
        r.selected_annot = Some(k);
        r.sel = None;
        v.show_page(page, Some(rect));
    });
}
