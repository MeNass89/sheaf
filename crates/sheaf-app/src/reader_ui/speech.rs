//! More ▸ Read aloud (Acrobat's View ▸ Read Out Loud, Preview's Speech): the pages from the one
//! shown to the end, read page by page by the system's voice; chosen again, it stops. More ▸
//! Reading speed: normal, slow or fast, kept.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// The voice speaking now, and the flag that stops the pages after it.
static SPEAKING: Mutex<Option<(std::process::Child, Arc<AtomicBool>)>> = Mutex::new(None);

/// Stops reading; true when something was being read.
fn stop() -> bool {
    let Some((mut child, go)) = SPEAKING.lock().ok().and_then(|mut s| s.take()) else { return false };
    go.store(false, Ordering::SeqCst);
    let _ = child.kill();
    let _ = child.wait();
    true
}

pub fn read_aloud(ui: &AppWindow, tab_id: i32) {
    if stop() {
        return crate::flow::toast(ui, "Reading stopped");
    }
    let Some((bytes, password, from, count)) = update(tab_id, |r, v| (r.snap.display.clone(), r.snap.password.clone(), v.current(), v.viewer.page_count())) else { return };
    let speed = crate::home::prefs().reading_speed;
    let go = Arc::new(AtomicBool::new(true));
    crate::flow::toast(ui, &format!("Reading from page {}: More ▸ Read aloud again stops", from + 1));
    std::thread::spawn(move || {
        let config = render::RenderConfig { password: password.as_deref().map(Arc::from), ..Default::default() };
        let mut pages = render::PageRenderer::new(bytes, config);
        for page in from..count {
            if !go.load(Ordering::SeqCst) {
                return;
            }
            let res = pages.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
            let text = res.text.map(|t| t.plain_text()).unwrap_or_default();
            if text.trim().is_empty() {
                continue;
            }
            let Some(child) = crate::run::speak(&text, speed) else { return };
            let mut slot = SPEAKING.lock().unwrap_or_else(|e| e.into_inner());
            if !go.load(Ordering::SeqCst) {
                return;
            }
            *slot = Some((child, go.clone()));
            drop(slot);
            // Until this page is read (or stopped: the slot emptied).
            loop {
                std::thread::sleep(std::time::Duration::from_millis(200));
                let mut slot = SPEAKING.lock().unwrap_or_else(|e| e.into_inner());
                let read = match slot.as_mut() {
                    Some((c, _)) => c.try_wait().ok().flatten().is_some(),
                    None => return,
                };
                if read {
                    slot.take();
                    break;
                }
            }
        }
    });
}

/// More ▸ Reading speed: the next of normal, slow, fast, said and kept.
pub fn reading_speed(ui: &AppWindow) {
    let s = crate::home::cycle_reading_speed();
    crate::flow::toast(ui, &format!("Reading speed: {}", ["normal", "slow", "fast"][s.min(2) as usize]));
}
