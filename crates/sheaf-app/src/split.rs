//! Split view (PDF Expert's View ▸ Split View, Acrobat's Window ▸ Split): a reader tab's pages in two
//! panes, each a viewer of its own, with its own zoom and place. The active pane is the tab's own
//! viewer, so every tool, click and shortcut works there as it does without a split; the other pane
//! is [`Split::view`], scrolled on its own or with the active one (Sync scroll). A press in the other
//! pane swaps the two: it becomes the tab's viewer, and the press goes on to it.
//!
//! The other pane may show another open tab's document instead (PDF Expert's split of two documents):
//! a viewer of that tab's working copy, followed as it changes. A press in it goes to that tab, which
//! then shows this one in its other pane, each half still showing what it showed.

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
    /// The tab whose document the other pane shows, and its name (None: this tab's own document).
    pub other: Option<i32>,
    pub title: String,
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
    tab.split_title = s.title.as_str().into();
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
            t.split = Some(Split { mode: mode.min(2), half: 0, view, other: None, title: String::new(), sync: false, seq: 1, swap: 0 });
        }
        refresh(a, tab_id);
    });
    viewer_ui::request(ui, tab_id);
}

/// The other open documents a split can show (More ▸ … ▸ Split with): every reader tab but this one.
fn others(a: &App, tab_id: i32) -> Vec<(i32, String)> {
    a.tabs
        .iter()
        .filter(|t| t.id != tab_id && t.viewer.is_some())
        .filter_map(|t| Some((t.id, t.reader.as_ref()?.path.file_name()?.to_string_lossy().into_owned())))
        .collect()
}

/// The names [`open_with`] offers, in its order.
pub fn names(tab_id: i32) -> Vec<String> {
    with(|a| others(a, tab_id).into_iter().map(|(_, n)| n).collect())
}

/// More ▸ … ▸ Split with `k`-th other document: the other pane shows it, where its tab stands, side
/// by side (or as the split already is); again, the split closes.
pub fn open_with(ui: &AppWindow, tab_id: i32, k: i32) {
    with(|a| {
        let Some((b, title)) = others(a, tab_id).into_iter().nth(k.max(0) as usize) else { return };
        let Some(view) = a.tab(b).and_then(|t| t.viewer.as_deref()).map(|v| Box::new(v.twin())) else { return };
        let Some(t) = a.tab(tab_id) else { return };
        if t.split.as_ref().is_some_and(|s| s.other == Some(b)) {
            t.split = None;
        } else {
            let mode = t.split.as_ref().map_or(1, |s| s.mode);
            t.split = Some(Split { mode, half: 0, view, other: Some(b), title, sync: false, seq: 1, swap: 0 });
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
    let went = with(|a| {
        let t = a.tab(tab_id)?;
        let (Some(main), Some(s)) = (t.viewer.as_mut(), t.split.as_mut()) else { return None };
        if let Some(b) = s.other {
            let j = hand_over(a, tab_id, b)?;
            refresh(a, tab_id);
            refresh(a, b);
            return Some((j, b));
        }
        swap_panes(main, s);
        refresh(a, tab_id);
        None
    });
    match went {
        Some((j, b)) => {
            ui.set_active_tab(j as i32);
            viewer_ui::request(ui, b);
        }
        None => viewer_ui::request(ui, tab_id),
    }
}

/// A press in the other pane when it shows tab `b`: `b` becomes the tab in view, its own viewer where
/// the pane stood and at its size, and this tab's document in its other pane, in the other half.
/// Returns `b`'s index.
fn hand_over(a: &mut App, tab_id: i32, b: i32) -> Option<usize> {
    let (i, j) = (a.tab_index(tab_id)?, a.tab_index(b)?);
    a.tabs[j].viewer.as_ref()?;
    let mine = Box::new(a.tabs[i].viewer.as_deref()?.twin());
    let title = a.tabs[i].reader.as_ref().and_then(|r| r.path.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let s = a.tabs[i].split.take()?;
    let tb = &mut a.tabs[j];
    let vb = tb.viewer.as_deref_mut()?;
    vb.viewer = s.view.viewer.clone();
    (vb.top, vb.left, vb.height) = (s.view.top, s.view.left, s.view.height);
    vb.scroll = vb.top;
    vb.scroll_x = Some(vb.left);
    tb.split = Some(Split { mode: s.mode, half: 1 - s.half, view: mine, other: Some(tab_id), title, sync: s.sync, seq: 1, swap: s.swap + 1 });
    Some(j)
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
/// Another tab's document: that tab's viewer; the split closes with that tab.
pub fn follow(a: &mut App, tab_id: i32) {
    let Some(i) = a.tab_index(tab_id) else { return };
    let (t, main) = match a.tabs[i].split.as_ref().map(|s| s.other) {
        None => return,
        Some(None) => {
            let t = &mut a.tabs[i];
            (&mut t.split, t.viewer.as_deref())
        }
        Some(Some(b)) => {
            let Some(j) = a.tab_index(b).filter(|&j| j != i) else {
                a.tabs[i].split = None;
                return;
            };
            let (t, o) = if i < j {
                let (x, y) = a.tabs.split_at_mut(j);
                (&mut x[i], &y[0])
            } else {
                let (x, y) = a.tabs.split_at_mut(i);
                (&mut y[0], &x[j])
            };
            (&mut t.split, o.viewer.as_deref())
        }
    };
    let Some(main) = main else {
        *t = None;
        return;
    };
    if let Some(s) = t.as_mut()
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
        let mut s = Split { mode: 1, half: 0, view: Box::new(main.twin()), other: None, title: String::new(), sync: false, seq: 1, swap: 0 };
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

    /// Two documents: tab 1's other pane shows tab 2's, follows it, and a press there hands over: tab 2
    /// in view where the pane stood, tab 1's document in its other pane, in the other half; the split
    /// closes with the other tab.
    #[test]
    fn a_split_with_another_document_hands_over_and_closes_with_its_tab() {
        let mut a = App::default();
        for (id, n) in [(1, 10), (2, 20)] {
            let mut t = TabState::new(id, sheaf_core::tools::ToolId::Redact);
            t.viewer = Some(viewer(n));
            a.tabs.push(t);
        }
        let view = Box::new(a.tabs[1].viewer.as_ref().unwrap().twin());
        a.tabs[0].split = Some(Split { mode: 1, half: 0, view, other: Some(2), title: "b.pdf".into(), sync: false, seq: 1, swap: 0 });
        follow(&mut a, 1);
        assert_eq!(a.tabs[0].split.as_ref().unwrap().seq, 1, "nothing changed");
        // Tab 2 loses a page: the pane is laid out again.
        let boxes = a.tabs[1].viewer.as_ref().unwrap().viewer.boxes[..19].to_vec();
        a.tabs[1].viewer.as_mut().unwrap().viewer = Viewer::new(boxes);
        follow(&mut a, 1);
        let s = a.tabs[0].split.as_mut().unwrap();
        assert_eq!((s.seq, s.view.viewer.page_count()), (2, 19));
        s.view.top = s.view.viewer.page_rect(12)[1];
        let pane_top = s.view.top;
        assert_eq!(hand_over(&mut a, 1, 2), Some(1));
        assert!(a.tabs[0].split.is_none());
        let (v2, s2) = (a.tabs[1].viewer.as_ref().unwrap(), a.tabs[1].split.as_ref().unwrap());
        assert_eq!((v2.top, v2.scroll), (pane_top, pane_top), "tab 2 where the pane stood");
        assert_eq!((s2.other, s2.half, s2.swap), (Some(1), 1, 1));
        assert!(s2.view.pane && s2.view.viewer.page_count() == 10, "tab 1's document in the other pane");
        // Tab 1 closed: tab 2's split goes.
        a.tabs.remove(0);
        follow(&mut a, 2);
        assert!(a.tabs[0].split.is_none());
    }
}
