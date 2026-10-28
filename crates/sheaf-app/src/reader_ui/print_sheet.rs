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

/// The print sheet's preview drawn again for `layout`, `gray` and `comments` (off the UI thread).
pub fn print_preview(ui: &AppWindow, tab_id: i32, layout: i32, gray: bool, comments: bool) {
    let Some((bytes, password, seq)) = update(tab_id, |r, _| {
        r.print_seq += 1;
        (r.snap.bytes.clone(), r.snap.password.clone(), r.print_seq)
    }) else {
        return;
    };
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let bytes = match password {
            Some(pw) => sheaf_app::print::unlocked(&bytes, &pw),
            None => Ok(bytes),
        };
        let made = bytes.and_then(|b| sheaf_app::print::preview(&b, layout, gray, comments));
        let _ = weak.upgrade_in_event_loop(move |_| {
            update(tab_id, |r, _| {
                if r.print_seq == seq {
                    r.print_preview = made.ok().map(|(w, h, px)| slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&px, w, h)));
                }
            });
        });
    });
}

pub fn print_cancel(tab_id: i32) {
    update(tab_id, |r, _| r.print_open = false);
}

/// Prints the document as it is now (unsaved edits included). `pages`: 0 all, 1 the current page,
/// 2 the typed `range`; `paper` 0 the printer's own, else [`sheaf_app::print::PAPERS`]` [paper - 1]`;
/// `orientation` 0 as each page is, 1 portrait, 2 landscape; `layout` 0–5 1, 2, 4, 6, 9 or 16 pages to
/// a sheet, 6 a booklet; `gray` in shades of grey; `comments` false leaves comments out.
#[allow(clippy::too_many_arguments)]
pub fn print(ui: &AppWindow, tab_id: i32, printer: i32, copies: i32, pages: i32, range: &str, fit: bool, two_sided: bool, paper: i32, orientation: i32, layout: i32, gray: bool, comments: bool) {
    let Some(prepared) = update(tab_id, |r, v| {
        if r.in_flight > 0 {
            return Err("Still applying your last change: print again in a moment".to_string());
        }
        // The page shows a paragraph as typed, not applied: it is not what would print.
        if r.preview.is_some() || r.live.is_some() {
            return Err("Finish editing the paragraph first (⌘↩ to apply, Escape to cancel)".to_string());
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
        let booklet = layout == 6;
        let per_sheet = [1, 2, 4, 6, 9, 16][layout.clamp(0, 5) as usize];
        // A booklet's sheets are numbered anew: a range picks the document's pages first.
        if booklet && pages.is_some() {
            return Err("A booklet prints every page: choose All".to_string());
        }
        let job = sheaf_app::print::Job { printer, copies: copies.clamp(1, 999) as u32, pages, fit, two_sided, media, orientation: orientation.clamp(0, 2) as u8, per_sheet, booklet, gray, quality: crate::home::prefs().settings.print.clamped().quality };
        let title = r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Document".into());
        Ok((job, title, r.snap.bytes.clone(), r.snap.password.clone()))
    }) else {
        return;
    };
    let (job, title, bytes, password) = match prepared {
        Ok(p) => p,
        Err(e) => return crate::flow::toast(ui, &e),
    };
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        // The print service gets bytes it can read: an encrypted file's are unprotected first.
        let bytes = match password {
            Some(pw) => sheaf_app::print::unlocked(&bytes, &pw),
            None => Ok(bytes),
        };
        let bytes = bytes
            .and_then(|b| if comments { Ok(b) } else { sheaf_app::print::without_comments(&b).map(Arc::new) })
            .and_then(|b| if job.booklet { sheaf_app::print::booklet(&b).map(Arc::new) } else { Ok(b) });
        let msg = match bytes.and_then(|b| sheaf_app::print::submit(&job, &title, &b)) {
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
