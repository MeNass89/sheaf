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
        state::push_tab_row(&t);
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
    let mut dlg = crate::dialog::new(ui).add_filter(info.name, info.accepts);
    if let Some(dir) = with(|a| a.tab(tab_id).and_then(|t| t.files.last()).and_then(|f| f.input.path.parent().map(|d| d.to_path_buf()))) {
        dlg = dlg.set_directory(dir);
    }
    if info.multi {
        crate::dialog::pick_files(ui, dlg, move |ui, paths| add_paths(ui, tab_id, paths));
    } else {
        crate::dialog::pick_file(ui, dlg, move |ui, p| add_paths(ui, tab_id, vec![p]));
    }
}

/// The documents open in reader tabs (as last saved) that tab `tab_id`'s tool takes and does
/// not have yet, in tab order; a single-file tool is offered the last one.
pub fn open_documents(tab_id: i32) -> Vec<PathBuf> {
    with(|a| {
        let Some(t) = a.tabs.iter().find(|t| t.id == tab_id) else { return Vec::new() };
        let have: Vec<&PathBuf> = t.files.iter().map(|f| &f.input.path).collect();
        let mut docs: Vec<PathBuf> = a.tabs.iter().filter_map(|r| r.reader.as_ref()).map(|r| r.path.clone()).filter(|p| files::accepts(t.tool, p) && !have.contains(&p)).collect();
        let mut seen = Vec::new();
        docs.retain(|p| !seen.contains(p) && {
            seen.push(p.clone());
            true
        });
        // Compare picks one document per slot from them all; another single-file tool takes the last.
        if !tools::find(t.tool).multi && t.tool != ToolId::Compare {
            docs = docs.pop().into_iter().collect();
        }
        docs
    })
}

/// Compare: open document `i` (of [`open_documents`]) in slot `slot` (0 Original, 1 Changed).
pub fn add_open_doc_to_slot(ui: &AppWindow, tab_id: i32, slot: i32, i: i32) {
    let Some(p) = open_documents(tab_id).into_iter().nth(i.max(0) as usize) else { return };
    let unsaved = with(|a| a.tabs.iter().filter_map(|t| t.reader.as_ref()).find(|r| r.dirty() && r.path == p).map(|r| r.name.clone()));
    add_paths_to(ui, tab_id, vec![p], Some(slot.clamp(0, 1) as usize));
    if let Some(name) = unsaved {
        toast(ui, &format!("{name} has unsaved changes: its saved version was added"));
    }
}

/// "Open documents (n)": adds them; one with unsaved edits goes in as last saved, and says so.
pub fn add_open_documents(ui: &AppWindow, tab_id: i32) {
    let docs = open_documents(tab_id);
    let unsaved: Vec<String> = with(|a| a.tabs.iter().filter_map(|t| t.reader.as_ref()).filter(|r| r.dirty() && docs.contains(&r.path)).map(|r| r.name.clone()).collect());
    add_paths(ui, tab_id, docs);
    match unsaved.as_slice() {
        [] => {}
        [one] => toast(ui, &format!("{one} has unsaved changes: its saved version was added")),
        more => toast(ui, &format!("{} documents have unsaved changes: their saved versions were added", more.len())),
    }
}

/// Adds files to a tab: rejected extensions get a toast; single-file tools keep the newest.
pub fn add_paths(ui: &AppWindow, tab_id: i32, paths: Vec<PathBuf>) {
    add_paths_to(ui, tab_id, paths, None);
}

/// Compare's "Open file" on one slot (0 Original, 1 Changed).
pub fn pick_for_slot(ui: &AppWindow, tab_id: i32, slot: i32) {
    let dir = with(|a| a.tab(tab_id).and_then(|t| t.files.last()).and_then(|f| f.input.path.parent().map(|d| d.to_path_buf())));
    let mut dlg = crate::dialog::new(ui).add_filter("PDF", &["pdf"]).set_title(if slot == 0 { "Choose the original PDF" } else { "Choose the changed PDF" });
    if let Some(dir) = dir {
        dlg = dlg.set_directory(dir);
    }
    crate::dialog::pick_file(ui, dlg, move |ui, p| add_paths_to(ui, tab_id, vec![p], Some(slot.clamp(0, 1) as usize)));
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
            let entry = FileEntry { uid, input: Input::new(&p), kind, size: files::file_size(&p), pages: None, thumb: None, thumb_failed: false, unreadable: false, locked: false, slot: 0, scanned: false, pages_spec: String::new() };
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
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| files::probe_pdf_full(&input, THUMB_PX))).unwrap_or_else(|p| {
                panicked = true;
                Err(OpdfError::Conversion(format!("internal error: {}", sheaf_app::jobs::panic_message(&*p))))
            });
            let _ = weak.upgrade_in_event_loop(move |ui| match r {
                Ok(files::Probe { pages, thumb, scanned, .. }) => {
                    let (organize, crop, view) = with(|a| {
                        let Some(t) = a.tab(tab_id) else { return (None, None, None) };
                        let Some(f) = t.files.iter_mut().find(|f| f.uid == uid) else { return (None, None, None) };
                        f.pages = Some(pages);
                        f.scanned = scanned;
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
                        // Organize and Split show every page (Split cuts between them).
                        let organize = if matches!(t.tool, ToolId::Organize | ToolId::Split) {
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
                            toast(&ui, &files::open_error(&input.path.file_name().unwrap_or_default().to_string_lossy(), &e));
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
        // An Office or HTML file: converted on the side for its page 1 and page count, as
        // iLovePDF shows them (none without LibreOffice; the card keeps its icon then).
        Kind::Document => {
            if sheaf_core::office::find_soffice().is_none() {
                return;
            }
            let r = catching(|| files::document_probe(&input.path, THUMB_PX));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with(|a| {
                    if let (Ok((pages, thumb)), Some(f)) = (r, a.tab(tab_id).and_then(|t| t.files.iter_mut().find(|f| f.uid == uid))) {
                        f.pages = Some(pages);
                        f.thumb = Some(state::to_image(thumb));
                    }
                    refresh(a, tab_id);
                });
            });
        }
    });
}

/// Organize and Split: render every page on the probe pool, posting each thumbnail as it is ready.
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

/// Where an item at `from` goes when dropped before slot `slot` (0..=len) of a list of `len`.
pub(crate) fn dropped_at(from: usize, slot: usize, len: usize) -> usize {
    let slot = slot.min(len);
    if slot > from { slot - 1 } else { slot }
}

/// A file card dragged and dropped before slot `slot` of the list (iLovePDF's drag to reorder).
pub fn place_file(tab_id: i32, uid: i32, slot: i32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id)
            && let Some(i) = t.files.iter().position(|f| f.uid == uid)
        {
            let to = dropped_at(i, slot.max(0) as usize, t.files.len());
            let f = t.files.remove(i);
            t.files.insert(to, f);
        }
        refresh(a, tab_id);
    });
}

/// Merge: the pages taken from file `uid` ("" = all), kept as typed; the card says when they
/// are not pages of the file and Merge waits until they are.
pub fn set_pages(tab_id: i32, uid: i32, text: &str) {
    with(|a| {
        if let Some(f) = a.tab(tab_id).and_then(|t| t.files.iter_mut().find(|f| f.uid == uid)) {
            let t = text.trim();
            f.pages_spec = if t.eq_ignore_ascii_case("all") { String::new() } else { t.to_string() };
        }
        refresh(a, tab_id);
    });
}

/// The files by name, A to Z ("file 2" before "file 10"); Z to A when they already are.
pub fn sort_files(tab_id: i32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            let name = |f: &state::FileEntry| f.input.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let ascending = t.files.windows(2).all(|w| files::natural_cmp(&name(&w[0]), &name(&w[1])).is_le());
            t.files.sort_by(|x, y| files::natural_cmp(&name(x), &name(y)));
            if ascending {
                t.files.reverse();
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
    crate::reader_ui::remember_position(tab_id);
    crate::reader_ui::stop_work(tab_id);
    crate::wm_preview::forget(tab_id);
    with(|a| {
        let t = &a.tabs[i];
        if matches!(t.phase, Phase::Running) {
            let inputs = t.files.iter().map(|f| f.input.path.clone()).collect();
            a.closed_jobs.insert(tab_id, (t.tool, inputs));
        }
        a.tabs.remove(i);
        a.pw_queue.retain(|(t, _)| *t != tab_id);
    });
    // The tab before it in the bar takes over (Home after the first).
    let before = state::tab_order().iter().take_while(|r| *r != i as i32).last();
    state::remove_tab_row(i);
    let active = ui.get_active_tab();
    if active == i as i32 {
        ui.set_active_tab(before.map_or(-1, |r| if r > i as i32 { r - 1 } else { r }));
    } else if active > i as i32 {
        ui.set_active_tab(active - 1);
    }
    crate::password::show_head(ui);
}

pub fn close_tab(ui: &AppWindow, tab_id: i32) {
    // A document with unsaved annotations asks first.
    if let Some(name) = crate::reader_ui::unsaved_name(tab_id) {
        with(|a| a.pending_close = Some(tab_id));
        ui.set_unsaved_redact_marks(0);
        ui.set_unsaved_file(name.into());
        return;
    }
    // Redact with marks not applied: closing drops them, asked first (Sol, 25/10).
    if let Some((name, n)) = crate::viewer_ui::redact_pending(tab_id) {
        with(|a| a.pending_close = Some(tab_id));
        ui.set_unsaved_redact_marks(n as i32);
        ui.set_unsaved_file(name.into());
        return;
    }
    let running = with(|a| a.tab(tab_id).is_some_and(|t| matches!(t.phase, Phase::Running)));
    if running {
        with(|a| a.pending_close = Some(tab_id));
        ui.set_confirm_visible(true);
    } else {
        remove_tab(ui, tab_id);
    }
}

/// A tab dragged from place `from` to `to` in the bar. Only the bar's order changes: every tab's
/// page, and which one is active, stay as they are.
pub fn move_tab(from: i32, to: i32) {
    if from >= 0 && to >= 0 {
        state::move_in_order(from as usize, to as usize);
    }
}

/// ⇧⌘[ and ⇧⌘]: the tab `d` places along the bar from the active one, wrapping (Home not included).
pub fn step_tab(ui: &AppWindow, d: i32) {
    let order: Vec<i32> = state::tab_order().iter().collect();
    let n = order.len() as i32;
    if n == 0 {
        return;
    }
    let at = order.iter().position(|r| *r == ui.get_active_tab()).map_or(0, |p| p as i32);
    ui.set_active_tab(order[(at + d).rem_euclid(n) as usize]);
}

/// The tab bar's menu: close the other tabs (`which` 0), those to the left (1) or to the right
/// (2) of tab `id`, or every tab (3: Close All Tabs, ⌥⌘W). Tabs with a running task stay open;
/// documents with unsaved edits are asked about in turn.
pub fn close_tabs_around(ui: &AppWindow, id: i32, which: i32) {
    let ids: Vec<i32> = with(|a| {
        // Left and right as the bar shows them.
        let bar: Vec<i32> = state::tab_order().iter().filter_map(|r| a.tabs.get(r as usize)).map(|t| t.id).collect();
        let Some(at) = bar.iter().position(|t| *t == id) else { return Vec::new() };
        bar.iter().enumerate().filter(|(i, _)| match which { 1 => *i < at, 2 => *i > at, 3 => true, _ => *i != at }).map(|(_, t)| *t).collect()
    });
    let mut kept = 0;
    let mut ask = Vec::new();
    for t in ids {
        let busy = with(|a| a.tab(t).is_some_and(|t| matches!(t.phase, Phase::Running)));
        if busy {
            kept += 1;
        } else if crate::reader_ui::unsaved_name(t).is_some() {
            ask.push(t);
        } else {
            remove_tab(ui, t);
        }
    }
    if kept > 0 {
        toast(ui, &format!("{kept} tab{} with a running task stayed open", if kept == 1 { "" } else { "s" }));
    }
    // Documents with unsaved edits are asked about one by one (Save / Don't Save / Cancel), as
    // closing each would.
    with(|a| a.close_queue = ask);
    next_close(ui);
}

/// The next document of a Close Other / Left / Right still to ask about; Cancel empties the queue.
pub fn next_close(ui: &AppWindow) {
    let next = with(|a| {
        while !a.close_queue.is_empty() {
            let id = a.close_queue.remove(0);
            if a.tab(id).is_some() {
                return Some(id);
            }
        }
        None
    });
    if let Some(id) = next {
        if let Some(i) = with(|a| a.tab_index(id)) {
            ui.set_active_tab(i as i32);
        }
        close_tab(ui, id);
    }
}

/// The Save / Don't Save / Cancel answer for the document being closed (1, 2, 0).
pub fn unsaved_answer(ui: &AppWindow, answer: i32) {
    ui.set_unsaved_file("".into());
    let Some(id) = with(|a| a.pending_close.take()) else { return };
    match answer {
        1 => {
            // The tab closes when the document thread says the file is written.
            with(|a| a.closing_after_save.insert(id));
            crate::reader_ui::save(ui, id);
            return;
        }
        2 => remove_tab(ui, id),
        _ => with(|a| {
            a.quitting = false;
            a.close_queue.clear();
        }),
    }
    if with(|a| a.quitting) {
        quit_or_ask(ui);
    } else {
        next_close(ui);
    }
}

/// The window's close button: documents with unsaved edits are asked about first.
pub fn close_requested(ui: &AppWindow) -> slint::CloseRequestResponse {
    with(|a| a.quitting = true);
    quit_or_ask(ui)
}

pub fn quit_or_ask(ui: &AppWindow) -> slint::CloseRequestResponse {
    let dirty = with(|a| a.tabs.iter().map(|t| t.id).collect::<Vec<_>>()).into_iter().find(|id| crate::reader_ui::unsaved_name(*id).is_some());
    match dirty {
        Some(id) => {
            if let Some(i) = with(|a| a.tab_index(id)) {
                ui.set_active_tab(i as i32);
            }
            close_tab(ui, id);
            slint::CloseRequestResponse::KeepWindowShown
        }
        None => {
            for id in with(|a| a.tabs.iter().map(|t| t.id).collect::<Vec<_>>()) {
                crate::reader_ui::remember_position(id);
            }
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        }
    }
}

// ---- drops and Home ---------------------------------------------------------------------------

/// A file dropped anywhere on the window.
pub fn dropped(ui: &AppWindow, path: PathBuf) {
    // On a document tab, a PDF dropped opens in its own tab.
    if let Some((id, _)) = active_tool(ui)
        && crate::reader_ui::is_reader(id)
    {
        return home_files(ui, vec![path]);
    }
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

/// What dropping `path` here would do, said while it is dragged over the window; true when the
/// tab shown refuses it (said in the warning colour, as the drop would toast).
pub fn drop_hint(ui: &AppWindow, path: &std::path::Path) -> (String, bool) {
    let active = active_tool(ui);
    let pdf = files::kind_of(path) == Kind::Pdf;
    if active.is_some_and(|(id, _)| crate::reader_ui::is_reader(id)) {
        return ((if pdf { "Drop to open in a new tab" } else { "Drop to choose a tool for it" }).into(), false);
    }
    match files::route_drop(active.map(|(_, t)| t), path) {
        DropAction::AddToTab => (format!("Drop to add to {}", active.map_or("", |(_, t)| tools::find(t).name)), false),
        DropAction::PickTool => ((if pdf { "Drop to open" } else { "Drop to choose a tool for it" }).into(), false),
        DropAction::Reject(msg) => (msg, true),
    }
}

/// PDFs opened from Home: kept for the next tool opened; Tools is shown to pick one.
pub fn home_files(ui: &AppWindow, paths: Vec<PathBuf>) {
    // PDFs open in the reader, as a double-click does in PDF Expert; other files wait for a tool.
    let (pdfs, paths): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|p| files::kind_of(p) == Kind::Pdf);
    for p in pdfs {
        crate::reader_ui::open(ui, p, None);
    }
    if paths.is_empty() {
        return;
    }
    let n = with(|a| {
        for p in paths {
            if !a.home_files.contains(&p) {
                a.home_files.push(p);
            }
        }
        a.home_files.len()
    });
    ui.set_active_tab(-1);
    ui.set_page(Page::Tools);
    let what = if n == 1 { "your file".to_string() } else { format!("your {n} files") };
    toast(ui, &format!("Choose a tool for {what}"));
}

#[cfg(test)]
mod tests {
    use super::dropped_at;

    #[test]
    fn a_dropped_card_lands_before_the_slot_it_is_dropped_on() {
        // [a b c d]: a dropped before slot 3 (before d) → [b c a d], at 2.
        assert_eq!(dropped_at(0, 3, 4), 2);
        // d dropped before slot 0 → first.
        assert_eq!(dropped_at(3, 0, 4), 0);
        // Past the end → last; on its own place → unchanged.
        assert_eq!(dropped_at(1, 9, 4), 3);
        assert_eq!(dropped_at(2, 2, 4), 2);
        assert_eq!(dropped_at(2, 3, 4), 2);
    }
}
