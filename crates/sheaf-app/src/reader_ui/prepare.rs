//! Prepare form (Acrobat's Prepare a Form, PDF Expert's form editing): the Form tool places fields
//! by a drag or a click, selects one to move it, delete it, copy it to every page or change its
//! properties, selects several (⇧-click) to move, align, size or space them together, and finds a
//! flat form's fields for you (Detect fields).

use super::*;
use sheaf_core::pdfcraft::engine::{FieldAction, FieldProps, FieldTrigger, NewField};
use std::fmt::Write as _;

/// The field kinds the options bar offers, in its order (Acrobat's Prepare Form toolbar).
pub const FIELD_KINDS: [&str; 9] = ["Text", "Check box", "Radio", "Dropdown", "List", "Date", "Button", "Signature", "Image"];

/// A field placed by a click, not a drag: Acrobat's default size for its kind, in points.
pub(super) fn default_size(kind: i32) -> (f64, f64) {
    match kind {
        1 | 2 => (14.0, 14.0),
        4 => (150.0, 60.0),
        5 => (100.0, 22.0),
        6 => (80.0, 22.0),
        7 => (180.0, 40.0),
        8 => (100.0, 100.0),
        _ => (150.0, 22.0),
    }
}

/// The field to add for the kind chosen. Radio buttons placed one after another make one group
/// (Acrobat asks; a new group starts when another kind is picked), each with its own choice.
pub(super) fn new_field(r: &mut ReaderUi) -> NewField {
    match r.field_kind {
        1 => NewField::CheckBox,
        2 => {
            let taken = |g: &str| r.snap.fields.iter().any(|f| f.name == g);
            let group = r.radio_group.clone().filter(|g| taken(g)).unwrap_or_else(|| (1..).map(|k| format!("Group{k}")).find(|g| !taken(g)).unwrap_or_default());
            let n = r.snap.fields.iter().find(|f| f.name == group).map_or(0, |f| f.widgets.len());
            r.radio_group = Some(group.clone());
            NewField::Radio { group: Some(group), export: format!("Choice{}", n + 1) }
        }
        3 => NewField::Combo { options: vec!["Option 1".into(), "Option 2".into(), "Option 3".into()], editable: false },
        4 => NewField::List { options: vec!["Option 1".into(), "Option 2".into(), "Option 3".into()], multi: false },
        5 => NewField::Date,
        6 => NewField::Button { caption: "Button".into() },
        7 => NewField::Signature,
        8 => NewField::Image,
        _ => NewField::Text { multiline: false },
    }
}

/// Any field's widget under view point `p` of `page`: (field, its rect in user space).
pub(super) fn field_hit(r: &ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) -> Option<(usize, [f64; 4])> {
    let size = v.boxes[page].view_size();
    r.snap.fields.iter().enumerate().find_map(|(i, f)| {
        f.widgets.iter().filter(|w| w.0 == page).find_map(|w| {
            let [x, y, ww, hh] = viewer::pdf_rect_to_view(&v.boxes[page], size, w.1);
            (p.0 >= x - 2.0 && p.0 <= x + ww + 2.0 && p.1 >= y - 2.0 && p.1 <= y + hh + 2.0).then_some((i, w.1))
        })
    })
}

/// The field selected with the Form tool, found again by its widget's place (a snapshot renumbers
/// the fields): (field, widget, page, rect).
pub(super) fn selected_field(r: &ReaderUi) -> Option<(usize, usize, usize, [f64; 4])> {
    let (page, rect) = r.form_sel?;
    widget_at(r, page, rect)
}

fn widget_at(r: &ReaderUi, page: usize, rect: [f64; 4]) -> Option<(usize, usize, usize, [f64; 4])> {
    let near = |a: [f64; 4]| (0..4).all(|k| (a[k] - rect[k]).abs() < 0.5);
    r.snap.fields.iter().enumerate().find_map(|(i, f)| f.widgets.iter().position(|w| w.0 == page && near(w.1)).map(|k| (i, k, page, f.widgets[k].1)))
}

/// Every field selected: the first one picked (the others follow it when arranged), then those
/// added with ⇧-click on its page.
pub(super) fn selected_fields(r: &ReaderUi) -> Vec<(usize, usize, usize, [f64; 4])> {
    let Some((page, _)) = r.form_sel else { return Vec::new() };
    selected_field(r).into_iter().chain(r.form_also.iter().filter_map(|rect| widget_at(r, page, *rect))).collect()
}

/// A press with the Form tool: on a field, it is selected and may be dragged (with the others
/// selected, when it is one of them); ⇧ adds it to the selection or takes it out (Acrobat's);
/// elsewhere a new field's box starts.
pub(super) fn form_pressed(r: &mut ReaderUi, v: &Viewer, page: usize, p: (f64, f64), shift: bool) {
    let near = |a: [f64; 4], b: [f64; 4]| (0..4).all(|k| (a[k] - b[k]).abs() < 0.5);
    match field_hit(r, v, page, p) {
        Some((_, rect)) if shift && r.form_sel.is_some_and(|s| s.0 == page) => {
            if r.form_sel.is_some_and(|s| near(s.1, rect)) {
                // The first one taken out: the next one leads.
                r.form_sel = (!r.form_also.is_empty()).then(|| (page, r.form_also.remove(0)));
            } else if let Some(k) = r.form_also.iter().position(|a| near(*a, rect)) {
                r.form_also.remove(k);
            } else {
                r.form_also.push(rect);
            }
            // The press is taken: let go where it was, it adds no field (dragged, the group moves).
            r.form_drag = Some((page, p, p));
            r.note_seq += 1;
        }
        Some((_, rect)) => {
            let in_group = r.form_sel.is_some_and(|s| s.0 == page && (near(s.1, rect) || r.form_also.iter().any(|a| near(*a, rect))));
            if !in_group {
                r.form_sel = Some((page, rect));
                r.form_also.clear();
            }
            r.form_drag = Some((page, p, p));
            // Its card starts from its own values.
            r.note_seq += 1;
        }
        None => {
            r.form_sel = None;
            r.form_also.clear();
            r.stroke = Some((page, vec![p]));
            r.shape_end = None;
        }
    }
}

/// The pointer moving with the Form tool pressed: a selected field follows; a new box grows.
pub(super) fn form_moved(r: &mut ReaderUi, page: usize, a: (f64, f64), b: (f64, f64)) {
    if let Some(d) = r.form_drag.as_mut() {
        d.2 = b;
        return;
    }
    r.shape_end = Some(b);
    r.stroke = Some((page, reader::shape_outline(0, a, b)));
}

/// The Form tool let go: a selected field dragged is moved; a box dragged, or a click on the page,
/// adds a field of the kind chosen (selected, to name it).
pub(super) fn form_released(r: &mut ReaderUi, v: &Viewer, press: Option<(usize, (f64, f64))>) -> Option<Pending> {
    if let Some((page, from, to)) = r.form_drag.take() {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        if dx.hypot(dy) < 2.0 {
            return None;
        }
        let b = &v.boxes[page];
        let size = b.view_size();
        // With the grid on, the first field's top-left corner lands on a crossing; the others keep their places to it.
        let (dx, dy) = match selected_field(r) {
            Some(s) if r.form_grid => {
                let [x, y, ..] = viewer::pdf_rect_to_view(b, size, s.3);
                let (sx, sy) = snapped(r, (x + dx, y + dy));
                (sx - x, sy - y)
            }
            _ => (dx, dy),
        };
        // Every field selected moves by the drag, one undo step.
        let moved: Vec<[f64; 4]> = selected_fields(r)
            .iter()
            .map(|s| {
                let [x, y, w, h] = viewer::pdf_rect_to_view(b, size, s.3);
                viewer::view_rect_to_pdf(b, size, (x + dx, y + dy), (x + dx + w, y + dy + h))
            })
            .collect();
        return place_fields(r, page, &moved, "Move fields").map(|e| Pending::Move(page, Box::new(e)));
    }
    r.stroke = None;
    let (page, a) = press?;
    let b = &v.boxes[page];
    let size = b.view_size();
    let rect = match r.shape_end.take().filter(|e| (a.0 - e.0).hypot(a.1 - e.1) >= 4.0) {
        Some(e) => {
            let (a, e) = (snapped(r, a), snapped(r, e));
            viewer::view_rect_to_pdf(b, size, (a.0.min(e.0), a.1.min(e.1)), (a.0.max(e.0), a.1.max(e.1)))
        }
        // A click: the kind's own size in points whatever the zoom (page points are view
        // points), its top-left corner at the click.
        None => {
            let (w, h) = default_size(r.field_kind);
            let a = snapped(r, a);
            viewer::view_rect_to_pdf(b, size, a, (a.0 + w, a.1 + h))
        }
    };
    let kind = new_field(r);
    r.form_sel = Some((page, rect));
    Some(Pending::Move(page, Box::new(Edit::AddField { page, rect, kind, name: None })))
}

/// The Form tool's grid step, in points: Acrobat's default grid (lines every inch, 8 subdivisions).
pub const GRID: f64 = 9.0;

/// A page point (points from the page's top-left corner, as shown) on the nearest grid crossing
/// when the grid is on.
fn snapped(r: &ReaderUi, p: (f64, f64)) -> (f64, f64) {
    // A guide near wins over the grid (Acrobat's Snap to guides).
    let page = r.form_sel.map_or(0, |s| s.0);
    let g = to_guides(r, page, p);
    if g != p {
        return g;
    }
    if r.form_grid { ((p.0 / GRID).round() * GRID, (p.1 / GRID).round() * GRID) } else { p }
}

/// More tools ▸ Rulers (Acrobat's View ▸ Show/Hide ▸ Rulers & Grids ▸ Rulers): a ruler along each
/// page's top and left edge, in inches (or centimetres, Preferences' units).
/// Help ▸ Keyboard shortcuts shown or closed.
pub fn show_help(tab_id: i32, on: bool) {
    update(tab_id, |r, _| r.help_open = on);
}

pub fn rulers(tab_id: i32) {
    update(tab_id, |r, _| r.rulers = !r.rulers);
}

/// A click on a page's ruler (`across` the top ruler, else the left one) at `at` page points from
/// the page's top-left: a guide there, across the whole page (Acrobat's Guides); a click on a
/// guide's mark (within 3 pt) takes it away.
pub fn ruler_clicked(tab_id: i32, page: i32, across: bool, at: f32) {
    update(tab_id, |r, _| toggle_guide(&mut r.guides, page.max(0) as usize, across, f64::from(at)));
}

/// A guide of `page` at `at` added (to the half point), or the one within 3 pt taken away.
pub(super) fn toggle_guide(guides: &mut Vec<(usize, bool, f64)>, page: usize, across: bool, at: f64) {
    match guides.iter().position(|g| g.0 == page && g.1 == across && (g.2 - at).abs() <= 3.0) {
        Some(k) => {
            guides.remove(k);
        }
        None => guides.push((page, across, (at * 2.0).round() / 2.0)),
    }
}

/// The rulers and guides over each page in view: (page box in content pixels, the ruler ticks and
/// the guides as path commands in that box's pixels, the labels every inch or centimetre).
pub(super) fn ruler_overlay(r: &ReaderUi, v: &ViewerUi, in_view: impl Fn(usize) -> bool) -> Vec<ReaderRuler> {
    if !r.rulers && r.guides.is_empty() {
        return Vec::new();
    }
    let cm = crate::home::prefs().metric;
    let unit = if cm { 72.0 / 2.54 } else { 72.0 };
    let tick = unit / if cm { 2.0 } else { 8.0 };
    (0..v.viewer.page_count())
        .filter(|p| in_view(*p))
        .map(|p| {
            let [x, y, w, h] = v.viewer.page_rect(p);
            let k = w / v.viewer.boxes[p].view_size().0.max(1e-9);
            let (mut ticks, mut guides, mut labels) = (String::new(), String::new(), Vec::new());
            if r.rulers {
                for (len, across) in [(w, true), (h, false)] {
                    let n = (len / (tick * k)).ceil() as usize;
                    for i in 1..n {
                        let at = i as f64 * tick * k;
                        let size = if (i as f64 * tick / unit).fract().abs() < 1e-6 { 10.0 } else if i % 2 == 0 { 6.0 } else { 3.5 };
                        let _ = if across { write!(ticks, "M {at:.1} 0 L {at:.1} {size} ") } else { write!(ticks, "M 0 {at:.1} L {size} {at:.1} ") };
                        if size == 10.0 {
                            labels.push(ReaderRulerLabel { across, at: at as f32, text: format!("{}", (i as f64 * tick / unit).round()).into() });
                        }
                    }
                }
            }
            for g in r.guides.iter().filter(|g| g.0 == p) {
                let at = g.2 * k;
                let _ = if g.1 { write!(guides, "M {at:.1} 0 L {at:.1} {h:.1} ") } else { write!(guides, "M 0 {at:.1} L {w:.1} {at:.1} ") };
            }
            ReaderRuler { page: p as i32, x: x as f32, y: y as f32, w: w as f32, h: h as f32, scale: k as f32, ticks: ticks.into(), guides: guides.into(), labels: slint::ModelRc::new(slint::VecModel::from(labels)) }
        })
        .collect()
}

/// A page point on the nearest guide of `page` within 4 pt, else as it is.
pub(super) fn to_guides(r: &ReaderUi, page: usize, p: (f64, f64)) -> (f64, f64) {
    let near = |across: bool, v: f64| r.guides.iter().filter(|g| g.0 == page && g.1 == across).map(|g| g.2).find(|g| (g - v).abs() <= 4.0).unwrap_or(v);
    (near(true, p.0), near(false, p.1))
}

#[cfg(test)]
mod ruler_tests {
    #[test]
    fn a_guide_is_made_by_a_click_on_a_ruler_and_taken_away_by_another() {
        let mut guides: Vec<(usize, bool, f64)> = Vec::new();
        super::toggle_guide(&mut guides, 0, true, 100.2);
        super::toggle_guide(&mut guides, 0, false, 50.0);
        assert_eq!(guides, [(0, true, 100.0), (0, false, 50.0)]);
        super::toggle_guide(&mut guides, 0, true, 101.5);
        assert_eq!(guides, [(0, false, 50.0)], "a click on its mark takes it away");
    }
}

/// The options bar's Grid, and More ▸ Grid in any tool (Acrobat's View ▸ Show/Hide ▸ Grid): shown
/// over the pages; with Prepare form, fields placed, dragged or moved snap to it.
pub fn form_grid(tab_id: i32) {
    update(tab_id, |r, _| r.form_grid = !r.form_grid);
}

/// The grid over each page in view (Grid on): (page box in content pixels, its minor and its
/// major lines as path commands in that box's pixels).
pub(super) fn grid_overlay(r: &ReaderUi, v: &ViewerUi, in_view: impl Fn(usize) -> bool) -> Vec<ReaderGrid> {
    if !r.form_grid {
        return Vec::new();
    }
    (0..v.viewer.page_count())
        .filter(|p| in_view(*p))
        .map(|p| {
            let [x, y, w, h] = v.viewer.page_rect(p);
            let k = w / v.viewer.boxes[p].view_size().0.max(1e-9) * GRID;
            let (mut minor, mut major) = (String::new(), String::new());
            let mut lines = |n: usize, len: f64, across: bool| {
                for i in 1..n {
                    let at = i as f64 * k;
                    let out = if i % 8 == 0 { &mut major } else { &mut minor };
                    let _ = if across { write!(out, "M {at:.1} 0 L {at:.1} {len:.1} ") } else { write!(out, "M 0 {at:.1} L {len:.1} {at:.1} ") };
                }
            };
            lines((w / k).ceil() as usize, h, true);
            lines((h / k).ceil() as usize, w, false);
            ReaderGrid { x: x as f32, y: y as f32, w: w as f32, h: h as f32, minor: minor.into(), major: major.into() }
        })
        .collect()
}

/// The options bar's field kind; another kind ends the radio group being placed.
pub fn field_kind(tab_id: i32, k: i32) {
    update(tab_id, |r, _| {
        if r.field_kind != k {
            r.radio_group = None;
        }
        r.field_kind = k.clamp(0, FIELD_KINDS.len() as i32 - 1);
    });
}

/// Detect fields: the flat form's lines, boxes and labelled blanks become fields, one undo step.
pub fn detect_fields(ui: &AppWindow, tab_id: i32) {
    let err = update(tab_id, |r, _| {
        if !r.snap.read_only.is_empty() {
            return Some(r.snap.read_only.clone());
        }
        if r.edits_in_flight > 0 {
            return Some("One moment: the last change is still being applied".to_string());
        }
        r.send(Op::DetectFields);
        r.undo_pages.push(Vec::new());
        r.redo_pages.clear();
        None
    })
    .flatten();
    if let Some(e) = err {
        crate::flow::toast(ui, &e);
    }
}

/// The selected field's properties card ▸ Done: what changed, one undo step; the card closes (the
/// field is let go, as Acrobat's Properties dialog closes). `options`: one per line. `look`: the
/// text's size typed ("" or "Auto": fitted) and its alignment (0 left, 1 centre, 2 right).
/// `font`: 0 Helvetica, 1 Times, 2 Courier. `click`: a button's action (0 nothing, 1 clear the form, 2 print, 3 open the address).
#[allow(clippy::too_many_arguments)]
pub fn field_props(ui: &AppWindow, tab_id: i32, name: &str, tooltip: &str, required: bool, read_only: bool, multiline: bool, options: &str, look: (&str, i32, i32), click: (i32, &str)) {
    let edit = update(tab_id, |r, _| {
        let typed = r.pending_scripts.take();
        let (fi, _, page, _) = selected_field(r)?;
        let f = &r.snap.fields[fi];
        let name = name.trim();
        let options: Vec<String> = options.lines().map(str::trim).filter(|o| !o.is_empty()).map(String::from).collect();
        let props = FieldProps {
            name: (!name.is_empty() && name != f.name).then(|| name.to_string()),
            tooltip: (tooltip.trim() != f.tooltip).then(|| tooltip.trim().to_string()),
            required: (required != f.required).then_some(required),
            read_only: (read_only != f.read_only).then_some(read_only),
            multiline: (f.exact == 0 && multiline != f.multiline).then_some(multiline),
            options: (matches!(f.exact, 3 | 4) && !options.is_empty() && options != f.options.iter().map(|o| o.1.clone()).collect::<Vec<_>>()).then_some(options),
            quadding: (matches!(f.exact, 0 | 3 | 4) && look.1 != f.align).then_some(look.1.clamp(0, 2) as i64),
            font_size: font_size(look.0).filter(|s| matches!(f.exact, 0 | 3 | 4) && (s - f.font_size).abs() > 0.01),
            // The text's font (Acrobat's Appearance ▸ Font; Sol, 25/10).
            appearance: (matches!(f.exact, 0 | 3 | 4 | 5) && look.2 != f.font).then(|| {
                use sheaf_core::pdfcraft::engine::{FieldFont, FieldLookPatch};
                FieldLookPatch { font: Some([FieldFont::Helvetica, FieldFont::Times, FieldFont::Courier][look.2.clamp(0, 2) as usize]), ..Default::default() }
            }),
            actions: (f.exact == 5 && (click.0 != f.action || (click.0 >= 3 && click.1.trim() != f.url))).then(|| click_action(click.0, click.1)),
            ..scripts_props(typed, f)
        };
        let edit = (props != FieldProps::default()).then(|| (page, Edit::SetFieldProps { name: f.name.clone(), props: Box::new(props) }));
        r.form_sel = None;
        edit
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// The card's Format, Validate and Calculate (Acrobat's tabs), kept until Done applies them with
/// the rest, one undo step.
pub fn field_scripts(tab_id: i32, format: i32, min: &str, max: &str, calc: &str) {
    update(tab_id, |r, _| r.pending_scripts = Some((format, min.trim().to_string(), max.trim().to_string(), calc.trim().to_string())));
}

/// The scripts that changed, as field properties: a text field's format, range and calculation.
fn scripts_props(typed: Option<(i32, String, String, String)>, f: &reader_doc::FieldEntry) -> FieldProps {
    use sheaf_core::pdfcraft::engine::form_scripts::{Calculate, Format, Validate};
    let Some((format, min, max, calc)) = typed.filter(|_| f.exact == 0) else { return FieldProps::default() };
    let number = |s: &str| s.replace(',', ".").parse::<f64>().ok();
    FieldProps {
        format: (format != f.format && format != 6).then(|| match format {
            1 => Format::Number { decimals: 2, sep: 0, neg: 0, currency: String::new(), prepend: true },
            2 => Format::Percent { decimals: 2, sep: 0 },
            3 => Format::Date("dd/mm/yyyy".into()),
            4 => Format::Special(2),
            5 => Format::Special(0),
            _ => Format::None,
        }),
        validate: (min != f.min || max != f.max).then(|| match (number(&min), number(&max)) {
            (None, None) => Validate::None,
            (min, max) => Validate::Range { min, max },
        }),
        calculate: (calc != f.calc).then_some(if calc.is_empty() { Calculate::None } else { Calculate::Notation(calc) }),
        ..Default::default()
    }
}

/// A text size typed in the card: points, or 0 for "Auto" (empty); `None` when it is no size.
pub(super) fn font_size(typed: &str) -> Option<f64> {
    let t = typed.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        return Some(0.0);
    }
    t.trim_end_matches("pt").trim().replace(',', ".").parse::<f64>().ok().filter(|s| (1.0..=300.0).contains(s))
}

/// A button's click as its Actions (Mouse Up): nothing, clear the form, print, open an address,
/// submit the form to an address (Acrobat's Submit a form).
pub(super) fn click_action(k: i32, url: &str) -> Vec<(FieldTrigger, FieldAction)> {
    match k {
        1 => vec![(FieldTrigger::MouseUp, FieldAction::Reset(Vec::new()))],
        2 => vec![(FieldTrigger::MouseUp, FieldAction::Named("Print".into()))],
        3 if !url.trim().is_empty() => vec![(FieldTrigger::MouseUp, FieldAction::Uri(url.trim().into()))],
        4 if !url.trim().is_empty() => vec![(FieldTrigger::MouseUp, FieldAction::Submit(url.trim().into()))],
        _ => Vec::new(),
    }
}

/// Prepare form ▸ Tab order (Acrobat's Page Properties ▸ Tab Order) for every page: 0 by rows, 1 by
/// columns, 2 the document's structure. One undo step.
pub fn tab_order(ui: &AppWindow, tab_id: i32, order: i32) {
    use sheaf_core::pdfcraft::engine::TabOrder;
    let pages = update(tab_id, |_, v| (0..v.viewer.page_count()).collect::<Vec<_>>()).unwrap_or_default();
    let order = [TabOrder::Row, TabOrder::Column, TabOrder::Structure][order.clamp(0, 2) as usize];
    apply(ui, tab_id, Edit::SetTabOrder { pages, order }, &[], None);
    crate::flow::toast(ui, "Tab order set for every page");
}

/// The fields selected, now at `rects` (in the order of [`selected_fields`]): one step setting
/// each widget's place; the selection follows them. None: nothing selected.
fn place_fields(r: &mut ReaderUi, page: usize, rects: &[[f64; 4]], label: &str) -> Option<Edit> {
    let sel = selected_fields(r);
    let edits: Vec<Edit> = sel
        .iter()
        .zip(rects)
        .map(|(s, rect)| Edit::SetFieldProps { name: r.snap.fields[s.0].name.clone(), props: Box::new(FieldProps { rect: Some((s.1, *rect)), ..Default::default() }) })
        .collect();
    let (first, rest) = rects.split_first()?;
    r.form_sel = Some((page, *first));
    r.form_also = rest.to_vec();
    match edits.len() {
        0 => None,
        1 => edits.into_iter().next(),
        _ => Some(Edit::Batch { label: label.into(), edits }),
    }
}

/// The fields selected together ▸ Align, Match size or Distribute (`how` as [`reader::arrange`]).
pub fn fields_arrange(ui: &AppWindow, tab_id: i32, how: i32) {
    let edit = update(tab_id, |r, _| {
        let sel = selected_fields(r);
        let page = sel.first()?.2;
        let rects = reader::arrange(&sel.iter().map(|s| s.3).collect::<Vec<_>>(), how);
        place_fields(r, page, &rects, "Arrange fields").map(|e| (page, e))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// The arrow keys with fields selected: they move by `dx`, `dy` points (1, or 10 with ⇧; `dy` up
/// the page), as Acrobat's. Held down, a key waits for the last move to be applied.
pub fn fields_nudge(ui: &AppWindow, tab_id: i32, dx: f32, dy: f32) {
    let edit = update(tab_id, |r, _| {
        if r.edits_in_flight > 0 {
            return None;
        }
        let sel = selected_fields(r);
        let page = sel.first()?.2;
        let (dx, dy) = (f64::from(dx), f64::from(dy));
        let rects: Vec<[f64; 4]> = sel.iter().map(|s| [s.3[0] + dx, s.3[1] + dy, s.3[2] + dx, s.3[3] + dy]).collect();
        place_fields(r, page, &rects, "Move fields").map(|e| (page, e))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// The selected fields ▸ Delete (or the Delete key), one step.
pub fn field_delete(ui: &AppWindow, tab_id: i32) {
    let edit = update(tab_id, |r, _| {
        let sel = selected_fields(r);
        let page = sel.first()?.2;
        let mut names: Vec<String> = sel.iter().map(|s| r.snap.fields[s.0].name.clone()).collect();
        names.sort();
        names.dedup();
        r.form_sel = None;
        r.form_also.clear();
        let mut edits: Vec<Edit> = names.into_iter().map(|name| Edit::DeleteField { name }).collect();
        Some((page, if edits.len() == 1 { edits.remove(0) } else { Edit::Batch { label: "Delete fields".into(), edits } }))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// The selected field ▸ On every page (Acrobat's Duplicate across pages): the same field, its value
/// shared, on each other page at the same place.
pub fn field_everywhere(ui: &AppWindow, tab_id: i32) {
    let edit = update(tab_id, |r, v| {
        let (fi, ..) = selected_field(r)?;
        let f = &r.snap.fields[fi];
        let pages: Vec<usize> = (0..v.viewer.page_count()).filter(|p| !f.widgets.iter().any(|w| w.0 == *p)).collect();
        (!pages.is_empty()).then(|| Edit::DuplicateField { name: f.name.clone(), pages })
    })
    .flatten();
    // Every page may change.
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[], None);
    }
}

/// The selected field ▸ Copies, `rows` down and `cols` across (Acrobat's Create Multiple Copies):
/// separate fields side by side with it, refused when the grid would run off the page.
pub fn field_copies(ui: &AppWindow, tab_id: i32, rows: i32, cols: i32) {
    let (rows, cols) = (rows.clamp(1, 50) as usize, cols.clamp(1, 20) as usize);
    let edit = update(tab_id, |r, v| {
        // The widget selected (a field on several pages copies the one picked: Sol, 26/10).
        let (fi, wi, ..) = selected_field(r)?;
        let f = &r.snap.fields[fi];
        let (page, [x0, y0, x1, y1]) = f.widgets.get(wi).map(|w| (w.0, w.1))?;
        let crop = v.viewer.boxes.get(page)?.crop;
        let fits = x0 + (x1 - x0) * cols as f64 <= crop[2] + 0.5 && y1 - (y1 - y0) * rows as f64 >= crop[1] - 0.5;
        Some((page, fits, Edit::CopyField { name: f.name.clone(), widget: wi, rows, cols }))
    })
    .flatten();
    match edit {
        Some(_) if rows * cols < 2 => crate::flow::toast(ui, "Ask for two copies or more"),
        Some((_, false, _)) => crate::flow::toast(ui, "The copies would run off the page: fewer, or start higher up"),
        Some((page, true, e)) => apply(ui, tab_id, e, &[page], None),
        None => {}
    }
}

/// The Form tool's overlay: every field on the pages in view, named, and the selected one's card.
pub(super) fn form_overlay(r: &ReaderUi, v: &ViewerUi, in_view: impl Fn(usize) -> bool) -> (Vec<ReaderFormField>, ReaderFieldProps) {
    if r.tool != FORM {
        return (Vec::new(), ReaderFieldProps::default());
    }
    let group = selected_fields(r);
    let sel = group.first().copied();
    let mut list = Vec::new();
    for (i, f) in r.snap.fields.iter().enumerate() {
        for (k, (page, rect, _, _)) in f.widgets.iter().enumerate().filter(|(_, w)| in_view(w.0)) {
            let mut c = v.viewer.overlay(*page, *rect);
            let selected = group.iter().any(|s| s.0 == i && s.1 == k);
            // The fields being dragged follow the pointer.
            if let Some((_, from, to)) = r.form_drag.filter(|_| selected) {
                c[0] += to.0 - from.0;
                c[1] += to.1 - from.1;
            }
            list.push(ReaderFormField { x: c[0] as f32, y: c[1] as f32, w: c[2] as f32, h: c[3] as f32, name: f.name.clone().into(), selected });
        }
    }
    let props = match sel.filter(|_| r.form_drag.is_none()) {
        Some((fi, _, page, rect)) => {
            let f = &r.snap.fields[fi];
            let c = v.viewer.overlay(page, rect);
            ReaderFieldProps {
                show: true,
                x: c[0] as f32,
                y: (c[1] + c[3] + 8.0) as f32,
                top: c[1] as f32,
                seq: r.note_seq,
                name: f.name.clone().into(),
                tooltip: f.tooltip.clone().into(),
                required: f.required,
                read_only: f.read_only,
                multiline: f.multiline,
                exact: f.exact,
                options: f.options.iter().map(|o| o.1.as_str()).collect::<Vec<_>>().join("\n").into(),
                align: f.align,
                font_size: if f.font_size > 0.0 { format!("{}", f.font_size).into() } else { "Auto".into() },
                font: f.font,
                action: f.action,
                url: f.url.clone().into(),
                format: f.format,
                min: f.min.clone().into(),
                max: f.max.clone().into(),
                calc: f.calc.clone().into(),
                other_pages: (0..v.viewer.page_count()).any(|p| !f.widgets.iter().any(|w| w.0 == p)),
                group: group.len() as i32,
            }
        }
        None => ReaderFieldProps::default(),
    };
    (list, props)
}

/// The required fields still empty: text and choices without a value, check boxes unticked, radio
/// groups with none picked.
pub(super) fn required_empty(fields: &[reader_doc::FieldEntry]) -> Vec<String> {
    fields.iter().filter(|f| f.required && matches!(f.exact, 0..=4) && f.value.iter().all(|v| v.trim().is_empty() || v == "Off")).map(|f| f.name.clone()).collect()
}

/// What the "Saved" message adds about them: ". 2 required fields are empty: Name, City".
pub(super) fn empty_note(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => format!(". The required field {one} is empty"),
        many => format!(". {} required fields are empty: {}", many.len(), many.iter().take(3).cloned().collect::<Vec<_>>().join(", ") + if many.len() > 3 { "…" } else { "" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, exact: i32, required: bool, value: &[&str]) -> reader_doc::FieldEntry {
        reader_doc::FieldEntry {
            name: name.into(),
            kind: exact.min(4),
            value: value.iter().map(|v| v.to_string()).collect(),
            options: Vec::new(),
            read_only: false,
            required,
            tooltip: String::new(),
            multiline: false,
            exact,
            align: 0,
            font_size: 0.0,
            font: 0,
            action: 0,
            url: String::new(),
            format: 0,
            min: String::new(),
            max: String::new(),
            calc: String::new(),
            widgets: Vec::new(),
        }
    }

    #[test]
    fn a_text_fields_format_range_and_calculation_become_its_scripts() {
        use sheaf_core::pdfcraft::engine::form_scripts::{Calculate, Format, Validate};
        let f = field("Total", 0, false, &[]);
        let p = scripts_props(Some((1, "0".into(), "1000".into(), "Price * Quantity".into())), &f);
        assert!(matches!(p.format, Some(Format::Number { decimals: 2, .. })), "{:?}", p.format);
        assert_eq!(p.validate, Some(Validate::Range { min: Some(0.0), max: Some(1000.0) }));
        assert_eq!(p.calculate, Some(Calculate::Notation("Price * Quantity".into())));
        // Nothing changed: nothing written; a check box has none of these.
        assert_eq!(scripts_props(Some((0, String::new(), String::new(), String::new())), &f), FieldProps::default());
        assert_eq!(scripts_props(Some((3, String::new(), String::new(), String::new())), &field("Agree", 1, false, &[])), FieldProps::default());
    }

    #[test]
    fn required_fields_left_empty_are_named_on_save() {
        let fields = [field("Name", 0, true, &[""]), field("City", 0, true, &["Paris"]), field("Group1", 2, true, &["Off"]), field("Notes", 0, false, &[]), field("Agree", 1, true, &[])];
        let empty = required_empty(&fields);
        assert_eq!(empty, ["Name", "Group1", "Agree"]);
        assert_eq!(empty_note(&empty), ". 3 required fields are empty: Name, Group1, Agree");
        assert_eq!(empty_note(&empty[..1]), ". The required field Name is empty");
        assert_eq!(empty_note(&[]), "");
    }

    #[test]
    fn a_size_typed_and_a_click_chosen_become_the_fields_properties() {
        assert_eq!((font_size(""), font_size("Auto"), font_size("12"), font_size("10,5 pt"), font_size("big")), (Some(0.0), Some(0.0), Some(12.0), Some(10.5), None));
        assert!(click_action(0, "").is_empty() && click_action(3, " ").is_empty());
        assert!(matches!(&click_action(3, "https://a.b")[..], [(FieldTrigger::MouseUp, FieldAction::Uri(u))] if u == "https://a.b"));
        assert!(matches!(&click_action(2, "")[..], [(_, FieldAction::Named(n))] if n == "Print"));
        assert!(matches!(&click_action(4, "https://forms.example/in")[..], [(_, FieldAction::Submit(u))] if u == "https://forms.example/in"));
    }
}
