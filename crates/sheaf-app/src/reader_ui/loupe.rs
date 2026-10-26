//! More ▸ Loupe (Acrobat's Loupe tool): a panel in the view's corner shows the part of the page
//! under the pointer three times larger, following it. One render at a time: a move while one is
//! out keeps only the latest area, asked for when the render comes back.

use super::*;

/// The area shown, in points around the pointer: three times larger, it fills the 360 × 240 panel.
const AREA: (f64, f64) = (120.0, 80.0);

pub fn toggle_loupe(ui: &AppWindow, tab_id: i32) {
    let on = update(tab_id, |r, _| {
        r.loupe = !r.loupe;
        r.loupe_image = None;
        r.loupe_next = None;
        r.loupe
    });
    if on == Some(true) {
        crate::flow::toast(ui, "Loupe: point at the page to magnify it");
    }
}

/// The pointer at content point (`x`, `y`): the area under it asked for.
pub fn loupe_at(tab_id: i32, x: f32, y: f32) {
    with(|a| {
        let Some(t) = a.tab(tab_id) else { return };
        let (Some(r), Some(v)) = (t.reader.as_mut(), t.viewer.as_ref()) else { return };
        let Some((page, ux, uy)) = v.viewer.hit(x as f64, y as f64) else { return };
        let rect = [ux - AREA.0 / 2.0, uy - AREA.1 / 2.0, ux + AREA.0 / 2.0, uy + AREA.1 / 2.0];
        if r.loupe_busy {
            r.loupe_next = Some((page, rect));
        } else {
            r.loupe_busy = true;
            r.send(Op::LoupeArea { page, rect });
        }
    });
}

/// A render back: shown, and the latest area asked since then asked for.
pub(super) fn shown(tab_id: i32, made: Result<(u32, u32, Vec<u8>), String>) {
    update(tab_id, |r, _| {
        r.loupe_busy = false;
        if !r.loupe {
            return;
        }
        if let Ok((w, h, rgba)) = made {
            r.loupe_image = Some(slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&rgba, w, h)));
        }
        if let Some((page, rect)) = r.loupe_next.take() {
            r.loupe_busy = true;
            r.send(Op::LoupeArea { page, rect });
        }
    });
}
