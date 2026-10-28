//! The Layers panel (Acrobat's Layers pane, iLovePDF's Enable all / Disable all / Reset): the
//! document's layers, each shown or hidden on screen. Pages with a layer hidden are drawn from a
//! copy made off the UI thread (`sheaf_core::layers::shown`); the file, its saves and its prints
//! keep the layers as the file sets them.

use super::*;

/// The layers shown now: as the file sets them, or as chosen here.
pub(super) fn shown(r: &ReaderUi) -> Vec<bool> {
    r.snap.layers.iter().map(|l| r.layers_off.as_ref().map_or(l.visible, |off| !off.contains(&l.id))).collect()
}

/// Layer `index` shown or hidden.
pub fn layer_set(ui: &AppWindow, tab_id: i32, index: i32, on: bool) {
    choose(ui, tab_id, |r| {
        let mut shown = shown(r);
        let i = usize::try_from(index).ok()?;
        if let Some(s) = shown.get_mut(i) {
            *s = on;
        }
        // A layer of a radio-button group shown: the others of its group hidden (Sol, 26/10).
        if on && let Some(id) = r.snap.layers.get(i).map(|l| l.id) {
            for g in r.snap.layer_groups.iter().filter(|g| g.contains(&id)) {
                for (k, l) in r.snap.layers.iter().enumerate() {
                    if l.id != id && g.contains(&l.id) {
                        shown[k] = false;
                    }
                }
            }
        }
        Some(shown)
    });
}

/// 0 every layer shown, 1 every one hidden, 2 back to how the file sets them.
pub fn layers_all(ui: &AppWindow, tab_id: i32, how: i32) {
    choose(ui, tab_id, |r| match how {
        0 | 1 => Some(vec![how == 0; r.snap.layers.len()]),
        _ => None,
    });
}

/// The layers shown set by `pick` (None: as the file sets them), and the pages drawn again.
fn choose(ui: &AppWindow, tab_id: i32, pick: impl FnOnce(&ReaderUi) -> Option<Vec<bool>>) {
    update(tab_id, |r, v| {
        r.layers_off = pick(r).map(|shown| r.snap.layers.iter().zip(shown).filter(|(_, on)| !on).map(|(l, _)| l.id).collect());
        // The file's own choice needs no copy.
        if r.layers_off.as_ref().is_some_and(|off| *off == file_off(r)) {
            r.layers_off = None;
        }
        remake(ui, tab_id, r, v);
        // A copy coming has the pages drawn again when it is made.
        if r.layers_off.is_none() {
            v.all_changed();
        }
    });
    crate::viewer_ui::request(ui, tab_id);
}

fn file_off(r: &ReaderUi) -> Vec<(u32, u16)> {
    r.snap.layers.iter().filter(|l| !l.visible).map(|l| l.id).collect()
}

/// The pages' source made again for the working copy (inside [`update`]): the copy itself when no
/// layer is chosen here, else a copy with the hidden layers left out, made off the UI thread (the
/// pages already drawn stay until it comes back, then are drawn again).
pub(super) fn remake(ui: &AppWindow, tab_id: i32, r: &mut ReaderUi, v: &mut ViewerUi) {
    r.layers_made += 1;
    // Layers gone from the document (an edit) are forgotten.
    if let Some(off) = r.layers_off.as_mut() {
        off.retain(|id| r.snap.layers.iter().any(|l| l.id == *id));
    }
    let Some(off) = r.layers_off.clone() else {
        v.source = Some(r.bytes());
        return;
    };
    let (bytes, password, made, weak) = (r.bytes(), r.password(), r.layers_made, ui.as_weak());
    std::thread::spawn(move || {
        let copy = sheaf_core::layers::shown(&bytes, password.as_deref(), &off);
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let ok = update(tab_id, |r, v| {
                // A later choice or edit has its own copy coming.
                if r.layers_made != made {
                    return true;
                }
                match copy {
                    Ok(copy) => {
                        v.source = Some(Arc::new(copy));
                        v.all_changed();
                        true
                    }
                    Err(_) => {
                        r.layers_off = None;
                        v.source = Some(r.bytes());
                        v.all_changed();
                        false
                    }
                }
            });
            if ok == Some(false) {
                crate::flow::toast(&ui, &sheaf_app::i18n::tr("The layers could not be hidden: they are shown as the file sets them"));
            }
            crate::viewer_ui::request(&ui, tab_id);
        });
    });
}
