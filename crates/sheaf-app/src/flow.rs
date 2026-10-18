//! Tool tabs: opening and closing them, adding files (dialog, FAB, drop), thumbnails on worker
//! threads, the password dialog queue, Organize page edits and the toast.

use std::path::PathBuf;
use std::time::Duration;

use sheaf_app::files::{self, DropAction, Kind, ProbeFailure};
use sheaf_app::jobs::catching;
use sheaf_app::{AppWindow, Page};
use sheaf_core::pages::Input;
use sheaf_core::tools::{self, ToolId};
use sheaf_core::OpdfError;
use slint::{ComponentHandle, Model as _};

use crate::state::{self, refresh, tabs_model, with, FileEntry, Phase, TabState};

/// Card thumbnail width in pixels (60 logical px on a Retina screen).
const THUMB_PX: i32 = 120;
const PAGE_THUMB_PX: i32 = 168;
/// Crop preview width in pixels.
const PREVIEW_PX: i32 = 300;

pub fn tool_from_str(s: &str) -> Option<ToolId> {
    ToolId::from_str(s)
}

pub fn toast(ui: &AppWindow, text: &str) {
    ui.set_toast_text(text.into());
    ui.set_toast_visible(true);
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::SingleShot, Duration::from_secs(3), move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_toast_visible(false);
        }
    });
    with(|a| a.toast_timer = Some(timer));
}

/// The tool of the active tab, `None` on Home.
pub fn active_tool(ui: &AppWindow) -> Option<(i32, ToolId)> {
    let i = ui.get_active_tab();
    if i < 0 {
        return None;
    }
    with(|a| a.tabs.get(i as usize).map(|t| (t.id, t.tool)))
}

/// Opens `tool` in a new tab, preloaded with `files` (those it accepts), and shows it.
pub fn open_tool(ui: &AppWindow, tool: ToolId, preload: Vec<PathBuf>) {
    let id = with(|a| {
        a.next_id += 1;
        let t = TabState::new(a.next_id, tool);
        tabs_model().push(state::build(&t));
        a.tabs.push(t);
        a.next_id
    });
    ui.set_active_tab(tabs_model().row_count() as i32 - 1);
    let keep: Vec<PathBuf> = preload.into_iter().filter(|p| files::accepts(tool, p)).collect();
    if !keep.is_empty() {
        add_paths(ui, id, keep);
    }
}

/// "Open file" / "+" in a tool tab: file dialog filtered by what the tool accepts.
pub fn pick_and_add(ui: &AppWindow, tab_id: i32) {
    let Some(tool) = with(|a| a.tab(tab_id).map(|t| t.tool)) else { return };
    let info = tools::find(tool);
    let mut dlg = rfd::FileDialog::new().add_filter(info.name, info.accepts);
    if let Some(dir) = with(|a| a.tab(tab_id).and_then(|t| t.files.last()).and_then(|f| f.input.path.parent().map(|d| d.to_path_buf()))) {
        dlg = dlg.set_directory(dir);
    }
    let picked = if info.multi { dlg.pick_files() } else { dlg.pick_file().map(|p| vec![p]) };
    if let Some(paths) = picked {
        add_paths(ui, tab_id, paths);
    }
}

/// Adds files to a tab: rejected extensions get a toast; single-file tools keep the newest.
pub fn add_paths(ui: &AppWindow, tab_id: i32, paths: Vec<PathBuf>) {
    add_paths_to(ui, tab_id, paths, None);
}

/// Compare's "Open file" on one slot (0 Original, 1 Changed).
pub fn pick_for_slot(ui: &AppWindow, tab_id: i32, slot: i32) {
    let dir = with(|a| a.tab(tab_id).and_then(|t| t.files.last()).and_then(|f| f.input.path.parent().map(|d| d.to_path_buf())));
    let mut dlg = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_title(if slot == 0 { "Choose the original PDF" } else { "Choose the changed PDF" });
    if let Some(dir) = dir {
        dlg = dlg.set_directory(dir);
    }
    if let Some(p) = dlg.pick_file() {
        add_paths_to(ui, tab_id, vec![p], Some(slot.clamp(0, 1) as usize));
    }
}

/// Compare: Original and Changed trade places.
pub fn swap_compare(tab_id: i32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            t.swap_compare();
        }
        refresh(a, tab_id);
    });
}

fn add_paths_to(ui: &AppWindow, tab_id: i32, paths: Vec<PathBuf>, slot: Option<usize>) {
    let Some(tool) = with(|a| a.tab(tab_id).filter(|t| matches!(t.phase, Phase::Edit)).map(|t| t.tool)) else { return };
    let multi = tools::find(tool).multi;
    let mut probes = Vec::new();
    for p in paths {
        if !files::accepts(tool, &p) || p.is_dir() {
            toast(ui, &files::rejection(tool, &p));
            continue;
        }
        with(|a| {
            a.next_uid += 1;
            let uid = a.next_uid;
            let Some(t) = a.tab(tab_id) else { return };
            let kind = files::kind_of(&p);
            let entry = FileEntry { uid, input: Input::new(&p), kind, size: files::file_size(&p), pages: None, thumb: None, thumb_failed: false, unreadable: false, locked: false, slot: 0 };
            if tool == ToolId::Compare {
                let taken: Vec<usize> = t.files.iter().map(|f| f.slot).collect();
                // Several files at once fill the slots in order; one asked-for slot is used once.
                t.put_compare(state::compare_slot(&taken, slot), entry);
                probes.push((uid, Input::new(&p), kind));
                return;
            }
            if !multi {
                t.files.clear();
                t.order.clear();
                t.page_thumbs.clear();
            }
            t.files.push(entry);
            probes.push((uid, Input::new(&p), kind));
        });
    }
    with(|a| refresh(a, tab_id));
    for (uid, input, kind) in probes {
        probe(ui, tab_id, uid, input, kind);
    }
}

/// Page count and thumbnail on the bounded probe pool; password errors queue the dialog.
/// A panicking probe is caught and treated as a failed probe, so the card never hangs.
pub fn probe(ui: &AppWindow, tab_id: i32, uid: i32, input: Input, kind: Kind) {
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || match kind {
        Kind::Pdf => {
            let mut panicked = false;
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| files::probe_pdf(&input, THUMB_PX))).unwrap_or_else(|p| {
                panicked = true;
                Err(OpdfError::Conversion(format!("internal error: {}", sheaf_app::jobs::panic_message(&*p))))
            });
            let _ = weak.upgrade_in_event_loop(move |ui| match r {
                Ok((pages, thumb)) => {
                    let (organize, crop, view) = with(|a| {
                        let Some(t) = a.tab(tab_id) else { return (None, None, None) };
                        let Some(f) = t.files.iter_mut().find(|f| f.uid == uid) else { return (None, None, None) };
                        f.pages = Some(pages);
                        f.thumb = Some(state::to_image(thumb));
                        f.locked = f.input.password.is_some();
                        let input = f.input.clone();
                        if t.tool == ToolId::Crop {
                            t.preview = None;
                            return (None, Some(input), None);
                        }
                        if matches!(t.tool, ToolId::Redact | ToolId::Sign | ToolId::Edit) {
                            t.new_viewer_file();
                            return (None, None, Some(input));
                        }
                        let organize = if t.tool == ToolId::Organize {
                            t.order = (1..=pages).collect();
                            t.page_thumbs.clear();
                            t.pages_failed = false;
                            Some(input)
                        } else {
                            None
                        };
                        (organize, None, None)
                    });
                    with(|a| refresh(a, tab_id));
                    crate::password::done(&ui, tab_id, uid);
                    if let Some(input) = organize {
                        page_thumbs(&ui, tab_id, input);
                    }
                    if let Some(input) = crop {
                        crop_preview(&ui, tab_id, uid, input);
                    }
                    if let Some(input) = view {
                        crate::viewer_ui::load(&ui, tab_id, input);
                    }
                }
                Err(e) => {
                    let Some(tool) = with(|a| a.tab(tab_id).map(|t| t.tool)) else { return };
                    match files::probe_failure(tool, &e, panicked) {
                        ProbeFailure::AskPassword { wrong } => crate::password::ask(&ui, tab_id, uid, wrong),
                        keep @ (ProbeFailure::KeepForRepair | ProbeFailure::KeepUnpreviewed) => {
                            with(|a| {
                                if let Some(f) = a.tab(tab_id).and_then(|t| t.files.iter_mut().find(|f| f.uid == uid)) {
                                    f.unreadable = keep == ProbeFailure::KeepForRepair;
                                    f.thumb_failed = keep == ProbeFailure::KeepUnpreviewed;
                                    f.locked = f.input.password.is_some();
                                }
                                refresh(a, tab_id);
                            });
                            crate::password::done(&ui, tab_id, uid);
                        }
                        ProbeFailure::Remove => {
                            toast(&ui, &format!("Cannot open {}: {e}", input.path.file_name().unwrap_or_default().to_string_lossy()));
                            remove_file(&ui, tab_id, uid);
                        }
                    }
                }
            });
        }
        Kind::Image => {
            let r = catching(|| files::image_thumb(&input.path, files::IMAGE_THUMB_PX));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with(|a| {
                    if let Some(f) = a.tab(tab_id).and_then(|t| t.files.iter_mut().find(|f| f.uid == uid)) {
                        match r {
                            Ok(thumb) => f.thumb = Some(state::to_image(thumb)),
                            Err(_) => f.thumb_failed = true,
                        }
                    }
                    refresh(a, tab_id);
                });
            });
        }
        Kind::Document => {}
    });
}

/// Organize: render every page on the probe pool, posting each thumbnail as it is ready.
fn page_thumbs(ui: &AppWindow, tab_id: i32, input: Input) {
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let w = weak.clone();
        let r = catching(|| files::page_thumbs(&input, PAGE_THUMB_PX, |n, thumb| {
            let _ = weak.upgrade_in_event_loop(move |_| {
                with(|a| {
                    if let Some(t) = a.tab(tab_id) {
                        t.page_thumbs.insert(n, state::to_image(thumb));
                    }
                    refresh(a, tab_id);
                });
            });
        }));
        if r.is_err() {
            let _ = w.upgrade_in_event_loop(move |_| {
                with(|a| {
                    if let Some(t) = a.tab(tab_id) {
                        t.pages_failed = true;
                    }
                    refresh(a, tab_id);
                });
            });
        }
    });
}

/// Crop: page 1 at `PREVIEW_PX` on the probe pool, shown under the margin fields. Dropped if
/// the file was replaced meanwhile.
fn crop_preview(ui: &AppWindow, tab_id: i32, uid: i32, input: Input) {
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let Ok((thumb, w, h)) = catching(|| files::first_page_preview(&input, PREVIEW_PX)) else { return };
        let _ = weak.upgrade_in_event_loop(move |_| {
            with(|a| {
                if let Some(t) = a.tab(tab_id).filter(|t| t.files.first().is_some_and(|f| f.uid == uid)) {
                    t.preview = Some((state::to_image(thumb), w, h));
                }
                refresh(a, tab_id);
            });
        });
    });
}

// ---- file and page edits ----------------------------------------------------------------------

pub fn remove_file(ui: &AppWindow, tab_id: i32, uid: i32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            t.remove_file(uid);
        }
        refresh(a, tab_id);
    });
    let queued = with(|a| a.pw_queue.contains(&(tab_id, uid)));
    if queued {
        with(|a| a.pw_queue.retain(|e| *e != (tab_id, uid)));
        crate::password::show_head(ui);
    }
}

pub fn move_file(tab_id: i32, uid: i32, delta: i32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id)
            && let Some(i) = t.files.iter().position(|f| f.uid == uid)
        {
            let j = i as i32 + delta;
            if j >= 0 && (j as usize) < t.files.len() {
                t.files.swap(i, j as usize);
            }
        }
        refresh(a, tab_id);
    });
}

pub fn edit_pages(tab_id: i32, f: impl FnOnce(&mut Vec<u32>)) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            f(&mut t.order);
        }
        refresh(a, tab_id);
    });
}

// ---- tabs -------------------------------------------------------------------------------------

/// Removes a tab and returns to the one before it.
pub fn remove_tab(ui: &AppWindow, tab_id: i32) {
    let Some(i) = with(|a| a.tab_index(tab_id)) else { return };
    with(|a| {
        let t = &a.tabs[i];
        if matches!(t.phase, Phase::Running) {
            let inputs = t.files.iter().map(|f| f.input.path.clone()).collect();
            a.closed_jobs.insert(tab_id, (t.tool, inputs));
        }
        a.tabs.remove(i);
        a.pw_queue.retain(|(t, _)| *t != tab_id);
    });
    tabs_model().remove(i);
    let active = ui.get_active_tab();
    if active == i as i32 {
        ui.set_active_tab(i as i32 - 1);
    } else if active > i as i32 {
        ui.set_active_tab(active - 1);
    }
    crate::password::show_head(ui);
}

pub fn close_tab(ui: &AppWindow, tab_id: i32) {
    let running = with(|a| a.tab(tab_id).is_some_and(|t| matches!(t.phase, Phase::Running)));
    if running {
        with(|a| a.pending_close = Some(tab_id));
        ui.set_confirm_visible(true);
    } else {
        remove_tab(ui, tab_id);
    }
}

// ---- drops and Home ---------------------------------------------------------------------------

/// A file dropped anywhere on the window.
pub fn dropped(ui: &AppWindow, path: PathBuf) {
    let active = active_tool(ui);
    match files::route_drop(active.map(|(_, t)| t), &path) {
        DropAction::AddToTab => {
            if let Some((id, _)) = active {
                add_paths(ui, id, vec![path]);
            }
        }
        DropAction::PickTool => home_files(ui, vec![path]),
        DropAction::Reject(msg) => toast(ui, &msg),
    }
}

/// PDFs opened from Home: kept for the next tool opened; Tools is shown to pick one.
pub fn home_files(ui: &AppWindow, paths: Vec<PathBuf>) {
    let n = with(|a| {
        a.home_files.extend(paths);
        a.home_files.len()
    });
    ui.set_active_tab(-1);
    ui.set_page(Page::Tools);
    let what = if n == 1 { "your file".to_string() } else { format!("your {n} files") };
    toast(ui, &format!("Choose a tool for {what}"));
}
