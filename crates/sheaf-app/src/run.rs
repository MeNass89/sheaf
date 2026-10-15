//! The action button: options → `Job`, where to save, running it, and the result page.

use std::path::PathBuf;

use sheaf_app::jobs::{self, Job, JobOutcome, SplitMode};
use sheaf_app::prefs::OutputMode;
use sheaf_app::{AppWindow, ToolOptions};
use sheaf_core::compress::Level;
use sheaf_core::crop::{self, CropSpec};
use sheaf_core::numbers::{self, NumberOptions, NumberText};
use sheaf_app::jobs::ExportKind;
use sheaf_core::excel::SheetLayout;
use sheaf_core::export::ExportOptions;
use sheaf_core::ocr::OcrOptions;
use sheaf_core::pdfa::PdfALevel;
use sheaf_core::watermark::{Mark, Placement, WatermarkOptions};
use sheaf_core::images::PageLayout;
use sheaf_core::pages::Input;
use sheaf_core::tools::{self, ToolId};
use sheaf_core::{OpdfError, Result};
use slint::ComponentHandle;

use crate::flow::toast;
use crate::dest::{ask_dest, out_name, parent_dir, pref_dest, Dest};
use crate::state::{refresh, with, Done, Phase};

fn parse_level(i: i32) -> Level {
    match i {
        0 => Level::Extreme,
        2 => Level::Low,
        _ => Level::Recommended,
    }
}

fn layout(opts: &ToolOptions) -> PageLayout {
    let margin_pt = [0.0, 18.0, 36.0][opts.margin.clamp(0, 2) as usize];
    match opts.layout {
        1 => PageLayout::A4 { landscape: false, margin_pt },
        2 => PageLayout::A4 { landscape: true, margin_pt },
        _ => PageLayout::FitImage,
    }
}

/// A typed page number ("" = none). Anything else is an error naming the field.
fn page_field(s: &str, what: &str) -> Result<Option<u32>> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    s.parse().ok().filter(|n| *n > 0).map(Some).ok_or_else(|| OpdfError::Unsupported(format!("{what}: \"{s}\" is not a page number")))
}

/// A typed millimetre value: decimal comma or point, "" = 0. `None` if it is not a number ≥ 0.
pub fn parse_mm(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    if s.is_empty() {
        return Some(0.0);
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

/// Whether `code` is in the set `set` (",a,b,").
pub fn has_code(set: &str, code: &str) -> bool {
    set.split(',').any(|c| c == code)
}

/// The set `set` (",a,b,") with `code` added or removed; the order of the others is kept.
pub fn toggle_code(set: &str, code: &str) -> String {
    let mut codes: Vec<&str> = set.split(',').filter(|c| !c.is_empty()).collect();
    match codes.iter().position(|c| *c == code) {
        Some(i) => {
            codes.remove(i);
        }
        None => codes.push(code),
    }
    format!(",{},", codes.join(","))
}

fn ocr_options(opts: &ToolOptions) -> Result<OcrOptions> {
    let language = opts.ocr_languages.split(',').find(|c| !c.is_empty()).ok_or_else(|| OpdfError::Unsupported("choose a document language".into()))?;
    Ok(OcrOptions { language: language.into(), read_text_pages: opts.ocr_all_pages, ..Default::default() })
}

/// A typed millimetre margin, in points.
fn mm_field(s: &str, what: &str) -> Result<f64> {
    parse_mm(s).map(crop::mm).ok_or_else(|| OpdfError::Unsupported(format!("{what} margin: \"{}\" is not a number of millimetres", s.trim())))
}

/// The page range typed in the panel; ignored for a batch (every page of every file).
fn range(opts: &ToolOptions, batch: bool) -> Result<(Option<u32>, Option<u32>)> {
    if batch {
        return Ok((None, None));
    }
    Ok((page_field(&opts.from_page, "From page")?, page_field(&opts.to_page, "To page")?))
}

fn number_options(opts: &ToolOptions, batch: bool) -> Result<NumberOptions> {
    let (from, to) = range(opts, batch)?;
    let start = opts.num_start.trim();
    let start = if start.is_empty() { 1 } else { start.parse().map_err(|_| OpdfError::Unsupported(format!("First number: \"{start}\" is not a number")))? };
    Ok(NumberOptions {
        position: opts.num_position.clamp(0, 8) as u8,
        double_sided: opts.num_double,
        cover: opts.num_cover,
        margin: numbers::MARGINS[opts.num_margin.clamp(0, 2) as usize],
        from,
        to,
        start,
        text: match opts.num_text {
            1 => NumberText::Page,
            2 => NumberText::PageOfTotal,
            3 => NumberText::Custom(opts.num_custom.to_string()),
            _ => NumberText::Number,
        },
        ..Default::default()
    })
}

/// Colours offered for watermark text.
const WM_COLOURS: [[f64; 3]; 4] = [[0.0, 0.0, 0.0], [0.5, 0.5, 0.5], [0.85, 0.1, 0.1], [0.1, 0.3, 0.85]];
const WM_ROTATIONS: [f64; 6] = [0.0, 45.0, 90.0, 180.0, 270.0, -45.0];

fn watermark_options(opts: &ToolOptions, batch: bool) -> Result<WatermarkOptions> {
    let (from, to) = range(opts, batch)?;
    let mark = if opts.wm_image_mode {
        Mark::Image { path: PathBuf::from(opts.wm_image.as_str()), scale: opts.wm_scale.clamp(5, 100) as f64 / 100.0 }
    } else {
        let fs = opts.wm_font_size.trim();
        let font_size = if fs.is_empty() { 48.0 } else { fs.parse::<f64>().ok().filter(|v| (1.0..=500.0).contains(v)).ok_or_else(|| OpdfError::Unsupported(format!("Font size: \"{fs}\"")))? };
        Mark::Text { text: opts.wm_text.to_string(), font_size, color: WM_COLOURS[opts.wm_color.clamp(0, 3) as usize] }
    };
    Ok(WatermarkOptions {
        mark,
        placement: if opts.wm_mosaic { Placement::Mosaic } else { Placement::Grid(opts.wm_position.clamp(0, 8) as u8) },
        opacity: [1.0, 0.75, 0.5, 0.25][opts.wm_transparency.clamp(0, 3) as usize],
        rotation: WM_ROTATIONS[opts.wm_rotation.clamp(0, 5) as usize],
        behind: opts.wm_below,
        from,
        to,
    })
}

fn crop_spec(opts: &ToolOptions) -> Result<CropSpec> {
    if opts.crop_mode == 2 {
        return Ok(CropSpec::Remove);
    }
    Ok(CropSpec::Margins {
        top: mm_field(&opts.crop_top, "Top")?,
        bottom: mm_field(&opts.crop_bottom, "Bottom")?,
        left: mm_field(&opts.crop_left, "Left")?,
        right: mm_field(&opts.crop_right, "Right")?,
    })
}

/// Builds the job. `order` is the Organize grid (1-based pages, 0 = blank).
fn build_job(tool: ToolId, inputs: Vec<Input>, order: Vec<u32>, opts: &ToolOptions, dest: Dest) -> Result<Job> {
    let each = matches!(dest, Dest::NextToEach);
    let (file, folder) = match dest {
        Dest::File(f) => {
            let d = parent_dir(&f);
            (Some(f), d)
        }
        Dest::Folder(d) => (None, d),
        // Folder-writing tools (Split, PDF to JPG) use the first input's folder.
        Dest::NextToEach => (None, inputs.first().map(|i| parent_dir(&i.path)).unwrap_or_else(|| PathBuf::from("."))),
    };
    // Batch pairs: the chosen file for a single input, else default names in the folder
    // (or next to each input).
    let pairs = |inputs: Vec<Input>| -> Vec<(Input, PathBuf)> {
        inputs
            .into_iter()
            .map(|i| {
                let dir = if each { parent_dir(&i.path) } else { folder.clone() };
                let o = file.clone().unwrap_or_else(|| dir.join(out_name(tool, &i.path, opts)));
                (i, o)
            })
            .collect()
    };
    let single = || file.clone().ok_or_else(|| OpdfError::Unsupported("no output file".into()));
    let one = |inputs: Vec<Input>| inputs.into_iter().next().ok_or_else(|| OpdfError::Unsupported("no input".into()));
    let pages = opts.pages.trim().to_string();
    Ok(match tool {
        ToolId::Merge => Job::Merge { inputs, out: single()? },
        ToolId::Split => Job::Split {
            input: one(inputs)?,
            mode: if opts.split_mode == 1 { SplitMode::Every(opts.every.max(1) as u32) } else { SplitMode::Ranges(opts.ranges.trim().into()) },
            out_dir: folder,
        },
        ToolId::RemovePages => Job::RemovePages { input: one(inputs)?, pages, out: single()? },
        ToolId::ExtractPages => Job::ExtractPages { input: one(inputs)?, pages, out: single()? },
        ToolId::Organize => Job::Organize { input: one(inputs)?, order, out: single()? },
        ToolId::Rotate => Job::Rotate {
            items: pairs(inputs),
            degrees: opts.degrees as i64,
            pages: (!pages.is_empty()).then_some(pages),
        },
        ToolId::Compress => Job::Compress { items: pairs(inputs), level: parse_level(opts.level) },
        ToolId::Protect => Job::Protect {
            items: pairs(inputs),
            password: opts.password.to_string(),
            owner: (!opts.owner.is_empty()).then(|| opts.owner.to_string()),
        },
        ToolId::Unlock => Job::Unlock { items: pairs(inputs) },
        ToolId::Repair => Job::Repair { items: pairs(inputs) },
        ToolId::JpgToPdf => Job::JpgToPdf { images: inputs.into_iter().map(|i| i.path).collect(), layout: layout(opts), out: single()? },
        ToolId::PdfToJpg => {
            let (dpi, q) = if opts.quality == 1 { (300, 92) } else { (150, 85) };
            Job::PdfToJpg { inputs, dpi, quality: q, out_dir: folder }
        }
        ToolId::WordToPdf | ToolId::PowerPointToPdf | ToolId::ExcelToPdf | ToolId::HtmlToPdf => {
            Job::Office { items: pairs(inputs).into_iter().map(|(i, o)| (i.path, o)).collect() }
        }
        ToolId::PageNumbers => {
            let options = number_options(opts, inputs.len() > 1)?;
            Job::PageNumbers { items: pairs(inputs), options }
        }
        ToolId::Watermark => {
            let options = watermark_options(opts, inputs.len() > 1)?;
            Job::Watermark { items: pairs(inputs), options }
        }
        ToolId::Crop => Job::Crop {
            input: one(inputs)?,
            pages: (opts.crop_mode == 1 && !pages.is_empty()).then_some(pages),
            spec: crop_spec(opts)?,
            out: single()?,
        },
        ToolId::Ocr => {
            let options = ocr_options(opts)?;
            Job::Ocr { items: pairs(inputs), options }
        }
        ToolId::PdfToWord | ToolId::PdfToPowerPoint | ToolId::PdfToExcel => {
            let kind = match tool {
                ToolId::PdfToWord => ExportKind::Word,
                ToolId::PdfToPowerPoint => ExportKind::PowerPoint,
                _ => ExportKind::Excel(if opts.excel_layout == 1 { SheetLayout::SheetPerTable } else { SheetLayout::OneSheet }),
            };
            Job::Export { items: pairs(inputs), kind, options: ExportOptions { ocr: opts.export_ocr, ..Default::default() } }
        }
        ToolId::PdfToPdfA => Job::PdfA { items: pairs(inputs), level: if opts.pdfa_level == 1 { PdfALevel::A3b } else { PdfALevel::A2b } },
        other => return Err(OpdfError::Unsupported(format!("{} is not available yet", tools::find(other).name))),
    })
}

/// The action button of tab `tab_id`.
pub fn run_tool(ui: &AppWindow, tab_id: i32, opts: ToolOptions) {
    let Some((tool, inputs, order)) = with(|a| {
        let t = a.tab(tab_id)?;
        matches!(t.phase, Phase::Edit).then(|| (t.tool, t.files.iter().map(|f| f.input.clone()).collect::<Vec<_>>(), t.order.clone()))
    }) else {
        return;
    };
    let paths: Vec<PathBuf> = inputs.iter().map(|i| i.path.clone()).collect();
    let prefs = crate::home::prefs();
    let dest = match pref_dest(tool, &paths, &opts, &prefs) {
        Some(d) => Some(d),
        None => {
            if prefs.output == OutputMode::Fixed {
                toast(ui, "Output folder not found");
            }
            // Modal dialog: no state borrow is held here.
            ask_dest(tool, &paths, &opts)
        }
    };
    let Some(dest) = dest else { return };
    let job = match build_job(tool, inputs, order, &opts, dest) {
        Ok(j) => j,
        Err(e) => return toast(ui, &e.to_string()),
    };
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            t.start_job();
        }
        refresh(a, tab_id);
    });
    jobs::spawn(ui.as_weak(), tab_id, job);
}

pub fn on_progress(tab_id: i32, p: f32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id).filter(|t| matches!(t.phase, Phase::Running)) {
            t.progress = p;
        }
        refresh(a, tab_id);
    });
}

pub fn on_done(ui: &AppWindow, tab_id: i32, r: Result<JobOutcome>, page_counts: Vec<Option<u32>>) {
    let Some((tool, inputs)) = with(|a| a.tab(tab_id).map(|t| (t.tool, t.files.iter().map(|f| f.input.path.clone()).collect::<Vec<_>>()))) else {
        return on_done_closed(ui, tab_id, r);
    };
    match r {
        Ok(outcome) => {
            crate::record_history(tool, &inputs, &outcome.outputs);
            // Per-output input sizes (Compress notes): failed inputs produced no output.
            let input_sizes = inputs
                .iter()
                .filter(|p| !outcome.failures.iter().any(|(f, _)| f == *p))
                .map(|p| sheaf_app::files::file_size(p))
                .collect();
            with(|a| {
                if let Some(t) = a.tab(tab_id) {
                    t.finish(Some(Done { outcome, page_counts, input_sizes }));
                }
                refresh(a, tab_id);
            });
        }
        Err(e) => {
            with(|a| {
                if let Some(t) = a.tab(tab_id) {
                    t.finish(None);
                }
                refresh(a, tab_id);
            });
            match e {
                OpdfError::Cancelled => toast(ui, "Cancelled"),
                e => toast(ui, &format!("{} failed: {e}", tools::find(tool).name)),
            }
        }
    }
}

/// A job whose tab was closed meanwhile. A cancelled one was already cleaned up by
/// `jobs::finalize`; a successful one is still recorded and announced.
fn on_done_closed(ui: &AppWindow, tab_id: i32, r: Result<JobOutcome>) {
    let Some((tool, inputs)) = with(|a| a.closed_jobs.remove(&tab_id)) else { return };
    let Ok(outcome) = r else { return };
    crate::record_history(tool, &inputs, &outcome.outputs);
    let first = outcome.outputs.first().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    toast(ui, &format!("{} finished: {first}", tools::find(tool).name));
}

/// Cancel button: the overlay shows "Cancelling…" until the job returns.
pub fn cancel(tab_id: i32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id).filter(|t| matches!(t.phase, Phase::Running)) {
            t.cancelling = true;
        }
        refresh(a, tab_id);
    });
    jobs::cancel(tab_id);
}

fn outputs(tab_id: i32) -> Vec<PathBuf> {
    with(|a| match a.tab(tab_id).map(|t| &t.phase) {
        Some(Phase::Done(d)) => d.outcome.outputs.clone(),
        _ => Vec::new(),
    })
}

/// Runs `open` with `args` and reaps it on a background thread (no zombie processes).
pub fn open_detached<S: AsRef<std::ffi::OsStr>>(args: &[S]) {
    if let Ok(mut child) = std::process::Command::new("open").args(args).spawn() {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

fn open_cmd(args: &[&std::ffi::OsStr]) {
    open_detached(args);
}

pub fn open_output(tab_id: i32, k: i32) {
    if let Some(p) = outputs(tab_id).get(k as usize) {
        open_cmd(&[p.as_os_str()]);
    }
}

pub fn reveal_output(tab_id: i32, k: i32) {
    if let Some(p) = outputs(tab_id).get(k as usize) {
        open_cmd(&["-R".as_ref(), p.as_os_str()]);
    }
}

pub fn open_folder(tab_id: i32) {
    if let Some(p) = outputs(tab_id).first() {
        open_cmd(&["-R".as_ref(), p.as_os_str()]);
    }
}

pub fn start_over(tab_id: i32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            t.phase = Phase::Edit;
            t.progress = 0.0;
            t.files.clear();
            t.order.clear();
            t.page_thumbs.clear();
        }
        refresh(a, tab_id);
    });
}

pub fn continue_to(ui: &AppWindow, tab_id: i32, tool: &str) {
    let Some(next) = crate::flow::tool_from_str(tool) else { return };
    let outs = outputs(tab_id);
    crate::flow::open_tool(ui, next, outs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn opts() -> ToolOptions {
        ToolOptions { level: 1, every: 1, degrees: 90, ..Default::default() }
    }

    #[test]
    fn batch_jobs_write_default_names_in_the_folder() {
        let inputs = vec![Input::new("/a/x.pdf"), Input::new("/b/y.pdf")];
        let job = build_job(ToolId::Compress, inputs, vec![], &opts(), Dest::Folder("/out".into())).unwrap();
        let Job::Compress { items, level } = job else { panic!() };
        assert_eq!(level, Level::Recommended);
        assert_eq!(items[0].1, PathBuf::from("/out/x-compressed.pdf"));
        assert_eq!(items[1].1, PathBuf::from("/out/y-compressed.pdf"));
    }

    #[test]
    fn next_to_each_puts_every_batch_output_beside_its_own_input() {
        let inputs = vec![Input::new("/a/x.pdf"), Input::new("/b/y.pdf")];
        let job = build_job(ToolId::Compress, inputs, vec![], &opts(), Dest::NextToEach).unwrap();
        let Job::Compress { items, .. } = job else { panic!() };
        assert_eq!(items[0].1, PathBuf::from("/a/x-compressed.pdf"));
        assert_eq!(items[1].1, PathBuf::from("/b/y-compressed.pdf"));
        let job = build_job(ToolId::Split, vec![Input::new("/a/x.pdf")], vec![], &opts(), Dest::NextToEach).unwrap();
        assert!(matches!(job, Job::Split { out_dir, .. } if out_dir == Path::new("/a")));
    }

    #[test]
    fn merge_uses_the_typed_name_and_the_save_dialog_path() {
        let mut o = opts();
        o.out_name = "report".into();
        assert_eq!(out_name(ToolId::Merge, Path::new("/a/x.pdf"), &o), "report.pdf");
        assert_eq!(out_name(ToolId::Merge, Path::new("/a/x.pdf"), &opts()), "x-merged.pdf");
        let job = build_job(ToolId::Merge, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/r.pdf".into())).unwrap();
        assert!(matches!(job, Job::Merge { out, .. } if out == Path::new("/s/r.pdf")));
    }

    #[test]
    fn page_numbers_watermark_and_crop_jobs() {
        let mut o = opts();
        o.num_position = 8;
        o.num_margin = 2;
        o.num_text = 3;
        o.num_custom = "{n}/{p}".into();
        o.num_start = "5".into();
        o.from_page = "2".into();
        let job = build_job(ToolId::PageNumbers, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/n.pdf".into())).unwrap();
        let Job::PageNumbers { items, options } = job else { panic!() };
        assert_eq!(items[0].1, PathBuf::from("/s/n.pdf"));
        assert_eq!((options.position, options.margin, options.start, options.from, options.to), (8, 54.0, 5, Some(2), None));
        assert_eq!(options.text, NumberText::Custom("{n}/{p}".into()));
        // A batch numbers every page of every file, with default names.
        let two = vec![Input::new("/a/x.pdf"), Input::new("/a/y.pdf")];
        let Job::PageNumbers { items, options } = build_job(ToolId::PageNumbers, two, vec![], &o, Dest::Folder("/o".into())).unwrap() else { panic!() };
        assert_eq!((options.from, items[1].1.clone()), (None, PathBuf::from("/o/y-numbered.pdf")));
        o.from_page = "x".into();
        assert!(build_job(ToolId::PageNumbers, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/n.pdf".into())).is_err());

        let mut o = opts();
        o.wm_text = "DRAFT".into();
        o.wm_font_size = "30".into();
        o.wm_transparency = 2;
        o.wm_rotation = 5;
        o.wm_mosaic = true;
        o.wm_below = true;
        let Job::Watermark { options, .. } = build_job(ToolId::Watermark, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/w.pdf".into())).unwrap() else { panic!() };
        assert_eq!(options.mark, Mark::Text { text: "DRAFT".into(), font_size: 30.0, color: [0.0; 3] });
        assert_eq!((options.placement, options.opacity, options.rotation, options.behind), (Placement::Mosaic, 0.5, -45.0, true));
        o.wm_image_mode = true;
        o.wm_image = "/i/logo.png".into();
        o.wm_scale = 25;
        let Job::Watermark { options, .. } = build_job(ToolId::Watermark, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/w.pdf".into())).unwrap() else { panic!() };
        assert_eq!(options.mark, Mark::Image { path: "/i/logo.png".into(), scale: 0.25 });

        let mut o = opts();
        o.crop_mode = 1;
        o.pages = "2-3".into();
        o.crop_top = "10".into();
        o.crop_left = "2,5".into();
        let Job::Crop { pages, spec, out, .. } = build_job(ToolId::Crop, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/c.pdf".into())).unwrap() else { panic!() };
        assert_eq!((pages.as_deref(), out), (Some("2-3"), PathBuf::from("/s/c.pdf")));
        assert_eq!(spec, CropSpec::Margins { top: crop::mm(10.0), bottom: 0.0, left: crop::mm(2.5), right: 0.0 });
        o.crop_mode = 2;
        let Job::Crop { pages, spec, .. } = build_job(ToolId::Crop, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/c.pdf".into())).unwrap() else { panic!() };
        assert_eq!((pages, spec), (None, CropSpec::Remove));
        assert_eq!(out_name(ToolId::Crop, Path::new("/a/x.pdf"), &o), "x-cropped.pdf");
    }

    #[test]
    fn pdfa_job() {
        let mut o = opts();
        let two = vec![Input::new("/a/x.pdf"), Input::new("/a/y.pdf")];
        let Job::PdfA { items, level } = build_job(ToolId::PdfToPdfA, two, vec![], &o, Dest::Folder("/o".into())).unwrap() else { panic!() };
        assert_eq!((level, items[1].1.clone()), (PdfALevel::A2b, PathBuf::from("/o/y-pdfa.pdf")));
        o.pdfa_level = 1;
        let Job::PdfA { level, .. } = build_job(ToolId::PdfToPdfA, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/p.pdf".into())).unwrap() else { panic!() };
        assert_eq!(level, PdfALevel::A3b);
    }

    #[test]
    fn ocr_job_uses_the_first_selected_language() {
        assert_eq!(toggle_code(",en,", "fr"), ",en,fr,");
        assert_eq!(toggle_code(",en,fr,", "en"), ",fr,");
        assert_eq!(toggle_code(",en,", "en"), ",,");
        assert!(has_code(",en,fr,", "fr") && !has_code(",en,", "e"));
        let mut o = opts();
        o.ocr_languages = ",fr,en,".into();
        o.ocr_all_pages = true;
        let Job::Ocr { items, options } = build_job(ToolId::Ocr, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/o.pdf".into())).unwrap() else { panic!() };
        assert_eq!((options.language.as_str(), options.read_text_pages, items[0].1.clone()), ("fr", true, PathBuf::from("/s/o.pdf")));
        o.ocr_languages = ",,".into();
        assert!(build_job(ToolId::Ocr, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/o.pdf".into())).is_err());
    }

    #[test]
    fn export_jobs() {
        let mut o = opts();
        let two = vec![Input::new("/a/x.pdf"), Input::new("/a/y.pdf")];
        let Job::Export { items, kind, options } = build_job(ToolId::PdfToWord, two, vec![], &o, Dest::Folder("/o".into())).unwrap() else { panic!() };
        assert_eq!((kind, options.ocr, items[1].1.clone()), (ExportKind::Word, false, PathBuf::from("/o/y.docx")));
        o.export_ocr = true;
        o.excel_layout = 1;
        let Job::Export { kind, options, .. } = build_job(ToolId::PdfToExcel, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/t.xlsx".into())).unwrap() else { panic!() };
        assert_eq!((kind, options.ocr), (ExportKind::Excel(SheetLayout::SheetPerTable), true));
        assert_eq!(out_name(ToolId::PdfToPowerPoint, Path::new("/a/x.pdf"), &o), "x.pptx");
    }

    #[test]
    fn millimetres_accept_a_decimal_comma() {
        assert_eq!(parse_mm("2,5"), Some(2.5));
        assert_eq!(parse_mm(" 2.5 "), Some(2.5));
        assert_eq!(parse_mm(""), Some(0.0));
        assert_eq!(parse_mm("-1"), None);
        assert_eq!(parse_mm("x"), None);
    }

    #[test]
    fn jpg_layout_and_split_mode() {
        let mut o = opts();
        o.layout = 2;
        o.margin = 1;
        assert_eq!(layout(&o), PageLayout::A4 { landscape: true, margin_pt: 18.0 });
        o.split_mode = 1;
        o.every = 3;
        let job = build_job(ToolId::Split, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::Folder("/o".into())).unwrap();
        assert!(matches!(job, Job::Split { mode: SplitMode::Every(3), .. }));
    }
}
