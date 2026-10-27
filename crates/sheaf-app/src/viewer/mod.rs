//! Page viewer logic with no UI types: page layout in a vertical scroll, which pages to
//! render, and the mapping between view pixels and PDF user space. The renders themselves (caches,
//! queue, drawing a page) are in [`renders`].
//!
//! The view of a page is what the user sees: the page's crop box turned by its `/Rotate`,
//! `render` pixels wide and high, origin top left, y down. PDF user space has y up and its
//! origin wherever the crop box puts it.

use std::ops::Range;

mod renders;
pub use renders::*;

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

/// Whether two marks show at the same place and size on their pages (`[x, y, w, h]` from the
/// page's top-left, as shown): how a signature's copies placed with Every page are known again.
pub fn same_place(a: [f64; 4], b: [f64; 4]) -> bool {
    (0..4).all(|k| (a[k] - b[k]).abs() < 1.0)
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
    let Some(i) = layout.iter().rposition(|(y, _)| *y <= mid) else { return 0 };
    // With two pages side by side, the row's first (left) page: page operations, printing the
    // current page and the page field all mean the pair's first page.
    let row = layout[i].0;
    (0..=i).rev().take_while(|&j| layout[j].0 == row).last().unwrap_or(i)
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


/// A redaction mark, search match or other box drawn over a page, in PDF user space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayRect {
    pub page: usize,
    pub rect: [f64; 4],
    pub kind: i32,
}

/// Width of a thumbnail in the strip, logical pixels.
pub const THUMB_WIDTH: f64 = 84.0;
/// A page's width in the page grid (spread 3) at 100 %.
pub const GRID_PAGE: f64 = 170.0;
/// Narrowest the view is laid out for, logical pixels.
const MIN_PAGE_WIDTH: f64 = 120.0;

/// Where every page of one document sits in the viewer's scrolling content (logical pixels,
/// y down), for a viewport `fit_width` wide at `zoom` percent of the fit width.
#[derive(Debug, Clone)]
pub struct Viewer {
    pub boxes: Vec<PageBox>,
    fit_width: f64,
    zoom: u32,
    layout: Vec<(f64, f64)>,
    /// Each page in the content: `[x, y, width, height]`.
    rects: Vec<[f64; 4]>,
    content_h: f64,
    page_width: f64,
    /// Pages side by side: 0 one column, 1 two per row, 2 two per row with the first page alone
    /// on the right (a book's cover, as Acrobat and PDF Expert show it), 3 a grid of as many
    /// columns as fit (PDF Expert's page thumbnails view).
    spread: u8,
    /// Pages side by side read right to left (Acrobat's Right-to-left page order: Arabic, Hebrew,
    /// manga): the first of a pair on the right, the cover on the left.
    rtl: bool,
    /// Width of a thumbnail in the strip or panel.
    pub thumb_width: f64,
    /// Rotate View (Acrobat's View ▸ Rotate View): every page shown turned this much more clockwise
    /// than its own /Rotate (0, 90, 180, 270); `boxes` carry it, the file does not.
    turn: u16,
    /// Pages across (PDF Expert's View ▸ Horizontal scrolling): the rows above laid one after the
    /// other in a single strip scrolled left to right (right to left with `rtl`). The page grid
    /// stays a grid.
    horizontal: bool,
    /// How far the view is scrolled across (the view reports it): with pages across, it decides
    /// which pages are in view and rendered, as the vertical scroll does down a column.
    left: f64,
    /// Each page's row (a pair of pages shares one), in reading order.
    row_of: Vec<usize>,
    content_w: f64,
    /// The view's height: pages across are centred in it.
    view_h: f64,
}

/// The file's page boxes as the view shows them, turned `turn` degrees clockwise more.
pub fn turned(boxes: &[PageBox], turn: u16) -> Vec<PageBox> {
    boxes.iter().map(|b| PageBox { rotation: (b.rotation + turn) % 360, ..*b }).collect()
}

impl Viewer {
    pub fn new(boxes: Vec<PageBox>) -> Self {
        let mut v = Self { boxes, fit_width: 600.0, zoom: 100, layout: Vec::new(), rects: Vec::new(), content_h: PAGE_GAP, page_width: 0.0, spread: 0, rtl: false, thumb_width: THUMB_WIDTH, turn: 0, horizontal: false, left: 0.0, row_of: Vec::new(), content_w: 0.0, view_h: 0.0 };
        v.relayout();
        v
    }

    fn columns(&self) -> f64 {
        match self.spread {
            0 => 1.0,
            1 | 2 => 2.0,
            // Pages about GRID_PAGE wide at 100 %, at least two to a row.
            _ => ((self.fit_width - PAGE_GAP) / (GRID_PAGE + PAGE_GAP)).floor().max(2.0),
        }
    }

    /// The width one page has at 100 % (fit width): the viewport's, shared by the columns.
    fn fit_page_width(&self) -> f64 {
        let cols = self.columns();
        ((self.fit_width - (cols + 1.0) * PAGE_GAP) / cols).max(1.0)
    }

    fn relayout(&mut self) {
        // Zoomed out, a page may go narrower than the window's floor: a very tall page fits whole.
        let w = (self.fit_page_width() * self.zoom as f64 / 100.0).max(4.0);
        // Rounded down when narrow: a page fitted to the height must not grow past it.
        self.page_width = if w < 100.0 { w.floor() } else { w.round() };
        let w = self.page_width;
        let heights: Vec<f64> = self.boxes.iter().map(|b| { let (bw, bh) = b.view_size(); w * bh / bw.max(1e-9) }).collect();
        let n = heights.len();
        // Rows of pages: one each, pairs, or the cover alone then pairs.
        let cols = self.columns();
        let rows: Vec<Vec<usize>> = match self.spread {
            0 => (0..n).map(|i| vec![i]).collect(),
            2 => std::iter::once(vec![0]).take(n.min(1)).chain((1..n).collect::<Vec<_>>().chunks(2).map(<[usize]>::to_vec)).collect(),
            _ => (0..n).collect::<Vec<_>>().chunks(cols as usize).map(<[usize]>::to_vec).collect(),
        };
        let row_w = cols * w + (cols - 1.0) * PAGE_GAP;
        let left = ((self.content_width_for(row_w) - row_w) / 2.0).round();
        let across = self.across();
        // Pages across: the strip's width, centred when the view is wider (a short document).
        let strip_w = rows.len() as f64 * (row_w + PAGE_GAP) + PAGE_GAP;
        let strip_left = ((self.fit_width - strip_w) / 2.0).max(0.0).round();
        let tallest = heights.iter().copied().fold(0.0, f64::max);
        let mut rects = vec![[0.0; 4]; n];
        let mut row_of = vec![0; n];
        let mut y = PAGE_GAP;
        for (r, row) in rows.iter().enumerate() {
            for (k, &i) in row.iter().enumerate() {
                let col = if self.spread == 2 && i == 0 { 1.0 } else { k as f64 };
                row_of[i] = r;
                rects[i] = if across {
                    // Each row keeps its slots (a pair stays a pair, the cover alone on the right),
                    // pages aligned at the top; right to left reads the strip from its right end.
                    let x = PAGE_GAP + r as f64 * (row_w + PAGE_GAP) + col * (w + PAGE_GAP);
                    [strip_left + if self.rtl { strip_w - x - w } else { x }, PAGE_GAP, w, heights[i]]
                } else {
                    let col = if self.rtl { cols - 1.0 - col } else { col };
                    [left + col * (w + PAGE_GAP), y, w, heights[i]]
                };
            }
            y += row.iter().map(|&i| heights[i]).fold(0.0, f64::max) + PAGE_GAP;
        }
        self.layout = rects.iter().map(|r| (r[1], r[3])).collect();
        self.rects = rects;
        self.row_of = row_of;
        (self.content_w, self.content_h) = if across { (self.fit_width.max(strip_w), tallest + 2.0 * PAGE_GAP) } else { (self.content_width_for(row_w), y) };
    }

    /// Whether the pages run across (see `horizontal`): never in the page grid.
    fn across(&self) -> bool {
        self.horizontal && self.spread != 3
    }

    /// Pages across or down (see `horizontal`); returns whether the layout changed.
    pub fn set_horizontal(&mut self, on: bool) -> bool {
        let changed = on != self.horizontal;
        self.horizontal = on;
        self.relayout();
        changed
    }

    pub fn horizontal(&self) -> bool {
        self.horizontal
    }

    /// The view is `height` high (see `view_h`); returns whether the layout changed.
    pub fn set_height(&mut self, height: f64) -> bool {
        let changed = (height - self.view_h).abs() >= 0.5;
        self.view_h = height;
        if changed && self.across() {
            self.relayout();
        }
        changed && self.across()
    }

    /// The view is scrolled `left` across (see `left`).
    pub fn set_left(&mut self, left: f64) {
        self.left = left;
    }

    /// Page `i`'s row: pages side by side share one (← and → step a row at a time).
    pub fn row_of(&self, i: usize) -> usize {
        self.row_of.get(i).copied().unwrap_or(0)
    }

    /// With pages across, the horizontal scroll that centres page `i`'s row in the view (its pair
    /// with it), held to the strip's ends; 0 down a column.
    pub fn scroll_x_to(&self, i: usize) -> f64 {
        if !self.across() || i >= self.rects.len() {
            return 0.0;
        }
        let row = self.row_of(i);
        let (lo, hi) = (0..self.rects.len())
            .filter(|&j| self.row_of[j] == row)
            .map(|j| self.rects[j])
            .fold((f64::MAX, f64::MIN), |(lo, hi), r| (lo.min(r[0]), hi.max(r[0] + r[2])));
        ((lo + hi) / 2.0 - self.fit_width / 2.0).clamp(0.0, self.max_scroll_x())
    }

    fn content_width_for(&self, row_w: f64) -> f64 {
        self.fit_width.max(row_w + 2.0 * PAGE_GAP)
    }

    /// Pages side by side (see `spread`); returns whether the layout changed.
    pub fn set_spread(&mut self, spread: u8) -> bool {
        let spread = spread.min(3);
        let changed = spread != self.spread;
        self.spread = spread;
        self.relayout();
        changed
    }

    pub fn spread(&self) -> u8 {
        self.spread
    }

    /// Pairs read right to left (see `rtl`); returns whether the layout changed.
    pub fn set_rtl(&mut self, rtl: bool) -> bool {
        let changed = rtl != self.rtl;
        self.rtl = rtl;
        self.relayout();
        changed
    }

    pub fn rtl(&self) -> bool {
        self.rtl
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
        // The internal zoom is relative to the fit width; the steps the user sees are actual sizes
        // (`actual_percent`), so this range is wider than theirs.
        let zoom = zoom.clamp(1, 1600);
        let changed = zoom != self.zoom;
        self.zoom = zoom;
        self.relayout();
        changed
    }

    pub fn zoom(&self) -> u32 {
        self.zoom
    }

    /// How far the view is turned clockwise (Rotate View): 0, 90, 180 or 270.
    pub fn turn(&self) -> u16 {
        self.turn
    }

    /// Turns the view to `turn` degrees clockwise (any multiple of 90): every page with it.
    pub fn set_turn(&mut self, turn: i32) {
        let turn = turn.rem_euclid(360) as u16 / 90 * 90;
        self.boxes = turned(&self.boxes, (turn + 360 - self.turn) % 360);
        self.turn = turn;
        self.relayout();
    }

    /// The zoom as PDF Expert and Preview show it: 100 % draws one point of page `i` as one logical
    /// pixel.
    pub fn actual_percent(&self, i: usize) -> f64 {
        let w = self.boxes.get(i).map(|b| b.view_size().0).unwrap_or(1.0).max(1e-9);
        self.page_width / w * 100.0
    }

    /// The internal zoom that shows page `i` at `pct` percent actual size.
    pub fn zoom_for_actual(&self, i: usize, pct: f64) -> u32 {
        (self.zoom as f64 * pct / self.actual_percent(i).max(1e-9)).round().max(1.0) as u32
    }

    /// The zoom (percent of the fit width) at which the tallest page of `pages` fits a viewport
    /// `height` high: the reader's Fit page.
    pub fn fit_page_zoom(&self, height: f64, pages: Range<usize>) -> u32 {
        let fit_w = self.fit_page_width();
        let ratio = pages.filter_map(|i| self.boxes.get(i)).map(|b| { let (w, h) = b.view_size(); h / w.max(1e-9) }).fold(0.0, f64::max);
        if ratio <= 0.0 {
            return 100;
        }
        let want_w = (height - 2.0 * PAGE_GAP).max(1.0) / ratio;
        // A very tall page (a receipt, a scroll) fits whole too: down to the viewer's own floor.
        ((want_w / fit_w * 100.0).floor() as u32).clamp(1, 100)
    }

    pub fn page_count(&self) -> usize {
        self.boxes.len()
    }

    pub fn layout(&self) -> &[(f64, f64)] {
        &self.layout
    }

    /// Width of the scrolling content: wider than the viewport when zoomed in.
    pub fn content_width(&self) -> f64 {
        self.content_w
    }

    pub fn content_height(&self) -> f64 {
        self.content_h
    }

    /// Page `i` in content coordinates: `[x, y, width, height]`, its row centred.
    pub fn page_rect(&self, i: usize) -> [f64; 4] {
        self.rects.get(i).copied().unwrap_or([0.0; 4])
    }

    /// The content point (x, y) as a point of the nearest page: (page, x and y as fractions of
    /// its width and height). A zoom keeps it where it is (the gaps between pages do not scale).
    pub fn anchor_of(&self, x: f64, y: f64) -> (usize, f64, f64) {
        let dist = |r: &[f64; 4]| {
            let dx = (r[0] - x).max(x - (r[0] + r[2])).max(0.0);
            let dy = (r[1] - y).max(y - (r[1] + r[3])).max(0.0);
            dx * dx + dy * dy
        };
        let i = (0..self.rects.len()).min_by(|a, b| dist(&self.rects[*a]).total_cmp(&dist(&self.rects[*b]))).unwrap_or(0);
        let [px, py, w, h] = self.page_rect(i);
        (i, ((x - px) / w.max(1e-9)).clamp(0.0, 1.0), ((y - py) / h.max(1e-9)).clamp(0.0, 1.0))
    }

    /// The content point of [`Self::anchor_of`]'s page point.
    pub fn point_of(&self, (i, fx, fy): (usize, f64, f64)) -> (f64, f64) {
        let [px, py, w, h] = self.page_rect(i);
        (px + fx * w, py + fy * h)
    }

    /// The view's width (the fit width).
    pub fn view_width(&self) -> f64 {
        self.fit_width
    }

    /// The widest horizontal scroll: the content width past the view's.
    pub fn max_scroll_x(&self) -> f64 {
        (self.content_width() - self.fit_width).max(0.0)
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

    /// Pages worth rendering for the viewport `[top, top + height)`, scrolled across as last
    /// reported ([`Self::set_left`]).
    pub fn wanted(&self, top: f64, height: f64) -> Range<usize> {
        self.wanted_at(self.left, top, height)
    }

    /// [`Self::wanted`] for a view scrolled `left` across (a split view's second pane).
    pub fn wanted_at(&self, left: f64, top: f64, height: f64) -> Range<usize> {
        if !self.across() {
            return wanted(&self.layout, top, height);
        }
        // Across: the pages within one view's width either side. They follow in reading order
        // however the strip runs, so those near make one run of indices.
        let (lo, hi) = (left - self.fit_width, left + 2.0 * self.fit_width);
        let near = |i: &usize| {
            let r = self.rects[*i];
            r[0] + r[2] >= lo && r[0] <= hi
        };
        let first = (0..self.rects.len()).find(near);
        match (first, (0..self.rects.len()).rev().find(near)) {
            (Some(a), Some(b)) => a..b + 1,
            _ => 0..0,
        }
    }

    /// The page in view: under the middle of the viewport, the first of its row.
    pub fn current(&self, top: f64, height: f64) -> usize {
        if !self.across() {
            return current(&self.layout, top, height);
        }
        let mid = self.left + self.fit_width / 2.0;
        let dist = |i: usize| {
            let [x, _, w, _] = self.rects[i];
            (x - mid).max(mid - x - w).max(0.0)
        };
        let Some(i) = (0..self.rects.len()).min_by(|a, b| dist(*a).total_cmp(&dist(*b))) else { return 0 };
        (0..=i).find(|&j| self.row_of[j] == self.row_of[i]).unwrap_or(i)
    }

    /// Scroll position that shows page `i` at the top (with its gap above it), or the point
    /// `y_pt` of it near the top third of a viewport `height` high when given.
    pub fn scroll_to(&self, i: usize, focus: Option<[f64; 4]>, height: f64) -> f64 {
        let top = match focus {
            // A third down the view, or higher when the focus would end below it (a /FitR
            // rectangle, an area zoomed to).
            Some(r) => {
                let [_, y, _, h] = self.overlay(i, r);
                let third = y - height / 3.0;
                // Past the view's bottom from a third down: up until it ends in view, its top kept in view.
                if y + h + 8.0 > third + height { (y + h + 8.0 - height).min(y - 8.0) } else { third }
            }
            None => self.page_rect(i)[1] - PAGE_GAP,
        };
        top.clamp(0.0, (self.content_height() - height).max(0.0))
    }

    /// The thumbnail strip's layout ([`THUMB_WIDTH`] wide pages).
    pub fn thumb_layout(&self) -> Vec<(f64, f64)> {
        layout(&self.boxes.iter().map(PageBox::view_size).collect::<Vec<_>>(), self.thumb_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A signature's copies on a Letter page and on an A4 page turned 90°: other boxes in the file,
    /// the same place as shown (Sol, 27/10: copies were matched by the file's box).
    #[test]
    fn copies_at_one_place_are_known_on_pages_of_other_sizes() {
        let letter = PageBox { crop: [0.0, 0.0, 612.0, 792.0], rotation: 0, unit: 1.0 };
        let a4 = PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 90, unit: 1.0 };
        let shown = [100.0, 50.0, 150.0, 40.0];
        let user = |b: &PageBox| view_rect_to_pdf(b, b.view_size(), (shown[0], shown[1]), (shown[0] + shown[2], shown[1] + shown[3]));
        let (u1, u2) = (user(&letter), user(&a4));
        assert!((0..4).any(|k| (u1[k] - u2[k]).abs() > 1.0), "{u1:?} {u2:?}");
        let (s1, s2) = (pdf_rect_to_view(&letter, letter.view_size(), u1), pdf_rect_to_view(&a4, a4.view_size(), u2));
        assert!(same_place(s1, s2), "{s1:?} {s2:?}");
        assert!(!same_place(s1, [100.0, 60.0, 150.0, 40.0]));
    }

    const A4: [f64; 4] = [0.0, 0.0, 595.0, 842.0];

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn fit_page_fits_a_very_tall_page_whole() {
        // 200 × 14,400 pt: a receipt or a scroll (Sol's case).
        let mut v = Viewer::new(vec![PageBox { crop: [0.0, 0.0, 200.0, 14400.0], rotation: 0, unit: 1.0 }]);
        v.set_width(1000.0);
        let z = v.fit_page_zoom(700.0, 0..1);
        assert!(z < 25, "{z}");
        v.set_zoom(z);
        assert!(v.page_rect(0)[3] <= 700.0, "{:?}", v.page_rect(0));
    }

    #[test]
    fn a_zoom_keeps_the_anchored_point_of_a_deep_page_in_place() {
        let pages = |n| (0..n).map(|_| PageBox { crop: A4, rotation: 0, unit: 1.0 }).collect::<Vec<_>>();
        let mut v = Viewer::new(pages(80));
        v.set_width(1000.0);
        v.set_zoom(60);
        // A point a third into page 50, 300 px below the top of the view, 400 px from its left.
        let p = v.page_rect(50);
        let (ax, ay, sx) = (400.0, 300.0, 0.0);
        let top = p[1] + p[3] / 3.0 - ay;
        let anchor = v.anchor_of(sx + ax, top + ay);
        assert_eq!(anchor.0, 50);
        v.set_zoom(150);
        let (x, y) = v.point_of(anchor);
        let (top2, sx2) = (y - ay, (x - ax).clamp(0.0, v.max_scroll_x()));
        let back = v.anchor_of(sx2 + ax, top2 + ay);
        assert_eq!(back.0, 50);
        assert!((back.1 - anchor.1).abs() < 1e-6 && (back.2 - anchor.2).abs() < 1e-6, "{anchor:?} → {back:?}");
        // A point between pages belongs to the nearest one.
        let g = v.page_rect(10);
        assert_eq!(v.anchor_of(g[0] + 5.0, g[1] - 2.0).0, 10);
    }

    #[test]
    fn two_pages_side_by_side_and_the_cover_alone() {
        let pages = |n| (0..n).map(|_| PageBox { crop: A4, rotation: 0, unit: 1.0 }).collect::<Vec<_>>();
        let mut v = Viewer::new(pages(5));
        v.set_width(1000.0);
        let single = v.page_rect(1);
        assert!(v.set_spread(1));
        let (p0, p1, p2) = (v.page_rect(0), v.page_rect(1), v.page_rect(2));
        assert_eq!(p0[1], p1[1], "pages 1 and 2 share a row");
        assert!(p1[0] > p0[0] + p0[2] && p2[1] > p0[1] + p0[3], "page 2 right of 1, page 3 below");
        assert!(p0[2] < single[2] / 1.5, "each page about half as wide at fit width");
        assert!(v.content_height() > p2[1] + p2[3]);
        // The page in view is the pair's first page.
        assert_eq!(v.current(p0[1], 100.0), 0);
        // Hit the right-hand page.
        assert_eq!(v.hit(p1[0] + 5.0, p1[1] + 5.0).map(|h| h.0), Some(1));
        // With the cover: page 1 alone on the right, then 2-3, 4-5.
        v.set_spread(2);
        let (c0, c1, c2) = (v.page_rect(0), v.page_rect(1), v.page_rect(2));
        assert!(c0[0] > c1[0] && c1[1] > c0[1] && c1[1] == c2[1] && c2[0] > c1[0]);
        assert_eq!(c0[0], c2[0], "the cover sits in the right column");
        // Right to left: the cover on the left, then 2 on the right of 3.
        assert!(v.set_rtl(true));
        let (r0, r1, r2) = (v.page_rect(0), v.page_rect(1), v.page_rect(2));
        assert!(r0[0] == r2[0] && r0[0] < c0[0], "the cover sits in the left column: {r0:?} {c0:?}");
        assert!(r1[0] > r2[0] && r1[1] == r2[1], "page 2 on the right of page 3: {r1:?} {r2:?}");
        v.set_rtl(false);
        // The grid: as many columns as fit in 1000 px, pages in reading order.
        v.set_spread(3);
        let (g0, g1, g4) = (v.page_rect(0), v.page_rect(1), v.page_rect(4));
        assert!(g0[1] == g1[1] && g1[0] > g0[0] && g4[1] == g0[1], "five pages in one row of five: {g0:?} {g1:?} {g4:?}");
        assert!(g0[2] > 150.0 && g0[2] < 200.0, "{g0:?}");
        // Back to one column: as before.
        v.set_spread(0);
        assert_eq!(v.page_rect(1), single);
    }

    #[test]
    fn the_view_turns_every_page_and_back() {
        let crop = [0.0, 0.0, 100.0, 200.0];
        let mut v = Viewer::new(vec![PageBox { crop, rotation: 0, unit: 1.0 }, PageBox { crop, rotation: 90, unit: 1.0 }]);
        v.set_turn(90);
        assert_eq!((v.turn(), v.boxes[0].rotation, v.boxes[1].rotation), (90, 90, 180));
        assert_eq!(v.boxes[0].view_size(), (200.0, 100.0));
        // Back past 0: 270.
        v.set_turn(-90);
        assert_eq!((v.turn(), v.boxes[0].rotation, v.boxes[1].rotation), (270, 270, 0));
        v.set_turn(0);
        assert_eq!((v.boxes[0].rotation, v.boxes[1].rotation), (0, 90));
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
        // The internal zoom (relative to the fit width) is held to 1–1600 % (1 %: Fit page on a very tall
        // page); the steps shown are actual sizes, 25–400 %.
        assert!(v.set_zoom(1));
        assert!(!v.set_zoom(0), "clamped to 1");
        assert!(v.set_zoom(50));
        // Jumping to a page puts it at the top, never past the end.
        let top = v.scroll_to(1, None, 400.0);
        assert_eq!(top, v.page_rect(1)[1] - PAGE_GAP);
        assert_eq!(v.scroll_to(2, None, 1e6), 0.0);
    }

    #[test]
    fn a_user_unit_page_maps_view_points_as_sign_and_edit_do() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("u.pdf");
        std::fs::write(&path, crate::viewer::tests::pdf_with_page("/MediaBox [0 0 200 100] /UserUnit 2 /Rotate 90")).unwrap();
        let boxes = page_boxes(&std::sync::Arc::new(std::fs::read(&path).unwrap()), None).unwrap();
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

    /// Pages across (PDF Expert's Horizontal scrolling): one strip, page 2 right of page 1, the
    /// pages in view and the one under the middle found from the horizontal scroll.
    #[test]
    fn pages_run_across_in_one_strip_scrolled_sideways() {
        let pages = |n| (0..n).map(|_| PageBox { crop: A4, rotation: 0, unit: 1.0 }).collect::<Vec<_>>();
        let mut v = Viewer::new(pages(40));
        v.set_width(1000.0);
        let down = (v.page_rect(1), v.content_height());
        assert!(v.set_horizontal(true));
        let (p0, p1, p39) = (v.page_rect(0), v.page_rect(1), v.page_rect(39));
        assert!(p0[1] == p1[1] && p1[0] > p0[0] + p0[2], "page 2 right of page 1: {p0:?} {p1:?}");
        assert_eq!(p0[2], down.0[2], "the same width as down a column");
        assert_eq!(v.content_height(), p0[3] + 2.0 * PAGE_GAP, "one page high");
        assert_eq!(v.content_width(), p39[0] + p39[2] + PAGE_GAP);
        assert!(v.max_scroll_x() > 30.0 * p0[2]);
        // Scrolled to page 21: it and its neighbours are wanted, not page 1; the vertical scroll
        // plays no part.
        let x = v.scroll_x_to(20);
        let r = v.page_rect(20);
        assert!((x + 500.0 - (r[0] + r[2] / 2.0)).abs() < 1.0, "page 21 centred: {x} {r:?}");
        v.set_left(x);
        assert_eq!(v.current(0.0, 700.0), 20);
        let w = v.wanted(0.0, 700.0);
        assert!(w.contains(&19) && w.contains(&20) && w.contains(&21) && !w.contains(&0), "{w:?}");
        assert!(w.len() <= 6, "{w:?}");
        assert_eq!(v.wanted_at(0.0, 0.0, 700.0).start, 0);
        // A zoom keeps a point of a page across too.
        let a = v.anchor_of(x + 300.0, 200.0);
        assert_eq!(a.0, 20);
        v.set_zoom(150);
        let (px, _) = v.point_of(a);
        assert!(v.anchor_of(px, v.point_of(a).1).0 == 20);
        v.set_zoom(100);
        // Two pages: pairs side by side in the strip, a step is a pair.
        v.set_spread(1);
        let (q0, q1, q2) = (v.page_rect(0), v.page_rect(1), v.page_rect(2));
        assert!(q1[0] > q0[0] && q2[0] > q1[0] && q0[1] == q2[1]);
        assert_eq!((v.row_of(0), v.row_of(1), v.row_of(2)), (0, 0, 1));
        v.set_left(v.scroll_x_to(3));
        assert_eq!(v.current(0.0, 700.0), 2, "the pair's first page");
        // Right to left: the strip starts at its right end.
        v.set_spread(0);
        v.set_rtl(true);
        let (r0, r1) = (v.page_rect(0), v.page_rect(1));
        assert!(r1[0] + r1[2] < r0[0], "page 2 left of page 1: {r0:?} {r1:?}");
        assert!(v.scroll_x_to(0) > v.scroll_x_to(39));
        v.set_left(v.scroll_x_to(5));
        assert_eq!(v.current(0.0, 700.0), 5);
        assert!(v.wanted(0.0, 700.0).contains(&5));
        v.set_rtl(false);
        // The grid stays a grid; back down the column, as before.
        v.set_spread(3);
        assert!(v.page_rect(10)[1] > v.page_rect(0)[1]);
        v.set_spread(0);
        assert!(v.set_horizontal(false));
        assert_eq!((v.page_rect(1), v.content_height()), down);
        assert_eq!(v.scroll_x_to(5), 0.0);
    }

    /// A short document across: the strip centred in a wider view.
    #[test]
    fn a_short_strip_is_centred() {
        let mut v = Viewer::new(vec![PageBox { crop: A4, rotation: 0, unit: 1.0 }; 2]);
        v.set_width(1000.0);
        v.set_horizontal(true);
        v.set_zoom(30);
        let (a, b) = (v.page_rect(0), v.page_rect(1));
        assert!((a[0] - (1000.0 - (b[0] + b[2]))).abs() <= 1.0, "{a:?} {b:?}");
        assert_eq!((v.content_width(), v.max_scroll_x()), (1000.0, 0.0));
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

}
