//! Print-production exports from the reader's current document, on its document thread.

use super::*;

/// The report stays a dated snapshot: saving it cannot silently inspect a different edit.
#[derive(Default)]
pub(super) struct State {
    pub open: bool,
    pub busy: bool,
    pub report: Option<String>,
}

/// Acrobat's Print Production ▸ Preflight: inspect on the document thread, read here.
pub fn print_preflight(ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| r.prepress.open = true);
    preflight_check(ui, tab_id, "150", "300", "2");
}

pub fn preflight_check(ui: &AppWindow, tab_id: i32, fail: &str, target: &str, bleed: &str) {
    let parse = || -> Option<sheaf_core::preflight::Profile> {
        Some(sheaf_core::preflight::Profile {
            fail_ppi: fail.trim().parse().ok()?,
            warn_ppi: target.trim().parse().ok()?,
            min_bleed_mm: bleed.trim().parse().ok()?,
        })
    };
    let Some(profile) = parse().filter(|p| sheaf_core::preflight::validate(*p).is_ok()) else {
        crate::flow::toast(ui, "Use image thresholds from 1 to 9600 ppi (fail ≤ target), and bleed from 0 to 50 mm");
        return;
    };
    update(tab_id, |r, _| {
        if r.prepress.busy { return; }
        r.prepress.busy = true;
        r.prepress.report = None;
        r.in_flight += 1;
        r.send(Op::PrintPreflight { profile });
    });
}

pub(super) fn checked(ui: &AppWindow, tab_id: i32, report: Result<String, String>) {
    let error = report.as_ref().err().cloned();
    update(tab_id, |r, _| {
        r.prepress.busy = false;
        r.prepress.report = report.ok();
    });
    if let Some(error) = error { crate::flow::toast(ui, &format!("Preflight could not finish: {error}")); }
}

pub fn preflight_close(tab_id: i32) {
    update(tab_id, |r, _| r.prepress.open = false);
}

pub fn preflight_page(_ui: &AppWindow, tab_id: i32, page: i32) {
    if page <= 0 { return; }
    update(tab_id, |r, v| {
        r.prepress.open = false;
        if (page as usize) <= v.viewer.page_count() { v.show_page(page as usize - 1, None); }
    });
}

pub fn preflight_save(ui: &AppWindow, tab_id: i32) {
    let Some((dir, stem, text)) = update(tab_id, |r, _| {
        Some((crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), r.prepress.report.clone()?))
    }).flatten() else { return };
    let d = crate::dialog::new(ui).set_title("Save print preflight report").set_directory(&dir).set_file_name(format!("{stem} - print preflight.txt")).add_filter("Text report", &["txt"]);
    crate::dialog::save_file(ui, d, move |ui, to| {
        match sheaf_core::paths::atomic_write(&to, |part| { std::fs::write(part, text.as_bytes())?; Ok(()) }) {
            Ok(to) => crate::flow::toast(ui, &format!("Report saved as {}", file_name(&to))),
            Err(e) => crate::flow::toast(ui, &format!("The report was not saved: {e}")),
        }
    });
}

/// Each finding is split, never elided: one title and at most four 40-character lines.
/// The complete report remains available to save, with no shortened diagnostic conditions.
pub(super) fn rows(report: &str) -> Vec<ReaderPrintCheck> {
    let mut rows = Vec::new();
    for line in report.lines().filter(|l| !l.is_empty()) {
        let page = line.strip_prefix("Page ").or_else(|| line.split_once(" on page ").map(|(_, rest)| rest))
            .and_then(|n| n.split(|c: char| !c.is_ascii_digit()).next()?.parse::<i32>().ok()).unwrap_or(0);
        let mut wrapped = Vec::new();
        let mut current = String::new();
        for word in line.split_whitespace() {
            if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > 40 {
                wrapped.push(std::mem::take(&mut current));
            }
            if !current.is_empty() { current.push(' '); }
            current.push_str(word);
        }
        if !current.is_empty() { wrapped.push(current); }
        for (k, chunk) in wrapped.chunks(4).enumerate() {
            rows.push(ReaderPrintCheck {
                title: if k > 0 { "Continued" } else if page > 0 { "Page finding" } else { "Print preflight" }.into(),
                detail: chunk.join("\n").into(), page,
            });
        }
    }
    rows
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_rows_keep_every_condition_and_split_long_findings() {
        let text = "Image Im1 on page 12: 72 ppi, below 300 for close-up print. Enlarging it would not add detail. Nothing was added or repaired; check the original artwork before using a different print profile.";
        let rows = rows(text);
        assert!(rows.len() > 1);
        assert!(rows.iter().all(|r| r.page == 12 && r.detail.lines().count() <= 4));
        assert!(rows.iter().all(|r| r.detail.lines().all(|l| l.chars().count() <= 40)));
        assert_eq!(rows.iter().map(|r| r.detail.as_str()).collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>(), text.split_whitespace().collect::<Vec<_>>());
    }
}
