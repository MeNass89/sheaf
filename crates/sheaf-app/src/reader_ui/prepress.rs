//! Print-production exports from the reader's current document, on its document thread.

use super::*;

/// Acrobat's Print Production ▸ Preflight: save the measured findings and the rules checked.
pub fn print_preflight(ui: &AppWindow, tab_id: i32) {
    let Some((dir, stem)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())) else { return };
    let d = crate::dialog::new(ui).set_title("Save print preflight report").set_directory(&dir).set_file_name(format!("{stem} - print preflight.txt")).add_filter("Text report", &["txt"]);
    crate::dialog::save_file(ui, d, move |_, to| {
        update(tab_id, |r, _| {
            r.in_flight += 1;
            r.send(Op::PrintPreflight { to });
        });
    });
}

/// Fix the selected pages, or the page shown. One click is one undo step; Form artwork and
/// annotation appearances are left alone (the command says "page hairlines").
pub fn fix_page_hairlines(ui: &AppWindow, tab_id: i32) {
    let Some(pages) = update(tab_id, |r, v| super::pages::op_pages(r, v)) else { return };
    apply(ui, tab_id, Edit::FixPageHairlines { pages: pages.clone(), minimum: 0.25 }, &pages, None);
    crate::flow::toast(ui, "Fixing page strokes only; grouped artwork and comments are unchanged");
}

/// Acrobat's Add Printer Marks: black crop marks outside the bleed, with a larger sheet.
pub fn add_crop_marks(ui: &AppWindow, tab_id: i32) {
    let Some(pages) = update(tab_id, |r, v| super::pages::op_pages(r, v)) else { return };
    apply(ui, tab_id, Edit::AddCropMarks { pages: pages.clone() }, &pages, None);
}
