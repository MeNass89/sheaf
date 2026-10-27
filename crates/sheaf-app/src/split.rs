//! Split view (PDF Expert's View ▸ Split View, Acrobat's Window ▸ Split): a reader tab's pages in two
//! panes, each a viewer of its own, with its own zoom and place. The active pane is the tab's own
//! viewer, so every tool, click and shortcut works there as it does without a split; the other pane
//! is [`Split::view`], scrolled on its own or with the active one (Sync scroll). A press in the other
//! pane swaps the two: it becomes the tab's viewer, and the press goes on to it.

use sheaf_app::{AppWindow, ToolTab};
use slint::{ModelRc, VecModel};

use crate::state::{refresh, with, App, TabState};
use crate::viewer_ui::{self, ViewerUi};

pub struct Split {
    /// 1 side by side, 2 one above the other.
    pub mode: i32,
    /// The half the active pane takes: 0 the first (left or top), 1 the second.
    pub half: i32,
    /// The other pane.
    pub view: Box<ViewerUi>,
    /// Sync scroll (PDF Expert's): the panes move together.
    pub sync: bool,
    /// Bumped when the other pane's scroll is set from here (laid out again: kept on its place).
    pub seq: i32,
    /// Bumped when the panes swap: the view sets both scrolls again, each pane showing what it showed.
    pub swap: i32,
}

/// The split's fields of `tab`, as the reading view shows them.
pub fn fill(tab: &mut ToolTab, t: &TabState) {
    let Some(s) = &t.split else { return };
    let v = &s.view;
    tab.split_mode = s.mode;
    tab.split_half = s.half;
    tab.split_sync = s.sync;
    tab.split_seq = s.seq;
    tab.split_swap = s.swap;
    tab.split_pages = ModelRc::new(VecModel::from(v.pane_pages()));
    tab.split_width = v.viewer.content_width() as f32;
    tab.split_height = v.viewer.content_height() as f32;
    tab.split_scroll = v.scroll as f32;
    tab.split_scroll_x = v.scroll_x.unwrap_or(v.left) as f32;
}

/// More ▸ More tools ▸ Scrolling, split view, hand ▸ Split view side by side (`mode` 1), one above the
/// other (2), or off (0). The other pane opens on the place, zoom and look of the active one.
pub fn open(ui: &AppWindow, tab_id: i32, mode: i32) {
    with(|a| {
        let Some(t) = a.tab(tab_id) else { return };
        if mode <= 0 {
            t.split = None;
        } else if let Some(s) = t.split.as_mut() {
            s.mode = mode.min(2);
        } else if let Some(v) = t.viewer.as_deref() {
            let view = Box::new(v.twin());
            t.split = Some(Split { mode: mode.min(2), half: 0, view, sync: false, seq: 1, swap: 0 });
        }
        refresh(a, tab_id);
    });
    viewer_ui::request(ui, tab_id);
}

/// The other pane scrolled (`left`, `top`) or resized: its pages are rendered; laid out again at a new
/// size, it stays on the same point of its page.
pub fn viewport(ui: &AppWindow, tab_id: i32, left: f32, top: f32, width: f32, height: f32) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    with(|a| {
        let Some(s) = a.tab(tab_id).and_then(|t| t.split.as_mut()) else { return };
        let v = &mut s.view;
        let place = v.place();
        v.left = left as f64;
        v.viewer.set_left(left as f64);
        v.top = top as f64;
        v.height = height as f64;
        let wide = v.viewer.set_width(width as f64);
        let high = v.viewer.set_height(height as f64);
        if wide || high {
            v.go_to_place(place);
            v.top = v.scroll;
            if let Some(x) = v.scroll_x {
                v.left = x;
                v.viewer.set_left(x);
            }
            s.seq += 1;
        }
        refresh(a, tab_id);
    });
    viewer_ui::request(ui, tab_id);
}

/// A press in the other pane: it becomes the active one (the tab's viewer: tools, zoom, the page
/// field and the thumbnails are its), each pane still showing what it showed.
pub fn focus(ui: &AppWindow, tab_id: i32) {
    with(|a| {
        let Some(t) = a.tab(tab_id) else { return };
        let (Some(main), Some(s)) = (t.viewer.as_mut(), t.split.as_mut()) else { return };
        swap_panes(main, s);
        refresh(a, tab_id);
    });
    viewer_ui::request(ui, tab_id);
}

/// The panes' viewers change places (see [`focus`]).
fn swap_panes(main: &mut Box<ViewerUi>, s: &mut Split) {
    std::mem::swap(main, &mut s.view);
    main.swap_strip(&mut s.view);
    main.pane = false;
    s.view.pane = true;
    s.half = 1 - s.half;
    for v in [&mut **main, &mut *s.view] {
        v.scroll = v.top;
        v.scroll_x = Some(v.left);
    }
    s.swap += 1;
}

/// The other pane's Sync scroll button.
pub fn toggle_sync(tab_id: i32) {
    with(|a| {
        if let Some(s) = a.tab(tab_id).and_then(|t| t.split.as_mut()) {
            s.sync = !s.sync;
        }
        refresh(a, tab_id);
    });
}

/// The other pane keeps up with the viewer the reader works in (see [`ViewerUi::follow`]).
pub fn follow(a: &mut App, tab_id: i32) {
    let Some(t) = a.tab(tab_id) else { return };
    if let (Some(main), Some(s)) = (t.viewer.as_deref(), t.split.as_mut())
        && s.view.follow(main)
    {
        s.seq += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_app::viewer::{PageBox, Viewer};
    use sheaf_core::pages::Input;

    fn viewer(n: usize) -> Box<ViewerUi> {
        let pages = (0..n).map(|_| PageBox { crop: [0.0, 0.0, 595.0, 842.0], rotation: 0, unit: 1.0 }).collect();
        let mut v = Viewer::new(pages);
        v.set_width(500.0);
        Box::new(ViewerUi::new(Input::new("x.pdf"), v, 1.0, (500.0, 0.0, 700.0), (0.0, 0.0)))
    }

    /// Split view: the other pane opens where the active one is, each keeps its own zoom; a press in
    /// the other pane swaps them, each still showing what it showed.
    #[test]
    fn the_panes_have_their_own_zoom_and_swap_on_a_press() {
        let mut main = viewer(20);
        main.top = main.viewer.page_rect(5)[1];
        let mut s = Split { mode: 1, half: 0, view: Box::new(main.twin()), sync: false, seq: 1, swap: 0 };
        assert!(s.view.pane && !main.pane);
        assert_eq!(s.view.scroll, main.top, "opens on the active pane's place");
        assert_ne!(s.view.load_id, main.load_id, "renders of its own");
        // Zoomed in the active pane only.
        main.viewer.set_zoom(200);
        assert_eq!(s.view.viewer.zoom(), 100);
        s.view.top = s.view.viewer.page_rect(15)[1];
        s.view.left = 0.0;
        let (main_load, other_load, main_top) = (main.load_id, s.view.load_id, main.top);
        swap_panes(&mut main, &mut s);
        assert_eq!((main.load_id, s.view.load_id), (other_load, main_load));
        assert_eq!((main.viewer.zoom(), s.view.viewer.zoom()), (100, 200));
        assert_eq!(main.current(), 15, "the pane pressed is the tab's viewer now");
        assert_eq!((main.scroll, s.view.scroll), (main.top, main_top), "each pane stays where it was");
        assert!(!main.pane && s.view.pane);
        assert_eq!((s.half, s.swap), (1, 1));
    }

    /// The other pane follows an edit of the active one (its revisions) and a page deleted (laid out
    /// again, at its own zoom).
    #[test]
    fn the_other_pane_follows_edits_and_page_changes() {
        let mut main = viewer(10);
        let mut other = main.twin();
        other.viewer.set_zoom(150);
        main.page_changed(3);
        assert!(!other.follow(&main), "the same pages: not laid out again");
        let key = |v: &ViewerUi| v.pane_pages().len();
        assert!(key(&other) > 0);
        main.viewer = Viewer::new(main.viewer.boxes[..9].to_vec());
        assert!(other.follow(&main));
        assert_eq!((other.viewer.page_count(), other.viewer.zoom()), (9, 150));
    }
}
