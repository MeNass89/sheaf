//! Acrobat's Output Preview from immutable working bytes, with cancellation on close/replacement.
use super::*;
use sheaf_app::{InkPlate, PrintPreviewBridge};
use std::sync::atomic::{AtomicU64, Ordering};

static GENERATION: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The ink plates on show, kept to compose the ticked ones again when a box is ticked or cleared.
    static PLATES: std::cell::RefCell<Vec<sheaf_core::separations::Plate>> = const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    /// The preset Print Production ▸ Preview traps asked for (Output Preview opened with mode 2).
    static TRAP: std::cell::RefCell<sheaf_core::trap::TrapPreset> = std::cell::RefCell::new(sheaf_core::trap::TrapPreset::default());
}

pub(super) fn set_trap(preset: sheaf_core::trap::TrapPreset) {
    TRAP.with(|t| *t.borrow_mut() = preset);
}

/// The composite of the ticked plates set on the bridge (Acrobat's Output Preview with several inks).
fn show_composite(bridge: &PrintPreviewBridge) {
    let shown: Vec<bool> = (0..bridge.get_plates().row_count()).filter_map(|i| bridge.get_plates().row_data(i).map(|p| p.shown)).collect();
    let image = PLATES.with(|p| sheaf_core::separations::composite(&p.borrow(), &shown));
    if let Some((w, h, rgba)) = image {
        bridge.set_composite(slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&rgba, w, h)));
    }
}

pub fn install(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<PrintPreviewBridge>().on_open(move |id, mode| {
        let Some(ui) = weak.upgrade() else { return };
        let snapshot = update(id, |r, v| {
            if r.edits_in_flight > 0 || r.structure_pending || r.draft.is_some() || r.preview.is_some() || r.live_in_flight || r.live_pending.is_some() { return None; }
            r.prepress.open = false;
            r.prepress.production_open = false;
            Some((r.snap.bytes.clone(), r.snap.password.clone(), v.current()))
        }).flatten();
        let Some((bytes, password, page)) = snapshot else {
            crate::flow::toast(&ui, &sheaf_app::i18n::tr("Wait for the document's edits to finish before previewing ink plates"));
            return;
        };
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        let bridge = ui.global::<PrintPreviewBridge>();
        let trap = TRAP.with(|t| t.borrow().clone());
        let tr = sheaf_app::i18n::tr;
        bridge.set_title(match mode { 1 => tr("Flattener preview"), 2 => sheaf_app::i18n::trf("Traps ({})", &[&trap.name]), _ => tr("Output preview") }.into());
        bridge.set_detail(match mode {
            1 => tr("Compare the captured page with PDF 1.3.\nThe working document stays unchanged.\nTransparent artwork may become images."),
            2 => tr("The plates with their traps, at their true width.\nLighter inks spread under darker ones.\nThe working document stays unchanged."),
            _ => tr("Black shows ink; white shows no ink.\nProcess and spot plates of the captured page.\nThis is not a calibrated press soft proof."),
        }.into());
        bridge.set_tab(id); bridge.set_page(page as i32 + 1); bridge.set_visible(true); bridge.set_busy(true);
        bridge.set_error("".into()); bridge.set_plates(ModelRc::default()); bridge.set_inks(mode != 1); bridge.set_selected(if mode == 1 { 0 } else { -1 });
        let weak = ui.as_weak();
        sheaf_app::pool::probes().submit(move || {
            let mut active = || GENERATION.load(Ordering::Relaxed) == generation;
            let plates = match mode {
                1 => sheaf_core::flattener_preview::preview(&bytes, password.as_deref(), page, &mut active),
                2 => sheaf_core::trap::preview(&bytes, password.as_deref(), page, &trap, &mut active),
                _ => sheaf_core::separations::preview(&bytes, password.as_deref(), page, &mut active),
            };
            let _ = weak.upgrade_in_event_loop(move |ui| {
                if GENERATION.load(Ordering::Relaxed) != generation { return; }
                let bridge = ui.global::<PrintPreviewBridge>();
                bridge.set_busy(false);
                if !with(|a| a.tab(id).is_some()) { bridge.set_visible(false); bridge.set_plates(ModelRc::default()); return; }
                match plates {
                    Ok(plates) => {
                        bridge.set_plates(ModelRc::new(VecModel::from(plates.iter().map(|p| InkPlate {
                            name: p.name.as_str().into(), image: slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&p.rgba, p.width, p.height)), shown: true,
                        }).collect::<Vec<_>>())));
                        PLATES.with(|s| *s.borrow_mut() = plates);
                        if mode != 1 { show_composite(&bridge); }
                    }
                    Err(error) => bridge.set_error(error.to_string().into()),
                }
            });
        });
    });
    let weak = ui.as_weak();
    ui.global::<PrintPreviewBridge>().on_toggle(move |i| {
        let Some(ui) = weak.upgrade() else { return };
        let bridge = ui.global::<PrintPreviewBridge>();
        let plates = bridge.get_plates();
        if let Some(mut p) = plates.row_data(i as usize) {
            p.shown = !p.shown;
            plates.set_row_data(i as usize, p);
            bridge.set_selected(-1);
            show_composite(&bridge);
        }
    });
    let weak = ui.as_weak();
    ui.global::<PrintPreviewBridge>().on_close(move || {
        GENERATION.fetch_add(1, Ordering::Relaxed);
        let Some(ui) = weak.upgrade() else { return };
        let bridge = ui.global::<PrintPreviewBridge>();
        bridge.set_visible(false); bridge.set_busy(false); bridge.set_plates(ModelRc::default());
        PLATES.with(|s| s.borrow_mut().clear());
    });
}
