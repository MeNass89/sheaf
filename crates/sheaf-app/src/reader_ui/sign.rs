//! The signature tool: the kept signature, placing it, the pad.

use super::*;
use sheaf_core::script::ScriptFont;

/// The signature kept in the preferences.
pub(super) fn saved_signature() -> Option<sheaf_core::sign::Graphic> {
    let p = crate::home::prefs();
    if !p.signature_strokes.is_empty() {
        return Some(sheaf_core::sign::Graphic::Drawn(p.signature_strokes));
    }
    let font = ScriptFont::from_index(p.signature_font as usize);
    p.signature_name.filter(|n| !n.trim().is_empty()).map(|n| sheaf_core::sign::Graphic::Typed(n, font))
}

/// The initials kept in the preferences, as [`saved_signature`].
pub(super) fn saved_initials() -> Option<sheaf_core::sign::Graphic> {
    let (strokes, name, _, font) = crate::home::prefs().initials?;
    if !strokes.is_empty() {
        return Some(sheaf_core::sign::Graphic::Drawn(strokes));
    }
    name.filter(|n| !n.trim().is_empty()).map(|n| sheaf_core::sign::Graphic::Typed(n, ScriptFont::from_index(font as usize)))
}

/// What the Signature tool places: the initials when Initials is on, else the signature.
pub(super) fn saved_mark(initials: bool) -> Option<sheaf_core::sign::Graphic> {
    if initials { saved_initials() } else { saved_signature() }
}

/// Places the kept signature (or initials) where the Signature tool was clicked.
pub(super) fn place_signature(ui: &AppWindow, tab_id: i32) {
    let initials = update(tab_id, |r, _| r.initials).unwrap_or(false);
    if let Some(graphic) = saved_mark(initials) {
        place_graphic(ui, tab_id, &graphic);
    }
}

/// Places `graphic` where the Signature tool was clicked, 150 pt wide as PDF Expert's default
/// (initials 60).
fn place_graphic(ui: &AppWindow, tab_id: i32, graphic: &sheaf_core::sign::Graphic) {
    let made = update(tab_id, |r, v| {
        let (page, at) = r.sign_at.take()?;
        let b = &v.viewer.boxes[page];
        let (w, h) = b.view_size();
        let info = render::PageInfo { width: w as f32, height: h as f32, label: String::new(), crop: b.crop.map(|c| c as f32), rotation: b.rotation };
        let width: f64 = if r.initials { 60.0 } else { 150.0 };
        // Every page: the same place on each, one undo step (initials down every page of a contract).
        if r.sign_every_page {
            let edits = (0..v.viewer.page_count())
                .map(|p| {
                    let b = &v.viewer.boxes[p];
                    let (pw, ph) = b.view_size();
                    let info = render::PageInfo { width: pw as f32, height: ph as f32, label: String::new(), crop: b.crop.map(|c| c as f32), rotation: b.rotation };
                    sheaf_core::sign::signature_edit(&info, p, [at.0.min(pw - 1.0), at.1.min(ph - 1.0)], width.min(pw * 0.6), graphic).map(with_pen)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|edits| Edit::Batch { label: if r.initials { "Initials on every page".into() } else { "Signature on every page".into() }, edits });
            return Some((usize::MAX, edits));
        }
        let edit = sheaf_core::sign::signature_edit(&info, page, [at.0, at.1], width.min(w * 0.6), graphic).map(with_pen);
        Some((page, edit))
    })
    .flatten();
    match made {
        // Every page changes.
        Some((usize::MAX, Ok(e))) => apply(ui, tab_id, e, &[], None),
        Some((page, Ok(e))) => apply(ui, tab_id, e, &[page], None),
        Some((_, Err(e))) => crate::flow::toast(ui, &e.to_string()),
        None => {}
    }
}

/// A drawn or typed signature in the pen chosen on the pad (colour and width; an image keeps its own).
fn with_pen(e: Edit) -> Edit {
    let (ink, width) = crate::home::prefs().signature_pen;
    match e {
        Edit::AddAnnotation(mut a) => {
            a.style.color = reader::SIGNATURE_INKS[(ink as usize).min(2)];
            if matches!(a.shape, Shape::Signature { .. }) {
                a.style.width = reader::SIGNATURE_WIDTHS[(width as usize).min(2)];
            }
            Edit::AddAnnotation(a)
        }
        other => other,
    }
}

/// The signature pad's answer: strokes drawn (pad commands and size) or a name typed; empty
/// cancels. It is placed where the tool was clicked, and kept for next time unless `keep` is
/// off (used once: nothing of it is stored).
/// `pen`: ink and width chosen on the pad (see `reader::SIGNATURE_INKS`, `SIGNATURE_WIDTHS`).
#[allow(clippy::too_many_arguments)]
pub fn signature_made(ui: &AppWindow, tab_id: i32, commands: &str, size: (f32, f32), name: &str, pen: (i32, i32), font: i32, keep: bool) {
    let (w, h) = size;
    crate::home::set_signature_pen((pen.0.clamp(0, 2) as u8, pen.1.clamp(0, 2) as u8));
    let strokes = crate::sign_ui::pad_strokes(commands, w, h);
    let name = name.trim().to_string();
    update(tab_id, |r, _| r.sign_open = false);
    if strokes.is_empty() && name.is_empty() {
        update(tab_id, |r, _| r.sign_at = None);
        return;
    }
    let font = ScriptFont::from_index(font.max(0) as usize);
    if !keep {
        let graphic = if strokes.is_empty() { sheaf_core::sign::Graphic::Typed(name, font) } else { sheaf_core::sign::Graphic::Drawn(strokes) };
        return place_graphic(ui, tab_id, &graphic);
    }
    if update(tab_id, |r, _| r.initials).unwrap_or(false) {
        crate::home::set_initials(strokes, (!name.is_empty()).then_some(name), font.index() as u8);
    } else {
        crate::home::set_signature(strokes, (!name.is_empty()).then_some(name), font.index() as u8);
    }
    place_signature(ui, tab_id);
}

/// Forget the kept signature and draw a new one at the next click.
pub fn signature_reset(tab_id: i32) {
    crate::home::set_signature(Vec::new(), None, 0);
    update(tab_id, |r, _| r.sign_open = r.sign_at.is_some());
}

/// Signature tool's options bar: kept signature `i` chosen (`remove`: forgotten).
pub fn signature_pick(tab_id: i32, i: i32, remove: bool) {
    crate::home::pick_signature(i.max(0) as usize, remove);
    update(tab_id, |r, _| r.version += 1);
}

thread_local! {
    /// Thumbnails of the kept signatures, built again only when the list changes.
    static THUMBS: std::cell::RefCell<(Vec<sheaf_app::prefs::KeptSignature>, Vec<slint::Image>)> = Default::default();
}

/// The kept signatures as small pictures (ink on transparent), newest first.
pub(super) fn signature_thumbs() -> Vec<slint::Image> {
    let list = crate::home::signatures();
    THUMBS.with(|t| {
        let mut t = t.borrow_mut();
        if t.0 != list {
            let images = list
                .iter()
                .filter_map(|(strokes, name, pen, font)| {
                    let g = if strokes.is_empty() {
                        sheaf_core::sign::Graphic::Typed(name.clone()?, ScriptFont::from_index(*font as usize))
                    } else {
                        sheaf_core::sign::Graphic::Drawn(strokes.clone())
                    };
                    let mut p = sheaf_app::sign_preview::render(&g, 120, 40)?;
                    // In its own ink, as it will be placed.
                    let ink = reader::SIGNATURE_INKS[(pen.0 as usize).min(2)].map(|c| (c * 255.0) as u8);
                    for px in p.rgba.chunks_mut(4).filter(|px| px[3] > 0) {
                        px[..3].copy_from_slice(&ink);
                    }
                    Some(slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&p.rgba, p.w, p.h)))
                })
                .collect();
            *t = (list, images);
        }
        t.1.clone()
    })
}

/// The pad's live preview of a typed signature: the name in handwriting font `font` (ink on
/// transparent).
pub fn signature_preview(name: slint::SharedString, font: i32) -> slint::Image {
    let g = sheaf_core::sign::Graphic::Typed(name.trim().to_string(), ScriptFont::from_index(font.max(0) as usize));
    sheaf_app::sign_preview::render(&g, 480, 96)
        .map(|p| slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&p.rgba, p.w, p.h)))
        .unwrap_or_default()
}
