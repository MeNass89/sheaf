//! The action button: options → `Job`, where to save, running it, and the result page.

use std::path::PathBuf;

use sheaf_app::jobs::{self, Job, JobOutcome, SplitMode};
use sheaf_app::prefs::OutputMode;
use sheaf_app::{AppWindow, ToolOptions};
use sheaf_core::compress::Level;
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
