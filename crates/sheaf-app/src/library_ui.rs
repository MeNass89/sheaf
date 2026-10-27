//! Local library screen and native file dialogs; copying and ZIP expansion leave the UI free.

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use sheaf_app::{AppWindow, LibraryBridge, LibraryRow, LibraryVersionRow, library::{Entry, Library}, store_file};

fn library() -> Library { Library::new(&store_file::data_dir()) }
fn status(ui: &AppWindow, text: impl AsRef<str>) { ui.global::<LibraryBridge>().set_status(text.as_ref().into()); }
fn selected(ui: &AppWindow) -> Option<Entry> {
    library().entry(ui.global::<LibraryBridge>().get_selected_id().as_str()).ok()
}

fn refresh(ui: &AppWindow, select: Option<&str>) {
    let entries = match library().list() { Ok(entries) => entries, Err(e) => return status(ui, e.to_string()) };
    let b = ui.global::<LibraryBridge>();
    b.set_rows(ModelRc::new(VecModel::from(entries.iter().map(|e| LibraryRow {
        id: e.id.as_str().into(), name: e.name.as_str().into(), detail: format!("{} version(s) · {} bytes", e.versions.len(), e.versions.last().map_or(0, |v| v.size)).into(),
    }).collect::<Vec<_>>())));
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

pub fn install(ui: &AppWindow) {
    refresh(ui, None);
    let weak = ui.as_weak();
    ui.global::<LibraryBridge>().on_command(move |what, i| { if let Some(ui) = weak.upgrade() { command(&ui, &what, i); } });
}

fn command(ui: &AppWindow, what: &str, index: i32) {
    if ui.global::<LibraryBridge>().get_busy() { return; }
    if what == "select" {
        let id = ui.global::<LibraryBridge>().get_rows().row_data(index as usize).map(|row| row.id.to_string());
        return refresh(ui, id.as_deref());
    }
    if what == "refresh" { return refresh(ui, selected(ui).as_ref().map(|e| e.id.as_str())); }
    if what == "add" {
        let d = crate::dialog::new(ui).set_title("Add private copies to Library");
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
            let d = crate::dialog::new(ui).set_title("Choose the new file version");
            crate::dialog::pick_file(ui, d, move |ui, path| {
                match library().update(&entry.id, &path) { Ok(()) => { refresh(ui, Some(&entry.id)); status(ui, "New version kept"); }, Err(e) => status(ui, e.to_string()) }
            });
        }
        "restore" => match library().restore(&entry.id, index as u32) {
            Ok(()) => { refresh(ui, Some(&entry.id)); status(ui, "Restored as a new version; all later versions kept"); }, Err(e) => status(ui, e.to_string()),
        },
        "delete" => match library().delete(&entry.id) { Ok(()) => { refresh(ui, None); status(ui, "File and its stored versions deleted. The original is untouched."); }, Err(e) => status(ui, e.to_string()) },
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
