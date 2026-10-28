//! Edit PDF panel: the text and images placed on the pages in the viewer, which one is selected,
//! and the job items built from them. Positions are in view space: points from the top-left of
//! the page as displayed (y down), as the page viewer and the Sign tool use.

use std::path::PathBuf;

use sheaf_app::jobs::EditPlaced;
use sheaf_app::viewer::Viewer;
use sheaf_app::{EditOverlay, ToolTab};
use sheaf_core::edit::{self, Picture};
use slint::{Color, ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::state::{refresh, with};

/// The original's text sizes ("Text size" menu), in points.
pub const SIZES: [f64; 16] = [4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 18.0, 24.0, 36.0, 48.0, 64.0, 72.0, 96.0, 144.0, 192.0, 200.0];
/// Index of 12 pt in [`SIZES`].
const DEFAULT_SIZE: usize = 4;
/// Text colours: black, grey, red, blue (the watermark palette).
pub const COLOURS: [[u8; 3]; 4] = [[0, 0, 0], [128, 128, 128], [217, 26, 26], [26, 77, 217]];
/// Image widths in points for Small, Medium and Large.
pub const WIDTHS: [f64; 3] = [100.0, 150.0, 250.0];
/// Pointer slack around an item when picking it, in points.
const SLACK: f64 = 3.0;

#[derive(Clone)]
pub enum Kind {
    /// `colour`: RGB, one of [`COLOURS`] or one typed in hex.
    Text { text: String, size: usize, colour: [u8; 3] },
    Image { path: PathBuf, aspect: f64, width: usize, image: Image },
}

#[derive(Clone)]
pub struct Item {
    pub page: usize,
    /// The page's size as displayed, in points: the item is kept on it.
    pub page_size: (f64, f64),
    /// Top-left corner, view space.
    pub at: (f64, f64),
    pub kind: Kind,
}

impl Item {
    /// Width and height in points as displayed. An empty text keeps a box to click on.
    pub fn size(&self) -> (f64, f64) {
        match &self.kind {
            Kind::Text { text, size, .. } => {
                let pt = SIZES[*size];
                let (w, h) = edit::text_extent(if text.is_empty() { "Your text" } else { text }, pt);
                (w.max(pt), h)
            }
            Kind::Image { aspect, width, .. } => (WIDTHS[*width], WIDTHS[*width] * aspect),
        }
    }

    fn contains(&self, page: usize, p: (f64, f64)) -> bool {
        let (w, h) = self.size();
        page == self.page && p.0 >= self.at.0 - SLACK && p.0 <= self.at.0 + w + SLACK && p.1 >= self.at.1 - SLACK && p.1 <= self.at.1 + h + SLACK
    }
}

/// A press on an item, to move it on release.
pub struct Grab {
    item: usize,
    /// Where the pointer went down, and the item's corner then (view space).
    from: (f64, f64),
    at: (f64, f64),
}

/// The image chosen for the next placement.
pub struct Chosen {
    pub path: PathBuf,
    pub aspect: f64,
    pub image: Image,
}

pub struct EditState {
    /// 0 Add text, 1 Add image.
    pub mode: i32,
    pub items: Vec<Item>,
    pub selected: Option<usize>,
    /// Bumped when the selection changes, so the panel reloads the selected text.
    pub selected_seq: i32,
    /// For the next text placed (and shown when no text is selected).
    pub size: usize,
    pub colour: [u8; 3],
    pub width: usize,
    pub image: Option<Chosen>,
    pub grab: Option<Grab>,
}

impl EditState {
    pub fn new() -> Self {
        Self { mode: 0, items: Vec::new(), selected: None, selected_seq: 0, size: DEFAULT_SIZE, colour: COLOURS[0], width: 1, image: None, grab: None }
    }

    fn select(&mut self, i: Option<usize>) {
        self.selected = i;
        self.selected_seq += 1;
        // The panel shows the selected item's settings.
        match i.map(|i| &self.items[i].kind) {
            Some(Kind::Text { size, colour, .. }) => {
                self.mode = 0;
                (self.size, self.colour) = (*size, *colour);
            }
            Some(Kind::Image { width, .. }) => {
                self.mode = 1;
                self.width = *width;
            }
            None => {}
        }
    }

    /// The topmost item under `p` on `page`.
    fn hit(&self, page: usize, p: (f64, f64)) -> Option<usize> {
        (0..self.items.len()).rev().find(|&i| self.items[i].contains(page, p))
    }

    /// A press at `p` (view space of `page`): grabs the item there, if any.
    pub fn press(&mut self, page: usize, p: (f64, f64)) {
        self.grab = self.hit(page, p).map(|i| Grab { item: i, from: p, at: self.items[i].at });
    }

    /// A release at `p` on the page pressed (`page_size` as displayed): moves the grabbed item by
    /// the drag (a click just selects it), or places a new item at a click on empty page.
    pub fn release(&mut self, page: usize, p: (f64, f64), page_size: (f64, f64)) {
        if let Some(Grab { item: i, from, at }) = self.grab.take() {
            self.items[i].page_size = page_size;
            self.items[i].at = (at.0 + p.0 - from.0, at.1 + p.1 - from.1);
            self.keep_on_page(i);
            self.select(Some(i));
            return;
        }
        let item = match self.mode {
            0 => Item { page, page_size, at: p, kind: Kind::Text { text: String::new(), size: self.size, colour: self.colour } },
            _ => {
                let Some(c) = &self.image else {
                    // Nothing to place yet: a click only clears the selection.
                    self.select(None);
                    return;
                };
                let (w, h) = (WIDTHS[self.width], WIDTHS[self.width] * c.aspect);
                // Centred on the click, as Sign places its signature.
                let kind = Kind::Image { path: c.path.clone(), aspect: c.aspect, width: self.width, image: c.image.clone() };
                Item { page, page_size, at: (p.0 - w / 2.0, p.1 - h / 2.0), kind }
            }
        };
        self.items.push(item);
        let i = self.items.len() - 1;
        self.keep_on_page(i);
        self.select(Some(i));
    }

    /// Moves item `i` back onto its page where it fits.
    fn keep_on_page(&mut self, i: usize) {
        let (pw, ph) = self.items[i].page_size;
        let (w, h) = self.items[i].size();
        let at = &mut self.items[i].at;
        *at = (at.0.clamp(0.0, (pw - w).max(0.0)), at.1.clamp(0.0, (ph - h).max(0.0)));
    }

    /// Why the job can't run yet, if it can't.
    pub fn problem(&self) -> Option<String> {
        self.placed().err()
    }

    /// The job's items. Refused with a message the user can act on when one is not ready.
    pub fn placed(&self) -> Result<Vec<EditPlaced>, String> {
        if self.items.is_empty() {
            return Err("Click on a page to add text or an image".into());
        }
        self.items
            .iter()
            .map(|it| match &it.kind {
                Kind::Text { text, size, colour } => {
                    if text.trim().is_empty() {
                        return Err(format!("A text box on page {} is empty: type its text or remove it", it.page + 1));
                    }
                    if let Some(why) = edit::text_problem(text) {
                        return Err(format!("A text box on page {}: {why}", it.page + 1));
                    }
                    Ok(EditPlaced::Text { page: it.page, at: [it.at.0, it.at.1], text: text.clone(), size: SIZES[*size], color: *colour })
                }
                Kind::Image { path, .. } => {
                    let (w, h) = it.size();
                    Ok(EditPlaced::Image { page: it.page, rect: [it.at.0, it.at.1, it.at.0 + w, it.at.1 + h], path: path.clone() })
                }
            })
            .collect()
    }
}

/// A content point as page `page`'s view-space point in points, held to the page.
pub fn view_point(viewer: &Viewer, page: usize, x: f64, y: f64) -> (f64, f64) {
    let [px, py, w, h] = viewer.page_rect(page);
    let (vw, vh) = viewer.boxes[page].view_size();
    let k = vw / w.max(1e-9);
    (((x - px).clamp(0.0, w) * k).clamp(0.0, vw), ((y - py).clamp(0.0, h) * k).clamp(0.0, vh))
}

fn colour(rgb: [u8; 3]) -> Color {
    Color::from_rgb_u8(rgb[0], rgb[1], rgb[2])
}

/// The Edit fields of `tab`: the panel's state and, with a viewer, the items over the pages.
pub fn fill(tab: &mut ToolTab, s: &EditState, viewer: Option<&Viewer>) {
    let sel = s.selected.map(|i| &s.items[i]);
    tab.edit_mode = s.mode;
    tab.edit_count = s.items.len() as i32;
    tab.edit_selected = sel.map(|it| if matches!(it.kind, Kind::Text { .. }) { 1 } else { 2 }).unwrap_or(0);
    tab.edit_selected_page = sel.map(|it| it.page as i32 + 1).unwrap_or(0);
    tab.edit_selected_seq = s.selected_seq;
    tab.edit_text = match sel.map(|it| &it.kind) {
        Some(Kind::Text { text, .. }) => text.as_str().into(),
        _ => "".into(),
    };
    tab.edit_size = SIZES[s.size] as i32;
    // A preset is ringed; a colour typed shows its hex in the field instead.
    tab.edit_colour = COLOURS.iter().position(|c| *c == s.colour).map_or(-1, |i| i as i32);
    tab.edit_hex = if tab.edit_colour < 0 { format!("{:02X}{:02X}{:02X}", s.colour[0], s.colour[1], s.colour[2]).into() } else { "".into() };
    tab.edit_width = s.width as i32;
    tab.edit_image_name = s.image.as_ref().and_then(|c| c.path.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into();
    let problem = s.problem();
    tab.edit_ready = problem.is_none();
    // Shown once something is placed: "nothing added yet" says the rest.
    tab.edit_problem = problem.filter(|_| !s.items.is_empty()).unwrap_or_default().into();
    let Some(v) = viewer else { return };
    let overlays: Vec<EditOverlay> = s
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| it.page < v.page_count())
        .map(|(i, it)| {
            let [px, py, w, _] = v.page_rect(it.page);
            let k = w / v.boxes[it.page].view_size().0.max(1e-9);
            let (iw, ih) = it.size();
            let mut o = EditOverlay {
                x: (px + it.at.0 * k) as f32,
                y: (py + it.at.1 * k) as f32,
                w: (iw * k) as f32,
                h: (ih * k) as f32,
                selected: s.selected == Some(i),
                ..Default::default()
            };
            match &it.kind {
                Kind::Text { text, size, colour: c } => {
                    o.is_text = true;
                    o.text = text.as_str().into();
                    o.font_size = (SIZES[*size] * k) as f32;
                    o.color = colour(*c);
                }
                Kind::Image { image, .. } => o.image = image.clone(),
            }
            o
        })
        .collect();
    tab.edit_overlays = ModelRc::new(VecModel::from(overlays));
}

fn update(tab_id: i32, f: impl FnOnce(&mut EditState)) {
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            f(&mut t.edit);
            // A longer text or a bigger size can push the item past the page edge.
            if let Some(i) = t.edit.selected {
                t.edit.keep_on_page(i);
            }
        }
        refresh(a, tab_id);
    });
}

pub fn mode(tab_id: i32, mode: i32) {
    update(tab_id, |s| {
        s.mode = mode.clamp(0, 1);
        // Switching mode leaves the selected item of the other kind.
        let keep = s.selected.is_some_and(|i| matches!(s.items[i].kind, Kind::Text { .. }) == (s.mode == 0));
        if !keep {
            s.select(None);
        }
    });
}

pub fn text(tab_id: i32, new: &str) {
    update(tab_id, |s| {
        if let Some(Item { kind: Kind::Text { text, .. }, .. }) = s.selected.and_then(|i| s.items.get_mut(i)) {
            *text = new.to_string();
        }
    });
}

/// Next (+1) or previous (−1) text size, for the selected text and the next one.
pub fn size_step(tab_id: i32, step: i32) {
    update(tab_id, |s| {
        s.size = (s.size as i32 + step).clamp(0, SIZES.len() as i32 - 1) as usize;
        let n = s.size;
        if let Some(Item { kind: Kind::Text { size, .. }, .. }) = s.selected.and_then(|i| s.items.get_mut(i)) {
            *size = n;
        }
    });
}

pub fn colour_pick(tab_id: i32, c: i32) {
    set_colour(tab_id, COLOURS[c.clamp(0, COLOURS.len() as i32 - 1) as usize]);
}

/// A colour typed as RRGGBB (Acrobat's Other Color); a hex not yet complete changes nothing.
pub fn colour_typed(tab_id: i32, hex: &str) {
    if let Some(rgb) = sheaf_core::numbers::parse_hex_colour(hex) {
        set_colour(tab_id, rgb.map(|v| (v * 255.0).round() as u8));
    }
}

/// The text colour: for text added next, and the selected text at once.
fn set_colour(tab_id: i32, rgb: [u8; 3]) {
    update(tab_id, |s| {
        s.colour = rgb;
        if let Some(Item { kind: Kind::Text { colour, .. }, .. }) = s.selected.and_then(|i| s.items.get_mut(i)) {
            *colour = rgb;
        }
    });
}

pub fn width(tab_id: i32, w: i32) {
    update(tab_id, |s| {
        s.width = w.clamp(0, WIDTHS.len() as i32 - 1) as usize;
        let n = s.width;
        if let Some(Item { kind: Kind::Image { width, .. }, .. }) = s.selected.and_then(|i| s.items.get_mut(i)) {
            *width = n;
        }
    });
}

pub fn remove(tab_id: i32) {
    update(tab_id, |s| {
        if let Some(i) = s.selected {
            s.items.remove(i);
            s.select(None);
        }
    });
}

pub fn pick_image(ui: &sheaf_app::AppWindow, tab_id: i32) {
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Choose an image")).add_filter("Image", &["png", "jpg", "jpeg"]);
    crate::dialog::pick_file(ui, d, move |ui, path| picked_image(ui, tab_id, path));
}

fn picked_image(ui: &sheaf_app::AppWindow, tab_id: i32, path: std::path::PathBuf) {
    // Checked and decoded once, off the UI thread; refused here, not when the PDF is written.
    // The preview is those pixels, upright as they will be embedded.
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let r = sheaf_app::jobs::catching(|| Picture::read(&path));
        let _ = weak.upgrade_in_event_loop(move |ui| match r {
            Ok(pic) => {
                let aspect = pic.aspect();
                let (w, h) = pic.rgba.dimensions();
                let image = Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pic.rgba.as_raw(), w, h));
                update(tab_id, |s| {
                    s.image = Some(Chosen { path, aspect, image });
                    s.mode = 1;
                });
            }
            Err(e) => crate::flow::toast(&ui, &e.to_string()),
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_item(s: &mut EditState, text: &str) {
        let i = s.selected.unwrap();
        if let Kind::Text { text: t, .. } = &mut s.items[i].kind {
            *t = text.into();
        }
    }

    #[test]
    fn a_click_places_text_a_drag_moves_it_and_it_stays_on_the_page() {
        let page = (400.0, 500.0);
        let mut s = EditState::new();
        assert!(s.placed().is_err(), "nothing placed");
        s.press(0, (50.0, 60.0));
        s.release(0, (50.0, 60.0), page);
        assert_eq!((s.items.len(), s.selected, s.items[0].at), (1, Some(0), (50.0, 60.0)));
        assert!(s.placed().unwrap_err().contains("empty"));
        text_item(&mut s, "Hello");
        // Dragged by (+30, +40) from inside the box.
        s.press(0, (55.0, 65.0));
        s.release(0, (85.0, 105.0), page);
        assert_eq!((s.items.len(), s.items[0].at), (1, (80.0, 100.0)));
        // Dragged off the page: held at its edge.
        s.press(0, (85.0, 105.0));
        s.release(0, (1000.0, -50.0), page);
        let (w, _) = s.items[0].size();
        assert_eq!(s.items[0].at, (400.0 - w, 0.0));
        let EditPlaced::Text { page, at, text, size, color } = &s.placed().unwrap()[0] else { panic!() };
        assert_eq!((*page, *at, text.as_str(), *size, *color), (0, [400.0 - w, 0.0], "Hello", 12.0, [0, 0, 0]));
    }

    #[test]
    fn size_colour_and_remove_apply_to_the_selection() {
        let mut s = EditState::new();
        s.release(1, (10.0, 10.0), (600.0, 800.0));
        text_item(&mut s, "A");
        size_colour(&mut s);
        let EditPlaced::Text { size, color, .. } = &s.placed().unwrap()[0] else { panic!() };
        assert_eq!((*size, *color), (18.0, COLOURS[2]));
        // A click on empty page in text mode adds a second item, with the current settings.
        s.release(1, (300.0, 400.0), (600.0, 800.0));
        assert_eq!((s.items.len(), s.selected), (2, Some(1)));
        let Kind::Text { size, colour, .. } = &s.items[1].kind else { panic!() };
        assert_eq!((SIZES[*size], *colour), (18.0, COLOURS[2]));
        s.items.remove(1);
        s.select(None);
        assert_eq!(s.items.len(), 1);
    }

    fn size_colour(s: &mut EditState) {
        s.size = 6;
        s.colour = COLOURS[2];
        let i = s.selected.unwrap();
        if let Kind::Text { size, colour, .. } = &mut s.items[i].kind {
            (*size, *colour) = (6, COLOURS[2]);
        }
    }

    #[test]
    fn a_colour_typed_in_hex_is_kept_and_shown_as_typed() {
        let mut s = EditState::new();
        s.release(1, (10.0, 10.0), (600.0, 800.0));
        text_item(&mut s, "A");
        let rgb = sheaf_core::numbers::parse_hex_colour("2E8B57").unwrap().map(|v| (v * 255.0).round() as u8);
        s.colour = rgb;
        if let Kind::Text { colour, .. } = &mut s.items[0].kind {
            *colour = rgb;
        }
        let EditPlaced::Text { color, .. } = &s.placed().unwrap()[0] else { panic!() };
        assert_eq!(*color, [0x2E, 0x8B, 0x57]);
        assert!(!COLOURS.contains(&s.colour), "not a preset: the field shows it");
    }

    #[test]
    fn a_longer_text_or_bigger_size_stays_on_the_page_and_undrawable_text_blocks_the_job() {
        let mut s = EditState::new();
        s.release(0, (380.0, 10.0), (400.0, 500.0));
        text_item(&mut s, "A much longer line of text than fits");
        s.keep_on_page(0);
        let (w, _) = s.items[0].size();
        assert!((s.items[0].at.0 + w - 400.0).abs() < 1e-9, "held at the right edge: {:?}", s.items[0].at);
        s.size = SIZES.len() - 1;
        if let Kind::Text { size, .. } = &mut s.items[0].kind {
            *size = SIZES.len() - 1;
        }
        s.keep_on_page(0);
        assert!(s.items[0].at.0 >= 0.0 && s.items[0].at.1 >= 0.0);
        text_item(&mut s, "Snowman \u{2603}");
        let why = s.problem().unwrap();
        assert!(why.contains('\u{2603}') && why.contains("Helvetica"), "{why}");
        text_item(&mut s, "Café — 5€");
        assert!(s.problem().is_none());
    }

    #[test]
    fn an_image_needs_choosing_first_and_is_centred_on_the_click() {
        let mut s = EditState::new();
        s.mode = 1;
        s.release(0, (100.0, 100.0), (400.0, 500.0));
        assert!(s.items.is_empty(), "no image chosen yet");
        s.image = Some(Chosen { path: "/i/logo.png".into(), aspect: 0.5, image: Image::default() });
        s.release(0, (100.0, 100.0), (400.0, 500.0));
        assert_eq!(s.items[0].at, (25.0, 62.5));
        let EditPlaced::Image { rect, .. } = &s.placed().unwrap()[0] else { panic!() };
        assert_eq!(*rect, [25.0, 62.5, 175.0, 137.5]);
        // Picking it again selects it rather than adding another.
        s.press(0, (30.0, 70.0));
        s.release(0, (30.0, 70.0), (400.0, 500.0));
        assert_eq!((s.items.len(), s.selected), (1, Some(0)));
    }
}
