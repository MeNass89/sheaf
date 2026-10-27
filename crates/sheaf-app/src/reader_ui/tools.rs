//! Tools, colours, text format, theme and the side panel.

use super::*;

pub fn set_tool(tab_id: i32, tool: i32) {
    update(tab_id, |r, v| {
        r.tool = tool.clamp(SELECT, FORM);
        if r.tool != FORM {
            r.form_sel = None;
            r.form_also.clear();
            r.form_drag = None;
        }
        // Each tool comes back with the style it was last used with, as PDF Expert's tools do.
        if let Some((c, w, o)) = crate::home::tool_style(r.tool) {
            if uses_light(r.tool) {
                r.light = (c as usize).min(CUSTOM);
            } else {
                r.ink_colour = (c as usize).min(CUSTOM);
            }
            r.pen_width = (w as i32).clamp(0, 3);
            r.opacity = (o as usize).min(100);
        } else {
            // A tool not styled yet starts from its defaults, not from the previous tool's style.
            if uses_light(r.tool) {
                r.light = 0;
            } else {
                // Fill & Sign marks start black (Acrobat's); drawing tools start red.
                r.ink_colour = if r.tool == STAMP { 1 } else { 0 };
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
    // Text being typed in place is added before the tool changes, as a click elsewhere adds it
    // (Sol, 26/10: another tool threw it away).
    let typed = update(tab_id, |r, _| r.draft.as_ref().is_some_and(|d| matches!(d.kind, 1 | 3) && d.block.is_none()).then(|| std::mem::take(&mut r.inplace_text))).flatten();
    if let Some(text) = typed.filter(|t| !t.trim().is_empty()) {
        super::note_commit(ui, tab_id, &text);
    }
    let has_sel = update(tab_id, |r, _| r.sel.is_some()).unwrap_or(false);
    set_tool(tab_id, tool);
    if has_sel && (HIGHLIGHT..=STRIKE).contains(&tool) {
        markup(ui, tab_id, tool);
    }
}

/// The colour of the current tool: highlights and notes take the light palette, strokes the ink one.
/// With an annotation selected, it is recoloured (PDF Expert's colour row does the same).
pub fn set_colour(ui: &AppWindow, tab_id: i32, k: i32) {
    let k = k.clamp(0, CUSTOM as i32) as usize;
    let restyle = update(tab_id, |r, _| {
        if let Some(a) = r.selected_annot.and_then(|i| r.annots().get(i)) {
            let light = matches!(a.subtype.as_str(), "Highlight" | "Text");
            let colour = if light { light_at(k) } else { ink_at(k) };
            return Some((a.page, Edit::StyleAnnotation { page: a.page, index: a.index, color: Some(colour), opacity: None, width: None, endings: None }));
        }
        if uses_light(r.tool) {
            r.light = k;
        } else {
            r.ink_colour = k;
        }
        remember_style(r);
        // A paragraph open in Edit text takes the colour (Sol, 25/10: it kept its own).
        if r.draft.as_ref().is_some_and(|d| d.kind == 2) {
            r.para_format |= 1;
        }
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
/// family (`v`), 2 bold toggle, 3 italic toggle, 4 alignment (`v`), 5 size typed (`v` points), 6 underline and 7 bullets
/// toggles, 8 a font of the computer's (`v` in `system_fonts::installed`), 9 that list asked for. With a text box selected (the row then shows its
/// format, see [`select_annot`]), it takes the new format at once; a click that changes nothing
/// adds no undo step.
/// A colour picked from the palette or typed (RRGGBB) becomes the row's sixth and is used at once, for
/// the selected comment or the tool (Acrobat's Other color). Not a colour: nothing changes.
pub fn set_custom_colour(ui: &AppWindow, tab_id: i32, hex: &str) {
    if sheaf_core::numbers::parse_hex_colour(hex).is_none() {
        return;
    }
    let light = update(tab_id, |r, _| match r.selected_annot.and_then(|i| r.annots().get(i)) {
        Some(a) => matches!(a.subtype.as_str(), "Highlight" | "Text"),
        None => uses_light(r.tool),
    });
    let Some(light) = light else { return };
    crate::home::set_custom_colour(light, hex);
    set_colour(ui, tab_id, CUSTOM as i32);
}

pub fn text_format(ui: &AppWindow, tab_id: i32, what: i32, v: i32) {
    // The font list opened: the computer's fonts, read once off the interface thread.
    if what == 9 {
        return list_system_fonts(ui);
    }
    // A font of the computer's (`v` in the list), for the open paragraph: its program read now.
    let picked = if what == 8 {
        let Some(f) = sheaf_core::system_fonts::installed().get(v.max(0) as usize) else { return };
        match std::fs::read(&f.path) {
            Ok(bytes) => Some((f.name.clone(), std::sync::Arc::new(bytes))),
            Err(e) => return crate::flow::toast(ui, &format!("{} can't be read: {e}", f.name)),
        }
    } else {
        None
    };
    let restyle = update(tab_id, |r, _| {
        match what {
            0 => r.text_pt = step_size(r.text_pt, v),
            5 => r.text_pt = (v as f64).clamp(1.0, 400.0),
            1 => {
                r.text_family = v.clamp(0, 2);
                r.text_font = None;
            }
            2 => r.text_bold = !r.text_bold,
            3 => r.text_italic = !r.text_italic,
            6 => r.text_underline = !r.text_underline,
            7 => r.text_bullets = !r.text_bullets,
            8 => r.text_font = picked,
            _ => r.text_align = v.clamp(0, 2),
        }
        if r.draft.as_ref().is_some_and(|d| d.kind == 2) {
            let (set, clear) = para_bits(what);
            r.para_format = (r.para_format | set) & !clear;
        }
        let a = r.selected_annot.and_then(|i| r.annots().get(i)).filter(|a| a.subtype == "FreeText" && !a.fill_sign)?;
        let now = a.font.unwrap_or(("Helv", 12.0, 0));
        // Only a family, bold or italic click changes the font: a size or alignment change keeps the
        // box's own (one the row does not list, an embedded one, stays).
        let font = if matches!(what, 1..=3) { text_font(r) } else { now.0 };
        let (size, align) = (r.text_pt, r.text_align as u8);
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

/// What a click on the format row (`what` as [`text_format`]'s) changes of the open paragraph:
/// the `para_format` bits it sets and those it clears. Underline and bullets apply to the
/// paragraph too (Sol, 26/10: only new text took them); a family picked replaces a font of the
/// computer's, and that font the family.
fn para_bits(what: i32) -> (u8, u8) {
    match what {
        0 | 5 => (2, 0),
        1 => (4, 64),
        2 | 3 => (4, 0),
        6 => (16, 0),
        7 => (32, 0),
        8 => (64, 4),
        _ => (8, 0),
    }
}

/// The computer's fonts for the format row's list (`ReaderBridge.system-fonts`): read on a thread
/// the first time, then at once.
fn list_system_fonts(ui: &AppWindow) {
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let names: Vec<slint::SharedString> = sheaf_core::system_fonts::installed().iter().map(|f| f.name.as_str().into()).collect();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                use slint::ComponentHandle as _;
                ui.global::<sheaf_app::ReaderBridge>().set_system_fonts(slint::ModelRc::new(slint::VecModel::from(names)));
            }
        });
    });
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
    r.annots_also.clear();
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
/// fill on or off, 2 the opacity in percent (`value`, 10–100), 3 the stamp (index in
/// `StampKind::ALL`, then the pictures kept in the library), 6 the fill colour (`value`: −1 a tint of
/// the stroke, else a highlight colour), 8 the arrow's next head, 9 its head at both ends or one,
/// 10 the next measuring scale, 11 callouts, 12 Calibrate, 13 initials, 14 new initials, 15 every
/// page, 16 the selected stamp or picture turned a quarter clockwise, 17 dashed lines (pen and
/// shapes; a selected drawing or shape changes at once). Shapes 4–9: polygon, cloud, polyline, distance, perimeter, area. A selected drawing or
/// shape takes the opacity at once (Acrobat's Properties bar).
pub fn shape_option(ui: &AppWindow, tab_id: i32, what: i32, value: i32) {
    // New initials: the kept ones forgotten, the pad opens at the next click (outside the reader's
    // state: the preferences are their own).
    if what == 14 {
        crate::home::set_initials(Vec::new(), None, 0);
        update(tab_id, |r, _| r.version += 1);
        return;
    }
    let restyle = update(tab_id, |r, _| {
        // Acrobat's Rotate on a stamp: a quarter turn clockwise, one undo step.
        if what == 16 {
            let a = r.selected_annot.and_then(|i| r.annots().get(i)).filter(|a| turnable(&a.subtype))?;
            return Some((a.page, Edit::TurnAnnotation { page: a.page, index: a.index, quarters: 1 }));
        }
        if what == 17 {
            r.dashed = !r.dashed;
            let a = r.selected_annot.and_then(|i| r.annots().get(i)).filter(|a| stroked(&a.subtype))?;
            return Some((a.page, Edit::DashAnnotation { page: a.page, index: a.index, dashed: r.dashed }));
        }
        match what {
            0 => {
                r.shape_kind = value.clamp(0, 9);
                r.poly = None;
            }
            1 => r.shape_fill = !r.shape_fill,
            3 => {
                // Past the standard stamps: a picture kept in the library.
                let first_picture = MARKS.len() + StampKind::ALL.len();
                let library = crate::home::stamp_library();
                r.stamp = (value.max(0) as usize).min(first_picture + library.len().saturating_sub(1));
                r.custom_stamp = r.stamp.checked_sub(first_picture).and_then(|k| library.get(k)).and_then(|p| super::edits::picture_stamp(p));
            }
            4 => r.link_visible = !r.link_visible,
            5 => r.link_dashed = !r.link_dashed,
            6 => {
                r.fill_colour = usize::try_from(value).ok().map(|k| k.min(CUSTOM));
                r.shape_fill = true;
            }
            8 => r.arrow_head = (r.arrow_head + 1) % reader::ARROW_HEADS.len(),
            9 => r.arrow_both = !r.arrow_both,
            10 => {
                // From a calibrated scale back to the presets, at the one it was on.
                if r.measure_custom.take().is_none() {
                    r.measure_scale = (r.measure_scale + 1) % reader::MEASURE_SCALES.len();
                }
            }
            11 => r.callout = !r.callout,
            12 => r.calibrating = !r.calibrating,
            13 => r.initials = !r.initials,
            15 => r.sign_every_page = !r.sign_every_page,
            // Acrobat's Measuring ▸ Precision: 0 to 3 decimals, a click goes to the next.
            18 => r.measure_decimals = (r.measure_decimals + 1) % 4,
            // Fill ▸ None: new shapes unfilled; a selected one loses its fill (below).
            19 => r.shape_fill = false,
            _ => {
                r.opacity = value.clamp(10, 100) as usize;
                remember_style(r);
            }
        }
        let a = r.selected_annot.and_then(|i| r.annots().get(i)).filter(|a| stroked(&a.subtype))?;
        // A selected rectangle, oval or polygon takes the fill chosen at once (Acrobat's Properties bar).
        if what == 19 && fillable(&a.subtype) {
            return Some((a.page, Edit::FillAnnotation { page: a.page, index: a.index, fill: None }));
        }
        if what == 6 && fillable(&a.subtype) {
            let stroke = a.color.map_or_else(|| ink_at(r.ink_colour), |c| c.map(f64::from));
            let fill = r.fill_colour.map_or_else(|| reader::fill_tint(stroke), light_at);
            return Some((a.page, Edit::FillAnnotation { page: a.page, index: a.index, fill: Some(fill) }));
        }
        (what == 2).then(|| (a.page, Edit::StyleAnnotation { page: a.page, index: a.index, color: None, opacity: Some(opacity_of(r)), width: None, endings: None }))
    })
    .flatten();
    if let Some((page, e)) = restyle {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// Drawings and shapes: the comments whose width and opacity the options bar sets.
pub(super) fn stroked(subtype: &str) -> bool {
    matches!(subtype, "Ink" | "Square" | "Circle" | "Line" | "Polygon" | "PolyLine")
}

/// The tool's opacity, 0–1. Kept as a percent; 0–3 are the four steps of before (preferences
/// saved then), 100 %, 75 %, 50 %, 25 %.
/// Comments with an inside to fill: rectangles, ovals, polygons and clouds.
pub(super) fn fillable(subtype: &str) -> bool {
    matches!(subtype, "Square" | "Circle" | "Polygon")
}

/// Stamps, pictures and signatures placed: the comments a quarter turn turns.
pub(super) fn turnable(subtype: &str) -> bool {
    subtype == "Stamp"
}

pub(super) fn opacity_of(r: &ReaderUi) -> f64 {
    match r.opacity {
        k @ 0..=3 => reader::OPACITIES[k],
        p => p.min(100) as f64 / 100.0,
    }
}

/// The width picked: the tool's, and a selected drawing's or shape's at once.
pub fn set_pen_width(ui: &AppWindow, tab_id: i32, k: i32) {
    let restyle = update(tab_id, |r, _| {
        r.pen_width = k.clamp(0, 3);
        remember_style(r);
        let a = r.selected_annot.and_then(|i| r.annots().get(i)).filter(|a| stroked(&a.subtype))?;
        Some((a.page, Edit::StyleAnnotation { page: a.page, index: a.index, color: None, opacity: None, width: Some(pen_points(r.pen_width)), endings: None }))
    })
    .flatten();
    if let Some((page, e)) = restyle {
        apply(ui, tab_id, e, &[page], None);
    }
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
    LinkStyle { visible: r.link_visible, color: ink_at(r.ink_colour), width: f64::from(r.pen_width.clamp(0, 2) + 1), dashed: r.link_dashed, underline: false, highlight: LinkHighlight::Invert }
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

/// Acrobat's Replace Text: the selected words are kept and an editor opens below them for the
/// words proposed instead (kind 10); committed, they are struck out and a caret holds the new ones.
pub fn replace_selection(tab_id: i32) {
    propose(tab_id, 10);
}

/// Acrobat's Insert Text: the words typed are proposed after the selection, in a caret, nothing
/// struck out (kind 11; Sol 25/10: only replacing was offered).
pub fn insert_after_selection(tab_id: i32) {
    propose(tab_id, 11);
}

/// Replace (10) or Insert (11) text from the selection: its quads kept, an editor below it.
fn propose(tab_id: i32, kind: i32) {
    update(tab_id, |r, v| {
        let Some((page, range)) = r.sel.clone() else { return };
        let Some(t) = r.text(page) else { return };
        let b = &v.viewer.boxes[page];
        let size = b.view_size();
        let lines = t.line_rects(range);
        let rects: Vec<[f64; 4]> = lines.iter().map(|q| viewer::view_rect_to_pdf(b, size, (q[0] as f64, q[1] as f64), (q[2] as f64, q[3] as f64))).collect();
        let Some(first) = rects.first().copied() else { return };
        let union = rects.iter().fold(first, |a, q| [a[0].min(q[0]), a[1].min(q[1]), a[2].max(q[2]), a[3].max(q[3])]);
        let [x, y, _, h] = viewer::pdf_rect_to_view(b, size, union);
        r.replace_quads = reader::quads(b, &lines);
        r.sel = None;
        r.draft = Some(Draft { page, at: (x, y + h + 6.0), kind, block: Some((usize::MAX, String::new(), union)), field: None });
        r.note_seq += 1;
    });
}

pub(super) fn pen_points(k: i32) -> f64 {
    [1.0, 2.0, 4.0, 8.0][k.clamp(0, 3) as usize]
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
        r.panel = panel.clamp(0, 5);
        r.panel_open = true;
        model::quote_texts(r, tab_id);
    });
}

/// The side panel's edge let go at `width` px: every document's thumbnails take the new width
/// (PDF Expert's sidebar), and it is remembered.
pub fn panel_resized(ui: &AppWindow, width: f32) {
    let w = crate::home::set_panel_width(ui, width);
    for id in with(|a| a.tabs.iter().map(|t| t.id).collect::<Vec<_>>()) {
        if update(id, |_, v| v.viewer.thumb_width = reader_thumb(w)).is_some() {
            crate::viewer_ui::request(ui, id);
        }
    }
}

/// Every document's view built again (a preference they show changed).
pub fn refresh_all() {
    for id in with(|a| a.tabs.iter().map(|t| t.id).collect::<Vec<_>>()) {
        update(id, |_, _| ());
    }
}

pub fn toggle_panel(tab_id: i32) {
    update(tab_id, |r, _| {
        r.panel_open = !r.panel_open;
        model::quote_texts(r, tab_id);
    });
}

/// Outline panel edits (PDF Expert's and Acrobat's bookmarks): `op` 0 bookmark the page in view
/// (titled with the selected text, as Acrobat names a new bookmark), 1 start renaming `row`, 2 rename
/// it to `text`, 3 stop renaming, 4 delete `row` and what is under it, 5 point `row` at the page in
/// view (Acrobat's Set Destination: the view, its place and zoom), 6 / 7 move `row` up / down among its
/// siblings, 8 indent it (the last child of the bookmark above), 9 outdent it (after its parent), 10 / 11
/// bold / italic on or off, 12 in the pen's colour, 13 in the reader's colour again (Acrobat's bookmark
/// Appearance).
pub fn outline_op(ui: &AppWindow, tab_id: i32, row: i32, op: i32, text: &str) {
    let edit = update(tab_id, |r, v| {
        let path = usize::try_from(row).ok().and_then(|i| r.rows.get(i)).map(|x| x.path.clone());
        let style = path.as_ref().and_then(|p| r.snap.outline_styles.iter().find(|s| s.0 == *p)).map_or((false, false, None), |s| (s.1, s.2, s.3));
        let restyle = |path: Vec<usize>, bold: bool, italic: bool, color: Option<[f32; 3]>| Edit::SetBookmarkStyle { path, bold, italic, color: color.map(|c| c.map(f64::from)) };
        match op {
            10 => return Some(restyle(path?, !style.0, style.1, style.2)),
            11 => return Some(restyle(path?, style.0, !style.1, style.2)),
            12 => return Some(restyle(path?, style.0, style.1, Some(ink_at(r.ink_colour).map(|c| c as f32)))),
            13 => return Some(restyle(path?, style.0, style.1, None)),
            _ => {}
        }
        match op {
            0 => {
                // At the top level, in page order.
                let page = v.current();
                let index = r.snap.outline.iter().filter(|o| o.page.is_some_and(|p| p <= page)).count();
                let selected = r.sel.as_ref().and_then(|(p, range)| r.texts.get(p).map(|t| reader::selected_text(t, range.clone())));
                let title = selected.map(|t| bookmark_title(&t)).filter(|t| !t.is_empty()).unwrap_or_else(|| format!("Page {}", label(r, page)));
                // From a selection, the bookmark goes to the selection's page; else to the view: its
                // place and zoom (Acrobat's new bookmark).
                match r.sel.as_ref() {
                    Some((p, _)) => Some(Edit::AddBookmark { parent: Vec::new(), index, title, page: *p }),
                    None => {
                        let add = Edit::AddBookmark { parent: Vec::new(), index, title, page };
                        let (left, top, zoom) = view_of(v, page);
                        Some(Edit::Batch { label: "Add bookmark".into(), edits: vec![add, Edit::SetBookmarkView { path: vec![index], page, left, top, zoom }] })
                    }
                }
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
            5 => {
                let page = v.current();
                let (left, top, zoom) = view_of(v, page);
                Some(Edit::SetBookmarkView { path: path?, page, left, top, zoom })
            }
            6 | 7 => {
                let path = path?;
                let (&last, parent) = path.split_last()?;
                let siblings = r.rows.iter().filter(|x| x.path.len() == path.len() && x.path[..parent.len()] == *parent).count();
                let to = if op == 6 { last.checked_sub(1)? } else { last + 1 };
                (to < siblings).then(|| Edit::MoveBookmark { from: path.clone(), to_parent: parent.to_vec(), index: to })
            }
            8 => {
                // Under the sibling above, as its last child.
                let path = path?;
                let (&last, parent) = path.split_last()?;
                let mut to_parent = parent.to_vec();
                to_parent.push(last.checked_sub(1)?);
                let children = r.rows.iter().filter(|x| x.path.len() == path.len() + 1 && x.path[..path.len()] == to_parent[..]).count();
                Some(Edit::MoveBookmark { from: path.clone(), to_parent, index: children })
            }
            9 => {
                // Right after its parent, one level up.
                let path = path?;
                let (_, parent) = path.split_last()?;
                let (&p_last, grand) = parent.split_last()?;
                Some(Edit::MoveBookmark { from: path.clone(), to_parent: grand.to_vec(), index: p_last + 1 })
            }
            _ => Some(Edit::DeleteBookmark { path: path? }),
        }
    })
    .flatten();
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[], None);
    }
}

/// A bookmark dragged in the panel (Acrobat's): from row `from` onto row `to`, inside it (its last
/// child) or before it; past the last row, last. One undo step; refused into its own children.
pub fn outline_drop(ui: &AppWindow, tab_id: i32, from: i32, to: i32, inside: bool) {
    let edit = update(tab_id, |r, _| {
        let from = r.rows.get(usize::try_from(from).ok()?)?.path.clone();
        let (to_parent, index) = match usize::try_from(to).ok().and_then(|t| r.rows.get(t)) {
            Some(t) if inside => {
                (t.path.clone(), outline_children(&r.snap.outline, &t.path))
            }
            Some(t) => {
                let (&last, parent) = t.path.split_last()?;
                // The index counts once `from` is out: one less after it among the same siblings.
                let same = from.len() == t.path.len() && from[..from.len() - 1] == *parent && from[from.len() - 1] < last;
                (parent.to_vec(), if same { last - 1 } else { last })
            }
            None => (Vec::new(), usize::MAX),
        };
        (!to_parent.starts_with(&from)).then_some(Edit::MoveBookmark { from, to_parent, index })
    })
    .flatten();
    match edit {
        Some(e) => apply(ui, tab_id, e, &[], None),
        None => crate::flow::toast(ui, "A bookmark cannot go inside its own children"),
    }
}

/// How many children the bookmark at `path` has (at the root, the top-level bookmarks).
fn outline_children(items: &[render::OutlineItem], path: &[usize]) -> usize {
    match path.split_first() {
        None => items.len(),
        Some((&i, rest)) => items.get(i).map_or(0, |it| outline_children(&it.children, rest)),
    }
}

/// The view as a destination on `page`: its top-left corner there (user space) and its zoom (1 =
/// actual size), as Acrobat's Set Destination keeps them.
fn view_of(v: &ViewerUi, page: usize) -> (Option<f64>, Option<f64>, Option<f64>) {
    let [px, py, pw, _] = v.viewer.page_rect(page);
    let b = &v.viewer.boxes[page];
    let size = b.view_size();
    let k = size.0 / pw.max(1e-9);
    let x = (v.left - px).max(0.0) * k;
    let y = (v.top - py).max(0.0) * k;
    let (l, t) = viewer::view_to_pdf(b, size, x, y);
    (Some(l), Some(t), Some(v.viewer.actual_percent(page) / 100.0))
}

/// A selection as a bookmark title: one line, spaces collapsed, at most 80 characters.
fn bookmark_title(text: &str) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match t.char_indices().nth(80) {
        Some((i, _)) => format!("{}…", t[..i].trim_end()),
        None => t,
    }
}

pub fn outline_clicked(ui: &AppWindow, tab_id: i32, row: i32) {
    update(tab_id, |r, v| {
        // The bookmark's own point on its page, not only the page's top (Acrobat).
        if let Some((p, view)) = r.rows.get(row.max(0) as usize).and_then(|x| Some((x.page?, x.view))) {
            jump_dest(r, v, p, &view);
        }
    });
    // A bookmark may set a zoom: drawn again at it.
    crate::viewer_ui::request(ui, tab_id);
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
        0 => r.notes_filter = value.clamp(0, 10),
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
/// Fillable fields tinted or not, in every document (a preference).
pub fn toggle_fields(tab_id: i32) {
    let on = crate::home::toggle_field_tint();
    update(tab_id, |r, _| r.show_fields = on);
}

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

    /// Underline, bullets and a font of the computer's change the paragraph open, as size and
    /// family do; a family and a font of the computer's replace each other.
    #[test]
    fn underline_bullets_and_fonts_apply_to_the_paragraph_open() {
        assert_eq!(para_bits(6), (16, 0));
        assert_eq!(para_bits(7), (32, 0));
        assert_eq!(para_bits(8), (64, 4));
        assert_eq!(para_bits(1), (4, 64));
        assert_eq!(para_bits(0), (2, 0));
        assert_eq!(para_bits(4), (8, 0));
    }
}

#[cfg(test)]
mod bookmark_tests {
    use super::*;

    #[test]
    fn bookmark_titles_are_one_short_line() {
        assert_eq!(bookmark_title("  Chapter\n 2:   Methods "), "Chapter 2: Methods");
        assert_eq!(bookmark_title(&"word ".repeat(30)).chars().count(), 80);
    }
}
