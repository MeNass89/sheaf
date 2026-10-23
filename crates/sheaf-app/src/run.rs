//! The action button: options → `Job`, where to save, running it, and the result page.

use std::path::PathBuf;

use sheaf_app::jobs::{self, Job, JobOutcome, SplitMode};
use sheaf_app::prefs::OutputMode;
use sheaf_app::{AppWindow, ToolOptions};
use sheaf_core::compress::Level;
use sheaf_core::crop::{self, CropSpec};
use sheaf_core::numbers::{self, standard_font, NumberOptions, NumberText};
use sheaf_app::jobs::ExportKind;
use sheaf_core::compare::CompareMode;
use sheaf_core::excel::SheetLayout;
use sheaf_core::export::ExportOptions;
use sheaf_core::ocr::OcrOptions;
use sheaf_core::pdfa::PdfALevel;
use sheaf_core::watermark::{Mark, Placement, WatermarkOptions};
use sheaf_core::images::{PageLayout, Paper};
use sheaf_core::pages::Input;
use sheaf_core::tools::{self, ToolId};
use sheaf_core::{OpdfError, Result};
use slint::ComponentHandle;

use crate::flow::toast;
use crate::dest::{ask_dest, out_name, parent_dir, pref_dest, Dest};
use crate::state::{refresh, with, Done, Phase};

/// Strength levels used by the Protect panel's three-segment meter.
pub fn password_strength(password: &str) -> i32 {
    use sheaf_core::security::PasswordStrength;
    match sheaf_core::security::password_strength(password) {
        PasswordStrength::Weak => 0,
        PasswordStrength::Fair => 1,
        PasswordStrength::Strong => 2,
    }
}

fn parse_level(i: i32) -> Level {
    match i {
        0 => Level::Extreme,
        2 => Level::Low,
        _ => Level::Recommended,
    }
}

fn layout(opts: &ToolOptions) -> PageLayout {
    let margin_pt = [0.0, 18.0, 36.0][opts.margin.clamp(0, 2) as usize];
    let paper = match opts.layout {
        1 => Paper::A4,
        2 => Paper::Letter,
        _ => return PageLayout::FitImage,
    };
    PageLayout::Paper { paper, landscape: opts.landscape, margin_pt }
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

/// A page number typed in the panel (`None` when empty or not a page number).
pub fn page_number(s: &str) -> Option<u32> {
    page_field(s, "page").ok().flatten()
}

/// The watermark the panel describes, for the preview of one page (no range).
pub fn preview_watermark_options(opts: &ToolOptions) -> Result<WatermarkOptions> {
    Ok(WatermarkOptions { from: None, to: None, ..watermark_options(opts, false)? })
}

/// The page range typed in the panel; for a batch it applies to every file, a "to" past a
/// file's end meaning its last page (see `clamp_to_last`).
fn range(opts: &ToolOptions) -> Result<(Option<u32>, Option<u32>)> {
    Ok((page_field(&opts.from_page, "from page")?, page_field(&opts.to_page, "to page")?))
}

fn number_options(opts: &ToolOptions, batch: bool) -> Result<NumberOptions> {
    let (from, to) = range(opts)?;
    let start = opts.num_start.trim();
    let start = if start.is_empty() { 1 } else { start.parse().map_err(|_| OpdfError::Unsupported(format!("first number: \"{start}\" is not a number")))? };
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
        font_size: {
            let fs = opts.num_font_size.trim();
            if fs.is_empty() { 12.0 } else { fs.parse::<f64>().ok().filter(|v| (4.0..=72.0).contains(v)).ok_or_else(|| OpdfError::Unsupported(format!("font size: \"{fs}\" (use 4 to 72 points)")))? }
        },
        color: text_colour(opts.num_color, &opts.num_hex),
        font: standard_font(opts.num_family.clamp(0, 2) as u8, opts.num_bold, opts.num_italic),
        underline: opts.num_underline,
        clamp_to_last: batch,
    })
}

/// Colours offered for watermark and page number text.
const WM_COLOURS: [[f64; 3]; 4] = [[0.0, 0.0, 0.0], [0.5, 0.5, 0.5], [0.85, 0.1, 0.1], [0.1, 0.3, 0.85]];

/// The text colour of a panel: the RRGGBB typed, else preset `k` (the panel refuses a bad hex).
pub fn text_colour(k: i32, hex: &str) -> [f64; 3] {
    numbers::parse_hex_colour(hex).unwrap_or(WM_COLOURS[k.clamp(0, 3) as usize])
}
const WM_ROTATIONS: [f64; 6] = [0.0, 45.0, 90.0, 180.0, 270.0, -45.0];

fn watermark_options(opts: &ToolOptions, batch: bool) -> Result<WatermarkOptions> {
    let (from, to) = range(opts)?;
    let mark = if opts.wm_image_mode {
        Mark::Image { path: PathBuf::from(opts.wm_image.as_str()), scale: opts.wm_scale.clamp(5, 100) as f64 / 100.0 }
    } else {
        let fs = opts.wm_font_size.trim();
        let font_size = if fs.is_empty() { 48.0 } else { fs.parse::<f64>().ok().filter(|v| (1.0..=500.0).contains(v)).ok_or_else(|| OpdfError::Unsupported(format!("font size: \"{fs}\" (use 1 to 500 points)")))? };
        Mark::Text { text: opts.wm_text.to_string(), font_size, color: text_colour(opts.wm_color, &opts.wm_hex) }
    };
    Ok(WatermarkOptions {
        mark,
        placement: if opts.wm_mosaic { Placement::Mosaic } else { Placement::Grid(opts.wm_position.clamp(0, 8) as u8) },
        opacity: [1.0, 0.75, 0.5, 0.25][opts.wm_transparency.clamp(0, 3) as usize],
        rotation: WM_ROTATIONS[opts.wm_rotation.clamp(0, 5) as usize],
        behind: opts.wm_below,
        from,
        to,
        font: standard_font(opts.wm_family.clamp(0, 2) as u8, opts.wm_bold, opts.wm_italic),
        underline: opts.wm_underline,
        clamp_to_last: batch,
    })
}

fn crop_spec(opts: &ToolOptions) -> Result<CropSpec> {
    if opts.crop_mode == 2 {
        return Ok(CropSpec::Remove);
    }
    Ok(CropSpec::Margins {
        top: mm_field(&opts.crop_top, "top")?,
        bottom: mm_field(&opts.crop_bottom, "bottom")?,
        left: mm_field(&opts.crop_left, "left")?,
        right: mm_field(&opts.crop_right, "right")?,
    })
}

/// Builds the job. `order` is the Organize grid (1-based pages, 0 = blank).
#[cfg(test)]
fn build_job(tool: ToolId, inputs: Vec<Input>, order: Vec<u32>, opts: &ToolOptions, dest: Dest) -> Result<Job> {
    build_job_with(tool, inputs, order, Vec::new(), opts, dest)
}

/// Builds the job. `order` is the Organize grid (1-based pages, 0 = blank); `marks` are
/// Redact's areas.
#[cfg(test)]
fn build_job_with(tool: ToolId, inputs: Vec<Input>, order: Vec<u32>, marks: Vec<sheaf_core::redact::Mark>, opts: &ToolOptions, dest: Dest) -> Result<Job> {
    build_job_full(tool, inputs, order, marks, None, opts, dest)
}

/// Sign: the graphic, the page and the box (view space, points) placed in the viewer.
pub type Placed = (sheaf_core::sign::Graphic, usize, [f64; 4]);

/// What was placed on the pages in the viewer.
#[derive(Debug, Clone, PartialEq)]
pub enum OnPages {
    Sign(Placed),
    Edit(Vec<sheaf_app::jobs::EditPlaced>),
}

/// [`build_job_with`], with what was placed in the page viewer (Sign, Edit).
fn build_job_full(tool: ToolId, inputs: Vec<Input>, order: Vec<u32>, marks: Vec<sheaf_core::redact::Mark>, placed: Option<OnPages>, opts: &ToolOptions, dest: Dest) -> Result<Job> {
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
            permissions: sheaf_core::security::ProtectPermissions {
                printing: opts.allow_printing,
                copying: opts.allow_copying,
                editing: opts.allow_editing,
                commenting: opts.allow_commenting,
                filling_forms: opts.allow_filling_forms,
            },
        },
        ToolId::Unlock => Job::Unlock { items: pairs(inputs) },
        ToolId::Repair => Job::Repair { items: pairs(inputs) },
        ToolId::JpgToPdf if opts.one_per_image && inputs.len() > 1 => {
            Job::ImagesEach { items: pairs(inputs).into_iter().map(|(i, o)| (i.path, o)).collect(), layout: layout(opts) }
        }
        ToolId::JpgToPdf => Job::JpgToPdf { images: inputs.into_iter().map(|i| i.path).collect(), layout: layout(opts), out: single()? },
        ToolId::PdfToJpg => {
            let (dpi, q) = match opts.quality { 2 => (600, 92), 1 => (300, 92), _ => (150, 85) };
            Job::PdfToJpg { inputs, dpi, quality: (!opts.png).then_some(q), out_dir: folder }
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
        ToolId::Compare => {
            let mut it = inputs.into_iter();
            let (Some(old), Some(new), None) = (it.next(), it.next(), it.next()) else {
                return Err(OpdfError::Unsupported("Compare needs an original and a changed PDF".into()));
            };
            Job::Compare { old, new, mode: if opts.compare_mode == 1 { CompareMode::Overlay } else { CompareMode::Text }, out: single()? }
        }
        ToolId::Redact => {
            if marks.is_empty() {
                return Err(OpdfError::Unsupported("Mark something for redaction first".into()));
            }
            Job::Redact { input: one(inputs)?, marks, out: single()? }
        }
        ToolId::Sign => {
            let Some(OnPages::Sign((graphic, page, view_box))) = placed else {
                return Err(OpdfError::Unsupported("Create your signature and click on a page to place it".into()));
            };
            Job::Sign { input: one(inputs)?, page, view_box, graphic, digital: crate::sign_ui::digital(opts)?, out: single()? }
        }
        ToolId::Edit => {
            let Some(OnPages::Edit(items)) = placed.filter(|p| !matches!(p, OnPages::Edit(v) if v.is_empty())) else {
                return Err(OpdfError::Unsupported("Click on a page to add text or an image".into()));
            };
            Job::Edit { input: one(inputs)?, items, out: single()? }
        }
        ToolId::PdfToPdfA => Job::PdfA { items: pairs(inputs), level: if opts.pdfa_level == 1 { PdfALevel::A3b } else { PdfALevel::A2b } },
    })
}

/// The action button of tab `tab_id`.
pub fn run_tool(ui: &AppWindow, tab_id: i32, opts: ToolOptions) {
    let Some((tool, inputs, order, marks, placed)) = with(|a| {
        let t = a.tab(tab_id)?;
        let marks = t.viewer.as_ref().map(|v| v.marks()).unwrap_or_default();
        let placed = match t.tool {
            ToolId::Edit => Some(t.edit.placed().map(OnPages::Edit)),
            _ => t.viewer.as_ref().and_then(|v| crate::viewer_ui::placed(v, &t.sign)).map(|p| Ok(OnPages::Sign(p))),
        };
        matches!(t.phase, Phase::Edit).then(|| (t.tool, t.files.iter().map(|f| f.input.clone()).collect::<Vec<_>>(), t.order.clone(), marks, placed))
    }) else {
        return;
    };
    // An item not ready (an empty text box) is said before the save dialog opens.
    let placed = match placed.transpose() {
        Ok(p) => p,
        Err(e) => return toast(ui, &e),
    };
    let paths: Vec<PathBuf> = inputs.iter().map(|i| i.path.clone()).collect();
    let prefs = crate::home::prefs();
    match pref_dest(tool, &paths, &opts, &prefs) {
        Some(dest) => start(ui, tab_id, tool, inputs, order, marks, placed, &opts, dest),
        None => {
            if prefs.output == OutputMode::Fixed {
                toast(ui, "Output folder not found");
            }
            // The dialog answers later, on the UI thread; the tab is checked again then.
            let o = opts.clone();
            ask_dest(ui, tool, &paths, &opts, move |ui, dest| {
                let still = with(|a| a.tab(tab_id).map(|t| matches!(t.phase, Phase::Edit))).unwrap_or(false);
                if still {
                    start(ui, tab_id, tool, inputs, order, marks, placed, &o, dest);
                }
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn start(ui: &AppWindow, tab_id: i32, tool: ToolId, inputs: Vec<Input>, order: Vec<u32>, marks: Vec<sheaf_core::redact::Mark>, placed: Option<OnPages>, opts: &ToolOptions, dest: Dest) {
    let job = match build_job_full(tool, inputs, order, marks, placed, opts, dest) {
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

/// Spawns `cmd` and reaps it on a background thread (no zombie processes).
fn spawn_detached(cmd: &str, args: &[&std::ffi::OsStr]) {
    if let Ok(mut child) = std::process::Command::new(cmd).args(args).spawn() {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

/// Opens a file or URL in its default app.
pub fn open_detached(target: impl AsRef<std::ffi::OsStr>) {
    let t = target.as_ref();
    #[cfg(target_os = "macos")]
    spawn_detached("open", &[t]);
    #[cfg(windows)]
    spawn_detached("explorer", &[t]);
    #[cfg(not(any(target_os = "macos", windows)))]
    spawn_detached("xdg-open", &[t]);
}

/// Shows a file selected in its folder (Linux: opens the folder).
pub fn reveal(p: &std::path::Path) {
    #[cfg(target_os = "macos")]
    spawn_detached("open", &["-R".as_ref(), p.as_os_str()]);
    #[cfg(windows)]
    {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(p.as_os_str());
        spawn_detached("explorer", &[&arg]);
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    if let Some(dir) = p.parent() {
        spawn_detached("xdg-open", &[dir.as_os_str()]);
    }
}

pub fn open_output(tab_id: i32, k: i32) {
    if let Some(p) = outputs(tab_id).get(k as usize) {
        open_detached(p);
    }
}

pub fn reveal_output(tab_id: i32, k: i32) {
    if let Some(p) = outputs(tab_id).get(k as usize) {
        reveal(p);
    }
}

pub fn open_folder(tab_id: i32) {
    if let Some(p) = outputs(tab_id).first() {
        reveal(p);
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
            t.viewer = None;
            t.viewer_error = None;
            t.sign = crate::sign_ui::SignState::new();
            t.edit = crate::edit_ui::EditState::new();
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
        o.num_font_size = "18".into();
        o.num_color = 2;
        o.num_start = "5".into();
        o.from_page = "2".into();
        let job = build_job(ToolId::PageNumbers, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/n.pdf".into())).unwrap();
        let Job::PageNumbers { items, options } = job else { panic!() };
        assert_eq!(items[0].1, PathBuf::from("/s/n.pdf"));
        assert_eq!((options.position, options.margin, options.start, options.from, options.to), (8, 54.0, 5, Some(2), None));
        assert_eq!(options.text, NumberText::Custom("{n}/{p}".into()));
        assert_eq!((options.font_size, options.color), (18.0, [0.85, 0.1, 0.1]));
        // A batch takes the same range for every file, a "to" past a file's end meaning its last
        // page, with default names.
        let two = vec![Input::new("/a/x.pdf"), Input::new("/a/y.pdf")];
        (o.num_family, o.num_bold) = (2, true);
        let Job::PageNumbers { items, options } = build_job(ToolId::PageNumbers, two, vec![], &o, Dest::Folder("/o".into())).unwrap() else { panic!() };
        assert_eq!((options.from, options.clamp_to_last, items[1].1.clone()), (Some(2), true, PathBuf::from("/o/y-numbered.pdf")));
        assert_eq!(options.font, Some(numbers::Std14::CourierBold));
        o.from_page = "x".into();
        let e = build_job(ToolId::PageNumbers, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/n.pdf".into())).unwrap_err().to_string();
        // Lowercase after the prefix, like the core's messages.
        assert_eq!(e, "unsupported input: from page: \"x\" is not a page number");

        let mut o = opts();
        o.wm_text = "DRAFT".into();
        o.wm_font_size = "30".into();
        o.wm_transparency = 2;
        o.wm_rotation = 5;
        o.wm_mosaic = true;
        o.wm_below = true;
        (o.wm_family, o.wm_bold, o.wm_italic) = (1, true, true);
        let Job::Watermark { options, .. } = build_job(ToolId::Watermark, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/w.pdf".into())).unwrap() else { panic!() };
        assert_eq!(options.mark, Mark::Text { text: "DRAFT".into(), font_size: 30.0, color: [0.0; 3] });
        assert_eq!(options.font, Some(numbers::Std14::TimesBoldItalic));
        o.wm_hex = "#1F6FEB".into();
        o.wm_underline = true;
        let Job::Watermark { options, .. } = build_job(ToolId::Watermark, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/w.pdf".into())).unwrap() else { panic!() };
        assert_eq!(options.mark, Mark::Text { text: "DRAFT".into(), font_size: 30.0, color: [31.0 / 255.0, 111.0 / 255.0, 235.0 / 255.0] });
        assert!(options.underline);
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
    fn sign_job_needs_a_placed_signature_and_names_the_output_signed() {
        let one = || vec![Input::new("/a/v.pdf")];
        let o = ToolOptions::default();
        let dest = || Dest::File("/s/v-signed.pdf".into());
        assert!(build_job_full(ToolId::Sign, one(), vec![], vec![], None, &o, dest()).is_err());
        let placed = (sheaf_core::sign::Graphic::Typed("Ada".into(), Default::default()), 2, [10.0, 20.0, 160.0, 60.0]);
        let Job::Sign { input, page, view_box, digital, out, .. } = build_job_full(ToolId::Sign, one(), vec![], vec![], Some(OnPages::Sign(placed.clone())), &o, dest()).unwrap() else { panic!() };
        assert_eq!((input.path, page, view_box, digital, out), (PathBuf::from("/a/v.pdf"), 2, [10.0, 20.0, 160.0, 60.0], None, PathBuf::from("/s/v-signed.pdf")));
        assert_eq!(out_name(ToolId::Sign, Path::new("/a/v.pdf"), &o), "v-signed.pdf");
        // A certificate asked for but not chosen: refused before anything runs.
        let o = ToolOptions { sign_cert: true, ..Default::default() };
        assert!(build_job_full(ToolId::Sign, one(), vec![], vec![], Some(OnPages::Sign(placed)), &o, dest()).is_err());
    }

    #[test]
    fn edit_job_needs_items_and_names_the_output_edited() {
        use sheaf_app::jobs::EditPlaced;
        let one = || vec![Input::new("/a/v.pdf")];
        let o = ToolOptions::default();
        let dest = || Dest::File("/s/v-edited.pdf".into());
        assert!(build_job_full(ToolId::Edit, one(), vec![], vec![], None, &o, dest()).is_err());
        assert!(build_job_full(ToolId::Edit, one(), vec![], vec![], Some(OnPages::Edit(vec![])), &o, dest()).is_err());
        let items = vec![EditPlaced::Text { page: 1, at: [10.0, 20.0], text: "Hi".into(), size: 12.0, color: [0; 3] }];
        let Job::Edit { input, items: got, out } = build_job_full(ToolId::Edit, one(), vec![], vec![], Some(OnPages::Edit(items.clone())), &o, dest()).unwrap() else { panic!() };
        assert_eq!((input.path, got, out), (PathBuf::from("/a/v.pdf"), items, PathBuf::from("/s/v-edited.pdf")));
        assert_eq!(out_name(ToolId::Edit, Path::new("/a/v.pdf"), &o), "v-edited.pdf");
    }

    #[test]
    fn redact_job_needs_marks() {
        let o = opts();
        let one = || vec![Input::new("/a/v.pdf")];
        let dest = || Dest::File("/s/v-redacted.pdf".into());
        assert!(build_job_with(ToolId::Redact, one(), vec![], vec![], &o, dest()).is_err());
        let marks = sheaf_core::redact::mark_rects(&[(0, [1.0, 2.0, 30.0, 40.0])]);
        let Job::Redact { input, marks: m, out } = build_job_with(ToolId::Redact, one(), vec![], marks.clone(), &o, dest()).unwrap() else { panic!() };
        assert_eq!((input.path, m, out), (PathBuf::from("/a/v.pdf"), marks, PathBuf::from("/s/v-redacted.pdf")));
        assert_eq!(out_name(ToolId::Redact, Path::new("/a/v.pdf"), &o), "v-redacted.pdf");
    }

    #[test]
    fn compare_job_takes_original_then_changed() {
        let mut o = opts();
        o.compare_mode = 1;
        let two = vec![Input::new("/a/v1.pdf"), Input::new("/a/v2.pdf")];
        let Job::Compare { old, new, mode, out } = build_job(ToolId::Compare, two, vec![], &o, Dest::File("/s/v1-comparison.pdf".into())).unwrap() else { panic!() };
        assert_eq!((old.path, new.path, mode, out), (PathBuf::from("/a/v1.pdf"), PathBuf::from("/a/v2.pdf"), CompareMode::Overlay, PathBuf::from("/s/v1-comparison.pdf")));
        assert!(build_job(ToolId::Compare, vec![Input::new("/a/v1.pdf")], vec![], &o, Dest::File("/s/c.pdf".into())).is_err());
        assert_eq!(out_name(ToolId::Compare, Path::new("/a/v1.pdf"), &o), "v1-comparison.pdf");
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
        o.landscape = true;
        o.margin = 1;
        assert_eq!(layout(&o), PageLayout::Paper { paper: Paper::Letter, landscape: true, margin_pt: 18.0 });
        // "Merge all images" off: a PDF for each image, next to it.
        o.one_per_image = true;
        let job = build_job(ToolId::JpgToPdf, vec![Input::new("/a/x.png"), Input::new("/b/y.jpg")], vec![], &o, Dest::NextToEach).unwrap();
        let Job::ImagesEach { items, .. } = job else { panic!("one PDF each") };
        assert_eq!(items, [("/a/x.png".into(), "/a/x.pdf".into()), ("/b/y.jpg".into(), "/b/y.pdf".into())]);
        o.split_mode = 1;
        o.every = 3;
        let job = build_job(ToolId::Split, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::Folder("/o".into())).unwrap();
        assert!(matches!(job, Job::Split { mode: SplitMode::Every(3), .. }));
    }
}
