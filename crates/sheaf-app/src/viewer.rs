//! Page viewer logic with no UI types: page layout in a vertical scroll, which pages to
//! render, the render cache, and the mapping between view pixels and PDF user space.
//!
//! The view of a page is what the user sees: the page's crop box turned by its `/Rotate`,
//! `render` pixels wide and high, origin top left, y down. PDF user space has y up and its
//! origin wherever the crop box puts it.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::ops::Range;

/// What the mapping needs to know about a page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBox {
    /// Crop box in user space `[x0, y0, x1, y1]` (the visible region).
    pub crop: [f64; 4],
    /// Clockwise `/Rotate`: 0, 90, 180 or 270.
    pub rotation: u16,
}

impl PageBox {
    /// Displayed size in points (width and height swap on quarter turns).
    pub fn view_size(&self) -> (f64, f64) {
        let (w, h) = (self.crop[2] - self.crop[0], self.crop[3] - self.crop[1]);
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

/// Zoom steps offered by ⌘+ / ⌘− (percent of the fit width).
pub const ZOOMS: [u32; 7] = [50, 67, 80, 100, 125, 150, 200];

/// The next zoom step up (`dir` > 0) or down from `zoom`, within 50–200 %.
pub fn step_zoom(zoom: u32, dir: i32) -> u32 {
    if dir > 0 {
        ZOOMS.iter().copied().find(|z| *z > zoom).unwrap_or(200)
    } else {
        ZOOMS.iter().rev().copied().find(|z| *z < zoom).unwrap_or(50)
    }
}

/// Most page renders kept at once. At 2× on a 1280 px window an A4 page is about 1.4 M
/// pixels (5.6 MB), so 40 of them stay under about 230 MB however long the document is.
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

    /// Inserts `v` as the most recently used and returns the keys evicted to stay at the cap.
    pub fn put(&mut self, k: K, v: V) -> Vec<K> {
        if self.map.insert(k.clone(), v).is_some() {
            self.touch(&k);
        } else {
            self.order.push_back(k);
        }
        let mut evicted = Vec::new();
        while self.map.len() > self.cap {
            let Some(old) = self.order.pop_front() else { break };
            self.map.remove(&old);
            evicted.push(old);
        }
        evicted
    }

    /// Drops every entry for which `keep` is false; returns the dropped keys.
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) -> Vec<K> {
        let dropped: Vec<K> = self.order.iter().filter(|k| !keep(k)).cloned().collect();
        for k in &dropped {
            self.map.remove(k);
        }
        self.order.retain(|k| keep(k));
        dropped
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
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
            let p = PageBox { crop, rotation };
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
        assert_eq!(PageBox { crop, rotation: 90 }.view_size(), (200.0, 100.0));
        assert_eq!(PageBox { crop, rotation: 180 }.view_size(), (100.0, 200.0));
    }

    #[test]
    fn a_point_one_step_right_on_screen_moves_the_right_way_in_user_space() {
        let render = (100.0, 100.0);
        let step = |rotation| {
            let p = PageBox { crop: [0.0, 0.0, 100.0, 100.0], rotation };
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
            let p = PageBox { crop: A4, rotation };
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
        assert_eq!(step_zoom(200, 1), 200);
        assert_eq!(step_zoom(50, -1), 50);
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
        assert_eq!(c.put((300, 800), 0), vec![(261, 800)]);
        // A new width is a new key; old widths are dropped on demand.
        c.put((5, 1000), 5);
        let dropped = c.retain(|(_, w)| *w == 1000);
        assert_eq!((dropped.len(), c.len()), (39, 1));
        // Replacing a value does not grow the cache.
        assert!(c.put((5, 1000), 6).is_empty());
        assert_eq!((c.len(), c.get(&(5, 1000)).copied()), (1, Some(6)));
    }
}
