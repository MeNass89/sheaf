//! Page viewer logic with no UI types: page layout in a vertical scroll, which pages to
//! render, the render cache, and the mapping between view pixels and PDF user space.
//!
//! The view of a page is what the user sees: the page's crop box turned by its `/Rotate`,
//! `render` pixels wide and high, origin top left, y down. PDF user space has y up and its
//! origin wherever the crop box puts it.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::ops::Range;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use sheaf_core::pdfcraft::render;

/// What the mapping needs to know about a page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBox {
    /// Crop box in user space `[x0, y0, x1, y1]` (the visible region).
    pub crop: [f64; 4],
    /// Clockwise `/Rotate`: 0, 90, 180 or 270.
    pub rotation: u16,
    /// `/UserUnit`: points per user-space unit (1 on almost every page).
    pub unit: f64,
}

impl PageBox {
    /// Displayed size in points, after `/Rotate` and `/UserUnit` (width and height swap on
    /// quarter turns): the view space Sign and Edit placements are given in, as PdfCraft's
    /// page info reads it.
    pub fn view_size(&self) -> (f64, f64) {
        let (w, h) = ((self.crop[2] - self.crop[0]) * self.unit, (self.crop[3] - self.crop[1]) * self.unit);
        if self.rotation % 180 == 90 { (h, w) } else { (w, h) }
    }
}

/// A point of the page as shown `render` pixels large (`(width, height)`, y down) → PDF user
/// space.
pub fn view_to_pdf(page: &PageBox, render: (f64, f64), x: f64, y: f64) -> (f64, f64) {
    let [cx0, cy0, cx1, cy1] = page.crop;
    // Fractions of the displayed page, then of the unrotated crop box (u right, v down).
    let (a, b) = (x / render.0.max(1e-9), y / render.1.max(1e-9));
    let (u, v) = match page.rotation {
        90 => (b, 1.0 - a),
        180 => (1.0 - a, 1.0 - b),
        270 => (1.0 - b, a),
        _ => (a, b),
    };
    (cx0 + u * (cx1 - cx0), cy1 - v * (cy1 - cy0))
}

/// The inverse of [`view_to_pdf`].
pub fn pdf_to_view(page: &PageBox, render: (f64, f64), x: f64, y: f64) -> (f64, f64) {
    let [cx0, cy0, cx1, cy1] = page.crop;
    let (u, v) = ((x - cx0) / (cx1 - cx0).max(1e-9), (cy1 - y) / (cy1 - cy0).max(1e-9));
    let (a, b) = match page.rotation {
        90 => (1.0 - v, u),
        180 => (1.0 - u, 1.0 - v),
        270 => (v, 1.0 - u),
        _ => (u, v),
    };
    (a * render.0, b * render.1)
}

/// A user-space rectangle `[x0, y0, x1, y1]` as a view rectangle `[x, y, w, h]` (y down).
pub fn pdf_rect_to_view(page: &PageBox, render: (f64, f64), r: [f64; 4]) -> [f64; 4] {
    let a = pdf_to_view(page, render, r[0], r[1]);
    let b = pdf_to_view(page, render, r[2], r[3]);
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    [x0, y0, x1 - x0, y1 - y0]
}

/// Two view points (a drag) as a normalised user-space rectangle.
pub fn view_rect_to_pdf(page: &PageBox, render: (f64, f64), a: (f64, f64), b: (f64, f64)) -> [f64; 4] {
    let p = view_to_pdf(page, render, a.0, a.1);
    let q = view_to_pdf(page, render, b.0, b.1);
    [p.0.min(q.0), p.1.min(q.1), p.0.max(q.0), p.1.max(q.1)]
}

/// Gap between pages, in logical pixels.
pub const PAGE_GAP: f64 = 16.0;

/// Top and height of each page when every page is `width` wide (logical pixels), stacked with
/// [`PAGE_GAP`] above, between and below them.
pub fn layout(sizes: &[(f64, f64)], width: f64) -> Vec<(f64, f64)> {
    let mut y = PAGE_GAP;
    sizes
        .iter()
        .map(|(w, h)| {
            let height = width * h / w.max(1e-9);
            let top = y;
            y += height + PAGE_GAP;
            (top, height)
        })
        .collect()
}

/// Total height of a [`layout`].
pub fn content_height(layout: &[(f64, f64)]) -> f64 {
    layout.last().map(|(y, h)| y + h + PAGE_GAP).unwrap_or(PAGE_GAP)
}

/// Pages within one screen above and below the viewport `[top, top + height)`: the pages
/// worth rendering.
pub fn wanted(layout: &[(f64, f64)], top: f64, height: f64) -> Range<usize> {
    let (lo, hi) = (top - height, top + 2.0 * height);
    let first = layout.iter().position(|(y, h)| y + h >= lo).unwrap_or(layout.len());
    let end = layout.iter().rposition(|(y, _)| *y <= hi).map(|i| i + 1).unwrap_or(0);
    first..end.max(first)
}

/// The page under the middle of the viewport (the one the strip highlights).
pub fn current(layout: &[(f64, f64)], top: f64, height: f64) -> usize {
    let mid = top + height / 2.0;
    layout.iter().rposition(|(y, _)| *y <= mid).unwrap_or(0)
}

/// Zoom steps offered by ⌘+ / ⌘− (percent of the fit width), PDF Expert's range.
pub const ZOOMS: [u32; 12] = [25, 33, 50, 67, 80, 100, 125, 150, 200, 250, 300, 400];
pub const MIN_ZOOM: u32 = 25;
pub const MAX_ZOOM: u32 = 400;

/// The next zoom step up (`dir` > 0) or down from `zoom`, within 25–400 %.
pub fn step_zoom(zoom: u32, dir: i32) -> u32 {
    if dir > 0 {
        ZOOMS.iter().copied().find(|z| *z > zoom).unwrap_or(MAX_ZOOM)
    } else {
        ZOOMS.iter().rev().copied().find(|z| *z < zoom).unwrap_or(MIN_ZOOM)
    }
}

/// Most page renders kept at once, whatever their size. Memory is bounded by [`CACHE_BYTES`]
/// (96 MB), which in practice evicts long before this count is reached; the cap only matters
/// for small renders (low zoom, small pages). Thumbnails have their own cap.
pub const CACHE_CAP: usize = 40;

/// A least-recently-used cache capped at `cap` entries.
pub struct Lru<K, V> {
    cap: usize,
    map: HashMap<K, V>,
    /// Keys from least to most recently used.
    order: VecDeque<K>,
}

impl<K: Hash + Eq + Clone, V> Lru<K, V> {
    pub fn new(cap: usize) -> Self {
        Self { cap: cap.max(1), map: HashMap::new(), order: VecDeque::new() }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn touch(&mut self, k: &K) {
        if let Some(i) = self.order.iter().position(|x| x == k) {
            let k = self.order.remove(i).expect("position is in range");
            self.order.push_back(k);
        }
    }

    /// The value for `k`, now the most recently used.
    pub fn get(&mut self, k: &K) -> Option<&V> {
        if self.map.contains_key(k) {
            self.touch(k);
        }
        self.map.get(k)
    }

    pub fn contains(&self, k: &K) -> bool {
        self.map.contains_key(k)
    }

    /// The value for `k`, without making it recently used.
    pub fn peek(&self, k: &K) -> Option<&V> {
        self.map.get(k)
    }

    /// Every key, least recently used first.
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> {
        self.order.iter()
    }

    /// Inserts `v` as the most recently used and returns what it replaced or evicted to stay at
    /// the cap.
    pub fn put(&mut self, k: K, v: V) -> Vec<(K, V)> {
        let mut evicted = Vec::new();
        if let Some(old) = self.map.insert(k.clone(), v) {
            self.touch(&k);
            evicted.push((k, old));
        } else {
            self.order.push_back(k);
        }
        while self.map.len() > self.cap {
            let Some(old) = self.order.pop_front() else { break };
            if let Some(v) = self.map.remove(&old) {
                evicted.push((old, v));
            }
        }
        evicted
    }

    /// Drops every entry for which `keep` is false; returns what was dropped.
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) -> Vec<(K, V)> {
        let gone: Vec<K> = self.order.iter().filter(|k| !keep(k)).cloned().collect();
        self.order.retain(|k| !gone.contains(k));
        gone.into_iter().filter_map(|k| self.map.remove(&k).map(|v| (k, v))).collect()
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    /// Removes and returns the least recently used entry.
    pub fn pop_oldest(&mut self) -> Option<(K, V)> {
        let k = self.order.pop_front()?;
        self.map.remove(&k).map(|v| (k, v))
    }
}

/// Byte budget for page renders on top of the [`CACHE_CAP`] count. At fit width on a Retina
/// screen one A4 page is about 15 MB (60 MB at 200 %), so 40 of them would not fit the memory a
/// viewer may use; 96 MB still holds the 4–6 pages within a screen of the viewport.
pub const CACHE_BYTES: usize = 96 << 20;

/// Gives memory freed by the allocator back to the system. A page render allocates and frees
/// large buffers; macOS keeps freed large blocks mapped (and counted against the app) until
/// asked, so after a few hundred pages the app would look like it holds a gigabyte.
pub fn release_free_memory() {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
        }
        // SAFETY: a null zone means every zone, and a goal of 0 means as much as possible;
        // the call only returns free pages to the system.
        unsafe {
            malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
        }
    }
}

/// Most pixels one page render may have: 4 M pixels (16 MB), an A4 page about 1680 × 2380.
pub const MAX_RENDER_PIXELS: u64 = 4_000_000;

/// A redaction mark, search match or other box drawn over a page, in PDF user space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayRect {
    pub page: usize,
    pub rect: [f64; 4],
    pub kind: i32,
}

/// Width of a thumbnail in the strip, logical pixels.
pub const THUMB_WIDTH: f64 = 84.0;
/// Narrowest a page is ever drawn, logical pixels.
const MIN_PAGE_WIDTH: f64 = 120.0;

/// Where every page of one document sits in the viewer's scrolling content (logical pixels,
/// y down), for a viewport `fit_width` wide at `zoom` percent of the fit width.
#[derive(Debug, Clone)]
pub struct Viewer {
    pub boxes: Vec<PageBox>,
    fit_width: f64,
    zoom: u32,
    layout: Vec<(f64, f64)>,
    page_width: f64,
}

impl Viewer {
    pub fn new(boxes: Vec<PageBox>) -> Self {
        let mut v = Self { boxes, fit_width: 600.0, zoom: 100, layout: Vec::new(), page_width: 0.0 };
        v.relayout();
        v
    }

    fn relayout(&mut self) {
        self.page_width = ((self.fit_width - 2.0 * PAGE_GAP) * self.zoom as f64 / 100.0).max(MIN_PAGE_WIDTH).round();
        let sizes: Vec<(f64, f64)> = self.boxes.iter().map(PageBox::view_size).collect();
        self.layout = layout(&sizes, self.page_width);
    }

    /// The viewport is `width` logical pixels wide; returns whether the layout changed.
    pub fn set_width(&mut self, width: f64) -> bool {
        let width = width.max(MIN_PAGE_WIDTH + 2.0 * PAGE_GAP).round();
        let changed = width != self.fit_width;
        self.fit_width = width;
        self.relayout();
        changed
    }

    pub fn set_zoom(&mut self, zoom: u32) -> bool {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let changed = zoom != self.zoom;
        self.zoom = zoom;
        self.relayout();
        changed
    }

    pub fn zoom(&self) -> u32 {
        self.zoom
    }

    /// The zoom (percent of the fit width) at which the tallest page of `pages` fits a viewport
    /// `height` high: the reader's Fit page.
    pub fn fit_page_zoom(&self, height: f64, pages: Range<usize>) -> u32 {
        let fit_w = (self.fit_width - 2.0 * PAGE_GAP).max(1.0);
        let ratio = pages.filter_map(|i| self.boxes.get(i)).map(|b| { let (w, h) = b.view_size(); h / w.max(1e-9) }).fold(0.0, f64::max);
        if ratio <= 0.0 {
            return 100;
        }
        let want_w = (height - 2.0 * PAGE_GAP).max(1.0) / ratio;
        ((want_w / fit_w * 100.0).floor() as u32).clamp(MIN_ZOOM, 100)
    }

    pub fn page_count(&self) -> usize {
        self.boxes.len()
    }

    pub fn layout(&self) -> &[(f64, f64)] {
        &self.layout
    }

    /// Width of the scrolling content: wider than the viewport when zoomed in.
    pub fn content_width(&self) -> f64 {
        self.fit_width.max(self.page_width + 2.0 * PAGE_GAP)
    }

    pub fn content_height(&self) -> f64 {
        content_height(&self.layout)
    }

    /// Page `i` in content coordinates: `[x, y, width, height]`, centred.
    pub fn page_rect(&self, i: usize) -> [f64; 4] {
        let (y, h) = self.layout.get(i).copied().unwrap_or((0.0, 0.0));
        [((self.content_width() - self.page_width) / 2.0).round(), y, self.page_width, h]
    }

    /// Device pixels page `i` is rendered wide at `scale` device pixels per logical pixel,
    /// at most [`MAX_RENDER_PIXELS`] in all (a zoomed-in page is then drawn slightly enlarged).
    pub fn render_px(&self, i: usize, scale: f64) -> u32 {
        let want = self.page_width * scale;
        let (w, h) = self.boxes.get(i).map(PageBox::view_size).unwrap_or((1.0, 1.0));
        let cap = (MAX_RENDER_PIXELS as f64 * w / h.max(1e-9)).sqrt();
        want.min(cap).round().max(1.0) as u32
    }

    /// The page under a content point and the point in its PDF user space.
    pub fn hit(&self, x: f64, y: f64) -> Option<(usize, f64, f64)> {
        (0..self.page_count()).find_map(|i| {
            let [px, py, w, h] = self.page_rect(i);
            (x >= px && x <= px + w && y >= py && y <= py + h).then(|| {
                let (ux, uy) = view_to_pdf(&self.boxes[i], (w, h), x - px, y - py);
                (i, ux, uy)
            })
        })
    }

    /// A content point as a point of page `i`, held to the page's edges (a drag that leaves
    /// the page it started on).
    pub fn to_page(&self, i: usize, x: f64, y: f64) -> (f64, f64) {
        let [px, py, w, h] = self.page_rect(i);
        view_to_pdf(&self.boxes[i], (w, h), (x - px).clamp(0.0, w), (y - py).clamp(0.0, h))
    }

    /// A user-space rectangle of page `i` in content coordinates `[x, y, w, h]`.
    pub fn overlay(&self, i: usize, r: [f64; 4]) -> [f64; 4] {
        let [px, py, w, h] = self.page_rect(i);
        let v = pdf_rect_to_view(&self.boxes[i], (w, h), r);
        [px + v[0], py + v[1], v[2], v[3]]
    }

    /// Pages worth rendering for the viewport `[top, top + height)`.
    pub fn wanted(&self, top: f64, height: f64) -> Range<usize> {
        wanted(&self.layout, top, height)
    }

    pub fn current(&self, top: f64, height: f64) -> usize {
        current(&self.layout, top, height)
    }

    /// Scroll position that shows page `i` at the top (with its gap above it), or the point
    /// `y_pt` of it near the top third of a viewport `height` high when given.
    pub fn scroll_to(&self, i: usize, focus: Option<[f64; 4]>, height: f64) -> f64 {
        let top = match focus {
            Some(r) => self.overlay(i, r)[1] - height / 3.0,
            None => self.page_rect(i)[1] - PAGE_GAP,
        };
        top.clamp(0.0, (self.content_height() - height).max(0.0))
    }

    /// The thumbnail strip's layout ([`THUMB_WIDTH`] wide pages).
    pub fn thumb_layout(&self) -> Vec<(f64, f64)> {
        layout(&self.boxes.iter().map(PageBox::view_size).collect::<Vec<_>>(), THUMB_WIDTH)
    }
}

/// One render: document `doc` (a load id), page, pixel width, thumbnail or page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderKey {
    pub doc: u64,
    pub page: usize,
    pub width: u32,
    pub thumb: bool,
    /// Revision of the page's look (an edit, a page theme): a newer one replaces older renders.
    pub rev: u64,
}

type Task = Box<dyn FnOnce() + Send>;

#[derive(Default)]
struct QueueState {
    tasks: VecDeque<(RenderKey, Task)>,
    running: HashSet<RenderKey>,
    peak: usize,
}

/// Page renders for the viewer on `size` threads of their own, so scrolling never waits
/// behind file probes or jobs. Queued renders the viewer no longer wants can be dropped
/// ([`Self::retain`]); a render already queued or running is not queued twice.
pub struct RenderQueue {
    inner: Arc<(Mutex<QueueState>, Condvar)>,
}

impl RenderQueue {
    pub fn new(size: usize) -> Self {
        let inner: Arc<(Mutex<QueueState>, Condvar)> = Arc::default();
        for i in 0..size.max(1) {
            let inner = inner.clone();
            std::thread::Builder::new()
                .name(format!("sheaf-render-{i}"))
                .spawn(move || loop {
                    let (key, task) = {
                        let (m, cv) = &*inner;
                        let mut st = m.lock().unwrap();
                        loop {
                            if let Some(t) = st.tasks.pop_front() {
                                st.running.insert(t.0);
                                let n = st.running.len();
                                st.peak = st.peak.max(n);
                                break t;
                            }
                            st = cv.wait(st).unwrap();
                        }
                    };
                    // A crashing render must not kill the worker.
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task));
                    let idle = {
                        let mut st = inner.0.lock().unwrap();
                        st.running.remove(&key);
                        st.tasks.is_empty()
                    };
                    if idle {
                        release_free_memory();
                    }
                })
                .expect("spawn render worker");
        }
        Self { inner }
    }

    /// Queues `task` under `key` unless that render is already queued or running. Returns
    /// whether it was queued.
    pub fn submit(&self, key: RenderKey, task: impl FnOnce() + Send + 'static) -> bool {
        let (m, cv) = &*self.inner;
        let mut st = m.lock().unwrap();
        if st.running.contains(&key) || st.tasks.iter().any(|(k, _)| *k == key) {
            return false;
        }
        st.tasks.push_back((key, Box::new(task)));
        cv.notify_one();
        true
    }

    /// Drops the queued renders `keep` refuses (running ones finish).
    pub fn retain(&self, mut keep: impl FnMut(&RenderKey) -> bool) {
        self.inner.0.lock().unwrap().tasks.retain(|(k, _)| keep(k));
    }

    /// Renders queued or running.
    pub fn pending(&self) -> usize {
        let st = self.inner.0.lock().unwrap();
        st.tasks.len() + st.running.len()
    }

    /// Most renders that ever ran at once.
    pub fn peak(&self) -> usize {
        self.inner.0.lock().unwrap().peak
    }
}

/// Renders running at once at most.
pub const RENDER_THREADS: usize = 3;

/// The app-wide viewer render queue.
pub fn renders() -> &'static RenderQueue {
    static Q: OnceLock<RenderQueue> = OnceLock::new();
    Q.get_or_init(|| RenderQueue::new(RENDER_THREADS))
}

/// A document's pages as the viewer needs them: displayed sizes, boxes and rotations. Reads
/// the file's structure only (no page is drawn).
pub fn page_boxes(bytes: &Arc<Vec<u8>>, password: Option<&str>) -> sheaf_core::Result<Vec<PageBox>> {
    let info = render::inspect(bytes.clone(), password).map_err(|e| sheaf_core::OpdfError::Engine(e.to_string()))?;
    Ok(boxes_of(&info.pages))
}

/// The viewer's page boxes from PdfCraft's page list (a document already inspected).
pub fn boxes_of(pages: &[render::PageInfo]) -> Vec<PageBox> {
    pages
        .iter()
        .map(|p| {
            let crop = p.crop.map(f64::from);
            let rotation = p.rotation % 360;
            // PdfCraft gives the displayed size with /UserUnit applied; the unit is its ratio
            // to the crop box.
            let across = if rotation % 180 == 90 { crop[3] - crop[1] } else { crop[2] - crop[0] };
            let unit = if across > 1e-9 && p.width > 0.0 { f64::from(p.width) / across } else { 1.0 };
            PageBox { crop, rotation, unit }
        })
        .collect()
}

/// Page `page` of `input` rendered `width` pixels wide, as RGBA.
///
/// Rasters come from pdfium (under its lock), the document opened for each render and closed
/// after. PdfCraft's renderer was measured first: on a picture-heavy 7-page guide it held
/// 600–800 MB once a few pages were drawn (its decoded-image caches stay with the process),
/// against about 200 MB for pdfium across all 301 pages of a 43-copy merge. The page boxes the
/// overlays are mapped with still come from PdfCraft ([`page_boxes`]), as do the redactions.
pub fn render_page(input: &sheaf_core::pages::Input, page: usize, width: u32) -> Result<slint::SharedPixelBuffer<slint::Rgba8Pixel>, String> {
    use pdfium_render::prelude::PdfRenderConfig;
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open().map_err(|e| e.to_string())?;
    let p = doc.pages().get(page as i32).map_err(|e| e.to_string())?;
    let bmp = p.render_with_config(&PdfRenderConfig::new().set_target_width(width as i32)).map_err(|e| e.to_string())?;
    let (w, h) = (bmp.width() as u32, bmp.height() as u32);
    Ok(slint::SharedPixelBuffer::clone_from_slice(&bmp.as_rgba_bytes(), w, h))
}

/// Page `page` of a PDF held in memory (`bytes`, opened with `password`) rendered `width` pixels
/// wide, as RGBA, then recoloured for `theme`: the reader's pages, which show edits not saved yet.
pub fn render_bytes(
    bytes: &[u8],
    password: Option<&str>,
    page: usize,
    width: u32,
    theme: crate::reader::PageTheme,
) -> Result<slint::SharedPixelBuffer<slint::Rgba8Pixel>, String> {
    use pdfium_render::prelude::PdfRenderConfig;
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = sheaf_core::engine::open_pdf_bytes(bytes, password).map_err(|e| e.to_string())?;
    let p = doc.pages().get(page as i32).map_err(|e| e.to_string())?;
    let config = PdfRenderConfig::new().set_target_width(width as i32).render_form_data(true).render_annotations(true);
    let bmp = p.render_with_config(&config).map_err(|e| e.to_string())?;
    let (w, h) = (bmp.width() as u32, bmp.height() as u32);
    let mut rgba = bmp.as_rgba_bytes();
    crate::reader::tint(&mut rgba, theme);
    Ok(slint::SharedPixelBuffer::clone_from_slice(&rgba, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: [f64; 4] = [0.0, 0.0, 595.0, 842.0];

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn view_points_map_to_user_space_for_every_rotation() {
        // A crop box away from the origin, as cropped pages have.
        let crop = [10.0, 20.0, 110.0, 220.0]; // 100 × 200 pt
        // The view's top-left corner, for each rotation (rendered at 2 px per point).
        let cases = [(0u16, (10.0, 220.0)), (90, (10.0, 20.0)), (180, (110.0, 20.0)), (270, (110.0, 220.0))];
        for (rotation, top_left) in cases {
            let p = PageBox { crop, rotation, unit: 1.0 };
            let (vw, vh) = p.view_size();
            let render = (vw * 2.0, vh * 2.0);
            assert!(close(view_to_pdf(&p, render, 0.0, 0.0), top_left), "rot {rotation}: top-left");
            // The centre is the centre whatever the rotation.
            assert!(close(view_to_pdf(&p, render, render.0 / 2.0, render.1 / 2.0), (60.0, 120.0)), "rot {rotation}: centre");
            // Round trip on an arbitrary point.
            let (x, y) = view_to_pdf(&p, render, 37.0, 91.0);
            assert!(close(pdf_to_view(&p, render, x, y), (37.0, 91.0)), "rot {rotation}: inverse");
        }
        // Quarter turns swap the displayed size.
        assert_eq!(PageBox { crop, rotation: 90, unit: 1.0 }.view_size(), (200.0, 100.0));
        assert_eq!(PageBox { crop, rotation: 180, unit: 1.0 }.view_size(), (100.0, 200.0));
    }

    #[test]
    fn a_point_one_step_right_on_screen_moves_the_right_way_in_user_space() {
        let render = (100.0, 100.0);
        let step = |rotation| {
            let p = PageBox { crop: [0.0, 0.0, 100.0, 100.0], rotation, unit: 1.0 };
            let a = view_to_pdf(&p, render, 50.0, 50.0);
            let b = view_to_pdf(&p, render, 60.0, 50.0);
            (b.0 - a.0, b.1 - a.1)
        };
        // Unrotated, right is +x. Turned 90° clockwise, the page's left edge is at the top of
        // the screen, so moving right on screen goes up the page: +y.
        assert!(close(step(0), (10.0, 0.0)));
        assert!(close(step(90), (0.0, 10.0)));
        assert!(close(step(180), (-10.0, 0.0)));
        assert!(close(step(270), (0.0, -10.0)));
    }

    #[test]
    fn rectangles_round_trip() {
        for rotation in [0, 90, 180, 270] {
            let p = PageBox { crop: A4, rotation, unit: 1.0 };
            let (vw, vh) = p.view_size();
            let r = view_rect_to_pdf(&p, (vw, vh), (100.0, 40.0), (20.0, 300.0));
            let v = pdf_rect_to_view(&p, (vw, vh), r);
            assert!(close((v[0], v[1]), (20.0, 40.0)) && close((v[2], v[3]), (80.0, 260.0)), "rot {rotation}: {v:?}");
        }
    }

    #[test]
    fn pages_stack_with_gaps_and_only_nearby_ones_are_wanted() {
        let sizes = vec![(595.0, 842.0); 300];
        let l = layout(&sizes, 595.0);
        assert_eq!(l[0], (16.0, 842.0));
        assert_eq!(l[1].0, 16.0 + 842.0 + 16.0);
        assert_eq!(content_height(&l), 300.0 * (842.0 + 16.0) + 16.0);
        // Viewport on page 100 (index 99), 800 px high: one screen either side.
        let top = l[99].0;
        let w = wanted(&l, top, 800.0);
        assert_eq!(w, 98..101, "page 101 starts 1716 px down: beyond one screen");
        assert!(w.len() <= 5, "{w:?}");
        assert_eq!(wanted(&l, 0.0, 800.0).start, 0);
        assert_eq!(current(&l, top, 800.0), 99);
        assert!(wanted(&[], 0.0, 800.0).is_empty());
    }

    #[test]
    fn zoom_steps_stay_within_50_and_200() {
        assert_eq!(step_zoom(100, 1), 125);
        assert_eq!(step_zoom(100, -1), 80);
        assert_eq!(step_zoom(400, 1), 400);
        assert_eq!(step_zoom(25, -1), 25);
        assert_eq!(step_zoom(200, 1), 250);
    }

    fn mixed() -> Viewer {
        // A4 portrait, a landscape page turned upright by /Rotate 90, a cropped page.
        Viewer::new(vec![
            PageBox { crop: A4, rotation: 0, unit: 1.0 },
            PageBox { crop: [0.0, 0.0, 842.0, 595.0], rotation: 90, unit: 1.0 },
            PageBox { crop: [50.0, 50.0, 350.0, 450.0], rotation: 180, unit: 1.0 },
        ])
    }

    #[test]
    fn the_viewer_lays_pages_out_centred_and_maps_clicks_back_to_the_page() {
        let mut v = mixed();
        v.set_width(632.0);
        assert_eq!(v.page_rect(0), [16.0, 16.0, 600.0, (600.0f64 * 842.0 / 595.0)]);
        // The turned landscape page shows upright, as tall as an A4 portrait one.
        assert!((v.page_rect(1)[3] - v.page_rect(0)[3]).abs() < 1e-9);
        assert_eq!(v.render_px(0, 2.0), 1200);
        // A click in the middle of page 3 is the middle of its crop box.
        let [x, y, w, h] = v.page_rect(2);
        let (p, ux, uy) = v.hit(x + w / 2.0, y + h / 2.0).unwrap();
        assert_eq!(p, 2);
        assert!((ux - 200.0).abs() < 1e-6 && (uy - 250.0).abs() < 1e-6);
        // Gaps are not pages.
        assert_eq!(v.hit(x + 10.0, y - 8.0), None);
        // An overlay round-trips through the hit test on every page.
        for i in 0..3 {
            let [ox, oy, ow, oh] = v.overlay(i, match i { 2 => [100.0, 100.0, 140.0, 120.0], _ => [100.0, 200.0, 300.0, 260.0] });
            let a = v.hit(ox + 0.5, oy + 0.5).unwrap();
            let b = v.hit(ox + ow - 0.5, oy + oh - 0.5).unwrap();
            assert_eq!((a.0, b.0), (i, i));
        }
        // A drag past the page edge stays on the page.
        let (ux, _) = v.to_page(0, -500.0, 100.0);
        assert!(ux.abs() < 1e-9);
    }

    #[test]
    fn zooming_widens_the_pages_and_the_content() {
        let mut v = mixed();
        v.set_width(632.0);
        assert!(v.set_zoom(200));
        assert_eq!(v.page_rect(0)[2], 1200.0);
        // 2400 px wide at 2× would be 8 M pixels: held to 4 M, same proportions.
        let px = v.render_px(0, 2.0) as u64;
        let h = px * 842 / 595;
        assert!(px < 2400 && px * h <= MAX_RENDER_PIXELS && px * h > MAX_RENDER_PIXELS * 99 / 100, "{px}");
        assert_eq!(v.content_width(), 1232.0);
        assert!(v.set_zoom(50));
        assert_eq!((v.page_rect(0)[2], v.content_width()), (300.0, 632.0));
        assert_eq!(v.page_rect(0)[0], 166.0, "centred");
        assert!(!v.set_zoom(50));
        assert!(!v.set_zoom(10), "clamped to 50");
        // Jumping to a page puts it at the top, never past the end.
        let top = v.scroll_to(1, None, 400.0);
        assert_eq!(top, v.page_rect(1)[1] - PAGE_GAP);
        assert_eq!(v.scroll_to(2, None, 1e6), 0.0);
    }

    #[test]
    fn the_render_queue_runs_three_at_most_and_drops_unwanted_renders() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let q = RenderQueue::new(3);
        let done = Arc::new(AtomicUsize::new(0));
        let key = |page| RenderKey { doc: 1, page, width: 100, thumb: false, rev: 0 };
        for page in 0..12 {
            let done = done.clone();
            assert!(q.submit(key(page), move || {
                std::thread::sleep(std::time::Duration::from_millis(40));
                done.fetch_add(1, Ordering::SeqCst);
            }));
        }
        // Queued twice is queued once.
        assert!(!q.submit(key(11), || {}));
        // The viewer scrolled away: pages 6.. are no longer wanted.
        std::thread::sleep(std::time::Duration::from_millis(10));
        q.retain(|k| k.page < 6);
        let t0 = std::time::Instant::now();
        while q.pending() > 0 && t0.elapsed().as_secs() < 5 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(q.peak(), 3);
        assert_eq!(done.load(Ordering::SeqCst), 6);
    }

    #[test]
    fn a_user_unit_page_maps_view_points_as_sign_and_edit_do() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("u.pdf");
        std::fs::write(&path, crate::viewer::tests::pdf_with_page("/MediaBox [0 0 200 100] /UserUnit 2 /Rotate 90")).unwrap();
        let boxes = page_boxes(&Arc::new(std::fs::read(&path).unwrap()), None).unwrap();
        let b = boxes[0];
        // Shown 100 × 200 units turned a quarter, 2 points each.
        assert_eq!((b.unit, b.view_size()), (2.0, (200.0, 400.0)));
        // The view space placements are given in is the one core maps back to user space.
        let info = sheaf_core::sign::page_info(&sheaf_core::pages::Input::new(&path), 0).unwrap();
        assert_eq!((info.width, info.height), (200.0, 400.0));
        let (vw, vh) = b.view_size();
        for (x, y) in [(0.0, 0.0), (vw, vh), (0.3 * vw, 0.7 * vh), (vw / 2.0, 0.1 * vh)] {
            let ours = view_to_pdf(&b, (vw, vh), x, y);
            let core = sheaf_core::edit::view_to_user(&info, [x, y]);
            assert!((ours.0 - core[0]).abs() < 1e-3 && (ours.1 - core[1]).abs() < 1e-3, "({x}, {y}): {ours:?} vs {core:?}");
        }
    }

    #[test]
    fn pages_render_at_the_asked_width_with_their_rotation() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("a.pdf");
        std::fs::write(&path, crate::viewer::tests::tiny_pdf()).unwrap();
        let bytes = Arc::new(std::fs::read(&path).unwrap());
        let boxes = page_boxes(&bytes, None).unwrap();
        assert_eq!(boxes, vec![PageBox { crop: [0.0, 0.0, 200.0, 100.0], rotation: 90, unit: 1.0 }]);
        let px = render_page(&sheaf_core::pages::Input::new(&path), 0, 250).unwrap();
        // Turned a quarter: 100 × 200 pt shown, 250 px wide.
        assert_eq!((px.width(), px.height()), (250, 500));
    }

    /// One 200 × 100 pt page with /Rotate 90, written by hand.
    pub(crate) fn tiny_pdf() -> Vec<u8> {
        pdf_with_page("/MediaBox [0 0 200 100] /Rotate 90")
    }

    /// A one-page PDF whose page dictionary holds `page`.
    pub(crate) fn pdf_with_page(page: &str) -> Vec<u8> {
        let page = format!("<< /Type /Page /Parent 2 0 R {page} >>");
        let objs = ["<< /Type /Catalog /Pages 2 0 R >>", "<< /Type /Pages /Kids [3 0 R] /Count 1 >>", page.as_str()];
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offs = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offs.push(out.len());
            out.extend(format!("{} 0 obj {o} endobj\n", i + 1).bytes());
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
        for o in offs {
            out.extend(format!("{o:010} 00000 n \n").bytes());
        }
        out.extend(format!("trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).bytes());
        out
    }

    #[test]
    fn the_cache_pops_its_oldest_entry_for_the_byte_budget() {
        let mut c: Lru<u32, usize> = Lru::new(10);
        for k in 0..3 {
            c.put(k, k as usize);
        }
        c.get(&0);
        assert_eq!(c.pop_oldest(), Some((1, 1)));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn the_cache_evicts_the_least_recently_used_beyond_its_cap() {
        let mut c: Lru<(usize, u32), usize> = Lru::new(CACHE_CAP);
        // Scrolling through 300 pages never holds more than 40 renders.
        let mut evicted = 0;
        for page in 0..300 {
            evicted += c.put((page, 800), page).len();
            assert!(c.len() <= CACHE_CAP);
        }
        assert_eq!((c.len(), evicted), (40, 260));
        assert!(c.contains(&(299, 800)) && c.contains(&(260, 800)) && !c.contains(&(259, 800)));
        // Using an entry protects it: the next insert evicts the oldest other one.
        assert!(c.get(&(260, 800)).is_some());
        assert_eq!(c.put((300, 800), 0), vec![((261, 800), 261)]);
        // A new width is a new key; old widths are dropped on demand.
        c.put((5, 1000), 5);
        let dropped = c.retain(|(_, w)| *w == 1000);
        assert_eq!((dropped.len(), c.len()), (39, 1));
        // Replacing a value does not grow the cache, and hands the old value back so the
        // viewer's byte count drops it (it once grew without bound and evicted the visible page).
        assert_eq!(c.put((5, 1000), 6), vec![((5, 1000), 5)]);
        assert_eq!((c.len(), c.get(&(5, 1000)).copied()), (1, Some(6)));
    }
}
