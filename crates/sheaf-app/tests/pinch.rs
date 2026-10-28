//! A pinch on the reader's pages, sent to its gesture handler through Slint's testing backend
//! (C-reader-010: Xvfb has no gesture events, so the GUI smoke can only send ⌘ + wheel). The
//! reader's screen is the app's own; what it asks the viewer for is recorded.

use std::{cell::RefCell, rc::Rc};

use i_slint_core::input::{BackendMouseEvent, TouchPhase};
use sheaf_app::{AppWindow, ReaderBridge, ToolTab, ViewerBridge};
use slint::{ComponentHandle, ModelRc, VecModel};

/// What the pages asked for: zoom-at (tab, percent, x, y), zoom-by (tab, factor, x, y), split-focus.
#[derive(Debug, Clone, PartialEq)]
enum Asked {
    At(i32, f32, f32, f32),
    By(i32, f32, f32, f32),
    Focus(i32),
}

/// The app's window, 1280 × 800, showing reader tab 3 at 100 %; split side by side when `other`
/// (the other pane, on the right, showing tab 5's document).
fn reader(other: bool) -> (AppWindow, Rc<RefCell<Vec<Asked>>>) {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1280.0, 800.0));
    let mut t = ToolTab { is_reader: true, id: 3, viewer_zoom: 100, viewer_width: 1000.0, viewer_height: 3000.0, split_other: -1, ..Default::default() };
    if other {
        (t.split_mode, t.split_half, t.split_title, t.split_other, t.split_frac) = (1, 0, "b.pdf".into(), 5, 0.5);
        (t.split_width, t.split_height) = (600.0, 3000.0);
    }
    ui.set_tabs(ModelRc::new(VecModel::from(vec![t])));
    ui.set_active_tab(0);
    ui.show().unwrap();
    let asked = Rc::new(RefCell::new(Vec::new()));
    let vb = ui.global::<ViewerBridge>();
    let a = asked.clone();
    vb.on_zoom_at(move |id, pct, x, y, sx| {
        a.borrow_mut().push(Asked::At(id, pct, x, y));
        sx
    });
    let a = asked.clone();
    vb.on_zoom_by(move |id, f, x, y, sx| {
        a.borrow_mut().push(Asked::By(id, f, x, y));
        sx
    });
    let a = asked.clone();
    ui.global::<ReaderBridge>().on_split_focus(move |id| a.borrow_mut().push(Asked::Focus(id)));
    (ui, asked)
}

/// A trackpad pinch event as the winit backend sends it (`delta`: the scale's change, the handler
/// multiplying its scale by 1 + delta).
fn pinch_event(ui: &AppWindow, delta: f32, x: f32, y: f32, phase: TouchPhase) {
    let position = i_slint_core::lengths::logical_point_from_api(slint::LogicalPosition::new(x, y));
    ui.window().dispatch_event(slint::platform::WindowEvent::internal(BackendMouseEvent::PinchGesture { position, delta, phase }));
}

/// A pinch at window point (x, y): fingers apart by 25 %, then 50 % in all.
fn pinch(ui: &AppWindow, x: f32, y: f32) {
    pinch_event(ui, 0.0, x, y, TouchPhase::Started);
    pinch_event(ui, 0.25, x, y, TouchPhase::Moved);
    pinch_event(ui, 0.2, x, y, TouchPhase::Moved);
    pinch_event(ui, 0.0, x, y, TouchPhase::Ended);
}

/// A pinch over the pages zooms them with the fingers' scale (100 % → 125 % → 150 %), around the
/// fingers: the point asked for moves with them, in the pages' own coordinates.
#[test]
fn a_pinch_zooms_the_pages_around_the_fingers() {
    let (ui, asked) = reader(false);
    pinch(&ui, 700.0, 400.0);
    pinch(&ui, 900.0, 300.0);
    let got = asked.borrow().clone();
    let at: Vec<_> = got.iter().filter_map(|a| if let Asked::At(id, p, x, y) = a { Some((*id, *p, *x, *y)) } else { None }).collect();
    assert!(at.len() >= 4, "{got:?}");
    let (first, last) = (at[0], at[at.len() - 1]);
    assert_eq!(first.0, 3);
    assert!((first.1 - 125.0).abs() < 0.5, "{at:?}");
    assert!(at.iter().any(|a| (a.1 - 150.0).abs() < 0.5), "the steps add up: {at:?}");
    // The second pinch is 200 px right and 100 up of the first, in the pages' coordinates too.
    assert!(((last.2 - first.2) - 200.0).abs() < 1.0 && ((first.3 - last.3) - 100.0).abs() < 1.0, "{at:?}");
    // Below the toolbar: the fingers' height is the pages', not the window's (the side panel is shut).
    assert!(first.3 > 0.0 && first.3 < 400.0, "relative to the pages, not the window: {at:?}");
}

/// Split with another document: a pinch over its pane (the right half) brings its tab forward and
/// zooms that document, by the pinch's steps (×1.25, then ×1.2), around the fingers as that pane
/// sees them; a pinch over this document's pane zooms this one.
#[test]
fn a_pinch_over_another_documents_pane_zooms_that_document() {
    let (ui, asked) = reader(true);
    pinch(&ui, 1200.0, 400.0);
    let got = asked.borrow().clone();
    assert_eq!(got.first(), Some(&Asked::Focus(3)), "{got:?}");
    let by: Vec<_> = got.iter().filter_map(|a| if let Asked::By(id, f, x, y) = a { Some((*id, *f, *x, *y)) } else { None }).collect();
    assert_eq!(by.len(), 2, "{got:?}");
    assert!(by.iter().all(|b| b.0 == 5), "that document zooms: {by:?}");
    assert!((by[0].1 - 1.25).abs() < 0.01 && (by[1].1 - 1.2).abs() < 0.01, "{by:?}");
    // The right pane starts past the middle of the pages: the fingers are in it, not at 1200.
    assert!(by[0].2 > 0.0 && by[0].2 < 640.0, "{by:?}");
    assert!(!got.iter().any(|a| matches!(a, Asked::At(..))), "this document stays: {got:?}");
    asked.borrow_mut().clear();
    pinch(&ui, 400.0, 400.0);
    let got = asked.borrow().clone();
    assert!(got.iter().any(|a| matches!(a, Asked::At(3, ..))) && !got.iter().any(|a| matches!(a, Asked::By(..) | Asked::Focus(_))), "{got:?}");
}
