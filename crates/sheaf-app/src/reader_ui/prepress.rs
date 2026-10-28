//! Print-production snapshots from the reader; slow press conversion runs in the background.

use super::*;

/// The report stays a dated snapshot: saving it cannot silently inspect a different edit.
#[derive(Default)]
pub(super) struct State {
    pub open: bool,
    pub production_open: bool,
    pub press_profile: Option<PathBuf>,
    pub press_condition: String,
    pub press_busy: bool,
    pub busy: bool,
    pub report: Option<String>,
    pub rows: ModelRc<ReaderPrintCheck>,
}

/// Acrobat's Print Production ▸ Preflight: inspect on the document thread, read here.
pub fn print_preflight(ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| r.prepress.open = true);
    preflight_check(ui, tab_id, "150", "300", "2", 0);
}

pub fn preflight_check(ui: &AppWindow, tab_id: i32, fail: &str, target: &str, bleed: &str, preset: i32) {
    let parse = || -> Option<sheaf_core::preflight::Profile> {
        Some(sheaf_core::preflight::Profile {
            fail_ppi: fail.trim().parse().ok()?,
            warn_ppi: target.trim().parse().ok()?,
            min_bleed_mm: bleed.trim().parse().ok()?,
        })
    };
    let Some(profile) = parse().filter(|p| sheaf_core::preflight::validate(*p).is_ok()) else {
        crate::flow::toast(ui, &sheaf_app::i18n::tr("Use image thresholds from 1 to 9600 ppi (fail ≤ target), and bleed from 0 to 50 mm"));
        return;
    };
    let preset = match preset { 2 => sheaf_core::preflight::Preset::DigitalPress, 3 => sheaf_core::preflight::Preset::PdfX1a, 4 => sheaf_core::preflight::Preset::PdfX4, _ => sheaf_core::preflight::Preset::Custom };
    update(tab_id, |r, _| {
        if r.prepress.busy { return; }
        r.prepress.busy = true;
        r.prepress.report = None;
        r.prepress.rows = ModelRc::default();
        r.send(Op::PrintPreflight { profile, preset });
    });
}

pub(super) fn checked(ui: &AppWindow, tab_id: i32, report: Result<String, String>) {
    let error = report.as_ref().err().cloned();
    update(tab_id, |r, _| {
        r.prepress.busy = false;
        r.prepress.report = report.ok();
        r.prepress.rows = ModelRc::new(VecModel::from(rows(r.prepress.report.as_deref().unwrap_or_default())));
    });
    if let Some(error) = error { crate::flow::toast(ui, &sheaf_app::i18n::trf("Preflight could not finish: {}", &[&error])); }
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
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Save print preflight report")).set_directory(&dir).set_file_name(format!("{stem} - print preflight.txt")).add_filter("Text report", &["txt"]);
    crate::dialog::save_file(ui, d, move |ui, to| {
        match sheaf_core::paths::atomic_write(&to, |part| { std::fs::write(part, text.as_bytes())?; Ok(()) }) {
            Ok(to) => crate::flow::toast(ui, &sheaf_app::i18n::trf("Report saved as {}", &[&file_name(&to)])),
            Err(e) => crate::flow::toast(ui, &sheaf_app::i18n::trf("The report was not saved: {}", &[&e])),
        }
    });
}

/// Each finding is split, never elided: one title and at most four 40-character lines.
/// The complete report remains available to save, with no shortened diagnostic conditions.
pub(super) fn rows(report: &str) -> Vec<ReaderPrintCheck> {
    let mut rows = Vec::new();
    for line in report.lines().filter(|l| !l.is_empty()) {
        let mut words = line.split_ascii_whitespace();
        let page = if line.starts_with("Page ") || line.starts_with("Image ") || line.starts_with("- ") {
            words.find(|w| w.eq_ignore_ascii_case("page"))
                .and_then(|_| words.next()?.trim_end_matches(':').parse::<i32>().ok()).unwrap_or(0)
        } else { 0 };
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

/// Both print commands open the same settings sheet before changing the document.
pub fn fix_page_hairlines(_ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| r.prepress.production_open = true);
}

pub fn add_crop_marks(ui: &AppWindow, tab_id: i32) {
    fix_page_hairlines(ui, tab_id);
}

pub fn production_close(tab_id: i32) {
    update(tab_id, |r, _| r.prepress.production_open = false);
}

fn production_pages(tab_id: i32, all: bool) -> Option<Vec<usize>> {
    update(tab_id, |r, v| if all { (0..v.viewer.page_count()).collect() } else { super::pages::op_pages(r, v) })
}

pub fn production_hairlines(ui: &AppWindow, tab_id: i32, minimum: &str, all: bool) {
    let minimum = minimum.trim().parse::<f64>().ok().filter(|v| v.is_finite() && (0.01..=10.0).contains(v));
    let Some(minimum) = minimum else {
        crate::flow::toast(ui, &sheaf_app::i18n::tr("Use a minimum stroke from 0.01 to 10 points"));
        return;
    };
    let Some(pages) = production_pages(tab_id, all) else { return };
    production_close(tab_id);
    apply(ui, tab_id, Edit::FixPageHairlines { pages: pages.clone(), minimum }, &pages, None);
}

pub fn production_marks(ui: &AppWindow, tab_id: i32, all: bool, style: i32, width: &str) {
    let width = width.trim().parse::<f64>().ok().filter(|v| v.is_finite() && (0.1..=2.0).contains(v));
    let Some(width) = width else { crate::flow::toast(ui, &sheaf_app::i18n::tr("Use a mark width from 0.1 to 2 points")); return };
    let Some(pages) = production_pages(tab_id, all) else { return };
    production_close(tab_id);
    apply(ui, tab_id, Edit::AddPrinterMarks {
        pages: pages.clone(), registration: style == 1 || style == 2,
        colour_bars: style == 1 || style == 3, page_information: style == 1 || style == 4, width,
    }, &pages, None);
}

/// Acrobat's Set Page Boxes: margins refer to each sheet's media box, before page rotation.
fn box_edit(pages: Vec<usize>, which: i32, margins: [&str; 4], reset: bool) -> Result<Edit, String> {
    use sheaf_core::pdfcraft::engine::{BoxSpec, PageBox};
    let which = match which { 0 => PageBox::Crop, 1 => PageBox::Trim, 2 => PageBox::Bleed, 3 => PageBox::Art, _ => return Err("Choose crop, trim, bleed or art box".into()) };
    let spec = if reset { BoxSpec::Remove } else {
        let mut values = [0.0; 4];
        for (i, text) in margins.into_iter().enumerate() {
            values[i] = text.trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or("Margins must be finite numbers, zero or more")?;
        }
        BoxSpec::Margins(values)
    };
    Ok(Edit::SetPageBox { pages, which, spec })
}

pub fn production_box(ui: &AppWindow, tab_id: i32, which: i32, margins: [&str; 4], reset: bool, all: bool) {
    let Some(pages) = production_pages(tab_id, all) else { return };
    match box_edit(pages.clone(), which, margins, reset) {
        Ok(edit) => {
            production_close(tab_id);
            apply(ui, tab_id, edit, &pages, None);
        }
        Err(error) => crate::flow::toast(ui, &error),
    }
}

/// Acrobat's Save as PDF/X: the operator supplies the actual press profile and condition.
pub fn press_profile(ui: &AppWindow, tab_id: i32) {
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Choose press ICC profile")).add_filter("ICC profile", &["icc", "icm"]);
    crate::dialog::pick_file(ui, d, move |_, path| { update(tab_id, |r, _| { r.prepress.press_profile = Some(path); r.prepress.press_condition.clear(); }); });
}

pub fn press_preset(ui: &AppWindow, tab_id: i32, preset: i32) {
    use sheaf_core::press::{Preset, installed_profile};
    let preset = match preset { 0 => Preset::Fogra39, 1 => Preset::Gracol, 2 => Preset::Swop, _ => return };
    let Some(path) = installed_profile(preset) else {
        crate::flow::toast(ui, &sheaf_app::i18n::trf("{} is not installed. Install its ICC locally, or choose the press's ICC file", &[&preset.label()]));
        return;
    };
    update(tab_id, |r, _| { r.prepress.press_profile = Some(path); r.prepress.press_condition = preset.label().into(); });
}

pub fn press_export(ui: &AppWindow, tab_id: i32, condition: &str, level: i32) {
    let Some((dir, stem, profile, preset_condition, busy)) = update(tab_id, |r, _| {
        (crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), r.prepress.press_profile.clone(), r.prepress.press_condition.clone(), r.prepress.press_busy)
    }) else { return };
    if busy { return; }
    let Some(profile) = profile else { crate::flow::toast(ui, &sheaf_app::i18n::tr("Choose the press's CMYK or Gray ICC profile first")); return };
    let level = match level { 0 => sheaf_core::press::Level::X3, 1 => sheaf_core::press::Level::X1a, 2 => sheaf_core::press::Level::X4, _ => return };
    let condition = if condition.trim().is_empty() { preset_condition } else { condition.to_string() };
    let options = sheaf_core::press::Options { level, profile, condition };
    if options.condition.trim().is_empty() { crate::flow::toast(ui, &sheaf_app::i18n::tr("Name the press output condition")); return; }
    let d = crate::dialog::new(ui).set_title(format!("Save {} copy", level.label())).set_directory(&dir).set_file_name(format!("{stem} - {}.pdf", level.label().replace('/', "").replace(':', "-"))).add_filter("PDF", &["pdf"]);
    crate::dialog::save_file(ui, d, move |ui, to| {
        update(tab_id, |r, _| {
            if r.prepress.press_busy { return; }
            r.prepress.press_busy = true;
            r.prepress.production_open = false;
            r.send(Op::ExportPdfX { to, options });
        });
        crate::flow::toast(ui, &sheaf_app::i18n::tr("Exporting a PDF/X snapshot in the background; you can keep editing"));
    });
}

pub(super) fn press_exported(ui: &AppWindow, tab_id: i32, result: Result<PathBuf, String>) {
    update(tab_id, |r, _| {
        r.prepress.press_busy = false;
    });
    match result {
        Ok(path) => crate::flow::toast(ui, &sheaf_app::i18n::trf("PDF/X copy saved as {}; full ISO validation was not performed", &[&file_name(&path)])),
        Err(error) => crate::flow::toast(ui, &sheaf_app::i18n::trf("PDF/X copy was not saved: {}", &[&error])),
    }
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
        assert_eq!(super::rows("- page 2: a missing appearance")[0].page, 2);
        assert_eq!(super::rows("- [warn] Page 3 has too little bleed")[0].page, 3);
        assert_eq!(super::rows("Document: page 9")[0].page, 0);
        assert!(rows.iter().all(|r| r.detail.lines().all(|l| l.chars().count() <= 40)));
        assert_eq!(rows.iter().map(|r| r.detail.as_str()).collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>(), text.split_whitespace().collect::<Vec<_>>());
    }
    #[test]
    fn print_boxes_preserve_margin_order_and_reject_bad_numbers() {
        use sheaf_core::pdfcraft::engine::{BoxSpec, PageBox};
        let edit = box_edit(vec![1, 4], 1, ["10", "20", "30", "40"], false).unwrap();
        let Edit::SetPageBox { pages, which, spec } = edit else { panic!("page box edit") };
        assert_eq!(pages, [1, 4]);
        assert_eq!(which, PageBox::Trim);
        assert_eq!(spec, BoxSpec::Margins([10.0, 20.0, 30.0, 40.0]));
        assert!(box_edit(vec![0], 0, ["NaN", "0", "0", "0"], false).is_err());
        assert!(box_edit(vec![0], 0, ["-1", "0", "0", "0"], false).is_err());
        assert!(matches!(box_edit(vec![0], 2, ["", "", "", ""], true).unwrap(), Edit::SetPageBox { which: PageBox::Bleed, spec: BoxSpec::Remove, .. }));
    }

}
