//! Acrobat's Output Preview from immutable working bytes, with cancellation on close/replacement.
use super::*;
use sheaf_app::{InkPlate, PrintPreviewBridge};
use std::sync::atomic::{AtomicU64, Ordering};

static GENERATION: AtomicU64 = AtomicU64::new(0);

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
            crate::flow::toast(&ui, "Wait for the document's edits to finish before previewing ink plates");
            return;
        };
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        let bridge = ui.global::<PrintPreviewBridge>();
        bridge.set_title(if mode == 1 { "Flattener preview" } else { "Output preview" }.into());
        bridge.set_detail(if mode == 1 { "Compare the captured page with PDF 1.3.\nThe working document stays unchanged.\nTransparent artwork may become images." } else { "Black shows ink; white shows no ink.\nProcess and spot plates of the captured page.\nThis is not a calibrated press soft proof." }.into());
        bridge.set_tab(id); bridge.set_page(page as i32 + 1); bridge.set_visible(true); bridge.set_busy(true);
        bridge.set_error("".into()); bridge.set_plates(ModelRc::default()); bridge.set_selected(0);
        let weak = ui.as_weak();
        sheaf_app::pool::probes().submit(move || {
            let mut active = || GENERATION.load(Ordering::Relaxed) == generation;
            let plates = if mode == 1 { sheaf_core::flattener_preview::preview(&bytes, password.as_deref(), page, &mut active) }
                else { sheaf_core::separations::preview(&bytes, password.as_deref(), page, &mut active) };
            let _ = weak.upgrade_in_event_loop(move |ui| {
                if GENERATION.load(Ordering::Relaxed) != generation { return; }
                let bridge = ui.global::<PrintPreviewBridge>();
                bridge.set_busy(false);
                if !with(|a| a.tab(id).is_some()) { bridge.set_visible(false); bridge.set_plates(ModelRc::default()); return; }
                match plates {
                    Ok(plates) => bridge.set_plates(ModelRc::new(VecModel::from(plates.into_iter().map(|p| InkPlate {
                        name: p.name.into(), image: slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&p.rgba, p.width, p.height)),
                    }).collect::<Vec<_>>()))),
                    Err(error) => bridge.set_error(error.to_string().into()),
                }
            });
        });
    });
    let weak = ui.as_weak();
    ui.global::<PrintPreviewBridge>().on_close(move || {
        GENERATION.fetch_add(1, Ordering::Relaxed);
        let Some(ui) = weak.upgrade() else { return };
        let bridge = ui.global::<PrintPreviewBridge>();
        bridge.set_visible(false); bridge.set_busy(false); bridge.set_plates(ModelRc::default());
    });
}
