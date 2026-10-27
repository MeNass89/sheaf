//! Page renders: the least-recently-used caches, the render queue and its threads, the page boxes
//! read from a file, and drawing one page at a width.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use sheaf_core::pdfcraft::render;

use super::PageBox;

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

/// The displayed page sizes read by pdfium without loading any page (`FPDF_GetPageSizeByIndexF`,
/// rotation applied), as boxes at the origin: enough to lay the pages out and render them while
/// PdfCraft parses the document; the real boxes (crop origin, rotation) replace them then.
pub fn fast_boxes(input: &sheaf_core::pages::Input) -> Result<Vec<PageBox>, String> {
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open().map_err(|e| e.to_string())?;
    let sizes = doc.pages().page_sizes().map_err(|e| e.to_string())?;
    Ok(sizes.iter().map(|r| PageBox { crop: [0.0, 0.0, r.width().value as f64, r.height().value as f64], rotation: 0, unit: 1.0 }).collect())
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

/// How the reader draws its pages: recoloured for `theme`; `comments` false leaves annotations out;
/// `turn` turns the page clockwise more (Rotate View); `smooth` the text, line art and images
/// anti-aliased (Preferences ▸ Rendering, Acrobat's Smooth text, line art and images); `grid`
/// the page drawn without its white paper over a grey checkerboard, so what is transparent shows
/// (Acrobat's Show transparency grid).
#[derive(Clone, Copy, Debug)]
pub struct PageLook {
    pub theme: crate::reader::PageTheme,
    pub comments: bool,
    pub turn: u16,
    pub smooth: [bool; 3],
    pub grid: bool,
}

impl Default for PageLook {
    fn default() -> Self {
        Self { theme: Default::default(), comments: true, turn: 0, smooth: [true; 3], grid: false }
    }
}

/// Page `page` of a PDF held in memory (`bytes`, opened with `password`) rendered `width` pixels
/// wide, as RGBA, drawn as `look` asks: the reader's pages, which show edits not saved yet.
pub fn render_bytes(bytes: &[u8], password: Option<&str>, page: usize, width: u32, look: PageLook) -> Result<slint::SharedPixelBuffer<slint::Rgba8Pixel>, String> {
    use pdfium_render::prelude::PdfRenderConfig;
    let PageLook { theme, comments, turn, smooth: [text, lines, images], grid } = look;
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = sheaf_core::engine::open_pdf_bytes(bytes, password).map_err(|e| e.to_string())?;
    let p = doc.pages().get(page as i32).map_err(|e| e.to_string())?;
    // Without comments (Hide all comments), form fields still draw.
    let config = PdfRenderConfig::new()
        .render_form_data(true)
        .render_annotations(comments)
        .set_text_smoothing(text)
        .set_path_smoothing(lines)
        .set_image_smoothing(images);
    let config = if grid { config.set_clear_color(pdfium_render::prelude::PdfColor::new(0, 0, 0, 0)) } else { config };
    // Drawn upright, then turned (pdfium-render's own rotation keeps the upright size): a quarter
    // turn asks for the height the width will be.
    let config = if turn % 180 == 90 { config.set_target_height(width as i32) } else { config.set_target_width(width as i32) };
    let bmp = p.render_with_config(&config).map_err(|e| e.to_string())?;
    let (w, h) = (bmp.width() as u32, bmp.height() as u32);
    let mut rgba = bmp.as_rgba_bytes();
    if grid {
        over_grid(&mut rgba, w);
    }
    crate::reader::tint(&mut rgba, theme);
    if turn.is_multiple_of(360) {
        return Ok(slint::SharedPixelBuffer::clone_from_slice(&rgba, w, h));
    }
    let upright = image::RgbaImage::from_raw(w, h, rgba).ok_or("the page's pixels are short")?;
    let turned = match turn % 360 {
        90 => image::imageops::rotate90(&upright),
        180 => image::imageops::rotate180(&upright),
        _ => image::imageops::rotate270(&upright),
    };
    Ok(slint::SharedPixelBuffer::clone_from_slice(turned.as_raw(), turned.width(), turned.height()))
}

/// RGBA pixels `width` wide laid over Acrobat's transparency grid: 8-pixel squares, white and light
/// grey; the result is opaque.
fn over_grid(rgba: &mut [u8], width: u32) {
    let width = width.max(1) as usize;
    for (i, p) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (i % width, i / width);
        let back = if (x / 8 + y / 8) % 2 == 0 { 255u16 } else { 204 };
        let a = u16::from(p[3]);
        for c in &mut p[..3] {
            *c = ((u16::from(*c) * a + back * (255 - a) + 127) / 255) as u8;
        }
        p[3] = 255;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // The view turned a quarter more (Rotate View): 200 × 100 shown, still 250 px wide; half a
        // turn: as tall as without.
        let px = render_bytes(&bytes, None, 0, 250, PageLook { turn: 90, ..Default::default() }).unwrap();
        assert_eq!((px.width(), px.height()), (250, 125));
        let px = render_bytes(&bytes, None, 0, 250, PageLook { turn: 180, ..Default::default() }).unwrap();
        assert_eq!((px.width(), px.height()), (250, 500));
        // The boxes the view maps its points with turn the same way.
        assert_eq!(crate::viewer::turned(&boxes, 90)[0].view_size(), (200.0, 100.0));
    }

    /// Preferences ▸ Rendering: a slanted line drawn smoothed has grey edges, drawn rough none.
    #[test]
    fn pages_drawn_without_smoothing_have_hard_edges() {
        use lopdf::{Document, Object, Stream, dictionary};
        let mut d = Document::with_version("1.7");
        let c = d.add_object(Stream::new(dictionary! {}, b"0 g 3 w 10 10 m 190 90 l S".to_vec()));
        let pages = d.new_object_id();
        let page = d.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 200.into(), 100.into()], "Contents" => c });
        d.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
        let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        d.trailer.set("Root", cat);
        let mut bytes = Vec::new();
        d.save_to(&mut bytes).unwrap();
        let greys = |smooth| {
            let px = render_bytes(&bytes, None, 0, 200, PageLook { smooth: [smooth; 3], ..Default::default() }).unwrap();
            px.as_bytes().as_chunks::<4>().0.iter().filter(|p| (30..225).contains(&p[0])).count()
        };
        assert!(greys(true) > 50, "smoothed edges are grey");
        assert!(greys(false) < greys(true) / 4, "rough edges are black or white");
    }

    /// Show transparency grid: a page's empty paper shows the grid's two greys, a drawn shape stays
    /// as it is; without it the paper is white.
    #[test]
    fn the_transparency_grid_shows_through_the_empty_paper() {
        use lopdf::{Document, Object, Stream, dictionary};
        let mut d = Document::with_version("1.7");
        let pages = d.new_object_id();
        let content = d.add_object(Stream::new(dictionary! {}, b"1 0 0 rg 0 0 50 50 re f".to_vec()));
        let page = d.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()], "Contents" => content });
        d.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
        let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        d.trailer.set("Root", cat);
        let mut bytes = Vec::new();
        d.save_to(&mut bytes).unwrap();
        let px = |look| {
            let b = render_bytes(&bytes, None, 0, 200, look).unwrap();
            let w = b.width() as usize;
            let s = b.as_slice().to_vec();
            move |x: usize, y: usize| { let p = s[y * w + x]; [p.r, p.g, p.b, p.a] }
        };
        let plain = px(PageLook::default());
        assert_eq!(plain(100, 4), [255, 255, 255, 255], "white paper");
        let grid = px(PageLook { grid: true, ..Default::default() });
        // Top left of the page (the shape is at the bottom left): squares alternate.
        assert_eq!(grid(4, 4), [255, 255, 255, 255]);
        assert_eq!(grid(12, 4), [204, 204, 204, 255]);
        assert_eq!(grid(12, 12), [255, 255, 255, 255]);
        // The red square, bottom left, is drawn over the grid.
        assert_eq!(grid(10, 190), [255, 0, 0, 255]);
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
