//! Edits to the document: applying them, markup, ink, notes and text boxes, delete, undo.

use super::*;

/// Sends `edit` to the document thread; the pages it touches render again when it is done.
/// `provisional` is drawn meanwhile.
pub(super) fn apply(ui: &AppWindow, tab_id: i32, edit: Edit, pages: &[usize], provisional: Option<Provisional>) {
    let err = update(tab_id, |r, _| {
        if !r.snap.read_only.is_empty() {
            return Some(r.snap.read_only.clone());
        }
        if r.structure_pending {
            return Some("One moment: the pages are being rearranged".into());
        }
        r.send(Op::Apply { edit: Box::new(edit), pages: pages.to_vec() });
        r.undo_pages.push(pages.to_vec());
        r.redo_pages.clear();
        r.provisional.extend(provisional);
        None
    })
    .flatten();
    if let Some(e) = err {
        crate::flow::toast(ui, &e);
    }
}

/// Pages were added, removed, moved or turned: everything held by page number is dropped
/// (search results, selections, drafts, marks on their way) rather than shown on the wrong page.
pub(super) fn forget_page_state(r: &mut ReaderUi) {
    r.search.generation += 1;
    r.search_live.store(r.search.generation, std::sync::atomic::Ordering::Relaxed);
    r.search.hits.clear();
    r.search.snippets.clear();
    r.search.current = None;
    r.search.busy = false;
    r.sel = None;
    r.draft = None;
    r.selected_annot = None;
    r.press = None;
    r.stroke = None;
    r.image_at = None;
    r.sign_at = None;
    r.provisional.clear();
    r.texts.clear();
    r.blocks.clear();
    r.images.clear();
    r.edit_press = None;
    r.blocks_asked.clear();
    r.back.clear();
    r.forward.clear();
    r.page_drag = None;
    r.pending_reduce = None;
    r.annot_drag = None;
    r.annot_resize = None;
    r.annot_candidate = None;
    let n = r.snap.boxes.len();
    r.page_sel.retain(|p| *p < n);
}

/// The working copy changed: new bytes to render, annotations to list.
pub(super) fn after_change(r: &mut ReaderUi, v: &mut ViewerUi, pages: Option<&[usize]>) {
    v.source = Some(r.bytes());
    if v.viewer.boxes != r.snap.boxes {
        // A page was added, removed or turned.
        let width = v.viewer.content_width();
        v.viewer = reader_viewer(r.snap.boxes.clone());
        v.viewer.set_width(width);
    }
    match pages {
        Some(ps) => ps.iter().for_each(|&p| {
            v.page_changed(p);
            r.blocks.remove(&p);
            r.images.remove(&p);
            r.blocks_asked.remove(&p);
        }),
        None => {
            v.all_changed();
            r.blocks.clear();
            r.images.clear();
            r.blocks_asked.clear();
        }
    }
    // Annotations don't change the text layer; edits of the page content do.
    let stale_text = r.text_renderer.as_ref().is_some_and(|(b, _)| !Arc::ptr_eq(b, &r.snap.bytes));
    if stale_text {
        match pages {
            Some(ps) => ps.iter().for_each(|p| {
                r.texts.remove(p);
            }),
            None => r.texts.clear(),
        }
    }
    r.version += 1;
    if r.selected_annot.is_some_and(|i| i >= r.snap.annots.len()) {
        r.selected_annot = None;
    }
    r.rows = reader::outline_rows(&r.snap.outline, &r.toggled);
}

pub(super) fn style(colour: [f64; 3], width: f64) -> Style {
    Style { color: colour, opacity: 1.0, width, fill: None }
}

/// Highlight, underline or strike the selection (`kind` 1, 2, 3).
pub fn markup(ui: &AppWindow, tab_id: i32, kind: i32) {
    let made = update(tab_id, |r, v| {
        let (page, range) = r.sel.clone()?;
        let text = r.text(page)?;
        let rects = text.line_rects(range.clone());
        let quads = reader::quads(&v.viewer.boxes[page], &rects);
        let contents = reader::selected_text(&text, range);
        let (m, colour) = match kind {
            UNDERLINE => (Markup::Underline, INK[r.ink_colour]),
            STRIKE => (Markup::StrikeOut, INK[r.ink_colour]),
            _ => (Markup::Highlight, COLOURS[r.light]),
        };
        r.sel = None;
        let prov = Provisional { page, rects, stroke: Vec::new(), colour, kind };
        Some((page, Edit::AddAnnotation(NewAnnotation { page, shape: Shape::TextMarkup { kind: m, quads }, style: style(colour, 1.0), contents, author: author() }), prov))
    })
    .flatten();
    if let Some((page, edit, prov)) = made {
        apply(ui, tab_id, edit, &[page], Some(prov));
    }
}

pub(super) fn author() -> String {
    crate::home::prefs().author.unwrap_or_default()
}

/// Chaikin corner cutting, twice: a hand-drawn stroke loses the jitter of the pointer (PDF Expert
/// smooths its pen the same way). The end points stay where they were.
pub fn smooth(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut cur = pts.to_vec();
    for _ in 0..2 {
        if cur.len() < 3 {
            break;
        }
        let mut next = vec![cur[0]];
        for w in cur.windows(2) {
            let (a, b) = (w[0], w[1]);
            next.push((0.75 * a.0 + 0.25 * b.0, 0.75 * a.1 + 0.25 * b.1));
            next.push((0.25 * a.0 + 0.75 * b.0, 0.25 * a.1 + 0.75 * b.1));
        }
        next.push(*cur.last().expect("at least 3 points"));
        cur = next;
    }
    cur
}

pub(super) fn add_ink(ui: &AppWindow, tab_id: i32, page: usize, pts: Vec<(f64, f64)>) {
    let pts = smooth(&pts);
    let edit = update(tab_id, |r, v| {
        let b = &v.viewer.boxes[page];
        let size = b.view_size();
        let stroke: Vec<[f64; 2]> = pts.iter().map(|p| { let (x, y) = viewer::view_to_pdf(b, size, p.0, p.1); [x, y] }).collect();
        let prov = Provisional { page, rects: Vec::new(), stroke: pts.clone(), colour: INK[r.ink_colour], kind: PEN };
        (Edit::AddAnnotation(NewAnnotation { page, shape: Shape::Ink { strokes: vec![stroke] }, style: style(INK[r.ink_colour], pen_points(r.pen_width)), contents: String::new(), author: author() }), prov)
    });
    if let Some((e, prov)) = edit {
        apply(ui, tab_id, e, &[page], Some(prov));
    }
}

pub fn note_commit(ui: &AppWindow, tab_id: i32, text: &str) {
    let text = text.trim().to_string();
    let edit = update(tab_id, |r, v| {
        let d = r.draft.take()?;
        let b = &v.viewer.boxes[d.page];
        let size = b.view_size();
        if d.kind == 6 {
            // An existing note or text box: its new text.
            let (index, old, _) = d.block?;
            if text == old.trim() {
                return None;
            }
            return Some((d.page, Edit::SetAnnotationContents { page: d.page, index, text: text.clone() }));
        }
        if d.kind == 4 {
            // A form text field: its new value (empty clears it).
            let (fi, _) = d.field?;
            let f = r.snap.fields.get(fi)?;
            if text == f.value.join("") {
                return None;
            }
            return Some((d.page, Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Text(text.clone()) }));
        }
        if d.kind == 2 {
            // The paragraph's new text (empty deletes it), rewrapped to its box by PdfCraft.
            let (block, old, _) = d.block?;
            if text == old.trim() {
                return None;
            }
            // A standard font in place of the embedded one (for characters its subset lacks) was
            // tried: PdfCraft then draws every row of the paragraph on one line. Kept as it is.
            let style = sheaf_core::pdfcraft::edit::BlockStyle::default();
            return Some((d.page, Edit::EditTextBlock { page: d.page, block, text: text.clone(), style }));
        }
        if text.is_empty() {
            return None;
        }
        if d.kind == 3 {
            // The font, size and weight chosen in the options bar (PDF Expert's text format row).
            let pt = TEXT_SIZES[r.text_size.clamp(0, TEXT_SIZES.len() as i32 - 1) as usize];
            let family = match r.text_family {
                1 => sheaf_core::pdfcraft::engine::FontFamily::Times,
                2 => sheaf_core::pdfcraft::engine::FontFamily::Courier,
                _ => sheaf_core::pdfcraft::engine::FontFamily::Helvetica,
            };
            let (w, h) = sheaf_core::edit::text_extent(&text, pt);
            let w = w * if r.text_bold { 1.1 } else { 1.0 };
            let x0 = d.at.0.min((size.0 - w - pt).max(0.0));
            let rect = [x0, size.1 - (d.at.1 + h), x0 + w + pt, size.1 - d.at.1];
            return Some((
                d.page,
                Edit::AddText { page: d.page, text: AddedText { rect, text: text.clone(), size: pt, color: INK[r.ink_colour], family, bold: r.text_bold, ..Default::default() } },
            ));
        }
        let (x, y) = viewer::view_to_pdf(b, size, d.at.0, d.at.1);
        let shape = if d.kind == 0 {
            Shape::Note { at: [x, y], icon: NoteIcon::Comment }
        } else {
            // A box 200 pt wide, tall enough for the lines typed (12 pt text), kept on the page.
            let lines = text.lines().count().max(1) as f64 + (text.len() as f64 / 38.0).floor();
            // 200 pt wide, moved left or up so the whole box is on the page (PDF Expert does the same).
            let (w, h) = (200.0_f64.min(size.0), (lines * 15.0 + 8.0).min(size.1));
            let at = (d.at.0.min(size.0 - w).max(0.0), d.at.1.min(size.1 - h).max(0.0));
            let r = viewer::view_rect_to_pdf(b, size, at, (at.0 + w, at.1 + h));
            Shape::TextBox { rect: r, font_size: 12.0 }
        };
        let colour = if d.kind == 0 { COLOURS[r.light] } else { INK[r.ink_colour] };
        Some((d.page, Edit::AddAnnotation(NewAnnotation { page: d.page, shape, style: style(colour, if d.kind == 0 { 1.0 } else { 0.0 }), contents: text, author: author() })))
    })
    .flatten();
    if let Some((page, e)) = edit {
        // A form value can change other pages (widgets, scripts): all render again.
        let pages = if matches!(e, Edit::SetFieldValue { .. }) { vec![] } else { vec![page] };
        apply(ui, tab_id, e, &pages, None);
    }
}

pub fn note_cancel(tab_id: i32) {
    update(tab_id, |r, _| r.draft = None);
}

pub fn delete_selected(ui: &AppWindow, tab_id: i32) {
    let edit = update(tab_id, |r, _| {
        let i = r.selected_annot.take()?;
        let a = r.annots().get(i)?;
        Some((a.page, Edit::DeleteAnnotation { page: a.page, index: a.index }))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

pub fn annot_delete(ui: &AppWindow, tab_id: i32, k: i32) {
    update(tab_id, |r, _| r.selected_annot = Some(k.max(0) as usize));
    delete_selected(ui, tab_id);
}

pub fn undo(_ui: &AppWindow, tab_id: i32, redo: bool) {
    update(tab_id, |r, _| {
        r.selected_annot = None;
        // The pages the undone edit touched, so only they render again.
        let (from, to) = if redo { (&mut r.redo_pages, &mut r.undo_pages) } else { (&mut r.undo_pages, &mut r.redo_pages) };
        let pages = from.pop();
        if let Some(p) = &pages {
            to.push(p.clone());
        }
        // A page edit (no pages listed) changes them all.
        let pages = pages.filter(|p| !p.is_empty());
        r.send(if redo { Op::Redo { pages } } else { Op::Undo { pages } });
    });
}
