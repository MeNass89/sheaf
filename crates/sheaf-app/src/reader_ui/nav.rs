//! Navigation: pages, links and history, zoom fitting, remembered position, Escape, select all.

use super::*;

/// The tab closes: a running search stops.
pub fn stop_work(tab_id: i32) {
    with(|a| {
        if let Some(r) = a.tab(tab_id).and_then(|t| t.reader.as_ref()) {
            r.search_live.store(u64::MAX, std::sync::atomic::Ordering::Relaxed);
        }
    });
}

/// Keeps the page a document tab shows, for its next opening.
pub fn remember_position(tab_id: i32) {
    if let Some((path, page, fit, zoom, spread)) = with(|a| {
        let t = a.tab(tab_id)?;
        let (r, v) = (t.reader.as_ref()?, t.viewer.as_ref()?);
        // Right to left rides on the spread's byte (bit 4), so older preferences read as before.
        // The grid is a passing view: the layout under it is kept.
        let spread = r.grid_prev.unwrap_or(v.viewer.spread());
        // A zoom chosen is kept as its actual size (tenths of a percent, mode 3): a share of the
        // window's width would change with the window (Sol, 25/10).
        let (fit, zoom) = match r.zoom_mode {
            2 => (3, (v.viewer.actual_percent(v.current()) * 10.0).round() as u32),
            m => (m, v.viewer.zoom()),
        };
        Some((r.path.clone(), v.current(), fit, zoom, spread | (v.viewer.rtl() as u8) << 4))
    }) {
        crate::home::remember_page(&path, page);
        crate::home::remember_view(&path, fit, zoom, spread);
    }
}

/// Whether the content point (`x`, `y`) is over a link (the hand cursor).
/// Where the link under the content point (`x`, `y`) goes ("Page 12", the web address), as the
/// tooltip PDF Expert shows; empty when there is none.
pub fn over_link(tab_id: i32, x: f32, y: f32) -> slint::SharedString {
    with(|a| {
        let t = a.tab(tab_id)?;
        let (r, v) = (t.reader.as_ref()?, t.viewer.as_ref()?);
        let page = v.viewer.hit(x as f64, y as f64)?.0;
        let p = to_view(&v.viewer, page, x as f64, y as f64);
        Some(match link_at(r, &v.viewer, page, p)? {
            render::LinkTarget::Page(p, _) => format!("Page {}", label(r, p)),
            render::LinkTarget::Uri(u) => u,
            _ => "Link".into(),
        })
    })
    .unwrap_or_default()
    .into()
}

/// Goes to `page`, remembering where the reader was for ⌘[ (the point in the page, Acrobat's
/// Previous View).
pub(super) fn jump(r: &mut ReaderUi, v: &mut ViewerUi, page: usize, focus: Option<[f64; 4]>) {
    remember_here(r, v);
    v.show_page(page, focus);
}

/// The place and zoom the reader leaves, for ⌘[.
fn remember_here(r: &mut ReaderUi, v: &ViewerUi) {
    let cur = (v.place(), v.viewer.zoom());
    if r.back.last() != Some(&cur) {
        r.back.push(cur);
    }
    r.forward.clear();
}

/// [`jump`] to a destination: its zoom, top and left edge (Acrobat's /XYZ left top zoom).
pub(super) fn jump_dest(r: &mut ReaderUi, v: &mut ViewerUi, page: usize, view: &render::DestView) {
    remember_here(r, v);
    if v.show_dest(page, view) {
        // A zoom of the destination's: the window's opening size no longer applies.
        r.zoom_mode = 2;
        v.pending_fit = false;
        v.pending_actual = None;
    }
}

/// ⌘[ (back, `dir` −1) and ⌘] (forward, +1).
pub fn history(tab_id: i32, dir: i32) {
    update(tab_id, |r, v| {
        let cur = (v.place(), v.viewer.zoom());
        let to = if dir < 0 { r.back.pop() } else { r.forward.pop() };
        if let Some((p, zoom)) = to {
            if dir < 0 { r.forward.push(cur) } else { r.back.push(cur) }
            // The zoom it was left at, too (Sol, 25/10: a link that zoomed kept its zoom on ⌘[).
            if v.viewer.set_zoom(zoom) {
                r.zoom_mode = 2;
            }
            v.go_to_place(p);
        }
    });
}


pub fn go_to_page(tab_id: i32, text: &str) {
    update(tab_id, |r, v| {
        let t = text.trim();
        let n = v.viewer.page_count();
        // A page label ("iv", "A-3", or "1" where the book's page 1 is the 13th), else a number.
        let page = r.snap.labels.iter().position(|l| !l.is_empty() && l.eq_ignore_ascii_case(t)).or_else(|| t.parse::<usize>().ok().filter(|p| (1..=n).contains(p)).map(|p| p - 1));
        if let Some(p) = page {
            jump(r, v, p, None);
        }
    });
}

pub fn page_step(tab_id: i32, step: i32) {
    update(tab_id, |r, v| {
        let n = v.viewer.page_count();
        if n == 0 {
            return;
        }
        let cur = v.current();
        // ±3: a page from the wheel, one per notch however many events it comes as.
        let step = match step {
            3 | -3 if r.wheel_turned.is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(250)) => return,
            3 | -3 => {
                r.wheel_turned = Some(std::time::Instant::now());
                step / 3
            }
            s => s,
        };
        match step {
            10 => v.show_page(n - 1, None),
            -10 => v.show_page(0, None),
            2 | -2 => {
                let h = v.height * 0.9;
                v.scroll = (v.top + if step > 0 { h } else { -h }).clamp(0.0, (v.viewer.content_height() - v.height).max(0.0));
                v.scroll_seq += 1;
            }
            s => {
                // From the top of the current page: ← goes to its start when it is scrolled into.
                // A step is a row: with two pages side by side, → goes to the next pair.
                let row_y = |i: usize| v.viewer.page_rect(i)[1];
                let top_of = row_y(cur) - viewer::PAGE_GAP;
                let target = if s < 0 && v.top > top_of + 4.0 {
                    cur
                } else if s > 0 {
                    (cur + 1..n).find(|&i| row_y(i) > row_y(cur)).unwrap_or(n - 1)
                } else {
                    (0..cur).rev().find(|&i| row_y(i) < row_y(cur)).map(|i| (0..=i).find(|&j| row_y(j) == row_y(i)).unwrap_or(i)).unwrap_or(0)
                };
                v.show_page(target, None);
            }
        }
    });
}

pub fn scroll_by(tab_id: i32, dy: f32) {
    update(tab_id, |_, v| {
        v.scroll = (v.top + dy as f64).clamp(0.0, (v.viewer.content_height() - v.height).max(0.0));
        v.scroll_seq += 1;
    });
}

/// PDF Expert's View menu: fit width (0, ⌘9), fit page (1, ⌘8), 100 % actual size (2, ⌘0).
pub fn zoom_fit(ui: &AppWindow, tab_id: i32, mode: i32) {
    let z = update(tab_id, |r, v| {
        let cur = v.current();
        r.zoom_mode = if mode == 2 { 2 } else { mode.clamp(0, 1) };
        match mode {
            1 => v.viewer.fit_page_zoom(v.height, cur..cur + 1),
            2 => v.viewer.zoom_for_actual(cur, 100.0),
            _ => 100,
        }
    });
    if let Some(z) = z {
        crate::viewer_ui::zoom_to(ui, tab_id, z);
        update(tab_id, |_, v| {
            let cur = v.current();
            v.show_page(cur, None);
        });
    }
}

/// Pages side by side, in turn: one column, two pages, two pages with the cover alone (PDF
/// Expert's Two Pages and Book mode). The page in view stays in view.
pub fn cycle_spread(ui: &AppWindow, tab_id: i32) {
    let next = with(|a| a.tab(tab_id)?.viewer.as_ref().map(|v| (v.viewer.spread() + 1) % 3));
    if let Some(next) = next {
        set_spread(ui, tab_id, next as i32);
    }
}

/// More ▸ Page grid (PDF Expert's page thumbnails): every page small, as many to a row as fit;
/// a click selects (⇧ a run), a double-click opens that page, the Pages panel's actions and
/// Delete act on the selection. Again: back to the layout it came from.
pub fn toggle_grid(ui: &AppWindow, tab_id: i32) {
    let done = update(tab_id, |r, v| {
        let cur = v.current();
        match r.grid_prev.take() {
            Some(prev) => {
                v.viewer.set_spread(prev);
            }
            None => {
                r.grid_prev = Some(v.viewer.spread());
                v.viewer.set_spread(3);
                v.viewer.set_zoom(100);
                r.zoom_mode = 0;
                // The opening size (Fit page, Actual size) must not come back over the grid.
                v.pending_fit = false;
                v.pending_actual = None;
            }
        }
        v.show_page(cur, None);
    });
    if done.is_some() {
        crate::viewer_ui::request(ui, tab_id);
    }
}

/// A double-click in the page grid: the grid closes on `page`.
pub(super) fn open_from_grid(r: &mut ReaderUi, v: &mut ViewerUi, page: usize) {
    if let Some(prev) = r.grid_prev.take() {
        v.viewer.set_spread(prev);
    }
    v.show_page(page, None);
}

/// More ▸ Rotate view (Acrobat's View ▸ Rotate View, ⇧⌘+ and ⇧⌘−): every page shown turned a quarter
/// clockwise (`dir` 1) or back (−1), the file untouched; the page in view stays in view.
pub fn rotate_view(ui: &AppWindow, tab_id: i32, dir: i32) {
    let done = update(tab_id, |_, v| {
        let cur = v.current();
        let turn = i32::from(v.viewer.turn()) + 90 * dir.signum();
        v.viewer.set_turn(turn);
        v.all_changed();
        v.show_page(cur, None);
    });
    if done.is_some() {
        crate::viewer_ui::request(ui, tab_id);
    }
}

/// More ▸ Right to left: pairs read from the right, kept with the document's view.
pub fn toggle_rtl(ui: &AppWindow, tab_id: i32) {
    let done = update(tab_id, |_, v| {
        let cur = v.current();
        let rtl = !v.viewer.rtl();
        v.viewer.set_rtl(rtl);
        v.show_page(cur, None);
    });
    if done.is_some() {
        crate::viewer_ui::request(ui, tab_id);
    }
}

/// Pages side by side as `mode` (0 one column, 1 two pages, 2 cover alone); a presentation shows one
/// page at a time and puts the mode back when it ends.
pub fn set_spread(ui: &AppWindow, tab_id: i32, mode: i32) {
    let done = update(tab_id, |r, v| {
        let cur = v.current();
        v.viewer.set_spread(mode.clamp(0, 2) as u8);
        // Fit page keeps fitting: the pages are narrower now.
        if r.zoom_mode == 1 {
            let z = v.viewer.fit_page_zoom(v.height, cur..cur + 1);
            v.viewer.set_zoom(z);
        }
        v.show_page(cur, None);
    });
    if done.is_some() {
        crate::viewer_ui::request(ui, tab_id);
    }
}

/// A zoom typed in the zoom field ("150", "150 %") or picked from its list: percent of actual size,
/// as PDF Expert shows it; anything else leaves the zoom as it is.
pub fn zoom_typed(ui: &AppWindow, tab_id: i32, text: &str) {
    let Some(pct) = reader::parse_zoom(text) else { return };
    let z = with(|a| {
        let v = a.tab(tab_id)?.viewer.as_deref()?;
        Some(v.viewer.zoom_for_actual(v.current(), pct as f64))
    });
    if let Some(z) = z {
        crate::viewer_ui::zoom_to(ui, tab_id, z);
        zoom_changed(tab_id);
    }
}

/// A zoom chosen with + / − or the wheel: the view no longer follows the window's size.
pub fn zoom_changed(tab_id: i32) {
    update(tab_id, |r, _| r.zoom_mode = 2);
}

/// Fit page is kept when the window or the panel changes size, as in PDF Expert.
pub(super) fn keep_fit(ui: &AppWindow, tab_id: i32) {
    let z = with(|a| {
        let t = a.tab(tab_id)?;
        let (r, v) = (t.reader.as_ref()?, t.viewer.as_ref()?);
        if r.zoom_mode != 1 {
            return None;
        }
        let cur = v.current();
        let z = v.viewer.fit_page_zoom(v.height, cur..cur + 1);
        (z != v.viewer.zoom()).then_some(z)
    });
    if let Some(z) = z {
        crate::viewer_ui::zoom_to(ui, tab_id, z);
    }
}

pub fn escape(tab_id: i32) {
    update(tab_id, |r, _| {
        if r.draft.take().is_some() || std::mem::take(&mut r.zoom_area) | std::mem::take(&mut r.snap_area) | std::mem::take(&mut r.auto_scroll) {
            return;
        }
        // A polygon being clicked out is dropped.
        if r.poly.take().is_some() {
            r.stroke = None;
            return;
        }
        if r.selected_image.take().is_some() | r.picked.take().is_some() {
            return;
        }
        // Prepare form: the fields selected are let go first, the tool stays.
        if r.form_sel.take().is_some() {
            r.form_also.clear();
            return;
        }
        if r.sel.take().is_some() || r.selected_annot.take().is_some() {
            return;
        }
        r.tool = SELECT;
    });
}

/// Zoom to area (Acrobat's Marquee Zoom, Z): the next box dragged on a page fills the view; Z
/// again or Escape lets it go.
pub fn zoom_area(ui: &AppWindow, tab_id: i32) {
    if update(tab_id, |r, _| {
        r.zoom_area = !r.zoom_area;
        r.snap_area = false;
        r.zoom_area
    }) == Some(true)
    {
        crate::flow::toast(ui, "Drag over the part of the page to zoom to");
    }
}

/// More ▸ Copy an area as a picture (Acrobat's Snapshot): the next box dragged on a page goes to
/// the clipboard as the page shows it, at 216 dpi.
pub fn snap_area(ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| {
        r.snap_area = true;
        r.zoom_area = false;
    });
    crate::flow::toast(ui, "Drag over the part of the page to copy as a picture");
}

/// More ▸ One page at a time (Acrobat's Single Page): the page shown fits the view, the wheel
/// and the arrow keys turn pages; off, the zoom stays and scrolling is continuous again.
pub fn page_at_a_time(ui: &AppWindow, tab_id: i32) {
    let on = update(tab_id, |r, v| {
        r.page_at_a_time = !r.page_at_a_time;
        if r.page_at_a_time {
            let cur = v.current();
            v.viewer.set_spread(0);
            v.show_page(cur, None);
        }
        r.page_at_a_time
    });
    if on == Some(true) {
        zoom_fit(ui, tab_id, 1);
    }
}

/// More ▸ Scroll automatically (Acrobat's): on or off; Escape stops it too.
pub fn auto_scroll(tab_id: i32) {
    update(tab_id, |r, _| r.auto_scroll = !r.auto_scroll);
}

pub fn select_all(tab_id: i32) {
    update(tab_id, |r, v| {
        let page = v.current();
        r.sel = r.text(page).filter(|t| !t.glyphs.is_empty()).map(|t| (page, 0..t.glyphs.len()));
    });
}
