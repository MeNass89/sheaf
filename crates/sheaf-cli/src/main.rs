//! `sheaf`: a thin command-line front-end over the sheaf-core engine.
//!
//! Every subcommand prints the path or paths it wrote, one per line, on stdout.
//! Exit codes: 0 success, 1 any error, 2 password required or wrong.
//!
//! For agents: `--json` turns the output into one JSON object (see [`agent`]), `sheaf tools`
//! describes every command as a JSON schema, and `sheaf mcp` serves them over MCP.

mod agent;
mod args;
mod mcp;

use args::{edit_items, Cli, CliCompare, CliPdfA, CliSheets, Cmd, NumberFormat, WmPosition};
use clap::{CommandFactory, FromArgMatches};
use sheaf_core::compress;
use sheaf_core::images::{self, PageLayout};
use sheaf_core::pages::{self, Input};
use sheaf_core::crop::{self, CropSpec};
use sheaf_core::numbers::{self, NumberOptions, NumberText};
use sheaf_core::compare::{self, CompareMode};
use sheaf_core::excel::{self, SheetLayout};
use sheaf_core::export::{self, ExportOptions, Office};
use sheaf_core::ocr::{self, OcrOptions};
use sheaf_core::pdfa::{self, PdfALevel};
use sheaf_core::redact::{self, Query};
use sheaf_core::edit::{self, EditSpec, Item};
use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
use sheaf_core::watermark::{self, Mark, Placement, WatermarkOptions};
use sheaf_core::{office, repair, rotate, security, OpdfError};
use std::path::{Path, PathBuf};

/// A summary, note or warning for the user: on stderr, or in the JSON result with `--json`.
macro_rules! note {
    ($($t:tt)*) => { crate::agent::note(format!($($t)*)) };
}


/// Drawn strokes from a JSON file: an array of strokes, each an array of [x, y] points.
fn read_strokes(path: &Path) -> sheaf_core::Result<Vec<Vec<[f64; 2]>>> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| OpdfError::Unsupported(format!("{}: not a list of strokes ({e})", path.display())))
}


/// "RRGGBB" → RGB in 0..=1.
fn parse_color(s: &str) -> sheaf_core::Result<[f64; 3]> {
    numbers::parse_hex_colour(s).ok_or_else(|| OpdfError::Unsupported(format!("colour \"{s}\" (use RRGGBB)")))
}


impl Cmd {
    /// The PDFs whose protection the output loses: every PDF input, except for Protect (output
    /// encrypted by design), Unlock (removing protection is the point) and Compress (it reports
    /// encryption itself).
    fn unprotecting_inputs(&self) -> Vec<&Path> {
        match self {
            Cmd::Merge { inputs, .. } => inputs.iter().map(PathBuf::as_path).collect(),
            Cmd::Split { input, .. }
            | Cmd::Extract { input, .. }
            | Cmd::Remove { input, .. }
            | Cmd::Organize { input, .. }
            | Cmd::Rotate { input, .. }
            | Cmd::Repair { input, .. }
            | Cmd::ToJpg { input, .. } => vec![input],
            // These report it themselves (from the engine), like Compress.
            Cmd::Number { .. } | Cmd::Watermark { .. } | Cmd::Crop { .. } | Cmd::Pdfa { .. } | Cmd::Ocr { .. } |
            Cmd::ToWord { .. } | Cmd::ToPowerpoint { .. } | Cmd::ToExcel { .. } | Cmd::Compare { .. } | Cmd::Redact { .. } |
            // Sign refuses protected inputs.
            Cmd::Sign { .. } |
            Cmd::Edit { .. } | Cmd::CorrectOcr { .. } | Cmd::ApplySecurityPolicy { .. } |
            Cmd::Compress { .. } | Cmd::Protect { .. } | Cmd::Unlock { .. } | Cmd::FromImages { .. } | Cmd::Office { .. } => vec![],
            // They write no PDF.
            Cmd::ReviewOcr { .. } | Cmd::SaveSecurityPolicy { .. } | Cmd::Index { .. } | Cmd::SearchIndex { .. } | Cmd::Info { .. } | Cmd::Text { .. } | Cmd::Render { .. } | Cmd::Tools | Cmd::Mcp => vec![],
        }
    }

    /// The directory that must exist before core writes: `-o` itself for
    /// directory outputs (`split`, `to-jpg`), else the parent of `-o`.
    fn out_dir(&self) -> Option<&Path> {
        match self {
            Cmd::Split { out, .. } | Cmd::ToJpg { out, .. } => Some(out),
            Cmd::ReviewOcr { .. } | Cmd::SearchIndex { .. } | Cmd::Info { .. } | Cmd::Text { .. } | Cmd::Tools | Cmd::Mcp => None,
            Cmd::CorrectOcr { out, .. }
            | Cmd::SaveSecurityPolicy { out, .. }
            | Cmd::ApplySecurityPolicy { out, .. }
            | Cmd::Index { out, .. }
            | Cmd::Merge { out, .. }
            | Cmd::Render { out, .. }
            | Cmd::Extract { out, .. }
            | Cmd::Remove { out, .. }
            | Cmd::Organize { out, .. }
            | Cmd::Rotate { out, .. }
            | Cmd::Compress { out, .. }
            | Cmd::Protect { out, .. }
            | Cmd::Unlock { out, .. }
            | Cmd::Repair { out, .. }
            | Cmd::FromImages { out, .. }
            | Cmd::Number { out, .. }
            | Cmd::Watermark { out, .. }
            | Cmd::Crop { out, .. }
            | Cmd::Pdfa { out, .. }
            | Cmd::Ocr { out, .. }
            | Cmd::ToWord { out, .. }
            | Cmd::ToPowerpoint { out, .. }
            | Cmd::ToExcel { out, .. }
            | Cmd::Compare { out, .. }
            | Cmd::Redact { out, .. }
            | Cmd::Sign { out, .. }
            | Cmd::Edit { out, .. }
            | Cmd::Office { out, .. } => out.parent().filter(|p| !p.as_os_str().is_empty()),
        }
    }
}

fn run(cli: Cli) -> sheaf_core::Result<Vec<PathBuf>> {
    if let Some(dir) = cli.cmd.out_dir() {
        std::fs::create_dir_all(dir)?;
    }
    let pw = cli.password;
    let input = |p: PathBuf| Input { path: p, password: pw.clone() };
    let unprotects = cli.cmd.unprotecting_inputs().into_iter().any(|p| input(p.to_path_buf()).is_encrypted());
    let r = dispatch(cli.cmd, &input);
    if unprotects && r.is_ok() {
        note!("{NOT_PROTECTED}");
    }
    r
}

const NOT_PROTECTED: &str = "note: the output is not password-protected";

/// `sheaf pdfa` wrote a file that still breaks rules of the standard: exit 1 after printing it.
static NOT_CONFORMING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn dispatch(cmd: Cmd, input: &dyn Fn(PathBuf) -> Input) -> sheaf_core::Result<Vec<PathBuf>> {
    let one = |r: sheaf_core::Result<PathBuf>| r.map(|p| vec![p]);
    match cmd {
        Cmd::Merge { inputs, out } => {
            let inputs: Vec<Input> = inputs.into_iter().map(input).collect();
            one(pages::merge(&inputs, &out))
        }
        Cmd::Split { input: i, ranges, every, out } => match (ranges, every) {
            (Some(r), _) => pages::split_ranges(&input(i), &r, &out),
            (None, Some(n)) => pages::split_every(&input(i), n, &out),
            (None, None) => unreachable!("clap requires --ranges or --every"),
        },
        Cmd::Extract { input: i, ranges, out } => one(pages::extract(&input(i), &ranges, &out)),
        Cmd::Remove { input: i, pages: p, out } => one(pages::remove(&input(i), &p, &out)),
        Cmd::Organize { input: i, order, out } => one(pages::organize(&input(i), &order, &out)),
        Cmd::Rotate { input: i, degrees, pages: p, out } => one(rotate::rotate(&input(i), p.as_deref(), degrees.rem_euclid(360), &out)),
        Cmd::Compress { input: i, level, out } => {
            let r = compress::compress(&input(i), level.into(), &out)?;
            note!("{} -> {} bytes", r.before, r.after);
            if r.was_encrypted {
                note!("{NOT_PROTECTED}");
            }
            if r.grew {
                note!("note: the unprotected copy is {}% larger than the protected original", grew_pct(r.before, r.after));
            }
            Ok(vec![r.path])
        }
        Cmd::Protect { input: i, user_password, owner_password, out } => {
            one(security::protect(&input(i), &user_password, owner_password.as_deref(), &out))
        }
        Cmd::SaveSecurityPolicy { name, no_printing, no_copying, no_editing, no_commenting, no_forms, leave_metadata_clear, out } => {
            let permissions = security::ProtectPermissions { printing: !no_printing, copying: !no_copying, editing: !no_editing, commenting: !no_commenting, filling_forms: !no_forms };
            let policy = sheaf_core::security_policy::SecurityPolicy::new(name, permissions, !leave_metadata_clear)?;
            one(policy.save(&out))
        }
        Cmd::ApplySecurityPolicy { input: i, policy, user_password, owner_password, out } => {
            one(sheaf_core::security_policy::SecurityPolicy::load(&policy)?.apply(&input(i), &user_password, owner_password.as_deref(), &out))
        }
        Cmd::Unlock { input: i, out } => one(security::unlock(&input(i), &out)),
        Cmd::Repair { input: i, out } => one(repair::repair(&input(i), &out)),
        Cmd::ToJpg { input: i, dpi, quality, png, out } => images::pdf_to_jpg(&input(i), dpi, (!png).then_some(quality), &out),
        Cmd::FromImages { images: imgs, a4, letter, landscape, out } => {
            let paper = if letter { Some(images::Paper::Letter) } else { a4.then_some(images::Paper::A4) };
            let layout = paper.map_or(PageLayout::FitImage, |paper| PageLayout::Paper { paper, landscape, margin_pt: 36.0 });
            one(images::images_to_pdf(&imgs, layout, &out))
        }
        Cmd::Office { input: i, out } => one(office::office_to_pdf(&i, &out)),
        Cmd::Number { input: i, position, double_sided, cover, margin, from, to, start, text, custom, font_size, font, color, underline, out } => {
            let text = match text {
                NumberFormat::Number => NumberText::Number,
                NumberFormat::Page => NumberText::Page,
                NumberFormat::PageOf => NumberText::PageOfTotal,
                NumberFormat::Custom => NumberText::Custom(custom.unwrap_or_default()),
            };
            let o = NumberOptions {
                position: position as u8,
                double_sided,
                cover,
                margin: numbers::MARGINS[margin as usize],
                from,
                to,
                start,
                text,
                font_size,
                font: font.std14(),
                color: parse_color(&color)?,
                underline,
                clamp_to_last: false,
                ..Default::default()
            };
            warned(numbers::number_pages(&input(i), &o, &out))
        }
        Cmd::Watermark { input: i, text, image, font_size, font, underline, color, scale, position, transparency, rotate, below, from, to, out } => {
            let mark = match (text, image) {
                (_, Some(path)) => Mark::Image { path, scale },
                (Some(text), None) => Mark::Text { text, font_size, color: parse_color(&color)? },
                (None, None) => unreachable!("clap requires --text or --image"),
            };
            let placement = match position {
                WmPosition::Mosaic => Placement::Mosaic,
                p => Placement::Grid(p as u8),
            };
            let o = WatermarkOptions {
                mark,
                placement,
                opacity: 1.0 - transparency as f64 / 100.0,
                rotation: rotate as f64,
                behind: below,
                from,
                to,
                font: font.std14(),
                underline,
                clamp_to_last: false,
                replace: false,
                remove: false,
            };
            warned(watermark::watermark(&input(i), &o, &out))
        }
        Cmd::Crop { input: i, top, bottom, left, right, remove, pages: p, out } => {
            let spec = if remove {
                CropSpec::Remove
            } else if top == 0.0 && bottom == 0.0 && left == 0.0 && right == 0.0 {
                return Err(OpdfError::Unsupported("give a margin (--top, --bottom, --left, --right) or --remove".into()));
            } else {
                CropSpec::Margins { top: crop::mm(top), bottom: crop::mm(bottom), left: crop::mm(left), right: crop::mm(right) }
            };
            warned(crop::crop(&input(i), p.as_deref(), &spec, &out))
        }
        Cmd::Ocr { input: i, language, local_engine, all_pages, out } => {
            let opts = OcrOptions { language, use_local_engine: local_engine, read_text_pages: all_pages, ..Default::default() };
            let r = ocr::ocr(&input(i), &opts, &out, &mut |_| true)?;
            note!("{}", ocr::summary(r.pages_read, r.words, r.pages_skipped));
            if let Some(w) = r.failure_warning() {
                note!("warning: {w}");
            }
            warned(Ok((r.path, r.was_encrypted)))
        }
        Cmd::ReviewOcr { input: i } => {
            let lines = sheaf_core::ocr_review::review(&input(i))?;
            agent::set_data(serde_json::to_value(lines).map_err(|e| OpdfError::Engine(e.to_string()))?);
            Ok(vec![])
        }
        Cmd::CorrectOcr { input: i, page, line, expected, text, not_text, out } => {
            let i = input(i);
            let replacement = if not_text { "" } else { text.as_deref().ok_or_else(|| OpdfError::Unsupported("Supply --text or --not-text".into()))? };
            let path = sheaf_core::ocr_review::correct(&i, page as usize - 1, line as usize - 1, &expected, replacement, &out)?;
            warned(Ok((path, i.is_encrypted())))
        }
        Cmd::ToWord { input: i, ocr, out } => office_export(&input(i), Office::Word, ocr, &out),
        Cmd::ToPowerpoint { input: i, ocr, out } => office_export(&input(i), Office::PowerPoint, ocr, &out),
        Cmd::ToExcel { input: i, layout, ocr, out } => {
            let layout = match layout {
                CliSheets::OneSheet => SheetLayout::OneSheet,
                CliSheets::SheetPerTable => SheetLayout::SheetPerTable,
            };
            let r = excel::to_excel(&input(i), layout, &ExportOptions { ocr, ..Default::default() }, &out, &mut |_| true)?;
            note!("{}", excel::summary(r.tables));
            Ok(vec![r.path])
        }
        Cmd::Compare { old, new, mode, out } => {
            let mode = match mode {
                CliCompare::Text => CompareMode::Text,
                CliCompare::Overlay => CompareMode::Overlay,
                CliCompare::SideBySide => CompareMode::SideBySide,
            };
            let r = compare::compare(&input(old), &input(new), mode, &out)?;
            note!("{}", r.summary());
            warned(Ok((r.path, r.was_encrypted)))
        }
        Cmd::Redact { input: i, text, match_case, whole_words, pattern, pages: whole, out } => {
            let i = input(i);
            let searched: Vec<redact::Needle> = text.iter().map(|t| redact::Needle { text: t.clone(), match_case, whole_word: whole_words }).collect();
            let mut queries: Vec<Query> = text.into_iter().map(|t| Query::Literal { text: t, case_insensitive: !match_case, whole_word: whole_words }).collect();
            queries.extend(pattern.into_iter().map(|p| Query::Pattern(p.into())));
            let mut marks = redact::mark_text(&redact::find_all(&i, &queries)?);
            if let Some(spec) = whole {
                let n = sheaf_core::pdfcraft::Doc::open(&i)?.document().info.pages.len() as u32;
                let pages: Vec<usize> = sheaf_core::ranges::parse_ranges(&spec, n)?.into_iter().flatten().map(|p| p as usize - 1).collect();
                marks.extend(redact::mark_pages(&i, &pages)?);
            }
            if marks.is_empty() {
                return Err(OpdfError::Unsupported("nothing to redact: no match and no page given".into()));
            }
            let r = redact::apply(&i, &marks, &searched, &out)?;
            note!("{}", redact::summary(r.counts));
            note!("{}", redact::VERIFIED);
            for w in &r.warnings {
                note!("warning: {w}");
            }
            warned(Ok((r.path, r.was_encrypted)))
        }
        Cmd::Sign { input: i, page, at, width, text, script_font, image, strokes, p12, p12_password, reason, location, certify, out } => {
            let i = input(i);
            let graphic = match (text, image, strokes) {
                (Some(t), _, _) => Graphic::Typed(t, script_font.font()),
                (_, Some(p), _) => Graphic::Image(p),
                (_, _, Some(p)) => Graphic::Drawn(read_strokes(&p)?),
                _ => unreachable!("clap requires one graphic"),
            };
            let page = page as usize - 1;
            let info = sign::page_info(&i, page)?;
            let aspect = sign::aspect(&graphic)?;
            let center = [at.0 + width / 2.0, at.1 + width * aspect / 2.0];
            // Certifying is a certificate signature: it needs the digital ID.
            if certify.is_some() && p12.is_none() {
                return Err(OpdfError::Unsupported("--certify needs --p12 (a certifying signature is a certificate one)".into()));
            }
            let digital = p12.map(|p12| DigitalSpec { p12, password: p12_password.unwrap_or_default(), reason, location, certify });
            let spec = SignSpec { page, rect: sign::box_at(&info, center, width, aspect), graphic, digital };
            let r = sign::sign(&i, &spec, &out)?;
            note!("{}", r.summary());
            Ok(vec![r.path])
        }
        Cmd::Edit { input: i, items, out, .. } => {
            let i = input(i);
            let mut spec = EditSpec::default();
            let mut infos = std::collections::HashMap::new();
            for it in items {
                let page = it.page.unwrap_or(1) as usize - 1;
                let what = it.text.as_deref().map(|t| format!("--text \"{t}\"")).unwrap_or_else(|| format!("--image {}", it.image.as_ref().map(|p| p.display().to_string()).unwrap_or_default()));
                let (x, y) = it.at.ok_or_else(|| OpdfError::Unsupported(format!("{what}: --at X,Y is missing")))?;
                let info = match infos.entry(page) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => e.insert(edit::page_info(&i, page)?),
                };
                spec.items.push(match (it.text, it.image) {
                    (Some(text), _) => {
                        if it.width.is_some() {
                            return Err(OpdfError::Unsupported(format!("{what}: --width is for images (a text's width follows its size)")));
                        }
                        let text = text.replace("\\n", "\n");
                        Item::Text { page, at: edit::view_to_user(info, [x, y]), text, font_size: it.size.unwrap_or(edit::DEFAULT_FONT_SIZE), color: it.color.unwrap_or([0; 3]) }
                    }
                    (None, Some(path)) => {
                        if it.size.is_some() || it.color.is_some() {
                            return Err(OpdfError::Unsupported(format!("{what}: --size and --color are for text")));
                        }
                        let width = it.width.ok_or_else(|| OpdfError::Unsupported(format!("{what}: --width is missing")))?;
                        let aspect = edit::Picture::read(&path)?.aspect();
                        Item::Image { page, rect: edit::image_rect(info, [x, y], width, aspect), path }
                    }
                    (None, None) => unreachable!("an item starts at --text or --image"),
                });
            }
            let r = edit::edit(&i, &spec, &out)?;
            note!("{}", r.summary());
            warned(Ok((r.path, r.was_encrypted)))
        }
        Cmd::Info { input: i } => {
            agent::set_data(agent::info(&input(i))?);
            Ok(vec![])
        }
        Cmd::Text { input: i, pages: spec } => {
            let i = input(i);
            let pages = match spec {
                Some(spec) => {
                    let n = sheaf_core::pdfcraft::Doc::open(&i)?.document().info.pages.len() as u32;
                    sheaf_core::ranges::parse_ranges(&spec, n)?.into_iter().flatten().map(|p| p as usize - 1).collect()
                }
                None => vec![],
            };
            agent::set_text(sheaf_core::text::page_texts(&i, &pages)?);
            Ok(vec![])
        }
        Cmd::Render { input: i, page, dpi, out } => one(images::render_png(&input(i), page as usize - 1, dpi, &out)),
        Cmd::Index { inputs, update, out } => {
            let inputs: Vec<Input> = inputs.into_iter().map(input).collect();
            let catalog = match update {
                Some(path) => sheaf_core::search_index::Catalog::load(&path)?.update(&inputs, &mut |_, _| true)?,
                None => sheaf_core::search_index::Catalog::build(&inputs, &mut |_, _| true)?,
            };
            one(catalog.save(&out))
        }
        Cmd::SearchIndex { index, query } => {
            let hits = sheaf_core::search_index::Catalog::load(&index)?.search(&query)?;
            agent::set_data(serde_json::to_value(hits).map_err(|e| OpdfError::Engine(e.to_string()))?);
            Ok(vec![])
        }
        Cmd::Tools => {
            agent::set_data(serde_json::json!({ "tools": mcp::tools() }));
            Ok(vec![])
        }
        Cmd::Mcp => {
            mcp::serve()?;
            Ok(vec![])
        }
        Cmd::Pdfa { input: i, level, out } => {
            let level = match level {
                CliPdfA::A2b => PdfALevel::A2b,
                CliPdfA::A3b => PdfALevel::A3b,
            };
            let r = pdfa::to_pdfa(&input(i), level, &out)?;
            if r.issues.is_empty() {
                note!("{}", pdfa::summary(level, 0));
            }
            for issue in &r.issues {
                note!("warning: {} ({}): {}", level.label(), issue.clause, issue.message);
            }
            if !r.issues.is_empty() {
                note!("error: the output does not fully conform to {}", level.label());
                NOT_CONFORMING.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            warned(Ok((r.path, r.was_encrypted)))
        }
    }
}

/// Word or PowerPoint export (the output is not a PDF: no protection note).
fn office_export(input: &Input, format: Office, ocr: bool, out: &Path) -> sheaf_core::Result<Vec<PathBuf>> {
    Ok(vec![export::to_office(input, format, &ExportOptions { ocr, ..Default::default() }, out, &mut |_| true)?.path])
}

/// A PdfCraft tool's result: the path, with the note when the protected input lost protection.
fn warned(r: sheaf_core::Result<(PathBuf, bool)>) -> sheaf_core::Result<Vec<PathBuf>> {
    let (p, was_encrypted) = r?;
    if was_encrypted {
        note!("{NOT_PROTECTED}");
    }
    Ok(vec![p])
}

/// How much larger `after` is than `before`, in whole percent (at least 1).
fn grew_pct(before: u64, after: u64) -> u64 {
    if before == 0 { return 0; }
    ((after.saturating_sub(before) as f64 / before as f64 * 100.0).round() as u64).max(1)
}

fn main() {
    let json = std::env::args().skip(1).any(|a| a == "--json");
    agent::set_json(json);
    // clap exits 2 on usage errors; keep 2 reserved for password errors.
    let fail = |e: clap::Error| -> ! {
        let code = if e.use_stderr() { 1 } else { 0 };
        if json && code == 1 {
            agent::print_error(&e.render().to_string(), "usage");
        } else {
            let _ = e.print();
        }
        std::process::exit(code);
    };
    let matches = Cli::command().try_get_matches().unwrap_or_else(|e| fail(e));
    let mut cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| fail(e));
    if let (Cmd::Edit { items, .. }, Some(("edit", m))) = (&mut cli.cmd, matches.subcommand()) {
        *items = edit_items(m).unwrap_or_else(|e| fail(Cli::command().error(clap::error::ErrorKind::ArgumentConflict, e)));
    }
    let not_conforming = || NOT_CONFORMING.load(std::sync::atomic::Ordering::Relaxed);
    match run(cli) {
        Ok(paths) if json => {
            agent::print_ok(&paths, not_conforming());
            if not_conforming() {
                std::process::exit(1);
            }
        }
        Ok(paths) => {
            agent::print_plain();
            for p in paths {
                println!("{}", p.display());
            }
            if not_conforming() {
                std::process::exit(1);
            }
        }
        Err(e) => {
            let code = match e {
                OpdfError::PasswordRequired(_) | OpdfError::WrongPassword(_) => 2,
                _ => 1,
            };
            if json {
                agent::print_error(&e.to_string(), agent::error_code(&e));
            } else {
                eprintln!("error: {e}");
            }
            std::process::exit(code);
        }
    }
}
