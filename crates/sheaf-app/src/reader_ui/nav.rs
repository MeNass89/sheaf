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
        Some((r.path.clone(), v.current(), r.zoom_mode, v.viewer.zoom(), v.viewer.spread()))
    }) {
        crate::home::remember_page(&path, page);
        crate::home::remember_view(&path, fit, zoom, spread);
    }
}

/// Whether the content point (`x`, `y`) is over a link (the hand cursor).
pub fn over_link(tab_id: i32, x: f32, y: f32) -> bool {
    with(|a| {
        let t = a.tab(tab_id)?;
        let (r, v) = (t.reader.as_ref()?, t.viewer.as_ref()?);
        let page = v.viewer.hit(x as f64, y as f64)?.0;
        let p = to_view(&v.viewer, page, x as f64, y as f64);
        link_at(r, &v.viewer, page, p).map(|_| ())
    })
    .is_some()
}

/// Goes to `page`, remembering where the reader was for ⌘[.
pub(super) fn jump(r: &mut ReaderUi, v: &mut ViewerUi, page: usize, focus: Option<[f64; 4]>) {
    let cur = v.current();
    if r.back.last() != Some(&cur) {
        r.back.push(cur);
    }
    r.forward.clear();
    v.show_page(page, focus);
}

/// ⌘[ (back, `dir` −1) and ⌘] (forward, +1).
pub fn history(tab_id: i32, dir: i32) {
    update(tab_id, |r, v| {
        let cur = v.current();
        let to = if dir < 0 { r.back.pop() } else { r.forward.pop() };
        if let Some(p) = to {
            if dir < 0 { r.forward.push(cur) } else { r.back.push(cur) }
            v.show_page(p, None);
        }
    });
}


pub fn go_to_page(tab_id: i32, text: &str) {
    update(tab_id, |r, v| {
        let t = text.trim();
        let n = v.viewer.page_count();
        // A number, or a page label ("iv", "A-3").
        let page = t.parse::<usize>().ok().filter(|p| (1..=n).contains(p)).map(|p| p - 1).or_else(|| r.snap.labels.iter().position(|l| l.eq_ignore_ascii_case(t)));
        if let Some(p) = page {
            jump(r, v, p, None);
        }
    });
}

pub fn page_step(tab_id: i32, step: i32) {
    update(tab_id, |_, v| {
        let n = v.viewer.page_count();
        if n == 0 {
            return;
        }
        let cur = v.current();
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
    let done = update(tab_id, |r, v| {
        let cur = v.current();
        let next = (v.viewer.spread() + 1) % 3;
        v.viewer.set_spread(next);
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
        if r.draft.take().is_some() {
            return;
        }
        if r.sel.take().is_some() || r.selected_annot.take().is_some() {
            return;
        }
        r.tool = SELECT;
    });
}

pub fn select_all(tab_id: i32) {
    update(tab_id, |r, v| {
        let page = v.current();
        r.sel = r.text(page).filter(|t| !t.glyphs.is_empty()).map(|t| (page, 0..t.glyphs.len()));
    });
}
