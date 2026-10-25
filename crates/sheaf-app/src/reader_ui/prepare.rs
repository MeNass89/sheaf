//! Prepare form (Acrobat's Prepare a Form, PDF Expert's form editing): the Form tool places fields
//! by a drag or a click, selects one to move it, delete it, copy it to every page or change its
//! properties, and finds a flat form's fields for you (Detect fields).

use super::*;
use sheaf_core::pdfcraft::engine::{FieldAction, FieldProps, FieldTrigger, NewField};

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
    let near = |a: [f64; 4]| (0..4).all(|k| (a[k] - rect[k]).abs() < 0.5);
    r.snap.fields.iter().enumerate().find_map(|(i, f)| f.widgets.iter().position(|w| w.0 == page && near(w.1)).map(|k| (i, k, page, f.widgets[k].1)))
}

/// A press with the Form tool: on a field, it is selected and may be dragged; elsewhere a new
/// field's box starts.
pub(super) fn form_pressed(r: &mut ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) {
    match field_hit(r, v, page, p) {
        Some((_, rect)) => {
            r.form_sel = Some((page, rect));
            r.form_drag = Some((page, p, p));
            // Its card starts from its own values.
            r.note_seq += 1;
        }
        None => {
            r.form_sel = None;
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
        let (fi, wi, _, rect) = selected_field(r)?;
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        if dx.hypot(dy) < 2.0 {
            return None;
        }
        let b = &v.boxes[page];
        let size = b.view_size();
        let [x, y, w, h] = viewer::pdf_rect_to_view(b, size, rect);
        let moved = viewer::view_rect_to_pdf(b, size, (x + dx, y + dy), (x + dx + w, y + dy + h));
        r.form_sel = Some((page, moved));
        let props = FieldProps { rect: Some((wi, moved)), ..Default::default() };
        return Some(Pending::Move(page, Box::new(Edit::SetFieldProps { name: r.snap.fields[fi].name.clone(), props: Box::new(props) })));
    }
    r.stroke = None;
    let (page, a) = press?;
    let b = &v.boxes[page];
    let size = b.view_size();
    let rect = match r.shape_end.take().filter(|e| (a.0 - e.0).hypot(a.1 - e.1) >= 4.0) {
        Some(e) => viewer::view_rect_to_pdf(b, size, (a.0.min(e.0), a.1.min(e.1)), (a.0.max(e.0), a.1.max(e.1))),
        // A click: the kind's own size, its top-left corner at the click.
        None => {
            let (w, h) = default_size(r.field_kind);
            let k = size.0 / v.page_rect(page)[2].max(1e-9);
            viewer::view_rect_to_pdf(b, size, a, (a.0 + w / k, a.1 + h / k))
        }
    };
    let kind = new_field(r);
    r.form_sel = Some((page, rect));
    Some(Pending::Move(page, Box::new(Edit::AddField { page, rect, kind, name: None })))
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
            ..Default::default()
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

/// The selected field ▸ Delete (or the Delete key).
pub fn field_delete(ui: &AppWindow, tab_id: i32) {
    let edit = update(tab_id, |r, _| {
        let (fi, _, page, _) = selected_field(r)?;
        r.form_sel = None;
        Some((page, Edit::DeleteField { name: r.snap.fields[fi].name.clone() }))
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

/// The Form tool's overlay: every field on the pages in view, named, and the selected one's card.
pub(super) fn form_overlay(r: &ReaderUi, v: &ViewerUi, in_view: impl Fn(usize) -> bool) -> (Vec<ReaderFormField>, ReaderFieldProps) {
    if r.tool != FORM {
        return (Vec::new(), ReaderFieldProps::default());
    }
    let sel = selected_field(r);
    let mut list = Vec::new();
    for (i, f) in r.snap.fields.iter().enumerate() {
        for (k, (page, rect, _, _)) in f.widgets.iter().enumerate().filter(|(_, w)| in_view(w.0)) {
            let mut c = v.viewer.overlay(*page, *rect);
            // A field being dragged follows the pointer.
            if let (Some((fi, wi, ..)), Some((_, from, to))) = (sel, r.form_drag)
                && fi == i
                && wi == k
            {
                c[0] += to.0 - from.0;
                c[1] += to.1 - from.1;
            }
            let selected = sel.is_some_and(|s| s.0 == i && s.1 == k);
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
                other_pages: (0..v.viewer.page_count()).any(|p| !f.widgets.iter().any(|w| w.0 == p)),
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
            widgets: Vec::new(),
        }
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
