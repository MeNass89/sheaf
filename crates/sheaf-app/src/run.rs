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
use crate::dest::{ask_dest, out_name, parent_dir, pref_dest, text_format, Dest};
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
    Ok(OcrOptions { language: language.into(), read_text_pages: opts.ocr_all_pages, enhance: opts.ocr_enhance, ..Default::default() })
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
            4 => NumberText::Nothing,
            5 => NumberText::Remove,
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
        texts: [&opts.hf_tl, &opts.hf_tc, &opts.hf_tr, &opts.hf_bl, &opts.hf_bc, &opts.hf_br].map(|t| t.to_string()),
        bates: numbers::Bates {
            digits: opts.bates_digits.clamp(3, 15) as u8,
            start: {
                let s = opts.bates_start.trim();
                if s.is_empty() { 1 } else { s.parse().map_err(|_| OpdfError::Unsupported(format!("Bates number: \"{s}\" is not a number")))? }
            },
            prefix: opts.bates_prefix.trim().to_string(),
            suffix: String::new(),
        },
        replace: opts.hf_replace,
    })
}

/// Split by size: the largest file typed in MB ("2", "1.5"), in bytes.
fn split_size(typed: &str) -> Result<u64> {
    let t = typed.trim().replace(',', ".");
    t.parse::<f64>().ok().filter(|mb| mb.is_finite() && *mb > 0.0).map(|mb| (mb * 1_048_576.0) as u64).ok_or_else(|| OpdfError::Unsupported(format!("file size: \"{}\" (type megabytes, such as 2 or 1.5)", typed.trim())))
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
        replace: opts.wm_replace,
        remove: opts.wm_remove,
    })
}

fn crop_spec(opts: &ToolOptions) -> Result<CropSpec> {
    if opts.crop_mode == 2 {
        return Ok(CropSpec::Remove);
    }
    // Acrobat's Remove White Margins: each page to what is drawn on it, 3 pt around.
    if opts.crop_mode == 3 {
        return Ok(CropSpec::WhiteMargins { pad: 3.0 });
    }
    // Acrobat's Change Page Size: a paper of `pages::PAPER`.
    if opts.crop_mode == 4 {
        let (_, w, h) = sheaf_core::pages::PAPER[opts.crop_paper.clamp(0, 4) as usize];
        return Ok(CropSpec::Resize { size: (w, h) });
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
        ToolId::Merge => Job::Merge { inputs, pages: Vec::new(), seq: Vec::new(), out: single()? },
        ToolId::Split => Job::Split {
            input: one(inputs)?,
            mode: match (opts.split_mode, opts.split_one_file) {
                (1, _) => SplitMode::Every(opts.every.max(1) as u32),
                (2, _) => SplitMode::Bookmarks,
                (3, _) => SplitMode::Size(split_size(&opts.split_size)?),
                (4, _) => SplitMode::Grid(opts.split_cols.clamp(1, 10) as u32, opts.split_rows.clamp(1, 10) as u32),
                (_, true) => SplitMode::OneFile(opts.ranges.trim().into()),
                _ => SplitMode::Ranges(opts.ranges.trim().into()),
            },
            out_dir: folder,
        },
        ToolId::RemovePages => Job::RemovePages { input: one(inputs)?, pages: if opts.remove_blank { format!("{}:{}", jobs::BLANK_PAGES, opts.blank_level.clamp(0, 2)) } else { pages }, out: single()? },
        ToolId::ExtractPages => Job::ExtractPages { input: one(inputs)?, pages, out: single()? },
        ToolId::Organize => Job::Organize {
            input: one(inputs.clone())?,
            // An order typed replaces the grid's (its pages counted in the file itself).
            order: match opts.organize_order.trim() {
                "" => order,
                typed => {
                    let n = inputs.first().and_then(|i| std::fs::read(&i.path).ok()).map_or(0, |b| sheaf_core::pages::count_in(&b)) as u32;
                    sheaf_core::ranges::order_spec(typed, n)?
                }
            },
            out: single()?, per_sheet: [1, 2, 4, 6, 9][opts.per_sheet.clamp(0, 4) as usize], booklet: opts.per_sheet == 5, long: opts.per_sheet == 6 },
        ToolId::Rotate => Job::Rotate {
            items: pairs(inputs),
            degrees: opts.degrees as i64,
            pages: (!pages.is_empty()).then_some(pages),
            filter: sheaf_core::rotate::PageFilter { parity: opts.rotate_parity.clamp(0, 2) as u8, orientation: opts.rotate_orientation.clamp(0, 2) as u8 },
        },
        ToolId::Compress => Job::Compress {
            items: pairs(inputs),
            level: if opts.level == 3 { Level::Target(split_size(&opts.compress_target)?) } else { parse_level(opts.level) },
            advanced: sheaf_core::compress::Advanced {
                max_ppi: opts.compress_ppi.trim().parse().ok(),
                quality: opts.compress_quality.trim().parse().ok(),
                discard_thumbnails: opts.discard_thumbs,
                discard_tags: opts.discard_tags,
                discard_metadata: opts.discard_meta,
                grayscale: opts.compress_gray,
                unembed_standard: opts.unembed_fonts,
            },
        },
        ToolId::Protect => Job::Protect {
            items: pairs(inputs),
            sanitize: opts.protect_sanitize,
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
        ToolId::AdjustColours => Job::AdjustColours {
            items: pairs(inputs),
            adjust: sheaf_core::adjust::Adjust {
                brightness: opts.adjust_brightness.clamp(-100, 100) as f32 / 100.0,
                contrast: opts.adjust_contrast.clamp(-100, 100) as f32 / 100.0,
                saturation: opts.adjust_saturation.clamp(-100, 100) as f32 / 100.0,
                invert: opts.adjust_invert,
            },
        },
        ToolId::JpgToPdf if opts.one_per_image && inputs.len() > 1 => {
            Job::ImagesEach { items: pairs(inputs).into_iter().map(|(i, o)| (i.path, o)).collect(), layout: layout(opts) }
        }
        ToolId::JpgToPdf => Job::JpgToPdf { images: inputs.into_iter().map(|i| i.path).collect(), layout: layout(opts), out: single()? },
        ToolId::PdfToJpg => {
            let (dpi, q) = match opts.quality { 2 => (600, 92), 1 => (300, 92), _ => (150, 85) };
            Job::PdfToJpg { inputs, dpi, quality: (!opts.png).then_some(q), extract: opts.extract_images, out_dir: folder }
        }
        ToolId::WordToPdf | ToolId::PowerPointToPdf | ToolId::ExcelToPdf | ToolId::HtmlToPdf => {
            let html = (tool == ToolId::HtmlToPdf && opts.layout != 0).then_some(sheaf_core::html::Options {
                paper: if opts.layout == 2 { sheaf_core::html::Paper::Letter } else { sheaf_core::html::Paper::A4 },
                landscape: opts.landscape,
                margin: [0.0, 18.0, 36.0][opts.margin.clamp(0, 2) as usize],
            });
            Job::Office { items: pairs(inputs).into_iter().map(|(i, o)| (i.path, o)).collect(), html, whole_sheets: tool == ToolId::ExcelToPdf && opts.layout == 1, data_sheets: (tool == ToolId::ExcelToPdf && opts.layout == 2).then(|| opts.pages.to_string()) }
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
        ToolId::PdfToWord | ToolId::PdfToPowerPoint | ToolId::PdfToExcel | ToolId::PdfToText => {
            let kind = match tool {
                ToolId::PdfToWord => ExportKind::Word,
                ToolId::PdfToPowerPoint => ExportKind::PowerPoint,
                ToolId::PdfToText => ExportKind::Text(text_format(opts)),
                _ => ExportKind::Excel(if opts.excel_layout == 1 { SheetLayout::SheetPerTable } else { SheetLayout::OneSheet }),
            };
            Job::Export { items: pairs(inputs), kind, options: ExportOptions { ocr: opts.export_ocr, exact_layout: opts.export_exact, ..Default::default() } }
        }
        ToolId::Compare => {
            let mut it = inputs.into_iter();
            let (Some(old), Some(new), None) = (it.next(), it.next(), it.next()) else {
                return Err(OpdfError::Unsupported("Compare needs an original and a changed PDF".into()));
            };
            let range = |s: &slint::SharedString| (!s.trim().is_empty()).then(|| s.trim().to_string());
            Job::Compare { old, new, mode: [CompareMode::Text, CompareMode::Overlay, CompareMode::SideBySide][opts.compare_mode.clamp(0, 2) as usize], out: single()?, pages: [range(&opts.compare_old_pages), range(&opts.compare_new_pages)] }
        }
        ToolId::Redact => {
            if marks.is_empty() {
                return Err(OpdfError::Unsupported("Mark something for redaction first".into()));
            }
            let fill = [[0.0; 3], [1.0; 3], [0.5; 3], [0.85, 0.1, 0.1]][opts.redact_fill.clamp(0, 3) as usize];
            let margin = opts.redact_margin.trim().trim_end_matches("pt").trim().replace(',', ".").parse::<f64>().unwrap_or(0.0);
            let look = sheaf_core::redact::Appearance { fill, overlay: opts.redact_overlay.to_string(), repeat: opts.redact_repeat, margin };
            Job::Redact { input: one(inputs)?, marks, look, out: single()? }
        }
        ToolId::Sign => {
            // Certifying with a digital ID needs no signature on a page (Acrobat's Certify
            // (invisible signature)).
            let digital = crate::sign_ui::digital(opts)?;
            if placed.is_none()
                && let Some(d) = digital.clone().filter(|d| d.certify.is_some())
            {
                return Ok(Job::Certify { input: one(inputs)?, digital: d, out: single()? });
            }
            let Some(OnPages::Sign((graphic, page, view_box))) = placed else {
                return Err(OpdfError::Unsupported("Create your signature and click on a page to place it, or certify with a digital ID".into()));
            };
            Job::Sign { input: one(inputs)?, page, view_box, graphic, digital, out: single()? }
        }
        ToolId::Edit => {
            let Some(OnPages::Edit(items)) = placed.filter(|p| !matches!(p, OnPages::Edit(v) if v.is_empty())) else {
                return Err(OpdfError::Unsupported("Click on a page to add text or an image".into()));
            };
            Job::Edit { input: one(inputs)?, items, out: single()? }
        }
        ToolId::PdfToPdfA => Job::PdfA { items: pairs(inputs), level: if opts.pdfa_level == 1 { PdfALevel::A3b } else { PdfALevel::A2b }, allow_incomplete: opts.pdfa_allow_incomplete },
    })
}

/// The action button of tab `tab_id`.
pub fn run_tool(ui: &AppWindow, tab_id: i32, opts: ToolOptions) {
    let Some((tool, inputs, specs, order, marks, placed)) = with(|a| {
        let t = a.tab(tab_id)?;
        let marks = t.viewer.as_ref().map(|v| v.marks()).unwrap_or_default();
        let placed = match t.tool {
            ToolId::Edit => Some(t.edit.placed().map(OnPages::Edit)),
            _ => t.viewer.as_ref().and_then(|v| crate::viewer_ui::placed(v, &t.sign)).map(|p| Ok(OnPages::Sign(p))),
        };
        let specs: Vec<String> = t.files.iter().map(|f| f.pages_spec.clone()).collect();
        matches!(t.phase, Phase::Edit).then(|| (t.tool, t.files.iter().map(|f| f.input.clone()).collect::<Vec<_>>(), specs, t.order.clone(), marks, placed))
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
        Some(dest) => start(ui, tab_id, tool, (inputs, specs), order, marks, placed, &opts, dest),
        None => {
            if prefs.output == OutputMode::Fixed {
                toast(ui, "Output folder not found");
            }
            // The dialog answers later, on the UI thread; the tab is checked again then.
            let o = opts.clone();
            ask_dest(ui, tool, &paths, &opts, move |ui, dest| {
                let still = with(|a| a.tab(tab_id).map(|t| matches!(t.phase, Phase::Edit))).unwrap_or(false);
                if still {
                    start(ui, tab_id, tool, (inputs, specs), order, marks, placed, &o, dest);
                }
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// `inputs`: the files, each with its Merge page choice ("" = all).
fn start(ui: &AppWindow, tab_id: i32, tool: ToolId, inputs: (Vec<Input>, Vec<String>), order: Vec<u32>, marks: Vec<sheaf_core::redact::Mark>, placed: Option<OnPages>, opts: &ToolOptions, dest: Dest) {
    let (inputs, specs) = inputs;
    let mut job = match build_job_full(tool, inputs, order, marks, placed, opts, dest) {
        Ok(j) => j,
        Err(e) => return toast(ui, &e.to_string()),
    };
    if let Job::Merge { pages, seq, .. } = &mut job {
        *pages = specs;
        // The Pages view: its order, each page by its file's place among the inputs.
        *seq = with(|a| a.tab(tab_id).and_then(|t| t.merge_seq())).unwrap_or_default();
    }
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            t.start_job();
            t.options = Some(opts.clone());
        }
        refresh(a, tab_id);
    });
    // PDFs made from other files carry the identity name as their Author (Acrobat's Create PDF).
    let author = matches!(job, Job::JpgToPdf { .. } | Job::ImagesEach { .. } | Job::Office { .. }).then(crate::home::author);
    jobs::spawn(ui.as_weak(), tab_id, job, author);
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
    let Some((tool, inputs, options)) = with(|a| a.tab(tab_id).map(|t| (t.tool, t.files.iter().map(|f| f.input.path.clone()).collect::<Vec<_>>(), t.options.clone()))) else {
        return on_done_closed(ui, tab_id, r);
    };
    match r {
        Ok(outcome) => {
            crate::record_history(tool, &inputs, &outcome.outputs, options.as_ref());
            // A long job says it is done with a system notification (Acrobat's, Smallpdf's): the
            // user has often gone to another window meanwhile.
            let took = with(|a| a.tab(tab_id).and_then(|t| t.started).map(|s| s.elapsed()));
            if took.is_some_and(|d| d >= NOTIFY_AFTER) || (cfg!(debug_assertions) && std::env::var_os("SHEAF_TEST_NOTIFY").is_some()) {
                let n = outcome.outputs.len();
                let body = match n {
                    1 => format!("{} is ready", file_name_of(&outcome.outputs[0])),
                    _ => format!("{n} files are ready"),
                };
                notify(&format!("{} finished", tools::find(tool).name), &body);
            }
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

/// `text` spoken by the system's voice (macOS's `say`, eSpeak NG or Speech Dispatcher on Linux,
/// Windows's speech synthesizer), at `speed` (0 normal, 1 slow, 2 fast); the process speaking,
/// to wait for or stop. Debug builds append the text to the file `SHEAF_TEST_SPEECH` names instead
/// (gui-smoke), with a process that ends at once.
pub fn speak(text: &str, speed: u8, voice: Option<&str>) -> Option<std::process::Child> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    if cfg!(debug_assertions)
        && let Some(log) = std::env::var_os("SHEAF_TEST_SPEECH")
    {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log).ok()?;
        writeln!(f, "{text}").ok()?;
        return Command::new("true").spawn().ok();
    }
    #[cfg(not(windows))]
    let wpm = [180, 140, 240][speed.min(2) as usize].to_string();
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("say");
        c.args(["-r", &wpm, "-f", "-"]);
        if let Some(v) = voice {
            c.args(["-v", v]);
        }
        c
    };
    #[cfg(windows)]
    let mut cmd = {
        let rate = ["0", "-3", "3"][speed.min(2) as usize];
        // A new reading starts unpaused.
        let _ = std::fs::remove_file(speech_control());
        let mut c = Command::new("powershell");
        c.args([
            "-NoProfile",
            "-Command",
            &format!(
                // Spoken asynchronously; the control file says pause or resume (Pause reading).
                "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Rate = {rate}; {}$null = $s.SpeakAsync([Console]::In.ReadToEnd()); \
                 $f = '{}'; while ($s.State -ne 'Ready') {{ Start-Sleep -Milliseconds 150; $c = Get-Content $f -ErrorAction SilentlyContinue; \
                 if ($c -eq 'pause' -and $s.State -eq 'Speaking') {{ $s.Pause() }} elseif ($c -eq 'resume' -and $s.State -eq 'Paused') {{ $s.Resume() }} }}",
                voice.map(|v| format!("$s.SelectVoice('{}'); ", v.replace('\'', "''"))).unwrap_or_default(),
                speech_control().display().to_string().replace('\'', "''")
            ),
        ]);
        c
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let mut cmd = {
        let mut c = Command::new("espeak-ng");
        c.args(["-s", &wpm, "--stdin"]);
        if let Some(v) = voice {
            c.args(["-v", v]);
        }
        c
    };
    let mut child = match cmd.stdin(Stdio::piped()).spawn() {
        Ok(c) => c,
        // Linux without eSpeak NG: Speech Dispatcher, which takes the text as an argument.
        #[cfg(not(any(target_os = "macos", windows)))]
        Err(_) => {
            let rate = ["0", "-40", "50"][speed.min(2) as usize];
            return Command::new("spd-say").args(["-w", "-r", rate, text]).spawn().ok();
        }
        #[cfg(any(target_os = "macos", windows))]
        Err(_) => return None,
    };
    let mut stdin = child.stdin.take()?;
    let text = text.to_string();
    // Written from a thread: a long page fills the pipe before the voice reads it.
    std::thread::spawn(move || {
        let _ = stdin.write_all(text.as_bytes());
    });
    Some(child)
}

/// File `f` put on the clipboard as a file (Finder's, Explorer's and file managers' copy), to paste
/// in a folder, a chat or a mail. Debug builds write its path to `SHEAF_TEST_CLIPFILE` (gui-smoke).
pub fn copy_file_to_clipboard(f: &std::path::Path) -> bool {
    if cfg!(debug_assertions)
        && let Some(log) = std::env::var_os("SHEAF_TEST_CLIPFILE")
    {
        return std::fs::write(log, f.to_string_lossy().as_bytes()).is_ok();
    }
    let p = f.to_string_lossy();
    #[cfg(target_os = "macos")]
    return std::process::Command::new("osascript").args(["-e", &format!("set the clipboard to (POSIX file \"{}\")", p.replace('"', "\\\""))]).status().is_ok_and(|s| s.success());
    #[cfg(windows)]
    return std::process::Command::new("powershell").args(["-NoProfile", "-Command", &format!("Set-Clipboard -Path '{}'", p.replace('\'', "''"))]).status().is_ok_and(|s| s.success());
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        use std::io::Write;
        let uri = format!("file://{}", p.replace(' ', "%20"));
        let gnome = format!("copy\n{uri}");
        let put = |target: &str, text: &str| {
            std::process::Command::new("xclip")
                .args(["-selection", "clipboard", "-t", target])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut c| {
                    c.stdin.take().map(|mut s| s.write_all(text.as_bytes())).transpose()?;
                    c.wait()
                })
                .is_ok_and(|s| s.success())
        };
        // GNOME's file managers read their own target; others the URI list.
        put("x-special/gnome-copied-files", &gnome) || put("text/uri-list", &uri) || std::process::Command::new("wl-copy").args(["--type", "text/uri-list", &uri]).status().is_ok_and(|s| s.success())
    }
}

/// macOS's AirDrop sheet with file `f`; false elsewhere or when it cannot open.
pub fn airdrop(f: &std::path::Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "ObjC.import('AppKit'); var s = $.NSSharingService.sharingServiceNamed($.NSSharingServiceNameSendViaAirDrop); var u = $.NSURL.fileURLWithPath('{}'); s.canPerformWithItems([u]) ? (s.performWithItems([u]), 'ok') : 'no'",
            f.display().to_string().replace('\'', "\\'")
        );
        return std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", &script]).output().is_ok_and(|o| o.stdout.starts_with(b"ok"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = f;
        false
    }
}

/// The file Windows's voice reads "pause" or "resume" from while it speaks.
#[cfg(not(unix))]
pub fn speech_control() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("sheaf-speech-{}.ctl", std::process::id()))
}

/// The system's voices by name (macOS's `say -v ?`, eSpeak NG's, Windows's installed voices).
pub fn voices() -> Vec<String> {
    let out = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    #[cfg(target_os = "macos")]
    let list = parse_voices(&out("say", &["-v", "?"]), 0);
    #[cfg(windows)]
    let list: Vec<String> = out("powershell", &["-NoProfile", "-Command", "Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() | % { $_.VoiceInfo.Name }"]).lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    #[cfg(not(any(target_os = "macos", windows)))]
    let list = parse_voices(&out("espeak-ng", &["--voices"]), 1);
    list
}

/// Voice names from a listing: macOS's (`Alex  en_US  # …`, `kind` 0: the name before two spaces)
/// or eSpeak NG's table (`kind` 1: its language column, what `-v` takes, after a header).
pub(crate) fn parse_voices(text: &str, kind: u8) -> Vec<String> {
    match kind {
        0 => text.lines().filter_map(|l| l.split("  ").next()).map(str::trim).filter(|n| !n.is_empty()).map(String::from).collect(),
        _ => text.lines().skip(1).filter_map(|l| l.split_whitespace().nth(1)).map(String::from).collect(),
    }
}

/// How long a job runs before its end is announced by a system notification.
const NOTIFY_AFTER: std::time::Duration = std::time::Duration::from_secs(10);

fn file_name_of(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A system notification (Notification Center, the desktop's notifications, a Windows balloon).
/// Debug builds append it to the file `SHEAF_TEST_NOTIFY` names instead (gui-smoke).
pub fn notify(title: &str, body: &str) {
    if cfg!(debug_assertions)
        && let Some(log) = std::env::var_os("SHEAF_TEST_NOTIFY")
    {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
            let _ = writeln!(f, "{title}: {body}");
        }
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let q = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("display notification \"{}\" with title \"{}\"", q(body), q(title));
        spawn_detached("osascript", &["-e".as_ref(), script.as_ref()]);
    }
    #[cfg(windows)]
    {
        let q = |s: &str| s.replace('\'', "''");
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $n = New-Object System.Windows.Forms.NotifyIcon; $n.Icon = [System.Drawing.SystemIcons]::Information; \
             $n.Visible = $true; $n.ShowBalloonTip(5000, '{}', '{}', 'Info'); Start-Sleep -Seconds 6; $n.Dispose()",
            q(title),
            q(body)
        );
        spawn_detached("powershell", &["-NoProfile".as_ref(), "-Command".as_ref(), script.as_ref()]);
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    spawn_detached("notify-send", &["-a".as_ref(), "Sheaf".as_ref(), title.as_ref(), body.as_ref()]);
}

/// A job whose tab was closed meanwhile. A cancelled one was already cleaned up by
/// `jobs::finalize`; a successful one is still recorded and announced.
fn on_done_closed(ui: &AppWindow, tab_id: i32, r: Result<JobOutcome>) {
    let Some((tool, inputs, options)) = with(|a| a.closed_jobs.remove(&tab_id)) else { return };
    let Ok(outcome) = r else { return };
    crate::record_history(tool, &inputs, &outcome.outputs, options.as_ref());
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

/// A new mail with `file` attached, in the mail app (Acrobat's and Preview's Share ▸ Mail): Apple
/// Mail on macOS, Outlook on Windows (else Thunderbird), the desktop's mail client through
/// xdg-email elsewhere. Debug builds write the
/// file's path to the file `SHEAF_TEST_MAIL` names instead (gui-smoke). False: no way to mail.
pub fn mail_attachment(file: &std::path::Path, subject: &str) -> bool {
    if cfg!(debug_assertions)
        && let Some(log) = std::env::var_os("SHEAF_TEST_MAIL")
    {
        return std::fs::write(log, file.to_string_lossy().as_bytes()).is_ok();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = subject;
        spawn_detached("open", &["-a".as_ref(), "Mail".as_ref(), file.as_os_str()]);
        true
    }
    #[cfg(windows)]
    {
        let quote = |s: &str| s.replace('\'', "''");
        let (path, subject) = (quote(&file.to_string_lossy()), quote(subject));
        let outlook = format!("$o = New-Object -ComObject Outlook.Application; $m = $o.CreateItem(0); $m.Subject = '{subject}'; [void]$m.Attachments.Add('{path}'); $m.Display()");
        let shown = std::process::Command::new("powershell").args(["-NoProfile", "-Command", &outlook]).status().is_ok_and(|s| s.success());
        shown || {
            let attach = format!("subject='{}',attachment='{}'", subject.replace(',', " "), file.to_string_lossy());
            std::process::Command::new("thunderbird").args(["-compose", &attach]).spawn().is_ok()
        }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        spawn_detached("xdg-email", &["--subject".as_ref(), subject.as_ref(), "--attach".as_ref(), file.as_os_str()]);
        true
    }
}

/// A screenshot of an area the user drags (macOS's screencapture -i; Windows's Snipping Tool, the
/// picture taken from the clipboard it fills; elsewhere GNOME's or KDE's tool, else ImageMagick's
/// import) written to `to` as PNG; blocks until it is taken. Debug builds
/// copy the image `SHEAF_TEST_SCREENSHOT` names instead (gui-smoke). False: none was taken.
pub fn capture_area(to: &std::path::Path) -> bool {
    if cfg!(debug_assertions)
        && let Some(image) = std::env::var_os("SHEAF_TEST_SCREENSHOT")
    {
        return std::fs::copy(image, to).is_ok();
    }
    let ok = |cmd: &str, args: &[&std::ffi::OsStr]| std::process::Command::new(cmd).args(args).status().is_ok_and(|s| s.success()) && to.is_file();
    let t = to.as_os_str();
    #[cfg(target_os = "macos")]
    return ok("screencapture", &["-i".as_ref(), t]);
    #[cfg(windows)]
    {
        // The snip lands on the clipboard: emptied first, then watched for two minutes.
        let path = to.to_string_lossy().replace('\'', "''");
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms, System.Drawing; [System.Windows.Forms.Clipboard]::Clear(); Start-Process 'ms-screenclip:'; \
             for ($i = 0; $i -lt 480; $i++) {{ Start-Sleep -Milliseconds 250; $img = [System.Windows.Forms.Clipboard]::GetImage(); \
             if ($img) {{ $img.Save('{path}', [System.Drawing.Imaging.ImageFormat]::Png); exit 0 }} }}; exit 1"
        );
        ok("powershell", &["-NoProfile".as_ref(), "-STA".as_ref(), "-Command".as_ref(), script.as_ref()])
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        ok("gnome-screenshot", &["-a".as_ref(), "-f".as_ref(), t]) || ok("spectacle", &["-r".as_ref(), "-b".as_ref(), "-n".as_ref(), "-o".as_ref(), t]) || ok("import", &[t])
    }
}

/// What the clipboard can make pages of: a PDF itself (Preview's and Acrobat's copy of pages), PDF
/// files copied in a file manager (in that order), else a picture, else text.
pub enum ClipPages {
    Pdf(Vec<u8>),
    Files(Vec<sheaf_core::pages::Input>),
    Pasted(sheaf_core::pages::Pasted),
}

/// The PDF data on the clipboard (`application/pdf`, macOS's `com.adobe.pdf`), when there is some.
fn clipboard_pdf() -> Option<Vec<u8>> {
    let read = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success()).map(|o| o.stdout);
    #[cfg(target_os = "macos")]
    let bytes = {
        let tmp = std::env::temp_dir().join(format!("sheaf-clip-{}.pdf", std::process::id()));
        let script = format!(
            "ObjC.import('AppKit'); var d = $.NSPasteboard.generalPasteboard.dataForType('com.adobe.pdf'); d.isNil() ? 'none' : (d.writeToFileAtomically('{}', true), 'ok')",
            tmp.display()
        );
        let ok = read("osascript", &["-l", "JavaScript", "-e", &script]).is_some_and(|o| o.starts_with(b"ok"));
        let b = ok.then(|| std::fs::read(&tmp).ok()).flatten();
        let _ = std::fs::remove_file(&tmp);
        b
    };
    // Windows: the formats Acrobat and browsers register for PDF data, written to a file by PowerShell
    // (Sol, 26/10: Windows's PDF data was ignored).
    #[cfg(windows)]
    let bytes = {
        let tmp = std::env::temp_dir().join(format!("sheaf-clip-{}.pdf", std::process::id()));
        let path = tmp.to_string_lossy().replace('\'', "''");
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; foreach ($f in 'Portable Document Format', 'application/pdf', 'PDF') {{ \
             $d = [System.Windows.Forms.Clipboard]::GetData($f); if ($d -is [System.IO.MemoryStream]) {{ [IO.File]::WriteAllBytes('{path}', $d.ToArray()); 'ok'; exit }} }}; 'none'"
        );
        let ok = read("powershell", &["-NoProfile", "-STA", "-Command", &script]).is_some_and(|o| o.starts_with(b"ok"));
        let b = ok.then(|| std::fs::read(&tmp).ok()).flatten();
        let _ = std::fs::remove_file(&tmp);
        b
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let bytes = read("xclip", &["-selection", "clipboard", "-t", "application/pdf", "-o"]).or_else(|| read("wl-paste", &["--type", "application/pdf"]));
    bytes.filter(|b| b.starts_with(b"%PDF"))
}

/// The clipboard's [`ClipPages`]; None: nothing pages can be made of.
pub fn clipboard_pages() -> Option<ClipPages> {
    use sheaf_core::pages::Pasted;
    if let Some(pdf) = clipboard_pdf() {
        return Some(ClipPages::Pdf(pdf));
    }
    let mut c = arboard::Clipboard::new().ok()?;
    // Copied PDF files (Sol, 26/10: a PDF on the clipboard was never read). A file manager's list
    // ends its lines with CR LF (text/uri-list); arboard splits at LF only.
    let pdfs: Vec<sheaf_core::pages::Input> = c
        .get()
        .file_list()
        .unwrap_or_default()
        .into_iter()
        .map(|p| std::path::PathBuf::from(p.to_string_lossy().trim_end_matches(['\r', '\n'])))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
        .map(sheaf_core::pages::Input::new)
        .collect();
    if !pdfs.is_empty() {
        return Some(ClipPages::Files(pdfs));
    }
    c.get_image()
        .ok()
        .map(|i| Pasted::Image { width: i.width as u32, height: i.height as u32, rgba: i.bytes.into_owned() })
        .or_else(|| c.get_text().ok().filter(|t| !t.trim().is_empty()).map(Pasted::Text))
        .map(ClipPages::Pasted)
}

/// The clipboard's pages as a PDF at `path`: the files merged, or a page of the picture or text.
pub fn clipboard_to_pdf(c: &ClipPages, path: &std::path::Path) -> sheaf_core::Result<PathBuf> {
    match c {
        ClipPages::Pdf(bytes) => std::fs::write(path, bytes).map(|()| path.to_path_buf()).map_err(Into::into),
        ClipPages::Files(pdfs) => sheaf_core::pages::merge(pdfs, path),
        ClipPages::Pasted(p) => sheaf_core::pages::from_pasted(p, path),
    }
}

/// A page scanned (Acrobat's Create PDF ▸ Scanner) to `to` as PNG; blocks until it is done: the
/// Windows scan dialog (WIA), SANE's scanimage elsewhere. Debug builds copy the image
/// `SHEAF_TEST_SCAN` names instead (gui-smoke). Err: what went wrong, said to the user.
pub fn scan_page(to: &std::path::Path) -> std::result::Result<(), String> {
    if cfg!(debug_assertions)
        && let Some(image) = std::env::var_os("SHEAF_TEST_SCAN")
    {
        return std::fs::copy(image, to).map(|_| ()).map_err(|e| e.to_string());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = to;
        Err("Scan with Image Capture, then open the file here: macOS gives other apps no scanner".into())
    }
    #[cfg(windows)]
    {
        let path = to.to_string_lossy().replace('\'', "''");
        let script = format!(
            "$d = New-Object -ComObject WIA.CommonDialog; $i = $d.ShowAcquireImage(1, 1, 0, '{{B96B3CAF-0728-11D3-9D7B-0000F81EF32E}}'); \
             if ($i) {{ $i.SaveFile('{path}'); exit 0 }}; exit 1"
        );
        let done = std::process::Command::new("powershell").args(["-NoProfile", "-STA", "-Command", &script]).status().is_ok_and(|s| s.success());
        if done && to.is_file() { Ok(()) } else { Err("No page was scanned".into()) }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let out = std::process::Command::new("scanimage").args(["--format=png", "--resolution", "300", "-o"]).arg(to).output();
        match out {
            Ok(o) if o.status.success() && to.is_file() => Ok(()),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("No scanner was found").trim().to_string()),
            Err(_) => Err("No scanner software: install SANE (sane-utils)".into()),
        }
    }
}

/// Preferences ▸ Open PDFs with Sheaf by default: Sheaf made the system's PDF handler (macOS's
/// LaunchServices, xdg-mime) or, on Windows, which asks the user itself, its Default apps page
/// opened. Debug builds write "default" to the file `SHEAF_TEST_DEFAULT` names instead (gui-smoke).
/// Returns what to tell the user.
pub fn make_default_pdf_app() -> String {
    if cfg!(debug_assertions)
        && let Some(f) = std::env::var_os("SHEAF_TEST_DEFAULT")
    {
        let _ = std::fs::write(f, "default");
        return "Sheaf now opens PDFs".into();
    }
    let ran = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).status().is_ok_and(|s| s.success());
    #[cfg(target_os = "macos")]
    let done = ran(
        "osascript",
        &["-l", "JavaScript", "-e", "ObjC.import('CoreServices'); $.LSSetDefaultRoleHandlerForContentType($('com.adobe.pdf'), $.kLSRolesAll, $('app.sheaf.Sheaf')) === 0"],
    );
    // Windows asks the user itself; Sheaf is first registered for this user as an app that opens
    // PDFs (its ProgID and .pdf's Open with list), so it is there to pick even from the portable
    // folder (Sol, 26/10: Default apps did not list it).
    #[cfg(windows)]
    let done = {
        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
        let open = format!("\"{exe}\" \"%1\"");
        let keys: [&[&str]; 4] = [
            &["add", r"HKCU\Software\Classes\Sheaf.PDF", "/ve", "/d", "PDF document", "/f"],
            &["add", r"HKCU\Software\Classes\Sheaf.PDF\shell\open\command", "/ve", "/d", &open, "/f"],
            &["add", r"HKCU\Software\Classes\.pdf\OpenWithProgids", "/v", "Sheaf.PDF", "/t", "REG_NONE", "/f"],
            &["add", r"HKCU\Software\Classes\Applications\sheaf.exe\SupportedTypes", "/v", ".pdf", "/t", "REG_SZ", "/f"],
        ];
        let mut registered = keys.iter().all(|k| ran("reg", k));
        // The tools as the PDF's verbs in Explorer's context menu.
        for (key, label) in sheaf_app::open_with::MENU_TOOLS {
            let verb = format!(r"HKCU\Software\Classes\SystemFileAssociations\.pdf\shell\Sheaf.{key}");
            let cmd = format!("\"{exe}\" --tool={key} \"%1\"");
            registered &= ran("reg", &["add", &verb, "/ve", "/d", label, "/f"]) && ran("reg", &["add", &format!(r"{verb}\command"), "/ve", "/d", &cmd, "/f"]);
        }
        spawn_detached("explorer", &["ms-settings:defaultapps".as_ref()]);
        return if registered { "Pick Sheaf for .pdf in Default apps".into() } else { "Sheaf could not be registered: pick it with Open with ▸ Choose another app".into() };
    };
    // The desktop entry xdg-mime names, written for this user when no package installed one (a
    // folder unpacked from the archive: Sol, 26/10), its Exec this very program.
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let apps = dirs::data_dir().unwrap_or_default().join("applications");
        let entry = apps.join("sheaf.desktop");
        let system = ["/usr/share/applications/sheaf.desktop", "/usr/local/share/applications/sheaf.desktop"].iter().any(|p| std::path::Path::new(p).exists());
        if !system && !entry.exists()
            && let Ok(exe) = std::env::current_exe()
        {
            let text = desktop_entry(&format!("\"{}\"", exe.display()));
            let _ = std::fs::create_dir_all(&apps).and_then(|_| std::fs::write(&entry, text));
            let _ = ran("update-desktop-database", &[&apps.to_string_lossy()]);
        }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let done = ran("xdg-mime", &["default", "sheaf.desktop", "application/pdf"]);
    if done { "Sheaf now opens PDFs".into() } else { "Sheaf could not be made the default: choose it in your system's settings".into() }
}

/// Sheaf's desktop entry, `exec` its program: opens PDFs, its tools as the file manager's actions
/// (the context menu's Compress with Sheaf…).
pub fn desktop_entry(exec: &str) -> String {
    use sheaf_app::open_with::MENU_TOOLS;
    let mut t = format!(
        "[Desktop Entry]\nType=Application\nName=Sheaf\nComment=Private PDF toolbox\nExec={exec} %F\nIcon=sheaf\nMimeType=application/pdf;\nCategories=Office;Graphics;\nActions={};\n",
        MENU_TOOLS.iter().map(|t| t.0).collect::<Vec<_>>().join(";")
    );
    for (key, label) in MENU_TOOLS {
        t.push_str(&format!("\n[Desktop Action {key}]\nName={label}\nExec={exec} --tool={key} %F\n"));
    }
    t
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

/// Opens `p` in a window of its own: Sheaf started again with it (a tab moved to a new window, as
/// browsers and PDF Expert do).
pub fn open_in_new_window(p: &std::path::Path) -> bool {
    match std::env::current_exe() {
        Ok(exe) => {
            spawn_detached(&exe.to_string_lossy(), &[p.as_os_str()]);
            true
        }
        Err(_) => false,
    }
}

/// Moves `p` to the system's trash (Finder's, the desktop's, the Recycle Bin), where it can be put
/// back from.
pub fn trash(p: &std::path::Path) -> std::result::Result<(), String> {
    use std::process::Command;
    #[cfg(target_os = "macos")]
    let out = {
        let path = p.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        Command::new("osascript").args(["-e", &format!("tell application \"Finder\" to delete POSIX file \"{path}\"")]).output()
    };
    #[cfg(windows)]
    let out = {
        let path = p.to_string_lossy().replace('\'', "''");
        Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile('{path}', 'OnlyErrorDialogs', 'SendToRecycleBin')")])
            .output()
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let out = Command::new("gio").arg("trash").arg(p).output();
    match out {
        Ok(o) if o.status.success() && !p.exists() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// A result's Open: Compare's report and marked copy open in Sheaf's reader (the marked copy on
/// its list of differences); other results in their own app.
pub fn open_output(ui: &AppWindow, tab_id: i32, k: i32) {
    let Some(p) = outputs(tab_id).get(k as usize).cloned() else { return };
    let compare = with(|a| a.tab(tab_id).is_some_and(|t| t.tool == ToolId::Compare));
    match (compare, p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))) {
        (true, true) => crate::reader_ui::open_on_notes(ui, p),
        _ => open_detached(&p),
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

    #[test]
    fn voices_are_read_from_the_systems_listings() {
        let mac = "Alex                en_US    # Most people recognize me by my voice.\nAmélie              fr_CA    # Bonjour, je m’appelle Amélie.\n";
        assert_eq!(parse_voices(mac, 0), ["Alex", "Amélie"]);
        let espeak = "Pty Language       Age/Gender VoiceName          File                 Other Languages\n 5  af              --/M      Afrikaans          gmw/af\n 5  en-gb           --/M      English_(Great_Britain) gmw/en\n";
        assert_eq!(parse_voices(espeak, 1), ["af", "en-gb"]);
    }
    use std::path::Path;

    fn opts() -> ToolOptions {
        ToolOptions { pdfa_allow_incomplete: true, level: 1, every: 1, degrees: 90, ..Default::default() }
    }

    #[test]
    fn batch_jobs_write_default_names_in_the_folder() {
        let inputs = vec![Input::new("/a/x.pdf"), Input::new("/b/y.pdf")];
        let job = build_job(ToolId::Compress, inputs, vec![], &opts(), Dest::Folder("/out".into())).unwrap();
        let Job::Compress { items, level, .. } = job else { panic!() };
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
        let Job::PdfA { items, level, allow_incomplete } = build_job(ToolId::PdfToPdfA, two, vec![], &o, Dest::Folder("/o".into())).unwrap() else { panic!() };
        assert_eq!((level, items[1].1.clone()), (PdfALevel::A2b, PathBuf::from("/o/y-pdfa.pdf")));
        assert!(allow_incomplete);
        o.pdfa_allow_incomplete = false;
        o.pdfa_level = 1;
        let Job::PdfA { level, allow_incomplete, .. } = build_job(ToolId::PdfToPdfA, vec![Input::new("/a/x.pdf")], vec![], &o, Dest::File("/s/p.pdf".into())).unwrap() else { panic!() };
        assert_eq!(level, PdfALevel::A3b);
        assert!(!allow_incomplete);
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
        let Job::Redact { input, marks: m, out, .. } = build_job_with(ToolId::Redact, one(), vec![], marks.clone(), &o, dest()).unwrap() else { panic!() };
        assert_eq!((input.path, m, out), (PathBuf::from("/a/v.pdf"), marks, PathBuf::from("/s/v-redacted.pdf")));
        assert_eq!(out_name(ToolId::Redact, Path::new("/a/v.pdf"), &o), "v-redacted.pdf");
    }

    #[test]
    fn compare_job_takes_original_then_changed() {
        let mut o = opts();
        o.compare_mode = 1;
        let two = vec![Input::new("/a/v1.pdf"), Input::new("/a/v2.pdf")];
        let Job::Compare { old, new, mode, out, .. } = build_job(ToolId::Compare, two, vec![], &o, Dest::File("/s/v1-comparison.pdf".into())).unwrap() else { panic!() };
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
    fn html_layout_reaches_the_job_without_reusing_the_default_conversion() {
        let mut o = opts();
        o.layout = 2;
        o.landscape = true;
        o.margin = 1;
        let input = || vec![Input::new("/a/page.html")];
        let job = build_job(ToolId::HtmlToPdf, input(), vec![], &o, Dest::NextToEach).unwrap();
        let Job::Office { html, .. } = job else { panic!("an HTML conversion job") };
        assert_eq!(html, Some(sheaf_core::html::Options { paper: sheaf_core::html::Paper::Letter, landscape: true, margin: 18.0 }));
        let job = build_job(ToolId::WordToPdf, input(), vec![], &o, Dest::NextToEach).unwrap();
        assert!(matches!(job, Job::Office { html: None, .. }));
        o.layout = 0;
        let job = build_job(ToolId::HtmlToPdf, input(), vec![], &o, Dest::NextToEach).unwrap();
        assert!(matches!(job, Job::Office { html: None, .. }));
    }

    #[test]
    fn whole_sheet_layout_is_only_used_for_spreadsheets_and_bypasses_html() {
        let mut o = opts();
        o.layout = 1;
        for (tool, expected) in [(ToolId::ExcelToPdf, true), (ToolId::WordToPdf, false), (ToolId::PowerPointToPdf, false)] {
            let job = build_job(tool, vec![Input::new("/a/book.xlsx")], vec![], &o, Dest::NextToEach).unwrap();
            assert!(matches!(job, Job::Office { whole_sheets, html: None, .. } if whole_sheets == expected));
        }
    }

    #[test]
    fn native_spreadsheet_choice_keeps_sheet_selection_and_bypasses_the_office_cache() {
        let mut o = opts();
        o.layout = 2;
        o.pages = "3,1-2".into();
        for tool in [ToolId::ExcelToPdf, ToolId::WordToPdf, ToolId::PowerPointToPdf] {
            let job = build_job(tool, vec![Input::new("/a/book.xlsx")], vec![], &o, Dest::NextToEach).unwrap();
            let Job::Office { data_sheets, whole_sheets, .. } = job else { panic!() };
            assert!(!whole_sheets);
            assert_eq!(data_sheets.as_deref(), (tool == ToolId::ExcelToPdf).then_some("3,1-2"));
        }
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
