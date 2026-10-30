//! Page renders: the least-recently-used caches, the render queue and its threads, the page boxes
//! read from a file, and drawing one page at a width.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use sheaf_core::pdfcraft::render;

use super::PageBox;

/// Most page renders kept at once, whatever their size. Memory is bounded by [`cache_bytes`]
/// (24–64 MB), which in practice evicts long before this count is reached; the cap only matters
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

    /// Removes and returns the least recently used entry `spare` does not keep.
    pub fn pop_oldest(&mut self, mut spare: impl FnMut(&K) -> bool) -> Option<(K, V)> {
        let i = self.order.iter().position(|k| !spare(k))?;
        let k = self.order.remove(i).expect("position is in range");
        self.map.remove(&k).map(|v| (k, v))
    }
}

/// Byte budget for page renders on top of the [`CACHE_CAP`] count: 1/256 of the machine's memory,
/// 24–64 MB. The pages in and near the view are kept whatever it says ([`wanted`](super::wanted));
/// the budget keeps a few more, the pages just left, so scrolling back shows them at once. At fit
/// width on a Retina screen one A4 page is about 16 MB (the [`MAX_RENDER_PIXELS`] cap), so 64 MB
/// is four pages; a 4 GB machine keeps a page and a half beyond the view's.
pub fn cache_bytes() -> usize {
    static BYTES: OnceLock<usize> = OnceLock::new();
    *BYTES.get_or_init(|| ((system_memory() / 256) as usize).clamp(24 << 20, 64 << 20))
}

/// The machine's memory in bytes (8 GB when it cannot be read).
pub fn system_memory() -> u64 {
    const UNKNOWN: u64 = 8 << 30;
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn sysctlbyname(name: *const std::ffi::c_char, old: *mut std::ffi::c_void, len: *mut usize, new: *mut std::ffi::c_void, new_len: usize) -> i32;
        }
        let (mut bytes, mut len) = (0u64, std::mem::size_of::<u64>());
        // SAFETY: hw.memsize is a 64-bit integer; `len` says how much room `bytes` has.
        let ok = unsafe { sysctlbyname(c"hw.memsize".as_ptr(), (&raw mut bytes).cast(), &mut len, std::ptr::null_mut(), 0) } == 0;
        if ok && bytes > 0 { bytes } else { UNKNOWN }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux: "MemTotal:  16314012 kB".
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|m| m.lines().find_map(|l| l.strip_prefix("MemTotal:")?.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()))
            .map_or(UNKNOWN, |kb| kb << 10)
    }
}

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
    render_page_themed(input, page, width, crate::reader::PageTheme::Day)
}

/// [`render_page`] recoloured for `theme`, the page's pictures left as drawn ([`crate::reader::kept_pictures`]).
pub fn render_page_themed(input: &sheaf_core::pages::Input, page: usize, width: u32, theme: crate::reader::PageTheme) -> Result<slint::SharedPixelBuffer<slint::Rgba8Pixel>, String> {
    use pdfium_render::prelude::PdfRenderConfig;
    let _g = sheaf_core::engine::pdfium_lock();
    let doc = input.open().map_err(|e| e.to_string())?;
    let p = doc.pages().get(page as i32).map_err(|e| e.to_string())?;
    let config = PdfRenderConfig::new().set_target_width(width as i32);
    let bmp = p.render_with_config(&config).map_err(|e| e.to_string())?;
    let (w, h) = (bmp.width() as u32, bmp.height() as u32);
    let mut px = slint::SharedPixelBuffer::clone_from_slice(&bmp.as_rgba_bytes(), w, h);
    if theme != crate::reader::PageTheme::Day {
        let keep = crate::reader::kept_pictures(&pictures(&p, &config, w, h), w, h);
        drop(bmp);
        drop(p);
        drop(doc);
        drop(_g);
        crate::reader::tint_except(px.make_mut_bytes(), w as usize, theme, &keep);
    }
    Ok(px)
}

/// The pictures page `p` draws (its image objects, inside Form XObjects too), in the pixels of a
/// render `w` × `h` made with `config`, upright (before Rotate View turns it).
fn pictures(p: &pdfium_render::prelude::PdfPage, config: &pdfium_render::prelude::PdfRenderConfig, w: u32, h: u32) -> Vec<crate::reader::Picture> {
    use pdfium_render::prelude::*;
    // A Form XObject's objects are bounded in its own space: `chain` maps them to the page's,
    // innermost matrix first.
    fn walk(object: PdfPageObject<'_>, chain: &mut Vec<PdfMatrix>, out: &mut Vec<(PdfQuadPoints, bool)>) {
        if let Some(image) = object.as_image_object() {
            let Ok(mut q) = object.bounds() else { return };
            for m in chain.iter().rev() {
                q = q.transform(*m);
            }
            out.push((q, image.bits_per_pixel().is_ok_and(|b| b == 1)));
        } else if let Some(form) = object.as_x_object_form_object()
            && chain.len() < 8
            && let Ok(m) = form.matrix()
        {
            chain.push(m);
            for i in form.as_range() {
                if let Ok(child) = form.get(i) {
                    walk(child, chain, out);
                }
            }
            chain.pop();
        }
    }
    let mut found = Vec::new();
    for object in p.objects().iter() {
        if matches!(object.object_type(), PdfPageObjectType::Image | PdfPageObjectType::XObjectForm) {
            walk(object, &mut Vec::new(), &mut found);
        }
    }
    // pdfium's points_to_pixels rounds to whole pixels, which left a photo's top and right edges a
    // pixel short (a light hairline, its edge tinted, at night). The exact map instead: the page
    // points of three bitmap corners (pixels_to_points gives fractions), inverted.
    let (w, h) = (w as f32, h as f32);
    let at = |x: f32, y: f32| p.pixels_to_points(x as i32, y as i32, config).ok().map(|(x, y)| (x.value, y.value));
    let (Some(o), Some(right), Some(down)) = (at(0.0, 0.0), at(w, 0.0), at(0.0, h)) else { return Vec::new() };
    let (ux, uy, vx, vy) = (right.0 - o.0, right.1 - o.1, down.0 - o.0, down.1 - o.1);
    let det = ux * vy - uy * vx;
    if det.abs() < 1e-6 {
        return Vec::new();
    }
    let to_pixels = |x: PdfPoints, y: PdfPoints| {
        let (dx, dy) = (x.value - o.0, y.value - o.1);
        ((dx * vy - dy * vx) / det * w, (ux * dy - uy * dx) / det * h)
    };
    found
        .into_iter()
        .map(|(q, one_bit)| {
            let corners = [(q.x1(), q.y1()), (q.x2(), q.y2()), (q.x3(), q.y3()), (q.x4(), q.y4())].map(|(x, y)| to_pixels(x, y));
            let (xs, ys) = (corners.iter().map(|c| c.0), corners.iter().map(|c| c.1));
            let rect = [xs.clone().fold(f32::MAX, f32::min), ys.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max), ys.fold(f32::MIN, f32::max)];
            crate::reader::Picture { rect, one_bit }
        })
        .collect()
}

/// How the reader draws its pages: recoloured for `theme`; `comments` false leaves annotations out;
/// `turn` turns the page clockwise more (Rotate View); `smooth` the text, line art and images
/// anti-aliased (Preferences ▸ Rendering, Acrobat's Smooth text, line art and images); `grid`
/// the page drawn without its white paper over a grey checkerboard, so what is transparent shows
/// (Acrobat's Show transparency grid).
#[derive(Clone, Debug)]
pub struct PageLook {
    pub theme: crate::reader::PageTheme,
    pub comments: bool,
    pub turn: u16,
    pub smooth: [bool; 3],
    pub grid: bool,
    pub display: Option<Arc<crate::display_color::DisplayTransform>>,
}

impl Default for PageLook {
    fn default() -> Self {
        Self { theme: Default::default(), comments: true, turn: 0, smooth: [true; 3], grid: false, display: None }
    }
}

/// Page `page` of a PDF held in memory (`bytes`, opened with `password`) rendered `width` pixels
/// wide, as RGBA, drawn as `look` asks: the reader's pages, which show edits not saved yet.
pub fn render_bytes(bytes: &[u8], password: Option<&str>, page: usize, width: u32, look: PageLook) -> Result<slint::SharedPixelBuffer<slint::Rgba8Pixel>, String> {
    use pdfium_render::prelude::PdfRenderConfig;
    let PageLook { theme, comments, turn, smooth: [text, lines, images], grid, display } = look;
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
    let keep = if theme == crate::reader::PageTheme::Day { Vec::new() } else { crate::reader::kept_pictures(&pictures(&p, &config, w, h), w, h) };
    drop(bmp);
    drop(p);
    drop(doc);
    drop(_g);
    if grid {
        over_grid(&mut rgba, w);
    }
    crate::reader::tint_except(&mut rgba, w as usize, theme, &keep);
    if let Some(display) = display { display.apply(&mut rgba)?; }
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

    /// Night and Sepia leave a page's photos as drawn (on the page and inside a Form XObject),
    /// theme a one-bit picture with the ink, and theme a scan whole.
    #[test]
    fn themes_keep_photos_and_theme_ink_and_scans() {
        use crate::reader::PageTheme;
        use lopdf::{Document, Object, Stream, dictionary};
        let pdf = |content: &[u8]| {
            let mut d = Document::with_version("1.7");
            let pages = d.new_object_id();
            let photo = d.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8 }, vec![120, 110, 100]));
            let ink = d.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 8, "Height" => 1, "ImageMask" => true, "BitsPerComponent" => 1, "Decode" => vec![1.into(), 0.into()] }, vec![0xff]));
            let form = d.add_object(Stream::new(
                dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 40.into(), 40.into()], "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => photo } } },
                b"q 40 0 0 40 0 0 cm /Im0 Do Q".to_vec(),
            ));
            let content = d.add_object(Stream::new(dictionary! {}, content.to_vec()));
            let resources = dictionary! { "XObject" => dictionary! { "Im0" => photo, "Im1" => ink, "Fm0" => form } };
            let page = d.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()], "Resources" => resources, "Contents" => content });
            d.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
            let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
            d.trailer.set("Root", cat);
            let mut bytes = Vec::new();
            d.save_to(&mut bytes).unwrap();
            bytes
        };
        let px = |bytes: &[u8], theme| {
            let b = render_bytes(bytes, None, 0, 200, PageLook { theme, ..Default::default() }).unwrap();
            let w = b.width() as usize;
            let s = b.as_slice().to_vec();
            move |x: usize, y: usize| { let p = s[y * w + x]; [p.r, p.g, p.b] }
        };
        // A muted photo (Night would invert it, being nearly grey) top left, the same photo in a form bottom right, black ink top right.
        let page = pdf(b"q 50 0 0 50 20 130 cm /Im0 Do Q q 1 0 0 1 120 20 cm /Fm0 Do Q 0 g q 50 0 0 50 120 130 cm /Im1 Do Q");
        for theme in [PageTheme::Night, PageTheme::Sepia] {
            let at = px(&page, theme);
            assert_eq!(at(45, 45), [120, 110, 100], "{theme:?}: the photo");
            assert_eq!(at(140, 160), [120, 110, 100], "{theme:?}: the photo in a form");
            assert_ne!(at(5, 5), [255, 255, 255], "{theme:?}: the paper");
            assert_ne!(at(145, 45), [0, 0, 0], "{theme:?}: the ink");
        }
        assert_eq!(px(&page, PageTheme::Night)(5, 5), [30, 30, 32]);
        assert_eq!(px(&page, PageTheme::Night)(145, 45), [222, 222, 220]);
        assert_eq!(px(&page, PageTheme::Day)(145, 45), [0, 0, 0]);
        // A scan: the picture over the whole page is themed.
        let scan = pdf(b"q 200 0 0 200 0 0 cm /Im0 Do Q");
        assert_ne!(px(&scan, PageTheme::Night)(100, 100), [120, 110, 100]);
    }

    #[test]
    fn the_cache_pops_its_oldest_entry_for_the_byte_budget() {
        let mut c: Lru<u32, usize> = Lru::new(10);
        for k in 0..3 {
            c.put(k, k as usize);
        }
        c.get(&0);
        assert_eq!(c.pop_oldest(|_| false), Some((1, 1)));
        assert_eq!(c.len(), 2);
        // The pages in view are spared: the next oldest goes, and none when all are spared.
        assert_eq!(c.pop_oldest(|k| *k == 2), Some((0, 0)));
        assert_eq!(c.pop_oldest(|_| true), None);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn the_render_budget_scales_with_the_machine_within_its_bounds() {
        assert!(system_memory() >= 1 << 30, "{}", system_memory());
        assert!((24 << 20..=64 << 20).contains(&cache_bytes()));
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
    #[test]
    fn monitor_icc_is_applied_to_real_rendered_pages_without_changing_pdf_bytes() {
        use lopdf::{dictionary, Stream};
        let mut pdf = lopdf::Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let content = pdf.add_object(Stream::new(dictionary! {}, b"1 0 0 rg 0 0 100 100 re f".to_vec()));
        let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),100.into(),100.into()], "Contents" => content });
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()] }.into());
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages }); pdf.trailer.set("Root", root);
        let mut bytes = Vec::new(); pdf.save_to(&mut bytes).unwrap();
        let before = bytes.clone();
        let plain = render_bytes(&bytes, None, 0, 100, PageLook::default()).unwrap();
        let display = crate::display_color::DisplayTransform::for_profile(&moxcms::ColorProfile::new_display_p3()).unwrap();
        let managed = render_bytes(&bytes, None, 0, 100, PageLook { display: Some(Arc::new(display)), ..Default::default() }).unwrap();
        assert_ne!(plain.as_bytes(), managed.as_bytes());
        assert_eq!(managed.as_bytes()[3], 255);
        assert_eq!(bytes, before);
    }
}
