//! Native file dialogs, always asynchronous and attached to the window.
//!
//! A blocking `rfd::FileDialog` runs the panel's modal loop inside the click handler that opened it.
//! On macOS that nested loop leaves the Save panel's buttons deaf to the mouse: only Enter
//! confirmed (reported on Compress). The async dialog opens as a sheet on our window (macOS),
//! on a helper thread (Linux portal) or as an owned dialog (Windows), after the click returns;
//! the answer comes back on the UI thread through `then`.

use std::future::Future;
use std::path::PathBuf;

use rfd::{AsyncFileDialog, FileHandle};
use slint::ComponentHandle;

use sheaf_app::AppWindow;

/// A dialog builder owned by `ui`'s window.
pub fn new(ui: &AppWindow) -> AsyncFileDialog {
    let d = AsyncFileDialog::new();
    // Sheets on macOS; owner window elsewhere. Without a handle (headless) it floats.
    let w = ui.window().window_handle();
    d.set_parent(&w)
}

/// Debug builds: the answer `SHEAF_TEST_DIALOG` names (a path, or several joined by `|`), read
/// afresh at each dialog, instead of a dialog the XDG portal of a test display cannot show
/// (gui-smoke). Release builds never read it.
fn test_answer() -> Option<Vec<PathBuf>> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let v = std::env::var("SHEAF_TEST_DIALOG").ok().filter(|v| !v.is_empty())?;
    // The file names the answer: the smoke writes it before the click that opens the dialog.
    let v = std::fs::read_to_string(&v).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())?;
    Some(v.split('|').map(PathBuf::from).collect())
}

/// Runs `fut` on the UI thread's executor and hands its answer to `then`, with the window.
fn finish<T: 'static>(ui: &AppWindow, fut: impl Future<Output = T> + 'static, then: impl FnOnce(&AppWindow, T) + 'static) {
    let weak = ui.as_weak();
    let _ = slint::spawn_local(async move {
        let r = fut.await;
        if let Some(ui) = weak.upgrade() {
            then(&ui, r);
        }
    });
}

fn path(h: FileHandle) -> PathBuf {
    h.path().to_path_buf()
}

pub fn pick_file(ui: &AppWindow, d: AsyncFileDialog, then: impl FnOnce(&AppWindow, PathBuf) + 'static) {
    if let Some(mut v) = test_answer() {
        return then(ui, v.remove(0));
    }
    finish(ui, d.pick_file(), move |ui, r| {
        if let Some(h) = r {
            then(ui, path(h));
        }
    });
}

pub fn pick_files(ui: &AppWindow, d: AsyncFileDialog, then: impl FnOnce(&AppWindow, Vec<PathBuf>) + 'static) {
    if let Some(v) = test_answer() {
        return then(ui, v);
    }
    finish(ui, d.pick_files(), move |ui, r| {
        if let Some(v) = r.filter(|v| !v.is_empty()) {
            then(ui, v.into_iter().map(path).collect());
        }
    });
}

pub fn pick_folder(ui: &AppWindow, d: AsyncFileDialog, then: impl FnOnce(&AppWindow, PathBuf) + 'static) {
    if let Some(mut v) = test_answer() {
        return then(ui, v.remove(0));
    }
    finish(ui, d.pick_folder(), move |ui, r| {
        if let Some(h) = r {
            then(ui, path(h));
        }
    });
}

pub fn save_file(ui: &AppWindow, d: AsyncFileDialog, then: impl FnOnce(&AppWindow, PathBuf) + 'static) {
    if let Some(mut v) = test_answer() {
        return then(ui, v.remove(0));
    }
    finish(ui, d.save_file(), move |ui, r| {
        if let Some(h) = r {
            then(ui, path(h));
        }
    });
}
