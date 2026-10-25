//! Pointer on the pages: press, drag and release for every tool, links under the pointer.

use super::*;

/// `clicks`: 1, or 2 on a double-click; plus 16 when ⇧ is held.
pub fn pressed(ui: &AppWindow, tab_id: i32, x: f32, y: f32, clicks: i32) {
    let (x, y) = (x as f64, y as f64);
    let (clicks, shift) = (clicks & 15, clicks & 16 != 0);
    let opened = update(tab_id, |r, v| {
        let Some(page) = page_at(&v.viewer, x, y) else { return false };
        let p = to_view(&v.viewer, page, x, y);
        r.press = Some((page, p));
        match r.tool {
            PEN => {
                r.stroke = Some((page, vec![p]));
                false
            }
            SHAPE | LINK => {
                r.stroke = Some((page, vec![p]));
                r.shape_end = None;
                r.shape_shift = shift;
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
                // image; a click on empty space adds text, a drag there draws a marquee (decided
                // on release). ⇧-click picks objects together; a drag on picked ones moves them all.
                let size = v.viewer.boxes[page].view_size();
                let user = viewer::view_to_pdf(&v.viewer.boxes[page], size, p.0, p.1);
                let objects = r.objects.get(&page).cloned().unwrap_or_default();
                let rects: Vec<[f64; 4]> = objects.iter().map(|o| o.1).collect();
                if shift {
                    if let Some(i) = reader::object_at(&rects, user) {
                        let mut list = match r.picked.take() {
                            Some((pp, l)) if pp == page => l,
                            _ => Vec::new(),
                        };
                        match list.iter().position(|o| o.0 == objects[i].0) {
                            Some(k) => {
                                list.remove(k);
                            }
                            None => list.push(objects[i]),
                        }
                        r.picked = (!list.is_empty()).then_some((page, list));
                    }
                    r.selected_image = None;
                    r.press = None;
                    return false;
                }
                // A side of the one picked paragraph: pulled, it rewraps to the new width.
                if let Some(corner) = paragraph_corner_at(r, v, page, p) {
                    r.para_resize = Some((corner, p, p));
                    return false;
                }
                // Several picked, or drawings (no other way to move them): a drag moves them all.
                if let Some((pp, list)) = &r.picked
                    && *pp == page
                    && (list.len() > 1 || list.iter().all(|o| o.0.kind == sheaf_core::pdfcraft::engine::ObjectKind::Path))
                    && reader::object_at(&list.iter().map(|o| o.1).collect::<Vec<_>>(), user).is_some()
                {
                    r.picked_drag = Some((p, p));
                    return false;
                }
                // A corner of the selected image resizes it.
                if let Some(corner) = image_corner_at(r, v, page, p) {
                    r.image_resize = Some((corner, p, p));
                    return false;
                }
                // Paragraphs not read yet (just switched tool): the click adds text instead.
                let blocks = r.blocks(page).unwrap_or_default();
                let inside = |rect: [f64; 4]| {
                    let [x, y, w, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[page], size, rect);
                    (p.0 >= x - 2.0 && p.0 <= x + w + 2.0 && p.1 >= y - 2.0 && p.1 <= y + h + 2.0).then_some((x, y))
                };
                let block = blocks.iter().enumerate().find_map(|(i, b)| inside(b.rect).map(|at| EditTarget::Block(i, b.text.clone(), b.drawn.clone(), b.rect, at)));
                let image = || r.images.get(&page).and_then(|im| im.iter().enumerate().rev().find_map(|(i, (vis, full))| inside(*vis).map(|_| EditTarget::Image(i, *vis, *full))));
                let target = block.or_else(image).unwrap_or(EditTarget::Empty);
                // On a drawing (no paragraph or image there): it is picked, a drag moves it.
                if matches!(target, EditTarget::Empty)
                    && let Some(o) = drawing_at(&objects, &v.viewer.boxes[page], user)
                {
                    r.picked = Some((page, vec![o]));
                    r.picked_drag = Some((p, p));
                    r.selected_image = None;
                    return false;
                }
                // An image is selected as soon as it is pressed (PDF Expert), to drag or resize;
                // it is also the one object picked (the arrow keys nudge it).
                r.selected_image = match target {
                    EditTarget::Image(i, ..) => Some((page, i)),
                    _ => None,
                };
                r.picked = match target {
                    EditTarget::Image(_, vis, _) => reader::picked_again(&[vis], &rects).first().map(|&i| (page, vec![objects[i]])),
                    _ => None,
                };
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

/// The image files Add image and Replace take (others than PNG and JPEG are stored as PNG).
pub(super) const IMAGE_TYPES: &[&str] = &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"];

/// Add image: the file is chosen, then placed 200 pt wide (less on a small page) at the click.
pub(super) fn pick_image(ui: &AppWindow, tab_id: i32) {
    let d = crate::dialog::new(ui).set_title("Choose an image").add_filter("Image", IMAGE_TYPES);
    crate::dialog::pick_file(ui, d, move |ui, path| {
        let pic = match sheaf_core::edit::Picture::read(&path) {
            Ok(p) => p,
            Err(e) => return crate::flow::toast(ui, &e.to_string()),
        };
        let Some((page, at)) = update(tab_id, |r, _| r.image_at.take()).flatten() else { return };
        place_image(ui, tab_id, page, at, 200.0, pic.aspect(), file_name(&path), pic.bytes.clone());
    });
}

/// ⌘V with an image on the clipboard: placed in the middle of the page in view, as large as its
/// pixels in points (at most 80 % of the page), as PDF Expert pastes one.
pub fn paste_image(ui: &AppWindow, tab_id: i32) {
    let Some(img) = arboard::Clipboard::new().ok().and_then(|mut c| c.get_image().ok()) else {
        return crate::flow::toast(ui, "The clipboard holds no image to paste");
    };
    let Some(rgba) = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned()) else { return };
    let mut png = Vec::new();
    if rgba.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).is_err() || rgba.width() == 0 {
        return crate::flow::toast(ui, "The image on the clipboard could not be read");
    }
    let aspect = rgba.height() as f64 / rgba.width() as f64;
    let Some((page, at, w)) = update(tab_id, |r, v| {
        let page = v.current();
        let size = v.viewer.boxes[page].view_size();
        let w = (rgba.width() as f64).min(size.0 * 0.8).min(size.1 * 0.8 / aspect);
        r.tool = EDIT_TEXT;
        Some((page, ((size.0 - w) / 2.0, (size.1 - w * aspect) / 2.0), w))
    })
    .flatten() else {
        return;
    };
    place_image(ui, tab_id, page, at, w, aspect, "Pasted image".into(), std::sync::Arc::new(png));
}

/// An image `w` points wide (less on a small page) of height `aspect` × width, its top-left at
/// `at` (view points) kept on the page; then the Edit tool, the new image selected once the
/// page's images are listed again (it is the last): its corners resize it at once, as in PDF
/// Expert.
#[allow(clippy::too_many_arguments)]
fn place_image(ui: &AppWindow, tab_id: i32, page: usize, at: (f64, f64), w: f64, aspect: f64, name: String, bytes: std::sync::Arc<Vec<u8>>) {
    let edit = update(tab_id, |r, v| {
        let size = v.viewer.boxes[page].view_size();
        // At most 80 % of the page each way (a tall image placed 200 pt wide could run off it).
        let w = w.min(size.0 * 0.8).min(size.1 * 0.8 / aspect.max(1e-9));
        let h = w * aspect;
        let x0 = at.0.clamp(0.0, (size.0 - w).max(0.0));
        let y0 = at.1.clamp(0.0, (size.1 - h).max(0.0));
        // Display space: points from the displayed page's bottom-left, y up.
        let rect = [x0, size.1 - (y0 + h), x0 + w, size.1 - y0];
        r.select_added = Some((page, r.images.get(&page).map_or(0, |im| im.len())));
        r.tool = EDIT_TEXT;
        Edit::AddImage { page, rect: Some(rect), name, bytes }
    });
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
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
        if let Some(d) = r.image_resize.as_mut() {
            d.2 = b;
            if let Some((page, e)) = pull_edit(r, v) {
                live_preview(r, page, e);
            }
            return;
        }
        if let Some((_, _, to)) = r.para_resize.as_mut() {
            *to = b;
            if let Some((page, e)) = paragraph_resize(r, v) {
                live_preview(r, page, e);
            }
            return;
        }
        if let Some((_, to)) = r.picked_drag.as_mut() {
            *to = b;
            if let Some((page, e)) = picked_move(r, v) {
                live_preview(r, page, e);
            }
            return;
        }
        if let Some(e) = r.edit_press.as_mut()
            && matches!(e.target, EditTarget::Empty)
            && (e.from.0 - b.0).hypot(e.from.1 - b.1) > 4.0
        {
            // On empty space, a drag draws a marquee.
            e.to = b;
            r.marquee = Some((e.page, e.from, b));
            return;
        }
        if let Some(e) = r.edit_press.as_mut() {
            e.to = b;
            if (e.from.0 - b.0).hypot(e.from.1 - b.1) > 4.0 && !matches!(e.target, EditTarget::Empty) {
                e.dragging = true;
            }
            // The paragraph or image itself follows the pointer, as in PDF Expert.
            if let Some((page, e)) = pull_edit(r, v) {
                live_preview(r, page, e);
            }
            return;
        }
        if r.tool == SHAPE {
            let b = if r.shape_shift { reader::constrained(r.shape_kind, a, b) } else { b };
            r.shape_end = Some(b);
            r.stroke = Some((page, reader::shape_outline(r.shape_kind, a, b)));
            return;
        }
        if r.tool == LINK {
            // The box the link will cover.
            r.shape_end = Some(b);
            r.stroke = Some((page, reader::shape_outline(0, a, b)));
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
            select_annot(r, i);
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
        // The picked paragraph's side pulled: rewrapped (none under 2 points).
        if r.para_resize.is_some() {
            let edit = paragraph_resize(r, v);
            r.para_resize = None;
            r.edit_press = None;
            end_live(r, edit.is_some());
            return edit.map(|(page, e)| Pending::Move(page, Box::new(e)));
        }
        // Picked objects dragged together: one move (none under 2 points).
        if r.picked_drag.is_some() {
            let edit = picked_move(r, v);
            r.picked_drag = None;
            end_live(r, edit.is_some());
            return edit.map(|(page, e)| Pending::Move(page, Box::new(e)));
        }
        // A marquee: the objects it touches are picked.
        if let Some((page, a, b)) = r.marquee.take() {
            r.edit_press = None;
            let bx = &v.viewer.boxes[page];
            let m = viewer::view_rect_to_pdf(bx, bx.view_size(), (a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)));
            let objects = r.objects.get(&page).cloned().unwrap_or_default();
            let found: Vec<_> = reader::objects_in(&objects.iter().map(|o| o.1).collect::<Vec<_>>(), m).into_iter().map(|i| objects[i]).collect();
            r.picked = (!found.is_empty()).then_some((page, found));
            return None;
        }
        // A corner pulled, an image or paragraph dragged: the edit (its live preview is taken
        // back by it, or by hand when there is none, as a pull of under 2 points).
        if r.image_resize.is_some() || r.edit_press.as_ref().is_some_and(|e| e.dragging) {
            let edit = pull_edit(r, v);
            r.image_resize = None;
            r.edit_press = None;
            end_live(r, edit.is_some());
            return edit.map(|(page, e)| Pending::Move(page, Box::new(e)));
        }
        if let Some(e) = r.edit_press.take() {
            match (e.target, e.dragging) {
                (EditTarget::Block(i, text, _, rect, at), false) => {
                    r.preview_text = text.trim().to_string();
                    r.draft = Some(Draft { page: e.page, at, kind: 2, block: Some((i, text, rect)), field: None });
                    r.note_seq += 1;
                }
                (EditTarget::Empty, _) => {
                    r.draft = Some(Draft { page: e.page, at: e.from, kind: 3, block: None, field: None });
                    r.note_seq += 1;
                }
                // An image pressed is selected (on the press); drags are handled above.
                _ => {}
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
            select_annot(r, i);
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
            LINK => {
                r.stroke = None;
                let ((page, a), end) = (press?, r.shape_end.take());
                let b = &v.viewer.boxes[page];
                let size = b.view_size();
                match end.filter(|e| (a.0 - e.0).hypot(a.1 - e.1) >= 4.0) {
                    // A box dragged: where should it go?
                    Some(e) => {
                        let rect = viewer::view_rect_to_pdf(b, size, (a.0.min(e.0), a.1.min(e.1)), (a.0.max(e.0), a.1.max(e.1)));
                        r.link_rects.clear();
                        r.link_error.clear();
                        r.draft = Some(Draft { page, at: (a.0.min(e.0), a.1.max(e.1) + 6.0), kind: 8, block: Some((usize::MAX, String::new(), rect)), field: None });
                        r.note_seq += 1;
                    }
                    // A click on a link: change where it goes, or remove it.
                    None => {
                        if let Some(l) = link_hit(r, &v.viewer, page, a) {
                            let (index, target, rect) = (l.index, reader::link_text(&l.target), l.rect.map(f64::from));
                            let [x, y, _, h] = viewer::pdf_rect_to_view(b, size, rect);
                            r.draft = Some(Draft { page, at: (x, y + h + 6.0), kind: 9, block: Some((index, target, rect)), field: None });
                            r.note_seq += 1;
                        }
                    }
                }
                None
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
        // A link to a local file (Acrobat asks before launching one): a PDF opens in its own tab; any
        // other file is shown in its folder, never run.
        Some(Pending::Link(render::LinkTarget::Uri(u))) if u.starts_with("file:") => {
            let dir = update(tab_id, |r, _| crate::dest::parent_dir(&r.path)).unwrap_or_default();
            match reader::file_link_path(&u, &dir) {
                Some(p) if p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) => crate::flow::home_files(ui, vec![p]),
                Some(p) if p.exists() => {
                    crate::run::reveal(&p);
                    crate::flow::toast(ui, &format!("Shown in its folder: {}", super::file_name(&p)));
                }
                _ => crate::flow::toast(ui, "The linked file was not found"),
            }
        }
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
    link_hit(r, v, page, p).map(|l| l.target.clone())
}

/// The link (its place in the page's `/Annots`, box and target) under view point `p` of `page`.
pub(super) fn link_hit<'a>(r: &'a ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) -> Option<&'a render::Link> {
    let size = v.boxes[page].view_size();
    r.snap.links.iter().filter(|l| l.page == page).find(|l| {
        let [x, y, w, h] = viewer::pdf_rect_to_view(&v.boxes[page], size, l.rect.map(f64::from));
        p.0 >= x && p.0 <= x + w && p.1 >= y && p.1 <= y + h
    })
}



/// The drawing under user-space point `p` of a page (`objects`, its box `b`), smallest first; not
/// one covering most of the page (a background: clicks there add text).
pub(super) fn drawing_at(objects: &[PageObject], b: &viewer::PageBox, p: (f64, f64)) -> Option<PageObject> {
    use sheaf_core::pdfcraft::engine::ObjectKind;
    let (w, h) = b.view_size();
    let drawings: Vec<PageObject> =
        objects.iter().copied().filter(|o| o.0.kind == ObjectKind::Path && (o.1[2] - o.1[0]) * (o.1[3] - o.1[1]) < 0.6 * w * h).collect();
    reader::object_at(&drawings.iter().map(|o| o.1).collect::<Vec<_>>(), p).map(|i| drawings[i])
}

/// Edit tool, the pointer up: the drawing under it is outlined (drawings are not outlined
/// otherwise, to keep dense pages readable). The view is refreshed only when that changes.
pub fn hover(tab_id: i32, x: f32, y: f32) {
    let changed = with(|a| {
        let t = a.tab(tab_id)?;
        let (r, v) = (t.reader.as_mut()?, t.viewer.as_ref()?);
        let found = (|| {
            let page = v.viewer.hit(x as f64, y as f64)?.0;
            let p = to_view(&v.viewer, page, x as f64, y as f64);
            let b = &v.viewer.boxes[page];
            let user = viewer::view_to_pdf(b, b.view_size(), p.0, p.1);
            // Not over a paragraph or an image: those are outlined already.
            let size = b.view_size();
            let on = |q: [f64; 4]| {
                let [x0, y0, w, h] = viewer::pdf_rect_to_view(b, size, q);
                p.0 >= x0 && p.0 <= x0 + w && p.1 >= y0 && p.1 <= y0 + h
            };
            if r.blocks.get(&page).is_some_and(|bl| bl.iter().any(|q| on(q.rect))) || r.images.get(&page).is_some_and(|im| im.iter().any(|q| on(q.0))) {
                return None;
            }
            drawing_at(r.objects.get(&page)?, b, user).map(|o| (page, o.1))
        })();
        (found != r.hover).then(|| r.hover = found)
    })
    .is_some();
    if changed {
        update(tab_id, |_, _| {});
    }
}

/// The one paragraph picked: its page, index among the page's paragraphs and box (user space).
pub(super) fn picked_paragraph(r: &ReaderUi) -> Option<(usize, usize, [f64; 4])> {
    use sheaf_core::pdfcraft::engine::ObjectKind;
    let (page, list) = r.picked.as_ref()?;
    let [(target, rect)] = list.as_slice() else { return None };
    if target.kind != ObjectKind::Text {
        return None;
    }
    let blocks = r.blocks.get(page)?;
    let i = *reader::picked_again(&[*rect], &blocks.iter().map(|b| b.rect).collect::<Vec<_>>()).first()?;
    Some((*page, i, blocks[i].rect))
}

/// The corner of the one picked paragraph at view point `p` of `page`, within 8 points: 0
/// top-left, 1 top-right, 2 bottom-left, 3 bottom-right.
fn paragraph_corner_at(r: &ReaderUi, v: &ViewerUi, page: usize, p: (f64, f64)) -> Option<u8> {
    let (pp, _, rect) = picked_paragraph(r).filter(|(pp, ..)| *pp == page)?;
    let b = &v.viewer.boxes[pp];
    let [x, y, w, h] = viewer::pdf_rect_to_view(b, b.view_size(), rect);
    [(x, y), (x + w, y), (x, y + h), (x + w, y + h)].iter().position(|c| (p.0 - c.0).abs() <= 8.0 && (p.1 - c.1).abs() <= 8.0).map(|c| c as u8)
}

/// The rewrap a pull of the picked paragraph's side makes (none under 2 points): a right corner
/// moves the right edge, a left corner the left edge (the box moves with it); at least 24 points
/// wide. Where it will be is kept to pick it again.
fn paragraph_resize(r: &mut ReaderUi, v: &ViewerUi) -> Option<(usize, Edit)> {
    let (corner, from, to) = r.para_resize?;
    let (page, i, rect) = picked_paragraph(r)?;
    let b = &v.viewer.boxes[page];
    let size = b.view_size();
    let dx = viewer::view_to_pdf(b, size, to.0, to.1).0 - viewer::view_to_pdf(b, size, from.0, from.1).0;
    if dx.abs() < 2.0 {
        return None;
    }
    let left = corner % 2 == 0;
    let w = (rect[2] - rect[0] + if left { -dx } else { dx }).max(24.0);
    let x0 = if left { rect[2] - w } else { rect[0] };
    let block = r.blocks.get(&page)?.get(i)?;
    let text = block.text.clone();
    let h = reader::rewrapped_height(&block.drawn, rect, w);
    r.repick = Some((page, vec![[x0, rect[3] - h, x0 + w, rect[3]]]));
    let style = sheaf_core::pdfcraft::edit::BlockStyle { width: Some(w), offset: left.then_some([x0 - rect[0], 0.0]), ..Default::default() };
    Some((page, Edit::EditTextBlock { page, block: i, text, style }))
}

/// The move a drag of the picked objects makes (none under 2 points); their boxes where it puts
/// them are kept to pick them again once the page is read again.
fn picked_move(r: &mut ReaderUi, v: &ViewerUi) -> Option<(usize, Edit)> {
    let (from, to) = r.picked_drag?;
    let (page, list) = r.picked.as_ref()?;
    let b = &v.viewer.boxes[*page];
    let size = b.view_size();
    let (p0, p1) = (viewer::view_to_pdf(b, size, from.0, from.1), viewer::view_to_pdf(b, size, to.0, to.1));
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    if dx.hypot(dy) < 2.0 {
        return None;
    }
    r.repick = Some((*page, list.iter().map(|o| [o.1[0] + dx, o.1[1] + dy, o.1[2] + dx, o.1[3] + dy]).collect()));
    Some((*page, Edit::MoveObjects { page: *page, objects: list.iter().map(|o| o.0).collect(), offset: [dx, dy] }))
}

/// The edit a pull or drag in progress makes (Edit tool): a corner of the selected image pulled
/// (none under 2 points), an image or paragraph dragged.
fn pull_edit(r: &ReaderUi, v: &ViewerUi) -> Option<(usize, Edit)> {
    use sheaf_core::pdfcraft::engine::ImageEdit;
    if let Some((corner, from, to)) = r.image_resize {
        if (to.0 - from.0).hypot(to.1 - from.1) < 2.0 {
            return None;
        }
        let (page, i) = r.selected_image?;
        let (vis, full) = *r.images.get(&page)?.get(i)?;
        let to = resized_image(&v.viewer.boxes[page], vis, full, corner, (to.0 - from.0, to.1 - from.1));
        return Some((page, Edit::EditPageImage { page, index: i, change: ImageEdit::Move(to) }));
    }
    let e = r.edit_press.as_ref().filter(|e| e.dragging)?;
    let b = &v.viewer.boxes[e.page];
    let size = b.view_size();
    let p0 = viewer::view_to_pdf(b, size, e.from.0, e.from.1);
    let p1 = viewer::view_to_pdf(b, size, e.to.0, e.to.1);
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    match &e.target {
        EditTarget::Block(i, _, text, _, _) => {
            let style = sheaf_core::pdfcraft::edit::BlockStyle { offset: Some([dx, dy]), ..Default::default() };
            Some((e.page, Edit::EditTextBlock { page: e.page, block: *i, text: text.clone(), style }))
        }
        // The full box moves (a cropped image keeps its crop and its size).
        EditTarget::Image(i, _, rc) => {
            Some((e.page, Edit::EditPageImage { page: e.page, index: *i, change: ImageEdit::Move([rc[0] + dx, rc[1] + dy, rc[2] + dx, rc[3] + dy]) }))
        }
        EditTarget::Empty => None,
    }
}

/// Shows `edit` on its page while a pull or drag goes on: one preview on its way at a time, the
/// latest one waiting for it (the document thread renders as fast as it can, no faster).
fn live_preview(r: &mut ReaderUi, page: usize, edit: Edit) {
    r.live = Some(page);
    if r.live_in_flight {
        r.live_pending = Some((page, edit));
    } else {
        r.live_in_flight = true;
        r.live_offset = match &edit {
            Edit::MoveObjects { offset, .. } => Some(*offset),
            _ => None,
        };
        r.send(Op::Preview { edit: Some(Box::new(edit)), pages: vec![page] });
    }
}

/// The pull or drag is over: its edit (`applied`) takes the live preview back; otherwise it is
/// taken back here.
fn end_live(r: &mut ReaderUi, applied: bool) {
    r.live_pending = None;
    if let Some(page) = r.live.take()
        && !applied
    {
        r.send(Op::Preview { edit: None, pages: vec![page] });
    }
}

/// The corner of the selected image (Edit tool) at view point `p` of `page`, within 8 points:
/// 0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right.
fn image_corner_at(r: &ReaderUi, v: &ViewerUi, page: usize, p: (f64, f64)) -> Option<u8> {
    let (sp, i) = r.selected_image?;
    if sp != page {
        return None;
    }
    let (vis, _) = *r.images.get(&page)?.get(i)?;
    let b = &v.viewer.boxes[page];
    let [x, y, w, h] = viewer::pdf_rect_to_view(b, b.view_size(), vis);
    [(x, y), (x + w, y), (x, y + h), (x + w, y + h)].iter().position(|c| (p.0 - c.0).abs() <= 8.0 && (p.1 - c.1).abs() <= 8.0).map(|c| c as u8)
}

/// The view box `[x, y, w, h]` of an image whose `corner` is pulled by `pull` (view points): the
/// opposite corner stays, the proportions too (PDF Expert's corners), the pull projected on the
/// diagonal; never under 8 points.
pub(super) fn pulled_box([x, y, w, h]: [f64; 4], corner: u8, pull: (f64, f64)) -> [f64; 4] {
    let (sx, sy) = (if corner % 2 == 1 { 1.0 } else { -1.0 }, if corner >= 2 { 1.0 } else { -1.0 });
    let d2 = (w * w + h * h).max(1e-9);
    let k = (1.0 + (sx * pull.0 * w + sy * pull.1 * h) / d2).max(8.0 / w.min(h).max(1e-9));
    let (w2, h2) = (w * k, h * k);
    let x2 = if sx > 0.0 { x } else { x + w - w2 };
    let y2 = if sy > 0.0 { y } else { y + h - h2 };
    [x2, y2, w2, h2]
}

/// The full box (user space) of an image whose visible box `vis` has had `corner` pulled by
/// `pull`: a cropped image's full box scales with it about the same fixed corner.
fn resized_image(b: &viewer::PageBox, vis: [f64; 4], full: [f64; 4], corner: u8, pull: (f64, f64)) -> [f64; 4] {
    let size = b.view_size();
    let v = viewer::pdf_rect_to_view(b, size, vis);
    let n = pulled_box(v, corner, pull);
    let k = n[2] / v[2].max(1e-9);
    // The fixed corner, the same point before and after.
    let (ax, ay) = (if corner % 2 == 1 { v[0] } else { v[0] + v[2] }, if corner >= 2 { v[1] } else { v[1] + v[3] });
    let [fx, fy, fw, fh] = viewer::pdf_rect_to_view(b, size, full);
    let (x0, y0) = (ax + (fx - ax) * k, ay + (fy - ay) * k);
    viewer::view_rect_to_pdf(b, size, (x0, y0), (x0 + fw * k, y0 + fh * k))
}

#[cfg(test)]
mod tests {
    use super::pulled_box;

    #[test]
    fn a_pulled_corner_keeps_the_proportions_and_the_opposite_corner() {
        let b = [100.0, 100.0, 200.0, 100.0];
        // Bottom-right pulled out along the diagonal: twice the size, top-left fixed.
        assert_eq!(pulled_box(b, 3, (200.0, 100.0)), [100.0, 100.0, 400.0, 200.0]);
        // Top-left pulled the same way in: half the size, bottom-right fixed (300, 200).
        assert_eq!(pulled_box(b, 0, (100.0, 50.0)), [200.0, 150.0, 100.0, 50.0]);
        // A sideways pull scales by its share of the diagonal; the ratio stays 2:1.
        let r = pulled_box(b, 1, (50.0, 0.0));
        assert!((r[2] / r[3] - 2.0).abs() < 1e-9 && r[0] == 100.0 && (r[1] + r[3] - 200.0).abs() < 1e-9, "{r:?}");
        // Never under 8 points.
        let r = pulled_box(b, 3, (-1000.0, -1000.0));
        assert!((r[3] - 8.0).abs() < 1e-9 && (r[2] - 16.0).abs() < 1e-9, "{r:?}");
    }
}
