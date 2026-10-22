//! Add watermark: the live preview of the first page it marks, redrawn as the options change.
//!
//! The panel calls [`request`] on every change; after a short pause (typing a word asks once)
//! the page is copied once into a one-page PDF kept for the tab, watermarked with the panel's
//! options in a temporary folder and rendered, off the UI thread. A newer request makes an
//! older result stale: it is dropped (the page copy is kept). One copy is made at a time per
//! tab: requests that come while it is being made are drawn when it is ready.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sheaf_app::files::{self, Thumb};
use sheaf_app::jobs::catching;
use sheaf_app::{AppWindow, ToolOptions};
use sheaf_core::pages::{self, Input};
use sheaf_core::watermark::{self, WatermarkOptions};
use slint::ComponentHandle;

use crate::state::{self, refresh, with};

/// Pause after the last change before drawing, so typing does not queue a render per key.
const PAUSE: Duration = Duration::from_millis(300);
/// Width the page is rendered at: about twice the box, sharp on a high-density screen.
const PREVIEW_PX: i32 = 640;

/// The page copied out for a tab: (file uid, page 1-based, the folder holding `page.pdf`; it is
/// deleted with the last reference to it).
type PageCopy = (i32, u32, Arc<tempfile::TempDir>);

#[derive(Default)]
struct Pending {
    timers: HashMap<i32, slint::Timer>,
    /// Bumped by each request: a render whose number is no longer the tab's latest is dropped.
    generation: HashMap<i32, u64>,
    pages: HashMap<i32, PageCopy>,
    /// A page copy being made for the tab, and the options asked for meanwhile.
    copying: HashMap<i32, (i32, u32)>,
    latest: HashMap<i32, ToolOptions>,
}

thread_local! {
    static PENDING: RefCell<Pending> = RefCell::new(Pending::default());
}

/// The panel's options changed (or its file): draw the preview again after [`PAUSE`].
pub fn request(ui: &AppWindow, tab_id: i32, opts: ToolOptions) {
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    let for_timer = opts.clone();
    timer.start(slint::TimerMode::SingleShot, PAUSE, move || {
        if let Some(ui) = weak.upgrade() {
            draw(&ui, tab_id, &for_timer);
        }
    });
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        *p.generation.entry(tab_id).or_default() += 1;
        p.timers.insert(tab_id, timer);
        p.latest.insert(tab_id, opts);
    });
}

/// The tab closed: its timer, and the page copied for it, go.
pub fn forget(tab_id: i32) {
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        p.timers.remove(&tab_id);
        p.generation.remove(&tab_id);
        p.pages.remove(&tab_id);
        p.copying.remove(&tab_id);
        p.latest.remove(&tab_id);
    });
}

/// Shows `preview` (or none) with `note` under it, if the tab still holds file `uid`.
fn show(tab_id: i32, uid: Option<i32>, preview: Option<(Thumb, f32, f32)>, note: &str) {
    with(|a| {
        if let Some(t) = a.tab(tab_id).filter(|t| uid.is_none_or(|u| t.files.first().is_some_and(|f| f.uid == u))) {
            t.preview = preview.map(|(thumb, w, h)| (state::to_image(thumb), w, h));
            t.preview_note = note.into();
        }
        refresh(a, tab_id);
    });
}

fn draw(ui: &AppWindow, tab_id: i32, opts: &ToolOptions) {
    let file = with(|a| a.tab(tab_id).and_then(|t| t.files.first().map(|f| (f.uid, f.input.clone()))));
    let Some((uid, input)) = file else {
        return show(tab_id, None, None, "");
    };
    // The first page the mark goes on; the options of a single file (no batch clamp).
    let page = crate::run::page_number(&opts.from_page).unwrap_or(1);
    let options = crate::run::preview_watermark_options(opts);
    let (generation, copy, busy) = PENDING.with(|p| {
        let p = p.borrow();
        let copy = p.pages.get(&tab_id).filter(|c| c.0 == uid && c.1 == page).map(|c| c.2.clone());
        (p.generation.get(&tab_id).copied().unwrap_or(0), copy, p.copying.get(&tab_id) == Some(&(uid, page)))
    });
    // The copy is on its way: it draws the newest options when it is ready.
    if copy.is_none() && busy {
        return;
    }
    if copy.is_none() {
        PENDING.with(|p| p.borrow_mut().copying.insert(tab_id, (uid, page)));
    }
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let made_copy = copy.is_none();
        let copy = match copy {
            Some(c) => Ok(c),
            None => catching(|| {
                let dir = tempfile::Builder::new().prefix("sheaf-wm-page-").tempdir()?;
                pages::extract(&input, &page.to_string(), &dir.path().join("page.pdf"))?;
                Ok(Arc::new(dir))
            }),
        };
        // Options that cannot make a mark yet (no text, no image) show the page as it is.
        let shown = copy.as_ref().map_err(|e| e.to_string()).and_then(|c| {
            let page_pdf = Input::new(c.path().join("page.pdf"));
            let marked = options.as_ref().map_err(|e| e.to_string()).and_then(|o| marked_page(&page_pdf, o).map_err(|e| e.to_string()));
            match marked {
                Ok(m) => Ok((m, String::new())),
                Err(why) => files::first_page_preview(&page_pdf, PREVIEW_PX).map(|p| (p, why)).map_err(|e| e.to_string()),
            }
        });
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let (latest, again) = PENDING.with(|p| {
                let mut p = p.borrow_mut();
                if made_copy && p.copying.get(&tab_id) == Some(&(uid, page)) {
                    p.copying.remove(&tab_id);
                    // Kept even when this render is stale: the next one needs it.
                    if let Ok(c) = &copy {
                        p.pages.insert(tab_id, (uid, page, c.clone()));
                    }
                }
                (p.generation.get(&tab_id).copied().unwrap_or(0), p.latest.get(&tab_id).cloned())
            });
            if latest != generation {
                // Changes came while the copy was being made: draw the newest now.
                if made_copy && let Some(o) = again {
                    draw(&ui, tab_id, &o);
                }
                return;
            }
            match shown {
                Ok((p, why)) => show(tab_id, Some(uid), Some(p), &note(&why)),
                Err(e) => show(tab_id, Some(uid), None, &note(&e)),
            }
        });
    });
}

/// Why there is no mark on the preview, as the panel says it ("" when nothing is wrong: the
/// text or the image is simply not there yet).
fn note(why: &str) -> String {
    if why.is_empty() || why.contains("text is empty") || why.contains("os error 2") {
        return String::new();
    }
    format!("No preview: {}", why.strip_prefix("unsupported input: ").unwrap_or(why))
}

/// `page_pdf` watermarked with `o` in a temporary folder, rendered.
fn marked_page(page_pdf: &Input, o: &WatermarkOptions) -> sheaf_core::Result<(Thumb, f32, f32)> {
    let dir = tempfile::Builder::new().prefix("sheaf-wm-preview-").tempdir()?;
    let (written, _) = watermark::watermark(page_pdf, o, &dir.path().join("marked.pdf"))?;
    files::first_page_preview(&Input::new(&written), PREVIEW_PX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_say_why_only_when_something_is_wrong() {
        assert_eq!(note(""), "");
        assert_eq!(note("unsupported input: the watermark text is empty"), "");
        assert_eq!(note("unsupported input: font size: \"900\" (use 1 to 500 points)"), "No preview: font size: \"900\" (use 1 to 500 points)");
    }
}
