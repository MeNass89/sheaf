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
        Some((page, sheaf_core::sign::signature_edit(&info, page, [at.0, at.1], 150.0_f64.min(w * 0.6), &graphic)))
    })
    .flatten();
    match made {
        Some((page, Ok(e))) => apply(ui, tab_id, e, &[page], None),
        Some((_, Err(e))) => crate::flow::toast(ui, &e.to_string()),
        None => {}
    }
}

/// The signature pad's answer: strokes drawn (pad commands and size) or a name typed; empty
/// cancels. The signature is kept and placed where the tool was clicked.
pub fn signature_made(ui: &AppWindow, tab_id: i32, commands: &str, w: f32, h: f32, name: &str) {
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
