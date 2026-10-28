//! More ▸ Loupe (Acrobat's Loupe tool): a panel in the view's corner shows the part of the page
//! under the pointer three times as large as the view shows it, following it. One render at a time: a move while one is
//! out keeps only the latest area, asked for when the render comes back.

use super::*;

/// The panel, in pixels; it shows what is under the pointer three times as large as the page shows
/// it, whatever the zoom (Sol, 26/10: a fixed 120 × 80 pt area went smaller than the page past 300 %).
const PANEL: (f64, f64) = (360.0, 240.0);
const MAGNIFY: f64 = 3.0;

pub fn toggle_loupe(ui: &AppWindow, tab_id: i32) {
    let on = update(tab_id, |r, _| {
        r.loupe = !r.loupe;
        r.loupe_image = None;
        r.loupe_next = None;
        r.loupe
    });
    if on == Some(true) {
        crate::flow::toast(ui, &sheaf_app::i18n::tr("Loupe: point at the page to magnify it"));
    }
}

/// The pointer at content point (`x`, `y`): the area under it asked for.
pub fn loupe_at(tab_id: i32, x: f32, y: f32) {
    with(|a| {
        let Some(t) = a.tab(tab_id) else { return };
        let (Some(r), Some(v)) = (t.reader.as_mut(), t.viewer.as_ref()) else { return };
        let Some((page, ux, uy)) = v.viewer.hit(x as f64, y as f64) else { return };
        // Pixels per point on screen now: the page's width shown over its width in points.
        let shown = v.viewer.page_rect(page)[2];
        let points = v.viewer.boxes.get(page).map_or(1.0, |b| b.view_size().0.max(1.0));
        let k = (shown / points).max(0.01) * MAGNIFY;
        let (hw, hh) = (PANEL.0 / k / 2.0, PANEL.1 / k / 2.0);
        let rect = [ux - hw, uy - hh, ux + hw, uy + hh];
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
            // Rotate View: the area turned as the pages are (Sol, 26/10: it showed the file's way up).
            let (w, h, rgba) = match image::RgbaImage::from_raw(w, h, rgba) {
                Some(img) if r.turn % 4 != 0 => {
                    let t = match r.turn % 4 {
                        1 => image::imageops::rotate90(&img),
                        2 => image::imageops::rotate180(&img),
                        _ => image::imageops::rotate270(&img),
                    };
                    (t.width(), t.height(), t.into_raw())
                }
                Some(img) => (img.width(), img.height(), img.into_raw()),
                None => return,
            };
            r.loupe_image = Some(slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&rgba, w, h)));
        }
        if let Some((page, rect)) = r.loupe_next.take() {
            r.loupe_busy = true;
            r.send(Op::LoupeArea { page, rect });
        }
    });
}
