//! Form filling: the field under a click, its editor, Tab order, check boxes and choices.

use super::*;

/// The fillable form field under view point `p` of `page`, with its widget's box.
pub(super) fn field_at(r: &ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) -> Option<(usize, [f64; 4])> {
    let size = v.boxes[page].view_size();
    r.snap.fields.iter().enumerate().filter(|(_, f)| !f.read_only && f.kind < 4).find_map(|(i, f)| {
        f.widgets.iter().filter(|w| w.0 == page).find_map(|w| {
            let [x, y, ww, hh] = viewer::pdf_rect_to_view(&v.boxes[page], size, w.1);
            (p.0 >= x && p.0 <= x + ww && p.1 >= y && p.1 <= y + hh).then_some((i, w.1))
        })
    })
}

/// Opens field `fi` (its widget at `rect` on `page`): an editor for text and choices; a check
/// box or radio button is set at once. Returns whether an editor opened.
pub(super) fn open_field(r: &mut ReaderUi, v: &ViewerUi, page: usize, fi: usize, rect: [f64; 4]) -> bool {
    let f = &r.snap.fields[fi];
    match f.kind {
        1 => {
            let on = f.value.first().is_some_and(|x| x != "Off");
            let edit = Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Check(!on) };
            let _ = page;
            r.pending_field = Some(edit);
            false
        }
        2 => {
            let state = f.widgets.iter().find(|w| w.0 == page && w.1 == rect).and_then(|w| w.2.clone());
            let edit = Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Radio(state) };
            r.pending_field = Some(edit);
            false
        }
        0 | 3 => {
            let size = v.viewer.boxes[page].view_size();
            let [x, y, _, _] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
            let text = if f.kind == 0 { f.value.join("") } else { String::new() };
            r.draft = Some(Draft { page, at: (x, y), kind: if f.kind == 0 { 4 } else { 5 }, block: Some((0, text, rect)), field: Some((fi, rect)) });
            r.note_seq += 1;
            true
        }
        _ => false,
    }
}

/// Field editor: Tab saves the field and opens the next one in the form's tab order.
pub fn field_tab(ui: &AppWindow, tab_id: i32, text: &str, back: bool) {
    let next = with(|a| {
        let t = a.tab(tab_id)?;
        let r = t.reader.as_ref()?;
        let (fi, rect) = r.draft.as_ref()?.field?;
        let order = |i: usize, rc: [f64; 4]| r.snap.fields[i].widgets.iter().find(|w| w.1 == rc).map(|w| w.3).unwrap_or(usize::MAX);
        let here = order(fi, rect);
        let mut all: Vec<(usize, usize, usize, [f64; 4])> = r
            .snap
            .fields
            .iter()
            .enumerate()
            // Text fields only: a check box reached by Tab must not change.
            .filter(|(_, f)| !f.read_only && f.kind == 0)
            .flat_map(|(i, f)| f.widgets.iter().map(move |w| (w.3, i, w.0, w.1)))
            .collect();
        all.sort_by_key(|w| w.0);
        let pos = all.iter().position(|w| w.0 == here && w.1 == fi)?;
        let n = all.len();
        let k = if back { (pos + n - 1) % n } else { (pos + 1) % n };
        Some((all[k].1, all[k].2, all[k].3))
    });
    note_commit(ui, tab_id, text);
    if let Some((fi, page, rect)) = next {
        update(tab_id, |r, v| {
            let size = v.viewer.boxes[page].view_size();
            let [_, y, _, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
            let top = v.viewer.page_rect(page)[1] + y / k(&v.viewer, page);
            if top < v.top || top + h > v.top + v.height {
                v.show_page(page, Some(rect));
            }
            open_field(r, v, page, fi, rect);
        });
        apply_pending_field(ui, tab_id);
    }
}

/// Clear the form (back to its defaults) or flatten it into the pages.
pub fn form_op(ui: &AppWindow, tab_id: i32, op: i32) {
    // Acrobat's Prepare Form ▸ More ▸ Export Data and Import Data: the field values to a file, or
    // from one (XFDF, FDF, XML, CSV or tab-separated text with a header row of field names).
    if op == 3 {
        let Some((dir, stem)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())) else { return };
        let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(format!("{stem} - data.xfdf")).add_filter("XFDF", &["xfdf"]).add_filter("FDF", &["fdf"]).add_filter("XML", &["xml"]).add_filter("CSV", &["csv"]).add_filter("Text", &["txt"]);
        return crate::dialog::save_file(ui, d, move |_, target| {
            update(tab_id, |r, _| r.send(Op::ExportFormData { to: target }));
        });
    }
    if op == 4 {
        let d = crate::dialog::new(ui).add_filter("Form data", &["xfdf", "fdf", "xml", "csv", "txt"]);
        return crate::dialog::pick_file(ui, d, move |ui, path| {
            let Ok(bytes) = std::fs::read(&path) else {
                return crate::flow::toast(ui, "That file could not be read");
            };
            apply_structure(ui, tab_id, Edit::ImportData { name: file_name(&path), bytes: Arc::new(bytes) });
        });
    }
    let edit = match op {
        0 => Edit::ResetForm { names: None },
        1 => Edit::Flatten { comments: false, fields: true },
        _ => Edit::Flatten { comments: true, fields: true },
    };
    apply_structure(ui, tab_id, edit);
}

/// A choice picked in a field's list.
pub fn choice_picked(ui: &AppWindow, tab_id: i32, k: i32) {
    let edit = update(tab_id, |r, _| {
        let d = r.draft.take()?;
        let (fi, _) = d.field?;
        let f = r.snap.fields.get(fi)?;
        let (export, _) = f.options.get(k.max(0) as usize)?;
        Some(Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Choice(vec![export.clone()]) })
    })
    .flatten();
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[], None);
    }
}

/// Form edits re-render every page: a field's widgets (a radio group) and its scripts can reach
/// other pages.
pub(super) fn apply_pending_field(ui: &AppWindow, tab_id: i32) {
    if let Some(e) = update(tab_id, |r, _| r.pending_field.take()).flatten() {
        apply(ui, tab_id, e, &[], None);
    }
}
