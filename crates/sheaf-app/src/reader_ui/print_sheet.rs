//! The print sheet (⌘P): printers, options, sending the job.

use super::*;

/// ⌘P: the print sheet, with the system's printers and its default one chosen.
pub fn print_open(tab_id: i32) {
    let (list, default) = sheaf_app::print::printers();
    update(tab_id, |r, _| {
        r.printer = default.and_then(|d| list.iter().position(|p| *p == d)).unwrap_or(0);
        r.printers = list;
        r.print_open = true;
    });
}

pub fn print_cancel(tab_id: i32) {
    update(tab_id, |r, _| r.print_open = false);
}

/// Prints the document as it is now (unsaved edits included). `pages`: 0 all, 1 the current page,
/// 2 the typed `range`; `paper` 0 the printer's own, else [`sheaf_app::print::PAPERS`]` [paper - 1]`;
/// `orientation` 0 as each page is, 1 portrait, 2 landscape.
#[allow(clippy::too_many_arguments)]
pub fn print(ui: &AppWindow, tab_id: i32, printer: i32, copies: i32, pages: i32, range: &str, fit: bool, two_sided: bool, paper: i32, orientation: i32) {
    let Some(prepared) = update(tab_id, |r, v| {
        if r.in_flight > 0 {
            return Err("Still applying your last change: print again in a moment".to_string());
        }
        // The page shows a paragraph as typed, not applied: it is not what would print.
        if r.preview.is_some() || r.live.is_some() {
            return Err("Finish editing the paragraph first (⌘↩ to apply, Escape to cancel)".to_string());
        }
        // The print service gets the file's bytes: an encrypted file would reach it unreadable.
        if r.snap.password.is_some() {
            return Err("This PDF is password-protected: remove its password (Tools ▸ Unlock) to print it".to_string());
        }
        let count = r.snap.boxes.len();
        let pages = match pages {
            1 => Some((v.current() + 1).to_string()),
            2 => Some(sheaf_app::print::page_range(range, count)?),
            _ => None,
        };
        let printer = r.printers.get(printer.max(0) as usize).cloned().ok_or("No printer is set up")?;
        r.printer = r.printers.iter().position(|p| *p == printer).unwrap_or(0);
        r.print_open = false;
        let media = (paper > 0).then(|| sheaf_app::print::PAPERS.get(paper as usize - 1)).flatten().map(|m| m.to_string());
        let job = sheaf_app::print::Job { printer, copies: copies.clamp(1, 999) as u32, pages, fit, two_sided, media, orientation: orientation.clamp(0, 2) as u8 };
        let title = r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Document".into());
        Ok((job, title, r.snap.bytes.clone()))
    }) else {
        return;
    };
    let (job, title, bytes) = match prepared {
        Ok(p) => p,
        Err(e) => return crate::flow::toast(ui, &e),
    };
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let msg = match sheaf_app::print::submit(&job, &title, &bytes) {
            Ok(_) => format!("Sent to {}", job.printer),
            Err(e) => format!("Not printed: {e}"),
        };
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                crate::flow::toast(&ui, &msg);
            }
        });
    });
}
