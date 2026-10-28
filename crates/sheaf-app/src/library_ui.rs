//! Local library screen and native file dialogs; copying and ZIP expansion leave the UI free.

use std::{cell::RefCell, collections::{BTreeMap, BTreeSet}};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use sheaf_app::{AppWindow, LibraryBridge, LibraryRow, LibraryVersionRow, library::{Entry, Library}, store_file};

#[derive(Default)]
struct Catalog {
    entries: Vec<Entry>,
    marked: BTreeSet<String>,
    pending_delete: Vec<String>,
    thumbs: BTreeMap<String, Option<slint::Image>>,
}
thread_local! { static CATALOG: RefCell<Catalog> = RefCell::new(Catalog::default()); }

fn library() -> Library { Library::new(&store_file::data_dir()) }
fn status(ui: &AppWindow, text: impl AsRef<str>) { ui.global::<LibraryBridge>().set_status(text.as_ref().into()); }
fn selected(ui: &AppWindow) -> Option<Entry> {
    CATALOG.with(|c| c.borrow().entries.iter().find(|e| e.id == ui.global::<LibraryBridge>().get_selected_id().as_str()).cloned())
}

pub fn refresh(ui: &AppWindow, select: Option<&str>) {
    let entries = match library().list() { Ok(entries) => entries, Err(e) => return status(ui, e.to_string()) };
    CATALOG.with(|c| { let mut c = c.borrow_mut(); c.marked.retain(|id| entries.iter().any(|e| &e.id == id)); c.entries = entries; });
    if let Ok(paths) = library().working_copies() { ui.global::<LibraryBridge>().set_working(ModelRc::new(VecModel::from(paths.iter().map(|p| p.to_string_lossy().as_ref().into()).collect::<Vec<slint::SharedString>>()))); }
    render(ui, select);
}

fn render(ui: &AppWindow, select: Option<&str>) {
    let mut entries = CATALOG.with(|c| sheaf_app::library::filter_entries(&c.borrow().entries, ui.global::<LibraryBridge>().get_query().as_str(), ui.global::<LibraryBridge>().get_folder().as_str()));
    let b = ui.global::<LibraryBridge>();
    sheaf_app::library::sort_entries(&mut entries, b.get_sort());
    let total = entries.len();
    let pages = total.div_ceil(96).max(1);
    let page = (b.get_page_index().max(0) as usize).min(pages - 1);
    b.set_page_index(page as i32); b.set_page_count(pages as i32); b.set_total(total as i32);
    let entries = entries.into_iter().skip(page * 96).take(96).collect::<Vec<_>>();
    b.set_marked_count(CATALOG.with(|c| c.borrow().marked.len()) as i32);
    let folders = CATALOG.with(|c| sheaf_app::library::folders(&c.borrow().entries));
    b.set_folders(ModelRc::new(VecModel::from(folders.into_iter().map(slint::SharedString::from).collect::<Vec<_>>())));
    b.set_rows(ModelRc::new(VecModel::from(entries.iter().map(|e| LibraryRow {
        id: e.id.as_str().into(), name: e.name.as_str().into(),
        marked: CATALOG.with(|c| c.borrow().marked.contains(&e.id)),
        thumb: CATALOG.with(|c| c.borrow().thumbs.get(&thumb_key(e)).cloned().flatten().unwrap_or_default()),
        has_thumb: CATALOG.with(|c| c.borrow().thumbs.get(&thumb_key(e)).is_some_and(Option::is_some)),
        detail: format!("{} version(s) · {} bytes{}", e.versions.len(), e.versions.last().map_or(0, |v| v.size), if e.folder.is_empty() { String::new() } else { format!(" · {}", e.folder) }).into(),
    }).collect::<Vec<_>>())));
    if ui.get_page() == sheaf_app::Page::Library { for entry in entries.iter().take(256) { thumbnail(ui, entry); } }
    let at = select.and_then(|id| entries.iter().position(|e| e.id == id));
    b.set_selected(at.map_or(-1, |i| i as i32));
    let entry = at.and_then(|i| entries.get(i));
    b.set_selected_id(entry.map(|e| e.id.as_str()).unwrap_or_default().into());
    b.set_name(entry.map(|e| e.name.as_str()).unwrap_or_default().into());
    b.set_versions(ModelRc::new(VecModel::from(entry.into_iter().flat_map(|e| e.versions.iter().rev()).map(|v| LibraryVersionRow {
        number: v.number as i32, label: v.label.as_str().into(),
        detail: format!("{} · {} bytes", chrono::DateTime::from_timestamp(v.created as i64, 0).map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default(), v.size).into(),
    }).collect::<Vec<_>>())));
}

fn thumb_key(entry: &Entry) -> String { format!("{}:{}", entry.id, entry.versions.last().map_or(0, |v| v.number)) }

fn thumbnail(ui: &AppWindow, entry: &Entry) {
    let ext = std::path::Path::new(&entry.name).extension().unwrap_or_default().to_string_lossy().to_lowercase();
    if !["pdf", "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"].contains(&ext.as_str()) { return; }
    let key = thumb_key(entry);
    let needed = CATALOG.with(|c| {
        let mut c = c.borrow_mut();
        if c.thumbs.contains_key(&key) { return false; }
        if c.thumbs.len() >= 256 { c.thumbs.clear(); }
        c.thumbs.insert(key.clone(), None); true
    });
    if !needed { return; }
    let Ok(path) = library().path(&entry.id, 0) else { return };
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let result = if ext == "pdf" {
            sheaf_app::viewer::render_page(&sheaf_core::pages::Input::new(path), 0, 120)
        } else {
            sheaf_app::library::raster_thumbnail(&path).map(|(w, h, rgba)| slint::SharedPixelBuffer::clone_from_slice(&rgba, w, h)).map_err(|e| e.to_string())
        };
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if let Ok(pixels) = result {
                let image = slint::Image::from_rgba8(pixels);
                CATALOG.with(|c| { c.borrow_mut().thumbs.insert(key.clone(), Some(image.clone())); });
                let rows = ui.global::<LibraryBridge>().get_rows();
                let entries = CATALOG.with(|c| c.borrow().entries.clone());
                for i in 0..rows.row_count() {
                    if let Some(mut row) = rows.row_data(i) && entries.iter().any(|e| e.id == row.id.as_str() && thumb_key(e) == key) {
                        row.thumb = image.clone(); row.has_thumb = true; rows.set_row_data(i, row);
                    }
                }
            }
        });
    });
}

pub fn install(ui: &AppWindow) {
    refresh(ui, None);
    let weak = ui.as_weak();
    ui.global::<LibraryBridge>().on_command(move |what, i| { if let Some(ui) = weak.upgrade() { command(&ui, &what, i); } });
}

fn command(ui: &AppWindow, what: &str, index: i32) {
    if ui.global::<LibraryBridge>().get_busy() { return; }
    if what == "open-working" {
        if let Some(path) = ui.global::<LibraryBridge>().get_working().row_data(index as usize) {
            crate::deliver(vec![std::path::PathBuf::from(path.as_str())]);
        }
        return;
    }
    if matches!(what, "filter" | "folder" | "sort") { ui.global::<LibraryBridge>().set_page_index(0); return render(ui, selected(ui).as_ref().map(|e| e.id.as_str())); }
    if what == "page" { return render(ui, selected(ui).as_ref().map(|e| e.id.as_str())); }
    if what == "move-marked" {
        let folder = ui.global::<LibraryBridge>().get_move_folder().to_string();
        let ids = CATALOG.with(|c| c.borrow().marked.iter().cloned().collect::<Vec<_>>());
        let mut completed = 0; let mut errors = Vec::new();
        for id in ids { match library().move_to_folder(&id, &folder) { Ok(_) => completed += 1, Err(e) => errors.push(e.to_string()) } }
        refresh(ui, None); status(ui, format!("Moved {completed} file(s). {}", errors.join(" · "))); return;
    }
    if what == "mark" {
        if let Some(row) = ui.global::<LibraryBridge>().get_rows().row_data(index as usize) {
            CATALOG.with(|c| { let mut c = c.borrow_mut(); if !c.marked.remove(row.id.as_str()) { c.marked.insert(row.id.to_string()); } });
        }
        return render(ui, selected(ui).as_ref().map(|e| e.id.as_str()));
    }
    if what == "mark-all" || what == "clear-marked" {
        let rows = ui.global::<LibraryBridge>().get_rows();
        CATALOG.with(|c| { let mut c = c.borrow_mut(); if what == "clear-marked" { c.marked.clear(); } else { for row in rows.iter() { c.marked.insert(row.id.to_string()); } } });
        return render(ui, selected(ui).as_ref().map(|e| e.id.as_str()));
    }
    if what == "export-marked" {
        let ids = CATALOG.with(|c| c.borrow().marked.iter().cloned().collect::<Vec<_>>());
        if ids.is_empty() { return; }
        let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Export selected library files"));
        crate::dialog::pick_folder(ui, d, move |ui, out| {
            ui.global::<LibraryBridge>().set_busy(true);
            let weak = ui.as_weak();
            std::thread::spawn(move || {
                let report = library().export_many(&ids, &out);
                let _ = weak.upgrade_in_event_loop(move |ui| { ui.global::<LibraryBridge>().set_busy(false); status(&ui, format!("{} file(s) exported. {}", report.completed, report.errors.join(" · "))); });
            });
        });
        return;
    }
    if what == "ask-delete-marked" {
        let ids = CATALOG.with(|c| c.borrow().marked.iter().cloned().collect::<Vec<_>>());
        if ids.is_empty() { return; }
        ui.global::<LibraryBridge>().set_delete_name(format!("{} selected files", ids.len()).into());
        ui.global::<LibraryBridge>().set_delete_id("batch".into());
        CATALOG.with(|c| c.borrow_mut().pending_delete = ids);
        return;
    }
    if what == "select" {
        let id = ui.global::<LibraryBridge>().get_rows().row_data(index as usize).map(|row| row.id.to_string());
        return render(ui, id.as_deref());
    }
    if what == "refresh" { return refresh(ui, selected(ui).as_ref().map(|e| e.id.as_str())); }
    if what == "add" {
        let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Add private copies to Library"));
        return crate::dialog::pick_files(ui, d, |ui, files| {
            ui.global::<LibraryBridge>().set_busy(true);
            status(ui, "Adding files…");
            let weak = ui.as_weak();
            std::thread::spawn(move || {
                let mut added = 0; let mut errors = Vec::new(); let mut id = None;
                for file in files {
                    match library().add(&file) { Ok(entry) => { added += 1; id = Some(entry.id); }, Err(e) => errors.push(format!("{}: {e}", file.display())) }
                }
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<LibraryBridge>().set_busy(false); refresh(&ui, id.as_deref());
                    status(&ui, format!("{added} file(s) added. {}", errors.join(" · ")));
                });
            });
        });
    }
    if what == "cancel-delete" {
        ui.global::<LibraryBridge>().set_delete_id("".into());
        return;
    }
    if what == "delete" {
        let id = ui.global::<LibraryBridge>().get_delete_id();
        if id.is_empty() { return; }
        ui.global::<LibraryBridge>().set_delete_id("".into());
        if id == "batch" {
            let ids = CATALOG.with(|c| std::mem::take(&mut c.borrow_mut().pending_delete));
            let report = library().delete_many(&ids);
            refresh(ui, None);
            status(ui, format!("{} file(s) deleted. {}", report.completed, report.errors.join(" · ")));
            return;
        }
        return match library().delete(&id) {
            Ok(()) => { refresh(ui, None); status(ui, "File and its stored versions deleted. The original is untouched."); },
            Err(e) => status(ui, e.to_string()),
        };
    }
    let Some(entry) = selected(ui) else { return };
    match what {
        "open" => match library().checkout(&entry.id) {
            Ok(path) => crate::deliver(vec![path]), Err(e) => status(ui, e.to_string()),
        },
        "export" => {
            let d = crate::dialog::new(ui).set_file_name(&entry.name);
            crate::dialog::save_file(ui, d, move |ui, out| match library().export(&entry.id, index as u32, &out) {
                Ok(path) => status(ui, format!("Copy saved to {}", path.display())), Err(e) => status(ui, e.to_string()),
            });
        }
        "update" => {
            let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Choose the new file version"));
            crate::dialog::pick_file(ui, d, move |ui, path| {
                match library().update(&entry.id, &path) { Ok(()) => { refresh(ui, Some(&entry.id)); status(ui, "New version kept"); }, Err(e) => status(ui, e.to_string()) }
            });
        }
        "restore" => match library().restore(&entry.id, index as u32) {
            Ok(()) => { refresh(ui, Some(&entry.id)); status(ui, "Restored as a new version; all later versions kept"); }, Err(e) => status(ui, e.to_string()),
        },
        "ask-delete" => {
            ui.global::<LibraryBridge>().set_delete_name(entry.name.into());
            ui.global::<LibraryBridge>().set_delete_id(entry.id.into());
        }
        "unzip" => {
            ui.global::<LibraryBridge>().set_busy(true); status(ui, "Unzipping…");
            let weak = ui.as_weak();
            std::thread::spawn(move || {
                let result = library().unzip(&entry.id);
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<LibraryBridge>().set_busy(false); refresh(&ui, Some(&entry.id));
                    match result { Ok(report) => status(&ui, format!("{} file(s) unpacked. {}", report.imported.len(), report.errors.join(" · "))), Err(e) => status(&ui, e.to_string()) }
                });
            });
        }
        _ => {}
    }
}
