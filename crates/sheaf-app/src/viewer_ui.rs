//! The page viewer of a tab (Redact, Sign): its document, what is rendered, and the Redact marks
//! and search results, or the signature being placed, drawn over it. Lives on the UI thread; renders run on the viewer's own
//! render queue and come back through the event loop.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use sheaf_app::viewer::{self, Lru, RenderKey, Viewer, CACHE_BYTES, CACHE_CAP};
use sheaf_app::{AppWindow, RedactMarkItem, RedactResult, SignOverlay, ToolTab, ViewerOverlay, ViewerPage, ViewerThumb};
use sheaf_core::pages::Input;
use sheaf_core::redact::{self, Mark, Pattern, Query, TextMatch};
use slint::{ComponentHandle, Image, ModelRc, VecModel};

use crate::state::{refresh, with};

/// Thumbnails kept (84 px wide at 2×, about 160 KB each).
const THUMB_CAP: usize = 300;
/// Pixels per logical pixel of a thumbnail render.
const THUMB_SCALE: f64 = 2.0;

/// A search match in the Redact panel.
pub struct Found {
    pub m: TextMatch,
    pub checked: bool,
}

/// An area marked by hand: a dragged rectangle or a whole page.
pub struct Manual {
    pub mark: Mark,
    pub label: String,
}

pub struct ViewerUi {
    /// Load id: renders of an earlier document are told apart and dropped.
    pub load_id: u64,
    pub input: Input,
    pub viewer: Viewer,
    /// Device pixels per logical pixel (2 on Retina).
    scale: f64,
    top: f64,
    height: f64,
    strip_top: f64,
    strip_height: f64,
    pages: Lru<RenderKey, Image>,
    page_bytes: usize,
    thumbs: Lru<RenderKey, Image>,
    /// A zoom change waits this timer (150 ms) before pages are rendered at the new size.
    zoom_timer: Option<slint::Timer>,
    zoom_settling: bool,
    scroll: f64,
    scroll_seq: i32,
    drag: Option<(usize, f64, f64)>,
    // Redact
    pub found: Vec<Found>,
    pub manual: Vec<Manual>,
    pub focused: Option<usize>,
    pub searched: bool,
    pub busy: bool,
    // Sign: where the signature is centred (page, view-space point of the displayed page, in
    // points), and its height-to-width ratio once a graphic is made.
    pub sign_at: Option<(usize, f64, f64)>,
}

impl ViewerUi {
    /// Every mark to apply: the checked search matches, then the hand-made ones.
    pub fn marks(&self) -> Vec<Mark> {
        let found = self.found.iter().filter(|f| f.checked).map(|f| Mark { page: f.m.page, quads: f.m.quads.clone() });
        found.chain(self.manual.iter().map(|m| m.mark.clone())).collect()
    }

    fn wanted_pages(&self) -> std::ops::Range<usize> {
        self.viewer.wanted(self.top, self.height)
    }

    fn wanted_thumbs(&self) -> std::ops::Range<usize> {
        viewer::wanted(&self.viewer.thumb_layout(), self.strip_top, self.strip_height)
    }

    fn page_key(&self, page: usize) -> RenderKey {
        RenderKey { doc: self.load_id, page, width: self.viewer.render_px(page, self.scale), thumb: false }
    }

    fn thumb_key(&self, page: usize) -> RenderKey {
        RenderKey { doc: self.load_id, page, width: (viewer::THUMB_WIDTH * THUMB_SCALE) as u32, thumb: true }
    }

    /// The render of `page` at the current width, else the newest at another width (shown
    /// stretched while the right one is made).
    fn any_page_image(&self, page: usize) -> Option<&Image> {
        let exact = self.page_key(page);
        self.pages.peek(&exact).or_else(|| self.pages.keys().rev().find(|k| k.page == page).and_then(|k| self.pages.peek(k)))
    }
}

fn next_load_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Opens `input` in the viewer of tab `tab_id` (reading its page boxes on a worker).
pub fn load(ui: &AppWindow, tab_id: i32, input: Input) {
    let weak = ui.as_weak();
    let scale = ui.window().scale_factor() as f64;
    sheaf_app::pool::probes().submit(move || {
        let r = sheaf_app::jobs::catching(|| {
            let bytes = Arc::new(std::fs::read(&input.path)?);
            let boxes = sheaf_app::viewer::page_boxes(&bytes, input.password.as_deref());
            // The whole file was read to find the page boxes: give that memory back now.
            drop(bytes);
            viewer::release_free_memory();
            boxes
        });
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let ok = with(|a| {
                let Some(t) = a.tab(tab_id) else { return false };
                // The file was replaced or removed meanwhile.
                if t.files.first().map(|f| &f.input.path) != Some(&input.path) {
                    return false;
                }
                let ((w, top, h), (stop, sh)) = (t.viewport, t.strip);
                let ok = match r {
                    Ok(boxes) => {
                        let mut viewer = Viewer::new(boxes);
                        viewer.set_width(w as f64);
                        t.viewer = Some(Box::new(ViewerUi {
                            load_id: next_load_id(),
                            input,
                            viewer,
                            scale,
                            top: top as f64,
                            height: h as f64,
                            strip_top: stop as f64,
                            strip_height: sh as f64,
                            pages: Lru::new(CACHE_CAP),
                            page_bytes: 0,
                            thumbs: Lru::new(THUMB_CAP),
                            zoom_timer: None,
                            zoom_settling: false,
                            scroll: 0.0,
                            scroll_seq: 0,
                            drag: None,
                            found: Vec::new(),
                            manual: Vec::new(),
                            focused: None,
                            searched: false,
                            busy: false,
                            sign_at: None,
                        }));
                        t.viewer_error = None;
                        true
                    }
                    Err(e) => {
                        t.viewer = None;
                        t.viewer_error = Some(format!("The pages can't be shown: {e}"));
                        false
                    }
                };
                refresh(a, tab_id);
                ok
            });
            if ok {
                request(&ui, tab_id);
            }
        });
    });
}

/// Runs `f` on the viewer of tab `tab_id`, then pushes the tab to the UI.
fn update<R>(tab_id: i32, f: impl FnOnce(&mut ViewerUi) -> R) -> Option<R> {
    with(|a| {
        let r = a.tab(tab_id).and_then(|t| t.viewer.as_deref_mut()).map(f);
        refresh(a, tab_id);
        r
    })
}

fn image_bytes(i: &Image) -> usize {
    i.size().width as usize * i.size().height as usize * 4
}

/// Queues the renders the viewport needs and drops queued ones it no longer needs.
pub fn request(ui: &AppWindow, tab_id: i32) {
    let todo = with(|a| {
        let v = a.tab(tab_id)?.viewer.as_deref()?;
        let mut keys: Vec<RenderKey> = Vec::new();
        if !v.zoom_settling {
            keys.extend(v.wanted_pages().map(|p| v.page_key(p)).filter(|k| !v.pages.contains(k)));
        }
        keys.extend(v.wanted_thumbs().map(|p| v.thumb_key(p)).filter(|k| !v.thumbs.contains(k)));
        Some((v.load_id, keys, v.input.clone()))
    });
    let Some((load_id, keys, input)) = todo else { return };
    let wanted: HashSet<RenderKey> = keys.iter().copied().collect();
    let q = viewer::renders();
    q.retain(|k| k.doc != load_id || wanted.contains(k));
    for key in keys {
        let (weak, input) = (ui.as_weak(), input.clone());
        q.submit(key, move || {
            let Ok(px) = viewer::render_page(&input, key.page, key.width) else { return };
            let _ = weak.upgrade_in_event_loop(move |_| {
                let image = Image::from_rgba8(px);
                update(tab_id, |v| {
                    if v.load_id != key.doc {
                        return;
                    }
                    if key.thumb {
                        v.thumbs.put(key, image);
                        return;
                    }
                    // The page's render at another zoom was only kept to show until this one came.
                    let mut evicted = v.pages.retain(|k| k.page != key.page || k.width == key.width);
                    v.page_bytes += image_bytes(&image);
                    evicted.extend(v.pages.put(key, image));
                    v.page_bytes -= evicted.iter().map(|(_, i)| image_bytes(i)).sum::<usize>();
                    let mut freed = !evicted.is_empty();
                    while v.page_bytes > CACHE_BYTES && v.pages.len() > 1 {
                        let Some((_, i)) = v.pages.pop_oldest() else { break };
                        v.page_bytes -= image_bytes(&i);
                        freed = true;
                    }
                    if freed {
                        viewer::release_free_memory();
                    }
                    if std::env::var_os("SHEAF_VIEWER_DEBUG").is_some() {
                        eprintln!("viewer cache: {} pages, {} MB, {} thumbs", v.pages.len(), v.page_bytes >> 20, v.thumbs.len());
                    }
                });
            });
        });
    }
}

// ---- events from the widget ------------------------------------------------------------------

pub fn viewport(ui: &AppWindow, tab_id: i32, width: f32, top: f32, height: f32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            t.viewport = (width, top, height);
        }
    });
    update(tab_id, |v| {
        v.viewer.set_width(width as f64);
        v.top = top as f64;
        v.height = height as f64;
    });
    request(ui, tab_id);
}

pub fn strip_viewport(ui: &AppWindow, tab_id: i32, top: f32, height: f32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            t.strip = (top, height);
        }
    });
    update(tab_id, |v| {
        v.strip_top = top as f64;
        v.strip_height = height as f64;
    });
    request(ui, tab_id);
}

pub fn thumb_clicked(tab_id: i32, page: i32) {
    update(tab_id, |v| {
        v.scroll = v.viewer.scroll_to(page.max(0) as usize, None, v.height);
        v.scroll_seq += 1;
    });
}

pub fn zoom_step(ui: &AppWindow, tab_id: i32, dir: i32) {
    let changed = update(tab_id, |v| {
        let z = if dir == 0 { 100 } else { viewer::step_zoom(v.viewer.zoom(), dir) };
        let old_h = v.viewer.content_height();
        if !v.viewer.set_zoom(z) {
            return false;
        }
        // Keep the same part of the document in view.
        v.scroll = (v.top * v.viewer.content_height() / old_h.max(1.0)).max(0.0);
        v.top = v.scroll;
        v.scroll_seq += 1;
        v.zoom_settling = true;
        true
    });
    if changed != Some(true) {
        return;
    }
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    timer.start(slint::TimerMode::SingleShot, Duration::from_millis(150), move || {
        if let Some(ui) = weak.upgrade() {
            update(tab_id, |v| v.zoom_settling = false);
            request(&ui, tab_id);
        }
    });
    update(tab_id, |v| v.zoom_timer = Some(timer));
}

pub fn pressed(tab_id: i32, x: f32, y: f32) {
    with(|a| {
        if let Some(v) = a.tab(tab_id).and_then(|t| t.viewer.as_deref_mut()) {
            v.drag = v.viewer.hit(x as f64, y as f64).map(|(p, _, _)| (p, x as f64, y as f64));
        }
    });
}

/// Least size of a dragged area, logical pixels: smaller is a click.
const MIN_DRAG: f64 = 4.0;

pub fn released(tab_id: i32, x: f32, y: f32) {
    let sign = with(|a| a.tab(tab_id).map(|t| t.tool == sheaf_core::tools::ToolId::Sign)).unwrap_or(false);
    update(tab_id, |v| {
        let Some((page, x0, y0)) = v.drag.take() else { return };
        if sign {
            // Click to place, drag to move: the signature is centred where the pointer is let go,
            // on the page it went down on, kept on that page.
            v.sign_at = Some(place_point(&v.viewer, page, x as f64, y as f64));
            return;
        }
        let (x1, y1) = (x as f64, y as f64);
        if (x1 - x0).abs() < MIN_DRAG || (y1 - y0).abs() < MIN_DRAG {
            return;
        }
        let a = v.viewer.to_page(page, x0, y0);
        let b = v.viewer.to_page(page, x1, y1);
        let rect = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
        v.manual.push(Manual { mark: redact::mark_rects(&[(page, rect)]).remove(0), label: "Selected area".into() });
    });
}

/// A content point as page `page`'s view-space point in points (y down, as displayed), held to
/// the page.
fn place_point(viewer: &Viewer, page: usize, x: f64, y: f64) -> (usize, f64, f64) {
    let [px, py, w, h] = viewer.page_rect(page);
    let (vw, vh) = viewer.boxes[page].view_size();
    let k = vw / w.max(1e-9);
    (page, ((x - px).clamp(0.0, w) * k).clamp(0.0, vw), ((y - py).clamp(0.0, h) * k).clamp(0.0, vh))
}

/// The signature's box in page `page`'s view space (points, `[x0, y0, x1, y1]`, y down): `width`
/// wide, `width * aspect` tall, centred at `at` and kept on the page where it fits.
pub fn sign_view_box(viewer: &Viewer, page: usize, at: (f64, f64), width: f64, aspect: f64) -> [f64; 4] {
    let (vw, vh) = viewer.boxes[page].view_size();
    let (w, h) = (width.min(vw), (width * aspect).min(vh));
    let x0 = (at.0 - w / 2.0).clamp(0.0, (vw - w).max(0.0));
    let y0 = (at.1 - h / 2.0).clamp(0.0, (vh - h).max(0.0));
    [x0, y0, x0 + w, y0 + h]
}

// ---- Redact panel ----------------------------------------------------------------------------

fn pattern(id: &str) -> Option<Pattern> {
    match id {
        "email" => Some(Pattern::Email),
        "phone" => Some(Pattern::Phone),
        "card" => Some(Pattern::CreditCard),
        "date" => Some(Pattern::Date),
        "ssn" => Some(Pattern::Ssn),
        _ => None,
    }
}

/// The queries for a typed text and the selected pattern codes (",email,phone,").
pub fn queries(text: &str, patterns: &str) -> Vec<Query> {
    let mut q = Vec::new();
    if !text.trim().is_empty() {
        q.push(Query::Literal { text: text.trim().into(), case_insensitive: true });
    }
    q.extend(patterns.split(',').filter_map(pattern).map(Query::Pattern));
    q
}

pub fn search(ui: &AppWindow, tab_id: i32, text: &str, patterns: &str) {
    let queries = queries(text, patterns);
    let Some(input) = update(tab_id, |v| {
        // Matches still checked from an earlier search stay marked.
        for f in v.found.drain(..).filter(|f| f.checked) {
            v.manual.push(Manual { label: f.m.text.clone(), mark: Mark { page: f.m.page, quads: f.m.quads } });
        }
        v.focused = None;
        v.searched = !queries.is_empty();
        v.busy = !queries.is_empty();
        v.input.clone()
    }) else {
        return;
    };
    if queries.is_empty() {
        return;
    }
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let r = sheaf_app::jobs::catching(|| redact::find_all(&input, &queries));
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let err = update(tab_id, |v| {
                v.busy = false;
                match r {
                    Ok(found) => {
                        // A match already marked is not offered twice.
                        v.found = found
                            .into_iter()
                            .filter(|m| !v.manual.iter().any(|x| x.mark.page == m.page && x.mark.quads == m.quads))
                            .map(|m| Found { m, checked: true })
                            .collect();
                        None
                    }
                    Err(e) => Some(e.to_string()),
                }
            })
            .flatten();
            if let Some(e) = err {
                crate::flow::toast(&ui, &format!("Search failed: {e}"));
            }
        });
    });
}

pub fn toggle_result(tab_id: i32, k: i32) {
    update(tab_id, |v| {
        if let Some(f) = v.found.get_mut(k as usize) {
            f.checked = !f.checked;
        }
    });
}

pub fn show_result(tab_id: i32, k: i32) {
    update(tab_id, |v| {
        let Some(f) = v.found.get(k as usize) else { return };
        let (page, focus) = (f.m.page, redact::bounds(&f.m.quads));
        v.focused = Some(k as usize);
        v.scroll = v.viewer.scroll_to(page, Some(focus), v.height);
        v.scroll_seq += 1;
    });
}

pub fn mark_page(ui: &AppWindow, tab_id: i32) {
    let Some((input, page)) = update(tab_id, |v| (v.input.clone(), v.viewer.current(v.top, v.height))) else { return };
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let r = sheaf_app::jobs::catching(|| redact::mark_pages(&input, &[page]));
        let _ = weak.upgrade_in_event_loop(move |ui| match r {
            Ok(mut m) => {
                update(tab_id, |v| v.manual.push(Manual { mark: m.remove(0), label: "Whole page".into() }));
            }
            Err(e) => crate::flow::toast(&ui, &e.to_string()),
        });
    });
}

/// The `k`-th item of "Marked for redaction" (checked matches first, then hand-made marks).
pub fn remove_mark(tab_id: i32, k: i32) {
    update(tab_id, |v| {
        let k = k.max(0) as usize;
        let checked: Vec<usize> = v.found.iter().enumerate().filter(|(_, f)| f.checked).map(|(i, _)| i).collect();
        match checked.get(k) {
            Some(&i) => v.found[i].checked = false,
            None if k - checked.len() < v.manual.len() => {
                v.manual.remove(k - checked.len());
            }
            None => {}
        }
    });
}

pub fn clear_marks(tab_id: i32) {
    update(tab_id, |v| {
        v.manual.clear();
        for f in &mut v.found {
            f.checked = false;
        }
    });
}

// ---- into the tab model ----------------------------------------------------------------------

fn len(v: f64) -> f32 {
    v as f32
}

fn page_label(p: usize) -> slint::SharedString {
    format!("Page {}", p + 1).into()
}

/// The viewer and Redact fields of `tab`.
pub fn fill(tab: &mut ToolTab, v: &ViewerUi) {
    let wanted = v.wanted_pages();
    let pages: Vec<ViewerPage> = wanted
        .clone()
        .map(|i| {
            let [x, y, w, h] = v.viewer.page_rect(i);
            let image = v.any_page_image(i).cloned();
            ViewerPage { x: len(x), y: len(y), w: len(w), h: len(h), ready: image.is_some(), image: image.unwrap_or_default() }
        })
        .collect();
    let tl = v.viewer.thumb_layout();
    let thumbs: Vec<ViewerThumb> = v
        .wanted_thumbs()
        .map(|i| {
            let image = v.thumbs.peek(&v.thumb_key(i)).cloned();
            ViewerThumb { page: i as i32, y: len(tl[i].0), h: len(tl[i].1), ready: image.is_some(), image: image.unwrap_or_default() }
        })
        .collect();
    let mut overlays = Vec::new();
    let mut boxes = |page: usize, quads: &[[f64; 8]], kind: i32| {
        if wanted.contains(&page) {
            for q in quads {
                let [x, y, w, h] = v.viewer.overlay(page, redact::bounds(std::slice::from_ref(q)));
                overlays.push(ViewerOverlay { x: len(x), y: len(y), w: len(w), h: len(h), kind });
            }
        }
    };
    for (i, f) in v.found.iter().enumerate() {
        let kind = if v.focused == Some(i) { 2 } else if f.checked { 0 } else { 1 };
        boxes(f.m.page, &f.m.quads, kind);
    }
    for m in &v.manual {
        boxes(m.mark.page, &m.mark.quads, 0);
    }
    let results: Vec<RedactResult> =
        v.found.iter().map(|f| RedactResult { text: f.m.text.clone().into(), page: page_label(f.m.page), checked: f.checked }).collect();
    let marks: Vec<RedactMarkItem> = v
        .found
        .iter()
        .filter(|f| f.checked)
        .map(|f| RedactMarkItem { label: f.m.text.clone().into(), page: page_label(f.m.page) })
        .chain(v.manual.iter().map(|m| RedactMarkItem { label: m.label.clone().into(), page: page_label(m.mark.page) }))
        .collect();
    let n = v.found.len();
    tab.viewer_pages = ModelRc::new(VecModel::from(pages));
    tab.viewer_thumbs = ModelRc::new(VecModel::from(thumbs));
    tab.viewer_overlays = ModelRc::new(VecModel::from(overlays));
    tab.viewer_width = len(v.viewer.content_width());
    tab.viewer_height = len(v.viewer.content_height());
    tab.viewer_thumbs_height = len(viewer::content_height(&tl));
    tab.viewer_current = v.viewer.current(v.top, v.height) as i32;
    tab.viewer_zoom = v.viewer.zoom() as i32;
    tab.viewer_scroll = len(v.scroll);
    tab.viewer_scroll_seq = v.scroll_seq;
    tab.redact_searched = v.searched;
    tab.redact_busy = v.busy;
    tab.redact_count = if v.busy {
        "Searching…".into()
    } else if n == 1 {
        "1 match".into()
    } else {
        format!("{n} matches").into()
    };
    tab.redact_results = ModelRc::new(VecModel::from(results));
    tab.redact_marks = ModelRc::new(VecModel::from(marks));
}

/// Sign: what the job needs from the viewer and the panel, once a signature is made and placed.
pub fn placed(v: &ViewerUi, s: &crate::sign_ui::SignState) -> Option<crate::run::Placed> {
    let (page, x, y) = v.sign_at?;
    let graphic = s.graphic()?;
    let aspect = s.preview.as_ref()?.1;
    Some((graphic, page, sign_view_box(&v.viewer, page, (x, y), s.width(), aspect)))
}

/// Sign: the signature's box over the pages, `width` points wide with height `width * aspect`.
pub fn fill_sign(tab: &mut ToolTab, v: &ViewerUi, width: f64, aspect: Option<f64>) {
    tab.sign_placed = v.sign_at.is_some();
    tab.sign_page = v.sign_at.map(|s| s.0 as i32).unwrap_or(0);
    let (Some((page, x, y)), Some(aspect)) = (v.sign_at, aspect) else { return };
    let [x0, y0, x1, y1] = sign_view_box(&v.viewer, page, (x, y), width, aspect);
    let [px, py, w, _] = v.viewer.page_rect(page);
    let k = w / v.viewer.boxes[page].view_size().0.max(1e-9);
    tab.sign_overlay = SignOverlay { show: true, x: len(px + x0 * k), y: len(py + y0 * k), w: len((x1 - x0) * k), h: len((y1 - y0) * k) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_app::viewer::PageBox;

    #[test]
    fn a_click_places_the_signature_centred_and_kept_on_the_page() {
        // A4 portrait and the same page turned a quarter (shown landscape).
        let mut v = Viewer::new(vec![PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0 }, PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 90 }]);
        v.set_width(595.0 + 2.0 * viewer::PAGE_GAP);
        let [px, py, w, _] = v.page_rect(0);
        assert_eq!(w, 595.0);
        let at = place_point(&v, 0, px + 100.0, py + 200.0);
        assert_eq!(at, (0, 100.0, 200.0));
        assert_eq!(sign_view_box(&v, 0, (at.1, at.2), 150.0, 0.4), [25.0, 170.0, 175.0, 230.0]);
        // Near the edge it stays on the page.
        assert_eq!(sign_view_box(&v, 0, (590.0, 5.0), 150.0, 0.4), [445.0, 0.0, 595.0, 60.0]);
        // The turned page is 842 wide as shown; points are in its displayed space.
        let [px, py, w, h] = v.page_rect(1);
        let at = place_point(&v, 1, px + w, py + h);
        assert!((at.1 - 842.0).abs() < 1e-6 && (at.2 - 595.0).abs() < 1e-6, "{at:?}");
    }

    #[test]
    fn typed_text_and_chips_become_queries() {
        assert_eq!(
            queries(" secret ", ",email,card,"),
            vec![
                Query::Literal { text: "secret".into(), case_insensitive: true },
                Query::Pattern(Pattern::Email),
                Query::Pattern(Pattern::CreditCard)
            ]
        );
        assert!(queries("  ", ",,").is_empty());
    }
}
