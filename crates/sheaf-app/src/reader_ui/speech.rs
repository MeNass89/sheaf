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

/// Whether the voice is paused (Pause reading).
static PAUSED: AtomicBool = AtomicBool::new(false);

/// More ▸ More tools ▸ Read aloud (`what` 0: from the page shown to the end; 2 this page only, 3 the
/// selection, Acrobat's Read This Page Only and a selection's Read aloud), and 4 Pause or resume
/// (Acrobat's Pause; Sol, 26/10: none of these). Chosen again while reading, Read aloud stops.
pub fn read_aloud(ui: &AppWindow, tab_id: i32, what: i32) {
    if what == 4 {
        return pause(ui);
    }
    if stop() {
        PAUSED.store(false, Ordering::SeqCst);
        return crate::flow::toast(ui, "Reading stopped");
    }
    let Some((bytes, password, from, count, selection)) = update(tab_id, |r, v| {
        let sel = r.sel.as_ref().and_then(|(p, range)| r.texts.get(p).map(|t| reader::selected_text(t, range.clone()))).unwrap_or_default();
        (r.snap.display.clone(), r.snap.password.clone(), v.current(), v.viewer.page_count(), sel)
    }) else {
        return;
    };
    let speed = crate::home::prefs().reading_speed;
    let voice = crate::home::prefs().reading_voice;
    if what == 3 {
        if selection.trim().is_empty() {
            return crate::flow::toast(ui, "Select some text to read first");
        }
        let go = Arc::new(AtomicBool::new(true));
        if let Some(child) = crate::run::speak(&selection, speed, voice.as_deref()) {
            *SPEAKING.lock().unwrap_or_else(|e| e.into_inner()) = Some((child, go));
        }
        return crate::flow::toast(ui, "Reading the selection");
    }
    let count = if what == 2 { from + 1 } else { count };
    let go = Arc::new(AtomicBool::new(true));
    crate::flow::toast(ui, &if what == 2 { format!("Reading page {}", from + 1) } else { format!("Reading from page {}: More ▸ Read aloud again stops", from + 1) });
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
            let Some(child) = crate::run::speak(&text, speed, voice.as_deref()) else { return };
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

/// Pause reading, or resume it (the voice's process stopped and continued; Windows's voice cannot
/// be paused from outside, it is stopped).
fn pause(ui: &AppWindow) {
    let slot = SPEAKING.lock().unwrap_or_else(|e| e.into_inner());
    let Some((child, _)) = slot.as_ref() else { return crate::flow::toast(ui, "Nothing is being read") };
    let paused = !PAUSED.load(Ordering::SeqCst);
    #[cfg(unix)]
    {
        let sig = if paused { "-STOP" } else { "-CONT" };
        let _ = std::process::Command::new("kill").args([sig, &child.id().to_string()]).status();
        PAUSED.store(paused, Ordering::SeqCst);
        crate::flow::toast(ui, if paused { "Reading paused: Pause reading again resumes" } else { "Reading resumed" });
    }
    #[cfg(not(unix))]
    {
        let _ = (child, paused);
        drop(slot);
        stop();
        crate::flow::toast(ui, "Reading stopped");
    }
}

/// More ▸ Voice: the system's voices, the one chosen kept (Acrobat's Reading ▸ Voice).
pub fn voices_open(tab_id: i32) {
    let list = crate::run::voices();
    update(tab_id, |r, _| {
        r.voices = list;
        r.voices_open = true;
    });
}

/// Voice `k` of the list chosen (−1: the system's default; −2 closes the sheet only).
pub fn voice_pick(ui: &AppWindow, tab_id: i32, k: i32) {
    if k == -2 {
        update(tab_id, |r, _| r.voices_open = false);
        return;
    }
    let name = update(tab_id, |r, _| {
        r.voices_open = false;
        usize::try_from(k).ok().and_then(|k| r.voices.get(k).cloned())
    })
    .flatten();
    crate::home::set_reading_voice(name.clone());
    crate::flow::toast(ui, &format!("Voice: {}", name.as_deref().unwrap_or("the system's")));
}

/// More ▸ Reading speed: the next of normal, slow, fast, said and kept.
pub fn reading_speed(ui: &AppWindow) {
    let s = crate::home::cycle_reading_speed();
    crate::flow::toast(ui, &format!("Reading speed: {}", ["normal", "slow", "fast"][s.min(2) as usize]));
}
