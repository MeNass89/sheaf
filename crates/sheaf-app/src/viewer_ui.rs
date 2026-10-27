//! The page viewer of a tab (Redact, Sign, Edit): its document, what is rendered, and the Redact
//! marks and search results, or what Sign and Edit place, drawn over it. Lives on the UI thread; renders run on the viewer's own
//! render queue and come back through the event loop.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use sheaf_app::viewer::{self, Lru, RenderKey, Viewer, CACHE_BYTES, CACHE_CAP};
use sheaf_app::{AppWindow, RedactMarkItem, RedactResult, SignOverlay, ToolTab, ViewerOverlay, ViewerPage, ViewerThumb};
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::render;
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
    /// Redact's searches started: an answer for an older one is dropped.
    pub search_seq: u64,
    /// An actual size (percent) kept as the view's width comes in (Preferences ▸ Open new documents
    /// at Actual size; the zoom is a share of the width), until the user zooms.
    pub pending_actual: Option<f64>,
    /// Fit page to apply once the view knows its height (Preferences ▸ Open new documents at Fit page).
    pub pending_fit: bool,
    /// The page to show once the view knows its size (a document's Initial View asks for one).
    pub pending_page: Option<usize>,
    pub input: Input,
    pub viewer: Viewer,
    /// Device pixels per logical pixel (2 on Retina).
    scale: f64,
    pub top: f64,
    /// How far the view is scrolled across (the view reports it), kept by ⌘[ and ⌘].
    pub left: f64,
    pub height: f64,
    strip_top: f64,
    strip_height: f64,
    pages: Lru<RenderKey, Image>,
    page_bytes: usize,
    thumbs: Lru<RenderKey, Image>,
    /// A zoom change waits this timer (150 ms) before pages are rendered at the new size.
    zoom_timer: Option<slint::Timer>,
    zoom_settling: bool,
    /// ⌘ + wheel: the zoom asked for (actual percent, unrounded) until it settles, so small trackpad
    /// steps add up instead of rounding away.
    zoom_target: Option<f64>,
    pub scroll: f64,
    pub scroll_seq: i32,
    /// The horizontal scroll a zoom around a point sets, with `scroll` (`None`: left as it is).
    pub scroll_x: Option<f64>,
    drag: Option<(usize, f64, f64)>,
    /// The reader renders its working copy (edits not saved yet) from memory; tools read the file.
    pub source: Option<Arc<Vec<u8>>>,
    pub theme: sheaf_app::reader::PageTheme,
    /// The reader's Hide all comments: pages render without annotations.
    pub hide_comments: bool,
    /// One page at a time: the page shown, the one gone to, whatever the zoom (Sol, 26/10: the one
    /// under the view's middle, page 2 when page 1 was asked for zoomed out). None: continuous.
    pub single_page: Option<usize>,
    /// Revision of each page's look: bumped by an edit of the page; `base_rev` by a change of
    /// every page (theme, undo). Renders of an older revision stay on screen until replaced.
    revs: std::collections::HashMap<usize, u64>,
    base_rev: u64,
    next_rev: u64,
    // Redact
    pub found: Vec<Found>,
    pub manual: Vec<Manual>,
    pub focused: Option<usize>,
    pub searched: bool,
    pub busy: bool,
    /// Pages' text, read when a drag in Redact starts on a page (text is marked by selecting it).
    texts: std::collections::HashMap<usize, Arc<sheaf_core::pdfcraft::render::PageText>>,
    /// The text a drag that started on text selects so far (drawn as it goes), and the last
    /// double click (a third click soon after, there, takes the whole line).
    live: Option<Mark>,
    selecting: bool,
    last_double: Option<(std::time::Instant, usize, f64, f64)>,
    text_renderer: Option<sheaf_core::pdfcraft::render::PageRenderer>,
    // Sign: where the signature is centred (page, view-space point of the displayed page, in
    // points), and its height-to-width ratio once a graphic is made.
    pub sign_at: Option<(usize, f64, f64)>,
    /// A split view's other pane (see `crate::split`): no thumbnails of its own.
    pub pane: bool,
}

impl ViewerUi {
    /// Every mark to apply: the checked search matches, then the hand-made ones.
    pub fn marks(&self) -> Vec<Mark> {
        let found = self.found.iter().filter(|f| f.checked).map(|f| redact::mark_text(std::slice::from_ref(&f.m)).remove(0));
        found.chain(self.manual.iter().map(|m| m.mark.clone())).collect()
    }

    fn wanted_pages(&self) -> std::ops::Range<usize> {
        self.viewer.wanted(self.top, self.height)
    }

    fn wanted_thumbs(&self) -> std::ops::Range<usize> {
        viewer::wanted(&self.viewer.thumb_layout(), self.strip_top, self.strip_height)
    }

    pub fn new(input: Input, viewer: Viewer, scale: f64, viewport: (f32, f32, f32), strip: (f32, f32)) -> Self {
        let ((_, top, h), (stop, sh)) = (viewport, strip);
        ViewerUi {
            load_id: next_load_id(),
            search_seq: 0,
            pending_actual: None,
            pending_fit: false,
            pending_page: None,
            input,
            viewer,
            scale,
            top: top as f64,
            left: 0.0,
            height: h as f64,
            strip_top: stop as f64,
            strip_height: sh as f64,
            pages: Lru::new(CACHE_CAP),
            page_bytes: 0,
            thumbs: Lru::new(THUMB_CAP),
            zoom_timer: None,
            zoom_settling: false,
            zoom_target: None,
            scroll: 0.0,
            scroll_seq: 0,
            scroll_x: None,
            drag: None,
            source: None,
            theme: Default::default(),
            hide_comments: false,
            single_page: None,
            revs: Default::default(),
            base_rev: 0,
            next_rev: 1,
            found: Vec::new(),
            manual: Vec::new(),
            focused: None,
            searched: false,
            busy: false,
            texts: Default::default(),
            live: None,
            selecting: false,
            last_double: None,
            text_renderer: None,
            sign_at: None,
            pane: false,
        }
    }

    /// A viewer of the same document at the same place, zoom and look, rendering on its own: a split
    /// view's other pane (PDF Expert's Split View). Its pages come from its own renders.
    pub fn twin(&self) -> ViewerUi {
        let mut t = ViewerUi::new(self.input.clone(), self.viewer.clone(), self.scale, (0.0, self.top as f32, self.height as f32), (0.0, 0.0));
        t.left = self.left;
        t.source = self.source.clone();
        t.theme = self.theme;
        t.hide_comments = self.hide_comments;
        t.revs = self.revs.clone();
        t.base_rev = self.base_rev;
        t.next_rev = self.next_rev;
        t.scroll = self.top;
        t.scroll_x = Some(self.left);
        t.pane = true;
        t
    }

    /// A twin keeps up with `main`, the viewer the reader works in: the working copy and its
    /// revisions (an edit renders again in both panes), the look, and the pages themselves (some
    /// deleted, inserted or the view turned: laid out again at this pane's own zoom and place).
    /// Returns whether it was laid out again (its scroll is then set anew).
    pub fn follow(&mut self, main: &ViewerUi) -> bool {
        if !self.source.as_ref().zip(main.source.as_ref()).is_some_and(|(a, b)| Arc::ptr_eq(a, b)) {
            self.source = main.source.clone();
            self.text_renderer = None;
            self.texts.clear();
        }
        self.theme = main.theme;
        self.hide_comments = main.hide_comments;
        if self.revs != main.revs || self.base_rev != main.base_rev {
            self.revs = main.revs.clone();
            self.base_rev = main.base_rev;
        }
        self.next_rev = self.next_rev.max(main.next_rev);
        if self.viewer.boxes != main.viewer.boxes {
            let place = self.place();
            let mut v = main.viewer.clone();
            v.set_width(self.viewer.view_width());
            v.set_height(self.height);
            v.set_spread(self.viewer.spread());
            v.set_rtl(self.viewer.rtl());
            v.set_horizontal(self.viewer.horizontal());
            v.set_zoom(self.viewer.zoom());
            v.set_left(self.left);
            self.viewer = v;
            self.go_to_place((place.0.min(self.viewer.page_count().saturating_sub(1)), place.1, place.2));
            return true;
        }
        false
    }

    /// A split view's panes swapped (see `crate::split`): the thumbnails strip, its place and its
    /// renders, stay with the active pane.
    pub fn swap_strip(&mut self, other: &mut ViewerUi) {
        std::mem::swap(&mut self.strip_top, &mut other.strip_top);
        std::mem::swap(&mut self.strip_height, &mut other.strip_height);
        std::mem::swap(&mut self.thumbs, &mut other.thumbs);
    }

    /// The pages in view, placed and with their renders (the newest one while the right one is made).
    pub fn pane_pages(&self) -> Vec<ViewerPage> {
        self.wanted_pages()
            .map(|i| {
                let [x, y, w, h] = self.viewer.page_rect(i);
                let image = self.any_page_image(i).cloned();
                ViewerPage { page: i as i32, x: len(x), y: len(y), w: len(w), h: len(h), ready: image.is_some(), image: image.unwrap_or_default() }
            })
            .collect()
    }

    /// Page `page`'s text (read once).
    fn text(&mut self, page: usize) -> Option<Arc<sheaf_core::pdfcraft::render::PageText>> {
        use sheaf_core::pdfcraft::render;
        if let Some(t) = self.texts.get(&page) {
            return Some(t.clone());
        }
        if self.text_renderer.is_none() {
            let config = render::RenderConfig { password: self.input.password.as_deref().map(Arc::from), ..Default::default() };
            // A tool's viewer renders from the file; the reader's from its edited bytes.
            let bytes = self.source.clone().or_else(|| std::fs::read(&self.input.path).ok().map(Arc::new))?;
            self.text_renderer = Some(render::PageRenderer::new(bytes, config));
        }
        let res = self.text_renderer.as_mut()?.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
        let t = res.text?;
        self.texts.insert(page, t.clone());
        Some(t)
    }

    fn rev(&self, page: usize) -> u64 {
        self.revs.get(&page).copied().unwrap_or(0).max(self.base_rev)
    }

    /// Page `page` looks different now: render it again (the old render stays until then).
    pub fn page_changed(&mut self, page: usize) {
        self.revs.insert(page, self.next_rev);
        self.next_rev += 1;
    }

    /// Every page looks different now (a theme, an undo).
    pub fn all_changed(&mut self) {
        self.base_rev = self.next_rev;
        self.next_rev += 1;
    }

    /// Scrolls so page `page` (and the point `at` of it, in view points) is in view.
    pub fn show_page(&mut self, page: usize, focus: Option<[f64; 4]>) {
        let page = page.min(self.viewer.page_count().saturating_sub(1));
        if self.single_page.is_some() {
            self.single_page = Some(page);
        }
        self.scroll = self.viewer.scroll_to(page, focus, self.height);
        self.scroll_seq += 1;
        // Pages across: the page's row centred in the view.
        self.scroll_x = self.viewer.across().then(|| self.viewer.scroll_x_to(page));
    }

    /// Shows a destination as Acrobat does: at its zoom when it has one (/XYZ's, 1.0 = actual
    /// size), its top edge or rectangle a third down the view, its left edge at the view's left
    /// when the page is wider than the view (/XYZ left, /FitR). Returns whether the zoom changed.
    pub fn show_dest(&mut self, page: usize, view: &render::DestView) -> bool {
        let page = page.min(self.viewer.page_count().saturating_sub(1));
        let zoomed = match *view {
            render::DestView::Xyz { zoom: Some(z), .. } => {
                let target = self.viewer.zoom_for_actual(page, z as f64 * 100.0);
                self.viewer.set_zoom(target)
            }
            // The whole page in the view, or its width (Sol, 25/10: a zoomed view stayed zoomed).
            render::DestView::Fit => {
                let z = self.viewer.fit_page_zoom(self.height, page..page + 1);
                self.viewer.set_zoom(z)
            }
            render::DestView::FitH { .. } => self.viewer.set_zoom(100),
            // The rectangle filling the view (Sol, 25/10: it kept the zoom and could stay cut off).
            render::DestView::FitR { rect } => self.zoom_to_fit(page, rect.map(f64::from)),
            _ => false,
        };
        let crop = self.viewer.boxes.get(page).map(|b| b.crop).unwrap_or_default();
        self.show_page(page, sheaf_app::reader::dest_focus(view, crop));
        // The view's left edge at the destination's as shown: on a page turned a quarter, a point's
        // place across depends on its height in user space (Sol, 26/10: it was taken from the crop
        // box's bottom, and the rectangle could stay out of view).
        let left = match *view {
            render::DestView::Xyz { left: Some(l), top, .. } => {
                let t = top.map_or(crop[1], f64::from);
                Some(self.viewer.overlay(page, [l as f64, t, l as f64 + 1.0, t + 1.0])[0])
            }
            render::DestView::FitR { rect } => Some(self.viewer.overlay(page, rect.map(f64::from))[0]),
            _ => None,
        };
        if let Some(x) = left {
            self.scroll_x = Some((x - 8.0).clamp(0.0, self.viewer.max_scroll_x()));
        }
        zoomed
    }

    /// The zoom at which `rect` (user space of `page`) fills the view with a little room around it,
    /// up to the reader's largest zoom (400 % of actual size: a word would ask for pages too big to
    /// draw): a /FitR destination's, and Zoom to area's. Returns whether the zoom changed.
    pub fn zoom_to_fit(&mut self, page: usize, rect: [f64; 4]) -> bool {
        let [_, _, w, h] = self.viewer.overlay(page, rect);
        if w < 1.0 || h < 1.0 {
            return false;
        }
        let k = (self.viewer.view_width() / w).min(self.height / h) * 0.92;
        let most = self.viewer.zoom_for_actual(page, f64::from(viewer::MAX_ZOOM));
        self.viewer.set_zoom(((f64::from(self.viewer.zoom()) * k).round() as u32).min(most))
    }

    /// Where the view stands: the page point at its top edge, which a zoom keeps (⌘[ comes back to
    /// it, not only to its page).
    pub fn place(&self) -> (usize, f64, f64) {
        let left = self.left.max(self.viewer.page_rect(self.current())[0]);
        self.viewer.anchor_of(left, self.top)
    }

    /// Scrolls back to a [`Self::place`].
    pub fn go_to_place(&mut self, place: (usize, f64, f64)) {
        let (x, y) = self.viewer.point_of(place);
        self.scroll = y.clamp(0.0, (self.viewer.content_height() - self.height).max(0.0));
        self.scroll_seq += 1;
        // Across too, on a page wider than the view (Sol, 25/10: only the height came back).
        self.scroll_x = (self.viewer.max_scroll_x() > 0.0).then(|| x.clamp(0.0, self.viewer.max_scroll_x()));
    }

    /// Zooms to `z` percent of the fit width, the view point (ax, ay), scrolled `sx` across, staying
    /// on the same point of its page; the zoom then settles. Returns the new horizontal scroll.
    fn zoom_around(&mut self, z: u32, (ax, ay, sx): (f64, f64, f64)) -> Option<f64> {
        // A wheel step right after another comes before the view has taken the first one's
        // horizontal scroll: while the zoom settles, ours is the current one.
        let sx = if self.zoom_settling { self.scroll_x.unwrap_or(sx) } else { sx };
        let anchor = self.viewer.anchor_of(sx + ax, self.top + ay);
        if !self.viewer.set_zoom(z) {
            return None;
        }
        let (x, y) = self.viewer.point_of(anchor);
        self.scroll = (y - ay).clamp(0.0, (self.viewer.content_height() - self.height).max(0.0));
        self.scroll_x = Some((x - ax).clamp(0.0, self.viewer.max_scroll_x()));
        self.top = self.scroll;
        self.scroll_seq += 1;
        self.zoom_settling = true;
        self.scroll_x
    }

    /// Where ⌘+ / ⌘−, a zoom typed and Fit keep the view: its top edge, scrolled across as it is, at
    /// its middle when pages run across (Sol, 27/10: deep in a strip, a zoom anchored on the strip's
    /// start and jumped back toward page 1).
    fn zoom_anchor(&self) -> (f64, f64, f64) {
        let ax = if self.viewer.across() { self.viewer.view_width() / 2.0 } else { 0.0 };
        (ax, 0.0, self.left)
    }

    /// The view is `width` × `height`, scrolled to `top`. With pages across, a new size keeps the
    /// point at the view's top middle on its page (Sol, 27/10: deep in a strip, a window or side
    /// panel resized slid toward another page: the strip is laid out again at the new width).
    fn resize(&mut self, width: f64, top: f64, height: f64) {
        let w = self.viewer.view_width();
        let anchor = self.viewer.across().then(|| self.viewer.anchor_of(self.left + w / 2.0, self.top));
        let wide = self.viewer.set_width(width);
        let high = self.viewer.set_height(height);
        self.top = top;
        self.height = height;
        if let Some(a) = anchor.filter(|_| wide || high) {
            let (x, y) = self.viewer.point_of(a);
            let w = self.viewer.view_width();
            // Down: the same point at the top when the view's top was in the page, else as it was.
            let y = if a.2 > 0.0 { y } else { self.top };
            self.scroll = y.clamp(0.0, (self.viewer.content_height() - height).max(0.0));
            self.top = self.scroll;
            let left = (x - w / 2.0).clamp(0.0, self.viewer.max_scroll_x());
            self.scroll_x = Some(left);
            self.left = left;
            self.viewer.set_left(left);
            self.scroll_seq += 1;
        }
    }

    /// The page under the middle of the viewport.
    pub fn current(&self) -> usize {
        self.single_page.unwrap_or_else(|| self.viewer.current(self.top, self.height))
    }

    fn page_key(&self, page: usize) -> RenderKey {
        RenderKey { doc: self.load_id, page, width: self.viewer.render_px(page, self.scale), thumb: false, rev: self.rev(page) }
    }

    fn thumb_key(&self, page: usize) -> RenderKey {
        RenderKey { doc: self.load_id, page, width: (self.viewer.thumb_width * THUMB_SCALE) as u32, thumb: true, rev: self.rev(page) }
    }

    /// The render of `page` at the current width, else the newest at another width or revision
    /// (shown stretched or stale while the right one is made).
    fn any_page_image(&self, page: usize) -> Option<&Image> {
        let exact = self.page_key(page);
        self.pages.peek(&exact).or_else(|| self.pages.keys().rev().find(|k| k.page == page).and_then(|k| self.pages.peek(k)))
    }

    fn any_thumb(&self, page: usize) -> Option<&Image> {
        let exact = self.thumb_key(page);
        self.thumbs.peek(&exact).or_else(|| self.thumbs.keys().rev().find(|k| k.page == page).and_then(|k| self.thumbs.peek(k)))
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
                        t.viewer = Some(Box::new(ViewerUi::new(input, viewer, scale, (w, top, h), (stop, sh))));
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
        crate::split::follow(a, tab_id);
        refresh(a, tab_id);
        r
    })
}

/// Runs `f` on whichever viewer of tab `tab_id` renders load `doc` (its own or its split view's
/// other pane), then pushes the tab to the UI; a render of an earlier document is dropped.
fn update_doc(tab_id: i32, doc: u64, f: impl FnOnce(&mut ViewerUi)) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            let own = t.viewer.as_deref_mut().filter(|v| v.load_id == doc);
            let v = match own {
                Some(v) => Some(v),
                None => t.split.as_mut().map(|s| &mut *s.view).filter(|v| v.load_id == doc),
            };
            if let Some(v) = v {
                f(v);
            }
        }
        refresh(a, tab_id);
    });
}

fn image_bytes(i: &Image) -> usize {
    i.size().width as usize * i.size().height as usize * 4
}

/// Every open document's pages drawn again (a rendering preference changed).
pub fn redraw_all(ui: &AppWindow) {
    for id in with(|a| a.tabs.iter().filter(|t| t.viewer.is_some()).map(|t| t.id).collect::<Vec<_>>()) {
        update(id, |v| v.all_changed());
        request(ui, id);
    }
}

/// Queues the renders the viewport needs and drops queued ones it no longer needs: the tab's viewer
/// and a split view's other pane, each its own renders.
pub fn request(ui: &AppWindow, tab_id: i32) {
    request_for(ui, tab_id, false);
    request_for(ui, tab_id, true);
}

fn request_for(ui: &AppWindow, tab_id: i32, pane: bool) {
    let todo = with(|a| {
        let t = a.tab(tab_id)?;
        let v: &ViewerUi = if pane { &t.split.as_ref()?.view } else { t.viewer.as_deref()? };
        let mut keys: Vec<RenderKey> = Vec::new();
        if !v.zoom_settling {
            keys.extend(v.wanted_pages().map(|p| v.page_key(p)).filter(|k| !v.pages.contains(k)));
        }
        if !v.pane {
            keys.extend(v.wanted_thumbs().map(|p| v.thumb_key(p)).filter(|k| !v.thumbs.contains(k)));
        }
        let rough = crate::home::rough();
        let look = viewer::PageLook { theme: v.theme, comments: !v.hide_comments, turn: v.viewer.turn(), smooth: [0, 1, 2].map(|k| rough & (1 << k) == 0) };
        Some((v.load_id, keys, v.input.clone(), v.source.clone(), look))
    });
    let Some((load_id, keys, input, source, look)) = todo else { return };
    let wanted: HashSet<RenderKey> = keys.iter().copied().collect();
    let q = viewer::renders();
    q.retain(|k| k.doc != load_id || wanted.contains(k));
    for key in keys {
        let (weak, input, source) = (ui.as_weak(), input.clone(), source.clone());
        q.submit(key, move || {
            let px = match &source {
                Some(bytes) => viewer::render_bytes(bytes, input.password.as_deref(), key.page, key.width, look),
                None => viewer::render_page(&input, key.page, key.width).map(|mut px| {
                    sheaf_app::reader::tint(px.make_mut_bytes(), look.theme);
                    px
                }),
            };
            let Ok(px) = px else { return };
            let _ = weak.upgrade_in_event_loop(move |_| {
                let image = Image::from_rgba8(px);
                update_doc(tab_id, key.doc, |v| {
                    if key.thumb {
                        v.thumbs.retain(|k| k.page != key.page || k.rev >= key.rev);
                        v.thumbs.put(key, image);
                        return;
                    }
                    // The page's render at another zoom or of an older revision was only kept to
                    // show until this one came.
                    let mut evicted = v.pages.retain(|k| k.page != key.page || (k.width == key.width && k.rev >= key.rev));
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

/// Redact's marks not applied yet in tool tab `tab_id`, and its file's name: (name, count).
pub fn redact_pending(tab_id: i32) -> Option<(String, usize)> {
    with(|a| {
        let t = a.tab(tab_id)?;
        if t.tool != sheaf_core::tools::ToolId::Redact || matches!(t.phase, crate::state::Phase::Done(_)) {
            return None;
        }
        let v = t.viewer.as_ref()?;
        let n = v.found.iter().filter(|f| f.checked).count() + v.manual.len();
        let name = v.input.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        (n > 0).then_some((name, n))
    })
}

/// The view scrolled across to `left` (its own scroll or one asked for).
pub fn view_left(tab_id: i32, left: f32) {
    update(tab_id, |v| {
        v.left = left as f64;
        v.viewer.set_left(left as f64);
    });
}

pub fn viewport(ui: &AppWindow, tab_id: i32, width: f32, top: f32, height: f32) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            t.viewport = (width, top, height);
        }
    });
    update(tab_id, |v| {
        v.resize(width as f64, top as f64, height as f64);
        // One page at a time, scrolled away from its page (the scroll bar dragged): the page now in view.
        if let Some(s) = v.single_page {
            let [_, y, _, h] = v.viewer.page_rect(s);
            if y + h < v.top || y > v.top + v.height {
                v.single_page = Some(v.viewer.current(v.top, v.height));
            }
        }
        if width > 0.0
            && let Some(pct) = v.pending_actual
        {
            let z = v.viewer.zoom_for_actual(v.current(), pct);
            v.viewer.set_zoom(z);
        }
        if width > 0.0 && height > 0.0 && v.pending_fit {
            let cur = v.current();
            let z = v.viewer.fit_page_zoom(height as f64, cur..cur + 1);
            v.viewer.set_zoom(z);
        }
    });
    // The Initial View's page, now that the pages have their real width: shown once this layout
    // pass is over (a scroll set during it is lost).
    if width > 0.0
        && height > 0.0
        && let Some(page) = update(tab_id, |v| v.pending_page.take()).flatten()
    {
        let weak = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(30), move || {
            let Some(ui) = weak.upgrade() else { return };
            update(tab_id, |v| v.show_page(page, None));
            request(&ui, tab_id);
        });
    }
    crate::reader_ui::viewport_changed(ui, tab_id);
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
        v.scroll_x = v.viewer.across().then(|| v.viewer.scroll_x_to(page.max(0) as usize));
    });
}

pub fn zoom_step(ui: &AppWindow, tab_id: i32, dir: i32) {
    // Steps of actual size (25 … 400 %), as PDF Expert; 0 is fit width.
    let Some(z) = with(|a| {
        a.tab(tab_id)?.viewer.as_deref().map(|v| {
            if dir == 0 {
                return 100;
            }
            let cur = v.current();
            let actual = v.viewer.actual_percent(cur).round() as u32;
            v.viewer.zoom_for_actual(cur, viewer::step_zoom(actual, dir) as f64)
        })
    }) else {
        return;
    };
    zoom_to(ui, tab_id, z);
    crate::reader_ui::zoom_changed(tab_id);
}

/// Pinch and ⌘ + wheel: `pct` percent of actual size (10–800), the point of the view (ax, ay)
/// staying on the same point of its page; `sx` is the horizontal scroll. Returns the new
/// horizontal scroll (the view sets it; the vertical one comes with the tab).
pub fn zoom_at(ui: &AppWindow, tab_id: i32, pct: f32, ax: f32, ay: f32, sx: f32) -> f32 {
    let z = with(|a| a.tab(tab_id)?.viewer.as_deref().map(|v| v.viewer.zoom_for_actual(v.current(), (pct as f64).clamp(10.0, 800.0))));
    let Some(z) = z else { return sx };
    let sx = zoom_to_at(ui, tab_id, z, (ax as f64, ay as f64, sx as f64)).unwrap_or(sx as f64);
    crate::reader_ui::zoom_changed(tab_id);
    sx as f32
}

/// The zoom asked for after a wheel step of `factor`: from the last one asked for while the zoom
/// settles (unrounded), else from the zoom shown; 10–800 %.
fn next_zoom_target(target: Option<f64>, shown: f64, factor: f64) -> f64 {
    (target.unwrap_or(shown) * factor).clamp(10.0, 800.0)
}

/// ⌘ + wheel: the zoom asked for so far times `factor`, around (ax, ay) as [`zoom_at`].
pub fn zoom_by(ui: &AppWindow, tab_id: i32, factor: f32, ax: f32, ay: f32, sx: f32) -> f32 {
    let pct = update(tab_id, |v| {
        let target = next_zoom_target(v.zoom_target, v.viewer.actual_percent(v.current()), factor as f64);
        v.zoom_target = Some(target);
        target
    });
    match pct {
        Some(p) => zoom_at(ui, tab_id, p as f32, ax, ay, sx),
        None => sx,
    }
}

/// Sets the zoom to `z` percent of the fit width, keeping the same part of the document in view;
/// pages render at the new size once the zoom has settled.
pub fn zoom_to(ui: &AppWindow, tab_id: i32, z: u32) {
    let at = with(|a| a.tab(tab_id)?.viewer.as_deref().map(ViewerUi::zoom_anchor)).unwrap_or_default();
    zoom_to_at(ui, tab_id, z, at);
}

/// [`zoom_to`], keeping the point (ax, ay) of the view, scrolled `sx` across, on the same point
/// of its page (page gaps do not scale with the zoom). Returns the new horizontal scroll.
fn zoom_to_at(ui: &AppWindow, tab_id: i32, z: u32, (ax, ay, sx): (f64, f64, f64)) -> Option<f64> {
    // Pages render at the new size once the zoom has settled (a pinch sends many changes); the
    // timer goes in with the zoom, so each change refreshes the tab once.
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    timer.start(slint::TimerMode::SingleShot, Duration::from_millis(150), move || {
        if let Some(ui) = weak.upgrade() {
            update(tab_id, |v| {
                v.zoom_settling = false;
                v.zoom_target = None;
                // The anchored scroll is sent again once the page sizes have settled: the view may
                // have cut it to a width it had in between.
                if v.scroll_x.is_some() {
                    v.scroll_seq += 1;
                }
            });
            request(&ui, tab_id);
        }
    });
    update(tab_id, move |v| {
        // A zoom of the user's own: the opening size no longer follows the width.
        v.pending_actual = None;
        v.pending_fit = false;
        let sx = v.zoom_around(z, (ax, ay, sx))?;
        v.zoom_timer = Some(timer);
        Some(sx)
    })
    .flatten()
}

pub fn pressed(tab_id: i32, x: f32, y: f32) {
    with(|a| {
        let Some(t) = a.tab(tab_id) else { return };
        let Some(v) = t.viewer.as_deref_mut() else { return };
        v.drag = v.viewer.hit(x as f64, y as f64).map(|(p, _, _)| (p, x as f64, y as f64));
        v.live = None;
        v.selecting = false;
        if t.tool == sheaf_core::tools::ToolId::Redact
            && let Some((page, ..)) = v.drag
        {
            let (_, px, py) = place_point(&v.viewer, page, x as f64, y as f64);
            // A third click soon after a double click, there: the line.
            if let Some((at, p, dx, dy)) = v.last_double.take()
                && p == page
                && at.elapsed() < std::time::Duration::from_millis(500)
                && (dx - x as f64).hypot(dy - y as f64) < 6.0
                && let Some(text) = v.text(page)
                && let Some(mark) = sheaf_app::reader::line(&text, (px as f32, py as f32)).and_then(|r| sheaf_app::reader::redact_range(&text, &v.viewer.boxes[page], page, r))
            {
                // It replaces the word the double click marked.
                if v.manual.last().is_some_and(|m| m.mark.page == page && m.label.starts_with('“')) {
                    v.manual.pop();
                }
                v.manual.push(Manual { label: quoted(&mark.text), mark });
                v.drag = None;
                return;
            }
            v.selecting = v.text(page).is_some_and(|t| t.glyphs.iter().any(|g| px as f32 >= g.rect[0] - 2.0 && px as f32 <= g.rect[2] + 2.0 && py as f32 >= g.rect[1] - 2.0 && py as f32 <= g.rect[3] + 2.0));
        }
        // Edit: a press on an item grabs it, to move it on release.
        if t.tool == sheaf_core::tools::ToolId::Edit
            && let Some((page, ..)) = v.drag
        {
            t.edit.press(page, crate::edit_ui::view_point(&v.viewer, page, x as f64, y as f64));
        }
    });
}

/// Least size of a dragged area, logical pixels: smaller is a click.
const MIN_DRAG: f64 = 4.0;

pub fn released(tab_id: i32, x: f32, y: f32) {
    let tool = with(|a| a.tab(tab_id).map(|t| t.tool));
    if tool == Some(sheaf_core::tools::ToolId::Edit) {
        // Click to place, drag an item to move it, on the page the pointer went down on.
        return with(|a| {
            let Some(t) = a.tab(tab_id) else { return };
            if let Some(v) = t.viewer.as_deref_mut()
                && let Some((page, ..)) = v.drag.take()
            {
                let p = crate::edit_ui::view_point(&v.viewer, page, x as f64, y as f64);
                t.edit.release(page, p, v.viewer.boxes[page].view_size());
            }
            refresh(a, tab_id);
        });
    }
    let sign = tool == Some(sheaf_core::tools::ToolId::Sign);
    update(tab_id, |v| {
        let Some((page, x0, y0)) = v.drag.take() else { return };
        if sign {
            // Click to place, drag to move: the signature is centred where the pointer is let go,
            // on the page it went down on, kept on that page.
            v.sign_at = Some(place_point(&v.viewer, page, x as f64, y as f64));
            return;
        }
        let (x1, y1) = (x as f64, y as f64);
        // A drag that starts on text selects it, as Acrobat marks text: its lines are marked.
        if (x1 - x0).hypot(y1 - y0) >= MIN_DRAG
            && let Some(text) = v.text(page)
        {
            let (a, b) = (place_point(&v.viewer, page, x0, y0), place_point(&v.viewer, page, x1, y1));
            v.live = None;
            v.selecting = false;
            if let Some(mark) = sheaf_app::reader::redact_selection(&text, &v.viewer.boxes[page], page, (a.1 as f32, a.2 as f32), (b.1 as f32, b.2 as f32)) {
                v.manual.push(Manual { label: quoted(&mark.text), mark });
                return;
            }
        }
        v.live = None;
        v.selecting = false;
        if (x1 - x0).abs() < MIN_DRAG || (y1 - y0).abs() < MIN_DRAG {
            return;
        }
        let a = v.viewer.to_page(page, x0, y0);
        let b = v.viewer.to_page(page, x1, y1);
        let rect = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
        v.manual.push(Manual { mark: redact::mark_rects(&[(page, rect)]).remove(0), label: "Selected area".into() });
    });
}

/// A list label for marked text: “its first 40 characters…”.
fn quoted(text: &str) -> String {
    let short: String = text.chars().take(40).collect();
    if short.len() < text.len() { format!("“{short}…”") } else { format!("“{short}”") }
}

/// Redact: the pointer moved with the button held; a drag that started on text shows what it
/// selects so far.
pub fn dragged(tab_id: i32, x: f32, y: f32) {
    let busy = with(|a| a.tab(tab_id).and_then(|t| t.viewer.as_deref()).is_none_or(|v| !v.selecting));
    if busy {
        return;
    }
    update(tab_id, |v| {
        let Some((page, x0, y0)) = v.drag else { return };
        let Some(text) = v.text(page) else { return };
        let (a, b) = (place_point(&v.viewer, page, x0, y0), place_point(&v.viewer, page, x as f64, y as f64));
        v.live = sheaf_app::reader::redact_selection(&text, &v.viewer.boxes[page], page, (a.1 as f32, a.2 as f32), (b.1 as f32, b.2 as f32));
    });
}

/// Redact: a double click on a word marks it (a third click, the line).
pub fn double_clicked(tab_id: i32, x: f32, y: f32) {
    update(tab_id, |v| {
        let Some((page, ..)) = v.viewer.hit(x as f64, y as f64) else { return };
        let Some(text) = v.text(page) else { return };
        let (_, px, py) = place_point(&v.viewer, page, x as f64, y as f64);
        if let Some(mark) = sheaf_app::reader::word(&text, (px as f32, py as f32)).and_then(|r| sheaf_app::reader::redact_range(&text, &v.viewer.boxes[page], page, r)) {
            // The drag the first click began marks nothing (it did not move).
            v.manual.push(Manual { label: quoted(&mark.text), mark });
            v.last_double = Some((std::time::Instant::now(), page, x as f64, y as f64));
        }
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

/// The queries for a typed text and the selected codes (",email,phone,"; "case" and "word" set
/// how the typed text matches).
pub fn queries(text: &str, patterns: &str) -> Vec<Query> {
    let mut q = Vec::new();
    let has = |c: &str| patterns.split(',').any(|p| p == c);
    if !text.trim().is_empty() {
        q.push(Query::Literal { text: text.trim().into(), case_insensitive: !has("case"), whole_word: has("word") });
    }
    q.extend(patterns.split(',').filter_map(pattern).map(Query::Pattern));
    q
}

pub fn search(ui: &AppWindow, tab_id: i32, text: &str, patterns: &str) {
    let queries = queries(text, patterns);
    let Some((load_id, seq, input)) = update(tab_id, |v| {
        // Matches still checked from an earlier search stay marked.
        for f in v.found.drain(..).filter(|f| f.checked) {
            v.manual.push(Manual { label: f.m.text.clone(), mark: redact::mark_text(std::slice::from_ref(&f.m)).remove(0) });
        }
        v.focused = None;
        v.searched = !queries.is_empty();
        v.busy = !queries.is_empty();
        v.search_seq += 1;
        (v.load_id, v.search_seq, v.input.clone())
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
                // The file was swapped, or a newer search started, meanwhile: these matches are stale.
                if v.load_id != load_id || v.search_seq != seq {
                    return None;
                }
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
        v.scroll_x = None;
    });
}

/// Whole pages marked for redaction (Acrobat's Redact Pages): those of `range` ("2-4, 7"), or the
/// page shown when it is empty; each is one mark of the list.
pub fn mark_page(ui: &AppWindow, tab_id: i32, range: &str) {
    let Some((load_id, input, page, count)) = update(tab_id, |v| (v.load_id, v.input.clone(), v.viewer.current(v.top, v.height), v.viewer.page_count())) else { return };
    let pages: Vec<usize> = if range.trim().is_empty() {
        vec![page]
    } else {
        match sheaf_core::ranges::parse_ranges(range, count as u32) {
            Ok(rs) => rs.into_iter().flatten().map(|p| p as usize - 1).collect(),
            Err(e) => return crate::flow::toast(ui, &format!("Pages: {e}")),
        }
    };
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let r = sheaf_app::jobs::catching(|| redact::mark_pages(&input, &pages));
        let _ = weak.upgrade_in_event_loop(move |ui| {
            // A mark of the file shown when the button was pressed, never of a file swapped in since.
            let current = update(tab_id, |v| v.load_id == load_id).unwrap_or(false);
            match r {
                Ok(m) if current => {
                    update(tab_id, |v| v.manual.extend(m.into_iter().map(|mark| Manual { label: format!("Whole page {}", mark.page + 1), mark })));
                }
                Err(e) if current => crate::flow::toast(&ui, &e.to_string()),
                _ => {}
            }
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

/// A mark of the list clicked (Acrobat's Redaction panel): the page shows it, a third down.
pub fn show_mark(tab_id: i32, k: i32) {
    update(tab_id, |v| {
        let k = k.max(0) as usize;
        let checked: Vec<usize> = v.found.iter().enumerate().filter(|(_, f)| f.checked).map(|(i, _)| i).collect();
        let (page, quads) = match checked.get(k) {
            Some(&i) => (v.found[i].m.page, v.found[i].m.quads.clone()),
            None => match v.manual.get(k - checked.len()) {
                Some(m) => (m.mark.page, m.mark.quads.clone()),
                None => return,
            },
        };
        // A whole page has no quads: its top.
        let focus = (!quads.is_empty()).then(|| redact::bounds(&quads));
        v.scroll = v.viewer.scroll_to(page, focus, v.height);
        v.scroll_seq += 1;
        v.scroll_x = None;
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
    let pages = v.pane_pages();
    let tl = v.viewer.thumb_layout();
    let thumbs: Vec<ViewerThumb> = v
        .wanted_thumbs()
        .map(|i| {
            let image = v.any_thumb(i).cloned();
            ViewerThumb { page: i as i32, y: len(tl[i].0), h: len(tl[i].1), ready: image.is_some(), image: image.unwrap_or_default(), selected: false }
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
    if let Some(m) = &v.live {
        boxes(m.page, &m.quads, 3);
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
    tab.viewer_selecting = v.selecting;
    tab.viewer_width = len(v.viewer.content_width());
    tab.viewer_height = len(v.viewer.content_height());
    tab.viewer_thumbs_height = len(viewer::content_height(&tl));
    tab.viewer_current = v.current() as i32;
    // Shown as the step it is (100 %, 200 %) when the integer zoom lands a hair off it.
    let actual = v.viewer.actual_percent(v.current());
    let step = viewer::ZOOMS.iter().copied().find(|z| (*z as f64 - actual).abs() <= (*z as f64) * 0.015);
    tab.viewer_zoom = step.map(|z| z as f64).unwrap_or(actual).round() as i32;
    tab.viewer_scroll = len(v.scroll);
    tab.viewer_scroll_x = len(v.scroll_x.unwrap_or(-1.0));
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

    #[test]
    fn a_second_wheel_step_before_the_view_scrolls_keeps_the_point() {
        let pages = (0..3).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        let (ax, ay) = (800.0, 300.0);
        let anchor = v.viewer.anchor_of(ax, ay);
        let sx = v.zoom_around(130, (ax, ay, 0.0)).unwrap();
        assert!(sx > 0.0);
        // The second step arrives with the view still scrolled 0 across.
        let sx = v.zoom_around(170, (ax, ay, 0.0)).unwrap();
        let back = v.viewer.anchor_of(sx + ax, v.top + ay);
        assert!(back.0 == anchor.0 && (back.1 - anchor.1).abs() < 1e-6 && (back.2 - anchor.2).abs() < 1e-6, "{anchor:?} → {back:?}");
    }

    #[test]
    fn a_destination_brings_its_zoom_its_top_and_its_left_edge() {
        let pages = (0..3).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        // /XYZ 200 300 3: page 2 at 300 %, the point (200, 300) near the view's top left.
        assert!(v.show_dest(1, &render::DestView::Xyz { left: Some(200.0), top: Some(300.0), zoom: Some(3.0) }));
        assert!((v.viewer.actual_percent(1) - 300.0).abs() < 1.0, "{}", v.viewer.actual_percent(1));
        let c = v.viewer.overlay(1, [200.0, 299.0, 201.0, 300.0]);
        let sx = v.scroll_x.expect("scrolled across");
        assert!((c[0] - sx - 8.0).abs() < 1.0 && c[1] > v.scroll && c[1] - v.scroll < 700.0 / 2.0, "{c:?} {sx} {}", v.scroll);
        // /Fit: the whole page in the view; /FitH: the width.
        assert!(v.show_dest(0, &render::DestView::Fit));
        assert_eq!(v.viewer.zoom(), v.viewer.fit_page_zoom(v.height, 0..1));
        assert!(v.show_dest(0, &render::DestView::FitH { top: Some(500.0) }) && v.viewer.zoom() == 100);
        // /FitR: the rectangle fills the view, its left edge at the view's left.
        assert!(v.show_dest(2, &render::DestView::FitR { rect: [100.0, 300.0, 500.0, 500.0] }));
        let r = v.viewer.overlay(2, [100.0, 300.0, 500.0, 500.0]);
        assert!(r[2] <= v.viewer.view_width() && r[3] <= 700.0 && (r[2] > 0.8 * v.viewer.view_width() || r[3] > 0.8 * 700.0), "{r:?}");
        assert!(r[1] >= v.scroll && r[1] + r[3] <= v.scroll + 700.0, "{r:?} {}", v.scroll);
        // A word's box: no further than 400 % of actual size (pages any larger are too big to draw).
        v.show_dest(2, &render::DestView::FitR { rect: [100.0, 400.0, 120.0, 405.0] });
        assert!((v.viewer.actual_percent(2) - 400.0).abs() < 1.0, "{}", v.viewer.actual_percent(2));
        // No zoom, no left: the zoom stays, no horizontal scroll asked for.
        let z = v.viewer.zoom();
        assert!(!v.show_dest(0, &render::DestView::Xyz { left: None, top: Some(500.0), zoom: None }));
        assert_eq!((v.viewer.zoom(), v.scroll_x), (z, None));
    }

    #[test]
    fn a_rectangle_on_a_turned_page_comes_into_view() {
        // A page turned a quarter (/Rotate 90): a small rectangle high on it in user space shows far to
        // the right; /FitR scrolls across to it (Sol, 26/10).
        let pages = (0..2).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 90, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        let rect = [400.0, 700.0, 450.0, 780.0];
        v.show_dest(1, &render::DestView::FitR { rect: rect.map(|c| c as f32) });
        let r = v.viewer.overlay(1, rect);
        let sx = v.scroll_x.expect("scrolled across");
        assert!(r[0] >= sx && r[0] + r[2] <= sx + v.viewer.view_width(), "{r:?} {sx}");
        assert!(r[1] >= v.scroll && r[1] + r[3] <= v.scroll + 700.0, "{r:?} {}", v.scroll);
    }

    #[test]
    fn a_place_keeps_how_far_across_a_wide_page_was_scrolled() {
        let pages = (0..3).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        v.viewer.set_zoom(250);
        (v.top, v.left) = (v.viewer.page_rect(1)[1] + 300.0, 600.0);
        let place = v.place();
        (v.top, v.left) = (0.0, 0.0);
        v.go_to_place(place);
        assert!(v.scroll_x.is_some_and(|x| (x - 600.0).abs() < 1.0), "{:?}", v.scroll_x);
    }

    #[test]
    fn one_page_at_a_time_shows_the_page_gone_to_at_any_zoom() {
        let pages = (0..3).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        // Zoomed out: page 1 at the top, the view's middle in page 2 (Sol, 26/10: page 2 was shown).
        v.viewer.set_zoom(25);
        v.single_page = Some(0);
        v.show_page(0, None);
        v.top = v.scroll;
        assert_eq!(v.current(), 0);
        v.show_page(2, None);
        v.top = v.scroll;
        assert_eq!(v.current(), 2, "the last page, though the view cannot scroll its top up to it");
    }

    #[test]
    fn a_place_in_a_page_is_found_again_after_a_zoom() {
        let pages = (0..3).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        v.top = v.viewer.page_rect(1)[1] + 400.0;
        let place = v.place();
        assert_eq!(place.0, 1);
        assert!(place.2 > 0.2 && place.2 < 0.5, "{place:?}");
        v.viewer.set_zoom(200);
        v.go_to_place(place);
        v.top = v.scroll;
        let back = v.place();
        assert!(back.0 == 1 && (back.2 - place.2).abs() < 1e-6, "{place:?} → {back:?}");
    }

    #[test]
    fn small_wheel_steps_add_up_while_the_zoom_settles() {
        // Fifty trackpad steps of ×1.002 from 100 %: each alone would round away.
        let mut t = None;
        for _ in 0..50 {
            t = Some(next_zoom_target(t, 100.0, 1.002));
        }
        assert!((t.unwrap() - 100.0 * 1.002f64.powi(50)).abs() < 1e-9, "{t:?}");
        assert_eq!(next_zoom_target(None, 700.0, 1.25), 800.0);
        assert_eq!(next_zoom_target(Some(12.0), 500.0, 0.8), 10.0);
    }
    use sheaf_app::viewer::PageBox;

    #[test]
    fn a_click_places_the_signature_centred_and_kept_on_the_page() {
        // A4 portrait and the same page turned a quarter (shown landscape).
        let mut v = Viewer::new(vec![PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }, PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 90, unit: 1.0 }]);
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
    fn match_case_and_whole_words_reach_the_query() {
        assert_eq!(queries("Secret", ",case,word,"), vec![Query::Literal { text: "Secret".into(), case_insensitive: false, whole_word: true }]);
    }

    #[test]
    fn typed_text_and_chips_become_queries() {
        assert_eq!(
            queries(" secret ", ",email,card,"),
            vec![
                Query::Literal { text: "secret".into(), case_insensitive: true, whole_word: false },
                Query::Pattern(Pattern::Email),
                Query::Pattern(Pattern::CreditCard)
            ]
        );
        assert!(queries("  ", ",,").is_empty());
    }

    /// Pages across: a jump centres the page across (the vertical scroll alone left it where it was),
    /// and the pages rendered follow the horizontal scroll.
    #[test]
    fn pages_across_are_gone_to_and_rendered_from_the_horizontal_scroll() {
        let pages = (0..30).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        v.show_page(12, None);
        assert_eq!(v.scroll_x, None, "down a column, nothing across");
        v.viewer.set_horizontal(true);
        v.show_page(12, None);
        let sx = v.scroll_x.expect("scrolled across");
        assert_eq!(sx, v.viewer.scroll_x_to(12));
        view_left_of(&mut v, sx);
        assert_eq!(v.current(), 12);
        let shown: Vec<i32> = v.pane_pages().iter().map(|p| p.page).collect();
        assert!(shown.contains(&12) && !shown.contains(&0), "{shown:?}");
    }

    /// ⌘+ deep in a strip of pages across stays on the page in view (Sol, 27/10: it jumped back
    /// toward page 1, the zoom anchored on the strip's start).
    #[test]
    fn a_zoom_deep_in_a_strip_stays_on_the_page_in_view() {
        let pages = (0..30).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        v.viewer.set_horizontal(true);
        v.show_page(20, None);
        let sx = v.scroll_x.unwrap();
        view_left_of(&mut v, sx);
        assert_eq!(v.current(), 20);
        for z in [150, 60, 100] {
            let anchor = v.zoom_anchor();
            let sx = v.zoom_around(z, anchor).expect("the zoom changed");
            view_left_of(&mut v, sx);
            assert_eq!(v.current(), 20, "at {z} %");
            v.zoom_settling = false;
        }
        // The page grid keeps the setting but scrolls down: nothing across.
        v.viewer.set_spread(3);
        assert_eq!(v.zoom_anchor().0, 0.0);
        v.show_page(10, None);
        assert_eq!(v.scroll_x, None);
    }

    /// Pages across: a window or side panel resized deep in a strip keeps the page in view (Sol, 27/10).
    #[test]
    fn a_resize_deep_in_a_strip_keeps_the_page_in_view() {
        let pages = (0..30).map(|_| viewer::PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut viewer = Viewer::new(pages);
        viewer.set_width(1000.0);
        let mut v = ViewerUi::new(Input::new("x.pdf"), viewer, 1.0, (1000.0, 0.0, 700.0), (0.0, 0.0));
        v.viewer.set_horizontal(true);
        v.show_page(22, None);
        view_left_of(&mut v, v.scroll_x.unwrap());
        for (w, h) in [(760.0, 700.0), (1240.0, 700.0), (1240.0, 900.0), (1000.0, 700.0)] {
            v.resize(w, v.top, h);
            assert_eq!(v.current(), 22, "at {w} × {h}");
        }
        // Down a column a resize is as before: the scroll asked for is left alone.
        v.viewer.set_horizontal(false);
        let seq = v.scroll_seq;
        v.resize(800.0, v.top, 700.0);
        assert_eq!(v.scroll_seq, seq);
    }

    /// What [`view_left`] does to a tab's viewer.
    fn view_left_of(v: &mut ViewerUi, left: f64) {
        v.left = left;
        v.viewer.set_left(left);
    }
}
