//! The signature tool: the kept signature, placing it, the pad.

use super::*;

/// The signature kept in the preferences.
pub(super) fn saved_signature() -> Option<sheaf_core::sign::Graphic> {
    let p = crate::home::prefs();
    if !p.signature_strokes.is_empty() {
        return Some(sheaf_core::sign::Graphic::Drawn(p.signature_strokes));
    }
    p.signature_name.filter(|n| !n.trim().is_empty()).map(sheaf_core::sign::Graphic::Typed)
}

/// Places the signature where the Signature tool was clicked, 150 pt wide as PDF Expert's default.
pub(super) fn place_signature(ui: &AppWindow, tab_id: i32) {
    let Some(graphic) = saved_signature() else { return };
    let made = update(tab_id, |r, v| {
        let (page, at) = r.sign_at.take()?;
        let b = &v.viewer.boxes[page];
        let (w, h) = b.view_size();
        let info = render::PageInfo { width: w as f32, height: h as f32, label: String::new(), crop: b.crop.map(|c| c as f32), rotation: b.rotation };
        let edit = sheaf_core::sign::signature_edit(&info, page, [at.0, at.1], 150.0_f64.min(w * 0.6), &graphic).map(with_pen);
        Some((page, edit))
    })
    .flatten();
    match made {
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
/// cancels. The signature is kept and placed where the tool was clicked.
/// `pen`: ink and width chosen on the pad (see `reader::SIGNATURE_INKS`, `SIGNATURE_WIDTHS`).
pub fn signature_made(ui: &AppWindow, tab_id: i32, commands: &str, size: (f32, f32), name: &str, pen: (i32, i32)) {
    let (w, h) = size;
    crate::home::set_signature_pen((pen.0.clamp(0, 2) as u8, pen.1.clamp(0, 2) as u8));
    let strokes = crate::sign_ui::pad_strokes(commands, w, h);
    let name = name.trim().to_string();
    update(tab_id, |r, _| r.sign_open = false);
    if strokes.is_empty() && name.is_empty() {
        update(tab_id, |r, _| r.sign_at = None);
        return;
    }
    crate::home::set_signature(strokes, (!name.is_empty()).then_some(name));
    place_signature(ui, tab_id);
}

/// Forget the kept signature and draw a new one at the next click.
pub fn signature_reset(tab_id: i32) {
    crate::home::set_signature(Vec::new(), None);
    update(tab_id, |r, _| r.sign_open = r.sign_at.is_some());
}
