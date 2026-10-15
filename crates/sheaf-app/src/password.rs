//! The password dialog: files whose probe reported PasswordRequired / WrongPassword queue up
//! here; the head of the queue is on screen. The entered password goes on the file's `Input`.

use sheaf_app::files::Kind;
use sheaf_app::AppWindow;

use crate::state::with;

pub fn ask(ui: &AppWindow, tab_id: i32, uid: i32, wrong: bool) {
    let head = with(|a| {
        if !a.pw_queue.contains(&(tab_id, uid)) {
            a.pw_queue.push_back((tab_id, uid));
        }
        a.pw_queue.front().copied()
    });
    if head == Some((tab_id, uid)) && wrong {
        ui.set_pw_wrong(true);
        ui.set_pw_attempt(ui.get_pw_attempt() + 1);
    }
    show_head(ui);
}

pub fn show_head(ui: &AppWindow) {
    let head = with(|a| {
        let (tab_id, uid) = *a.pw_queue.front()?;
        let f = a.tab(tab_id)?.files.iter().find(|f| f.uid == uid)?;
        Some(f.input.path.file_name().unwrap_or_default().to_string_lossy().into_owned())
    });
    match head {
        Some(name) => {
            if ui.get_pw_file() != name.as_str() || !ui.get_pw_visible() {
                ui.set_pw_wrong(false);
            }
            ui.set_pw_file(name.into());
            ui.set_pw_visible(true);
        }
        None => {
            // Head file vanished (tab closed): drop it and try the next one.
            let more = with(|a| {
                a.pw_queue.pop_front();
                !a.pw_queue.is_empty()
            });
            if more {
                show_head(ui);
            } else {
                ui.set_pw_visible(false);
                ui.set_pw_wrong(false);
            }
        }
    }
}

/// A file opened: if it was the one the dialog asked about, move on.
pub fn done(ui: &AppWindow, tab_id: i32, uid: i32) {
    let was_head = with(|a| {
        let head = a.pw_queue.front() == Some(&(tab_id, uid));
        a.pw_queue.retain(|e| *e != (tab_id, uid));
        head
    });
    if was_head {
        ui.set_pw_wrong(false);
        if with(|a| a.pw_queue.is_empty()) {
            ui.set_pw_visible(false);
        } else {
            show_head(ui);
        }
    }
}

pub fn entered(ui: &AppWindow, pw: String) {
    let Some((tab_id, uid, input)) = with(|a| {
        let (tab_id, uid) = *a.pw_queue.front()?;
        let f = a.tab(tab_id)?.files.iter_mut().find(|f| f.uid == uid)?;
        f.input.password = Some(pw);
        Some((tab_id, uid, f.input.clone()))
    }) else {
        return show_head(ui);
    };
    crate::flow::probe(ui, tab_id, uid, input, Kind::Pdf);
}

pub fn skipped(ui: &AppWindow) {
    if let Some((tab_id, uid)) = with(|a| a.pw_queue.pop_front()) {
        crate::flow::remove_file(ui, tab_id, uid);
    }
    ui.set_pw_wrong(false);
    show_head(ui);
}
