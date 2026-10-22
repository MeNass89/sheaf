//! Add watermark: the live preview of the first page it marks, redrawn as the options change.
//!
//! The panel calls [`request`] on every change; after a short pause (typing a word asks once)
//! the page is copied once into a one-page PDF kept for the tab, watermarked with the panel's
//! options into a temporary file and rendered, off the UI thread. A newer request makes an
//! older result stale: it is dropped.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sheaf_app::files;
use sheaf_app::jobs::catching;
use sheaf_app::{AppWindow, ToolOptions};
use sheaf_core::pages::{self, Input};
use sheaf_core::watermark;
use slint::ComponentHandle;

use crate::state::{self, refresh, with};

/// Pause after the last change before drawing, so typing does not queue a render per key.
const PAUSE: Duration = Duration::from_millis(300);
/// Width the page is rendered at, as Crop's preview.
const PREVIEW_PX: i32 = 300;

/// The page copied out for a tab: (file uid, page 1-based, the one-page PDF; deleted on drop).
type PageCopy = (i32, u32, Arc<tempfile::TempPath>);

#[derive(Default)]
struct Pending {
    timers: HashMap<i32, slint::Timer>,
    /// Bumped by each request: a render whose number is no longer the tab's latest is dropped.
    generation: HashMap<i32, u64>,
    pages: HashMap<i32, PageCopy>,
}

thread_local! {
    static PENDING: RefCell<Pending> = RefCell::new(Pending::default());
}

/// The panel's options changed (or its file): draw the preview again after [`PAUSE`].
pub fn request(ui: &AppWindow, tab_id: i32, opts: ToolOptions) {
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::SingleShot, PAUSE, move || {
        if let Some(ui) = weak.upgrade() {
            draw(&ui, tab_id, &opts);
        }
    });
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        *p.generation.entry(tab_id).or_default() += 1;
        p.timers.insert(tab_id, timer);
    });
}

/// The tab closed: its timer, and the page copied for it, go.
pub fn forget(tab_id: i32) {
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        p.timers.remove(&tab_id);
        p.generation.remove(&tab_id);
        p.pages.remove(&tab_id);
    });
}

/// `page_pdf` watermarked with `o` in a temporary file, rendered.
fn marked_page(page_pdf: &Input, o: &watermark::WatermarkOptions) -> sheaf_core::Result<(files::Thumb, f32, f32)> {
    let out = tempfile::Builder::new().prefix("sheaf-wm-preview-").suffix(".pdf").tempfile()?.into_temp_path();
    let (written, _) = watermark::watermark(page_pdf, o, &out)?;
    let r = files::first_page_preview(&Input::new(&written), PREVIEW_PX);
    // `watermark` never overwrites: it wrote beside the empty temporary file.
    if written != out.to_path_buf() {
        let _ = std::fs::remove_file(&written);
    }
    r
}

fn draw(ui: &AppWindow, tab_id: i32, opts: &ToolOptions) {
    let file = with(|a| a.tab(tab_id).and_then(|t| t.files.first().map(|f| (f.uid, f.input.clone()))));
    let Some((uid, input)) = file else {
        with(|a| {
            if let Some(t) = a.tab(tab_id) {
                t.preview = None;
            }
            refresh(a, tab_id);
        });
        return;
    };
    // The first page the mark goes on; the options of a single file (no batch clamp).
    let page = crate::run::page_number(&opts.from_page).unwrap_or(1);
    let options = crate::run::preview_watermark_options(opts);
    let (generation, copy) = PENDING.with(|p| {
        let p = p.borrow();
        let copy = p.pages.get(&tab_id).filter(|c| c.0 == uid && c.1 == page).map(|c| c.2.clone());
        (p.generation.get(&tab_id).copied().unwrap_or(0), copy)
    });
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let made = catching(|| {
            let copy = match copy {
                Some(c) => c,
                None => {
                    let tmp = tempfile::Builder::new().prefix("sheaf-wm-page-").suffix(".pdf").tempfile()?.into_temp_path();
                    let written = pages::extract(&input, &page.to_string(), &tmp)?;
                    // `extract` never overwrites: it may have written beside the empty file.
                    if written != tmp.to_path_buf() {
                        std::fs::rename(&written, &tmp)?;
                    }
                    Arc::new(tmp)
                }
            };
            let page_pdf = Input::new(copy.to_path_buf());
            // Options the mark cannot be made with yet (no text, no image): the page as it is.
            let marked = options.as_ref().ok().and_then(|o| marked_page(&page_pdf, o).ok());
            let shown = match marked {
                Some(m) => m,
                None => files::first_page_preview(&page_pdf, PREVIEW_PX)?,
            };
            Ok((copy, shown))
        });
        let _ = weak.upgrade_in_event_loop(move |_| {
            let latest = PENDING.with(|p| p.borrow().generation.get(&tab_id).copied().unwrap_or(0));
            if latest != generation {
                return;
            }
            let Ok((copy, (thumb, w, h))) = made else { return };
            PENDING.with(|p| p.borrow_mut().pages.insert(tab_id, (uid, page, copy)));
            with(|a| {
                if let Some(t) = a.tab(tab_id).filter(|t| t.files.first().is_some_and(|f| f.uid == uid)) {
                    t.preview = Some((state::to_image(thumb), w, h));
                }
                refresh(a, tab_id);
            });
        });
    });
}
