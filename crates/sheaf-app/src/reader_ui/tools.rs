//! Tools, colours, text format, theme and the side panel.

use super::*;

pub fn set_tool(tab_id: i32, tool: i32) {
    update(tab_id, |r, v| {
        r.tool = tool.clamp(SELECT, LINK);
        // Each tool comes back with the style it was last used with, as PDF Expert's tools do.
        if let Some((c, w, o)) = crate::home::tool_style(r.tool) {
            if uses_light(r.tool) {
                r.light = (c as usize).min(COLOURS.len() - 1);
            } else {
                r.ink_colour = (c as usize).min(INK.len() - 1);
            }
            r.pen_width = (w as i32).clamp(0, 2);
            r.opacity = (o as usize).min(reader::OPACITIES.len() - 1);
        } else {
            // A tool not styled yet starts from its defaults, not from the previous tool's style.
            if uses_light(r.tool) {
                r.light = 0;
            } else {
                r.ink_colour = 0;
            }
            r.pen_width = 1;
            r.opacity = 0;
        }
        r.draft = None;
        r.stroke = None;
        r.selected_image = None;
        r.image_resize = None;
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
        remember_style(r);
        // Picked in the selection bar: it is the colour its Highlight button uses, so the
        // highlighter keeps it (the bar takes the highlighter's saved colour).
        if r.tool == SELECT && r.sel.is_some() {
            let (_, w, o) = crate::home::tool_style(HIGHLIGHT).unwrap_or((0, 1, 0));
            crate::home::set_tool_style(HIGHLIGHT, (k as u8, w, o));
        }
        None
    })
    .flatten();
    if let Some((page, e)) = restyle {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// Text format (added text, text boxes): `what` 0 size step (±1 in `v`, to the next preset), 1
/// family (`v`), 2 bold toggle, 3 italic toggle, 4 alignment (`v`), 5 size typed (`v` points). With a text box selected (the row then shows its
/// format, see [`select_annot`]), it takes the new format at once; a click that changes nothing
/// adds no undo step.
pub fn text_format(ui: &AppWindow, tab_id: i32, what: i32, v: i32) {
    let restyle = update(tab_id, |r, _| {
        match what {
            0 => r.text_pt = step_size(r.text_pt, v),
            5 => r.text_pt = (v as f64).clamp(1.0, 400.0),
            1 => r.text_family = v.clamp(0, 2),
            2 => r.text_bold = !r.text_bold,
            3 => r.text_italic = !r.text_italic,
            _ => r.text_align = v.clamp(0, 2),
        }
        let a = r.selected_annot.and_then(|i| r.annots().get(i)).filter(|a| a.subtype == "FreeText" && !a.fill_sign)?;
        let (font, size, align) = (text_font(r), r.text_pt, r.text_align as u8);
        let now = a.font.unwrap_or(("Helv", 12.0, 0));
        let size = ((what == 0 || what == 5) && size != now.1).then_some(size);
        let align = (align != now.2).then_some(align);
        if font == now.0 && size.is_none() && align.is_none() {
            return None;
        }
        Some((a.page, Edit::SetTextFont { page: a.page, index: a.index, font: font.into(), size, align }))
    })
    .flatten();
    if let Some((page, e)) = restyle {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// A size typed in the format row: its leading number ("16", "16 pt", "10,5"); anything else is
/// ignored.
pub fn text_size_typed(ui: &AppWindow, tab_id: i32, s: &str) {
    if let Some(v) = typed_size(s) {
        text_format(ui, tab_id, 5, v.round() as i32);
    }
}

fn typed_size(s: &str) -> Option<f64> {
    let num: String = s.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();
    num.replace(',', ".").parse::<f64>().ok().filter(|v| *v >= 1.0)
}

/// The size after `pt` one step up (`dir` > 0) or down: the next preset of TEXT_SIZES, past them
/// a quarter more or one point less (1–400 pt).
fn step_size(pt: f64, dir: i32) -> f64 {
    let next = if dir > 0 {
        TEXT_SIZES.iter().copied().find(|s| *s > pt).unwrap_or((pt * 1.25).round())
    } else {
        TEXT_SIZES.iter().rev().copied().find(|s| *s < pt).unwrap_or(pt - 1.0)
    };
    next.clamp(1.0, 400.0)
}

/// The text box fonts by family (Helvetica, Times, Courier), then regular, bold, italic, bold
/// italic, as PDF standard font resource names.
const FONTS: [[&str; 4]; 3] = [["Helv", "HeBo", "HeOb", "HeBO"], ["TiRo", "TiBo", "TiIt", "TiBI"], ["Cour", "CoBo", "CoOb", "CoBO"]];

/// The text box font of the format row, as a PDF standard font resource name ("TiBo").
pub(super) fn text_font(r: &ReaderUi) -> &'static str {
    FONTS[r.text_family.clamp(0, 2) as usize][r.text_bold as usize + 2 * r.text_italic as usize]
}

/// Selects annotation `i`; a text box sets the format row to its font, size and alignment, as PDF
/// Expert's row shows the selected box's format.
pub(super) fn select_annot(r: &mut ReaderUi, i: usize) {
    r.selected_annot = Some(i);
    let Some((font, size, align)) = r.annots().get(i).and_then(|a| a.font) else { return };
    r.text_align = align as i32;
    if let Some((family, style)) = FONTS.iter().enumerate().find_map(|(f, row)| row.iter().position(|n| *n == font).map(|s| (f, s))) {
        r.text_family = family as i32;
        r.text_bold = style % 2 == 1;
        r.text_italic = style >= 2;
    }
    r.text_pt = size;
}

/// Shapes, pen and stamp options: `what` 0 the shape (0 rectangle, 1 oval, 2 line, 3 arrow), 1
/// fill on or off, 2 the next opacity, 3 the stamp (index in `StampKind::ALL`).
pub fn shape_option(tab_id: i32, what: i32, value: i32) {
    update(tab_id, |r, _| match what {
        0 => r.shape_kind = value.clamp(0, 3),
        1 => r.shape_fill = !r.shape_fill,
        3 => r.stamp = (value.max(0) as usize).min(MARKS.len() + StampKind::ALL.len() - 1),
        4 => r.link_visible = !r.link_visible,
        5 => r.link_dashed = !r.link_dashed,
        _ => {
            r.opacity = (r.opacity + 1) % reader::OPACITIES.len();
            remember_style(r);
        }
    });
}

/// Pen thickness in points: 0 fine, 1 medium, 2 thick.
pub fn set_pen_width(tab_id: i32, k: i32) {
    update(tab_id, |r, _| {
        r.pen_width = k.clamp(0, 2);
        remember_style(r);
    });
}

/// The current tool's style, kept for the next time it is picked (and the next session).
fn remember_style(r: &ReaderUi) {
    let colour = if uses_light(r.tool) { r.light } else { r.ink_colour };
    crate::home::set_tool_style(r.tool, (colour as u8, r.pen_width as u8, r.opacity as u8));
}

/// The look of a new link (Acrobat's Link Properties ▸ Appearance): invisible by default, as
/// Acrobat makes them; else a border in the ink colour, 1–3 points, solid or dashed.
pub(super) fn link_style(r: &ReaderUi) -> sheaf_core::pdfcraft::engine::LinkStyle {
    use sheaf_core::pdfcraft::engine::{LinkHighlight, LinkStyle};
    LinkStyle { visible: r.link_visible, color: INK[r.ink_colour], width: f64::from(r.pen_width.clamp(0, 2) + 1), dashed: r.link_dashed, underline: false, highlight: LinkHighlight::Invert }
}

/// Link the web addresses in the document's text that no link covers yet, in the Link tool's look.
pub fn link_urls(ui: &AppWindow, tab_id: i32) {
    let err = update(tab_id, |r, _| {
        if !r.snap.read_only.is_empty() {
            return Some(r.snap.read_only.clone());
        }
        if r.edits_in_flight > 0 {
            return Some("One moment: the last change is still being applied".to_string());
        }
        let style = link_style(r);
        r.send(Op::LinkUrls { style });
        // All pages may change.
        r.undo_pages.push(Vec::new());
        r.redo_pages.clear();
        None
    })
    .flatten();
    if let Some(e) = err {
        crate::flow::toast(ui, &e);
    }
}

/// The selection bar's Link: the selected text's lines become the link's boxes; its target is asked.
pub fn link_selection(tab_id: i32) {
    update(tab_id, |r, v| {
        let Some((page, range)) = r.sel.clone() else { return };
        let Some(t) = r.text(page) else { return };
        let b = &v.viewer.boxes[page];
        let size = b.view_size();
        let rects: Vec<[f64; 4]> = t.line_rects(range).iter().map(|q| viewer::view_rect_to_pdf(b, size, (q[0] as f64, q[1] as f64), (q[2] as f64, q[3] as f64))).collect();
        let Some(first) = rects.first().copied() else { return };
        let union = rects.iter().fold(first, |a, q| [a[0].min(q[0]), a[1].min(q[1]), a[2].max(q[2]), a[3].max(q[3])]);
        let [x, y, _, h] = viewer::pdf_rect_to_view(b, size, union);
        r.link_rects = rects;
        r.link_error.clear();
        r.sel = None;
        r.draft = Some(Draft { page, at: (x, y + h + 6.0), kind: 8, block: Some((usize::MAX, String::new(), union)), field: None });
        r.note_seq += 1;
    });
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
        r.panel = panel.clamp(0, 4);
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
        select_annot(r, k as usize);
        r.draft = Some(Draft { page, at: (x, y + h + 4.0), kind: 7, block: Some((index, String::new(), rect)), field: None });
        r.note_seq += 1;
        v.show_page(page, Some(rect));
    });
}

/// View ▸ Highlight links (⇧⌘L): links tinted on the pages, or not.
pub fn toggle_links(tab_id: i32) {
    update(tab_id, |r, _| r.show_links = !r.show_links);
}

pub fn annot_clicked(tab_id: i32, k: i32) {
    update(tab_id, |r, v| {
        let k = k.max(0) as usize;
        let Some(a) = r.annots().get(k) else { return };
        let (page, rect) = (a.page, a.rect);
        select_annot(r, k);
        r.sel = None;
        v.show_page(page, Some(rect));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_sizes_step_through_the_presets_and_can_be_typed() {
        assert_eq!(step_size(12.0, 1), 14.0);
        assert_eq!(step_size(13.0, 1), 14.0);
        assert_eq!(step_size(13.0, -1), 12.0);
        assert_eq!(step_size(72.0, 1), 90.0);
        assert_eq!(step_size(8.0, -1), 7.0);
        assert_eq!(step_size(400.0, 1), 400.0);
        assert_eq!(step_size(1.0, -1), 1.0);
        assert_eq!(typed_size("16"), Some(16.0));
        assert_eq!(typed_size(" 16 pt"), Some(16.0));
        assert_eq!(typed_size("10,5"), Some(10.5));
        for bad in ["", "pt", "0", "-3", "abc"] {
            assert_eq!(typed_size(bad), None, "{bad:?}");
        }
    }
}
