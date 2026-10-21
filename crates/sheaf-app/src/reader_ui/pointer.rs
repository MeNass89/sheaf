//! Pointer on the pages: press, drag and release for every tool, links under the pointer.

use super::*;

pub fn pressed(ui: &AppWindow, tab_id: i32, x: f32, y: f32, clicks: i32) {
    let (x, y) = (x as f64, y as f64);
    let opened = update(tab_id, |r, v| {
        let Some(page) = page_at(&v.viewer, x, y) else { return false };
        let p = to_view(&v.viewer, page, x, y);
        r.press = Some((page, p));
        match r.tool {
            PEN => {
                r.stroke = Some((page, vec![p]));
                false
            }
            SHAPE => {
                r.stroke = Some((page, vec![p]));
                r.shape_end = None;
                false
            }
            ERASER => {
                r.stroke = Some((page, vec![p]));
                false
            }
            NOTE | TEXTBOX => {
                r.draft = Some(Draft { page, at: p, kind: if r.tool == NOTE { 0 } else { 1 }, block: None, field: None });
                r.note_seq += 1;
                r.press = None;
                true
            }
            EDIT_TEXT => {
                // A click on a paragraph edits it, a drag moves it; a drag on an image moves the
                // image; a click on empty space adds text (decided on release).
                let size = v.viewer.boxes[page].view_size();
                // Paragraphs not read yet (just switched tool): the click adds text instead.
                let blocks = r.blocks(page).unwrap_or_default();
                let inside = |rect: [f64; 4]| {
                    let [x, y, w, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
                    (p.0 >= x - 2.0 && p.0 <= x + w + 2.0 && p.1 >= y - 2.0 && p.1 <= y + h + 2.0).then_some((x, y))
                };
                let block = blocks.iter().enumerate().find_map(|(i, b)| inside(b.rect).map(|at| EditTarget::Block(i, b.text.clone(), b.rect, at)));
                let image = || r.images.get(&page).and_then(|im| im.iter().enumerate().rev().find_map(|(i, (vis, full))| inside(*vis).map(|_| EditTarget::Image(i, *vis, *full))));
                let target = block.or_else(image).unwrap_or(EditTarget::Empty);
                r.edit_press = Some(EditPress { page, target, from: p, to: p, dragging: false });
                false
            }
            ADD_IMAGE => {
                r.press = None;
                r.image_at = Some((page, p));
                true
            }
            SIGN => {
                // Placed at once when a signature exists; otherwise it is made first.
                r.press = None;
                r.sign_at = Some((page, p));
                if saved_signature().is_none() {
                    r.sign_open = true;
                }
                false
            }
            _ => {
                if clicks == 2 {
                    // Double-click: the word.
                    r.sel = r.text(page).and_then(|t| reader::word(&t, (p.0 as f32, p.1 as f32))).map(|w| (page, w));
                    r.press = None;
                    return false;
                }
                // A form field under the pointer is filled: text and choices open an editor,
                // check boxes and radio buttons change at once.
                if r.tool == SELECT
                    && let Some((fi, rect)) = field_at(r, &v.viewer, page, p)
                {
                    r.press = None;
                    r.sel = None;
                    return open_field(r, v, page, fi, rect);
                }
                // The handle at the bottom-right corner of the selected comment resizes it.
                if r.tool == SELECT
                    && let Some(i) = r.selected_annot
                    && let Some(a) = r.annots().get(i).filter(|a| a.page == page && resizable(a))
                {
                    let size = v.viewer.boxes[page].view_size();
                    let [x, y, w, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, a.rect);
                    if (p.0 - (x + w)).abs() <= 8.0 && (p.1 - (y + h)).abs() <= 8.0 {
                        r.annot_resize = Some((i, p, p));
                        return false;
                    }
                }
                // A click (no drag) on an annotation with the select tool selects it on release;
                // a drag that starts on one selects the text under it, as in PDF Expert.
                r.annot_candidate = if r.tool == SELECT { annot_at(r, &v.viewer, page, p) } else { None };
                r.selected_annot = None;
                r.sel = None;
                false
            }
        }
    });
    if with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).is_some_and(|r| r.sign_at.is_some() && !r.sign_open)) {
        place_signature(ui, tab_id);
    }
    if opened == Some(true) && with(|a| a.tab(tab_id).and_then(|t| t.reader.as_ref()).is_some_and(|r| r.image_at.is_some())) {
        pick_image(ui, tab_id);
    }
    apply_pending_field(ui, tab_id);
}

/// Add image: the file is chosen, then placed 200 pt wide (less on a small page) at the click.
pub(super) fn pick_image(ui: &AppWindow, tab_id: i32) {
    let d = crate::dialog::new(ui).set_title("Choose an image").add_filter("Image", &["png", "jpg", "jpeg"]);
    crate::dialog::pick_file(ui, d, move |ui, path| {
        let pic = match sheaf_core::edit::Picture::read(&path) {
            Ok(p) => p,
            Err(e) => return crate::flow::toast(ui, &e.to_string()),
        };
        let edit = update(tab_id, |r, v| {
            let (page, at) = r.image_at.take()?;
            let size = v.viewer.boxes[page].view_size();
            let w = 200.0_f64.min(size.0 * 0.8);
            let h = w * pic.aspect();
            let x0 = at.0.clamp(0.0, (size.0 - w).max(0.0));
            let y0 = at.1.clamp(0.0, (size.1 - h).max(0.0));
            // Display space: points from the displayed page's bottom-left, y up.
            let rect = [x0, size.1 - (y0 + h), x0 + w, size.1 - y0];
            let name = file_name(&path);
            Some((page, Edit::AddImage { page, rect: Some(rect), name, bytes: pic.bytes.clone() }))
        })
        .flatten();
        if let Some((page, e)) = edit {
            apply(ui, tab_id, e, &[page], None);
        }
    });
}

pub fn moved(tab_id: i32, x: f32, y: f32) {
    let (x, y) = (x as f64, y as f64);
    update(tab_id, |r, v| {
        let Some((page, a)) = r.press else { return };
        let b = to_view(&v.viewer, page, x, y);
        if let Some(d) = r.annot_drag.as_mut() {
            d.2 = b;
            return;
        }
        if let Some(d) = r.annot_resize.as_mut() {
            d.2 = b;
            return;
        }
        if let Some(e) = r.edit_press.as_mut() {
            e.to = b;
            if (e.from.0 - b.0).hypot(e.from.1 - b.1) > 4.0 && !matches!(e.target, EditTarget::Empty) {
                e.dragging = true;
            }
            return;
        }
        if r.tool == SHAPE {
            r.shape_end = Some(b);
            r.stroke = Some((page, reader::shape_outline(r.shape_kind, a, b)));
            return;
        }
        if r.tool == PEN || r.tool == ERASER {
            if let Some((_, pts)) = r.stroke.as_mut() {
                let last = *pts.last().unwrap_or(&b);
                if (last.0 - b.0).hypot(last.1 - b.1) >= 0.75 {
                    pts.push(b);
                }
            }
            return;
        }
        if (a.0 - b.0).hypot(a.1 - b.1) < 1.5 {
            return;
        }
        // A drag that starts on a note, drawing, text box, shape or signature moves it; on text
        // markup it selects the text under it (PDF Expert does both the same way).
        if let Some(i) = r.annot_candidate
            && r.annots().get(i).is_some_and(|x| !matches!(x.subtype.as_str(), "Highlight" | "Underline" | "StrikeOut" | "Squiggly"))
        {
            r.annot_drag = Some((i, a, b));
            r.selected_annot = Some(i);
            return;
        }
        r.annot_candidate = None;
        r.sel = r.text(page).and_then(|t| reader::selection(&t, (a.0 as f32, a.1 as f32), (b.0 as f32, b.1 as f32))).map(|s| (page, s));
    });
}

pub fn released(ui: &AppWindow, tab_id: i32, x: f32, y: f32) {
    moved(tab_id, x, y);
    let action = update(tab_id, |r, v| {
        let press = r.press.take();
        if let Some((i, from, to)) = r.annot_resize.take() {
            let a = r.annots().get(i)?.clone();
            let b = &v.viewer.boxes[a.page];
            let size = b.view_size();
            let [x, y, w, h] = viewer::pdf_rect_to_view(b, size, a.rect);
            // The bottom-right corner follows the pointer; a box never gets smaller than 8 pt.
            let (mut w2, mut h2) = ((w + to.0 - from.0).max(8.0), (h + to.1 - from.1).max(8.0));
            if a.fill_sign && w > 0.0 && h > 0.0 {
                // A signature keeps its proportions and its top-left corner: the pull is projected
                // on the box's diagonal, so a thin signature does not jump with a small vertical move.
                let k = (1.0 + ((to.0 - from.0) * w + (to.1 - from.1) * h) / (w * w + h * h)).max(8.0 / w);
                (w2, h2) = (w * k, h * k);
            }
            let user = viewer::view_rect_to_pdf(b, size, (x, y), (x + w2, y + h2));
            // PdfCraft scales a drawing's strokes (a signature is one) as it scales a box.
            let edit = Edit::ResizeAnnotation { page: a.page, index: a.index, rect: user };
            r.selected_annot = None;
            return Some(Pending::Move(a.page, Box::new(edit)));
        }
        if let Some(e) = r.edit_press.take() {
            let b = &v.viewer.boxes[e.page];
            let size = b.view_size();
            let p0 = viewer::view_to_pdf(b, size, e.from.0, e.from.1);
            let p1 = viewer::view_to_pdf(b, size, e.to.0, e.to.1);
            let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
            match (e.target, e.dragging) {
                (EditTarget::Block(i, text, _, _), true) => {
                    let style = sheaf_core::pdfcraft::edit::BlockStyle { offset: Some([dx, dy]), ..Default::default() };
                    return Some(Pending::Move(e.page, Box::new(Edit::EditTextBlock { page: e.page, block: i, text, style })));
                }
                (EditTarget::Image(i, _, rc), true) => {
                    // The full box moves (a cropped image keeps its crop and its size).
                    let to = [rc[0] + dx, rc[1] + dy, rc[2] + dx, rc[3] + dy];
                    return Some(Pending::Move(e.page, Box::new(Edit::EditPageImage { page: e.page, index: i, change: sheaf_core::pdfcraft::engine::ImageEdit::Move(to) })));
                }
                (EditTarget::Block(i, text, rect, at), false) => {
                    r.draft = Some(Draft { page: e.page, at, kind: 2, block: Some((i, text, rect)), field: None });
                    r.note_seq += 1;
                }
                (EditTarget::Empty, _) => {
                    r.draft = Some(Draft { page: e.page, at: e.from, kind: 3, block: None, field: None });
                    r.note_seq += 1;
                }
                (EditTarget::Image(..), false) => {}
            }
            return None;
        }
        if let Some((i, from, to)) = r.annot_drag.take() {
            r.annot_candidate = None;
            let a = r.annots().get(i)?;
            let b = &v.viewer.boxes[a.page];
            let size = b.view_size();
            let p0 = viewer::view_to_pdf(b, size, from.0, from.1);
            let p1 = viewer::view_to_pdf(b, size, to.0, to.1);
            return Some(Pending::Move(a.page, Box::new(Edit::MoveAnnotation { page: a.page, index: a.index, dx: p1.0 - p0.0, dy: p1.1 - p0.1 })));
        }
        if let Some(i) = r.annot_candidate.take()
            && r.sel.is_none()
        {
            r.selected_annot = Some(i);
            // A note or text box opens to read and edit its text, as PDF Expert's pop-ups do.
            if let Some(a) = r.annots().get(i)
                && matches!(a.subtype.as_str(), "Text" | "FreeText")
            {
                let (page, index, contents, rect) = (a.page, a.index, a.contents.clone(), a.rect);
                let size = v.viewer.boxes[page].view_size();
                let [x, y, _, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
                reader_doc::debug(format_args!("open comment {index}: {contents:?}"));
                r.draft = Some(Draft { page, at: (x, y + h + 4.0), kind: 6, block: Some((index, contents, rect)), field: None });
                r.note_seq += 1;
            }
            return None;
        }
        // A click (no drag) on a link with the select tool follows it.
        if r.tool == SELECT
            && r.sel.is_none()
            && let Some((page, p)) = press
            && let Some(target) = link_at(r, &v.viewer, page, p)
        {
            return Some(Pending::Link(target));
        }
        match r.tool {
            PEN => r.stroke.take().filter(|(_, pts)| pts.len() >= 2).map(|(page, pts)| Pending::Ink(page, pts)),
            ERASER => r.stroke.take().map(|(page, pts)| Pending::Erase(page, pts)),
            STAMP => press.map(|(page, p)| Pending::Stamp(page, p)),
            SHAPE => {
                r.stroke = None;
                // A click without a drag draws nothing.
                press.zip(r.shape_end.take()).filter(|((_, a), b)| (a.0 - b.0).hypot(a.1 - b.1) >= 4.0).map(|((page, a), b)| Pending::Shape(page, a, b))
            }
            HIGHLIGHT | UNDERLINE | STRIKE if press.is_some() && r.sel.is_some() => Some(Pending::Markup(r.tool)),
            _ => None,
        }
    })
    .flatten();
    match action {
        Some(Pending::Ink(page, pts)) => add_ink(ui, tab_id, page, pts),
        Some(Pending::Shape(page, a, b)) => add_shape(ui, tab_id, page, a, b),
        Some(Pending::Erase(page, pts)) => erase(ui, tab_id, page, pts),
        Some(Pending::Stamp(page, at)) => add_stamp(ui, tab_id, page, at),
        Some(Pending::Markup(kind)) => markup(ui, tab_id, kind),
        Some(Pending::Move(page, e)) => apply(ui, tab_id, *e, &[page], None),
        Some(Pending::Link(render::LinkTarget::Page(page, view))) => {
            update(tab_id, |r, v| {
                // An /XYZ destination's top edge, when it has one, is brought near the top.
                let focus = match view {
                    render::DestView::Xyz { top: Some(t), .. } => {
                        let b = &v.viewer.boxes[page.min(v.viewer.page_count().saturating_sub(1))];
                        Some([b.crop[0], t as f64 - 1.0, b.crop[2], t as f64])
                    }
                    _ => None,
                };
                jump(r, v, page, focus);
            });
        }
        Some(Pending::Link(render::LinkTarget::Uri(u))) if u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:") => crate::run::open_detached(&u),
        Some(Pending::Link(_)) | None => {}
    }
}

/// Comments with a size handle: text boxes, rectangles, ovals and placed signatures.
pub(super) fn resizable(a: &AnnotEntry) -> bool {
    a.fill_sign || matches!(a.subtype.as_str(), "FreeText" | "Square" | "Circle")
}

pub(super) enum Pending {
    Ink(usize, Vec<(f64, f64)>),
    /// A stamp placed with a click (view point).
    Stamp(usize, (f64, f64)),
    /// The eraser's path on a page (view points).
    Erase(usize, Vec<(f64, f64)>),
    /// A shape dragged on a page, from and to (view points).
    Shape(usize, (f64, f64), (f64, f64)),
    Markup(i32),
    Move(usize, Box<Edit>),
    Link(render::LinkTarget),
}

/// The link under view point `p` of `page`.
pub(super) fn link_at(r: &ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) -> Option<render::LinkTarget> {
    let size = v.boxes[page].view_size();
    r.snap.links.iter().filter(|l| l.page == page).find_map(|l| {
        let [x, y, w, h] = viewer::pdf_rect_to_view(&v.boxes[page], size, l.rect.map(f64::from));
        (p.0 >= x && p.0 <= x + w && p.1 >= y && p.1 <= y + h).then(|| l.target.clone())
    })
}
