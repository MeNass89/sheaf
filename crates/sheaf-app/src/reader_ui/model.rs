//! Into the tab model: the ReaderState pushed to Slint, its marks, panels and rows.

use super::*;
use std::collections::HashMap;

/// The palette entry (light or ink) closest to `c`, if one is close.
pub(super) fn palette_index(c: [f32; 3]) -> i32 {
    let d = |p: &[f64; 3]| (0..3).map(|i| (p[i] - c[i] as f64).powi(2)).sum::<f64>();
    // The presets, then the custom colours (both count as the sixth place).
    let custom = [custom_colour(true), custom_colour(false)];
    COLOURS.iter().chain(INK.iter()).chain(custom.iter()).enumerate().map(|(i, p)| (if i >= 10 { CUSTOM } else { i % 5 }, d(p))).filter(|(_, d)| *d < 0.01).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i as i32).unwrap_or(-1)
}

pub(super) fn colour_of(c: [f64; 3]) -> Color {
    Color::from_rgb_u8((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
}

pub(super) fn overlay(r: [f64; 4], kind: i32) -> ViewerOverlay {
    ViewerOverlay { x: r[0] as f32, y: r[1] as f32, w: r[2] as f32, h: r[3] as f32, kind }
}

pub fn fill(tab: &mut ToolTab, r: &ReaderUi, v: &ViewerUi) {
    let wanted = v.viewer.wanted(v.top, v.height);
    let cur = v.current();
    let mut marks = Vec::new();
    // Preferences ▸ Show art, trim and bleed boxes: each that is not the crop box, outlined.
    if crate::home::show_boxes() {
        for p in wanted.clone() {
            let Some(b) = r.snap.page_boxes.get(p) else { continue };
            for (k, kind) in [(2, 15), (3, 14), (4, 13)] {
                if (0..4).any(|i| (b[k][i] - b[1][i]).abs() > 0.5) {
                    marks.push(overlay(v.viewer.overlay(p, b[k]), kind));
                }
            }
        }
    }
    // The page grid's selection: each page ringed.
    if r.grid_prev.is_some() {
        for &p in &r.page_sel {
            let [x, y, w, h] = v.viewer.page_rect(p);
            marks.push(overlay([x - 4.0, y - 4.0, w + 8.0, h + 8.0], 12));
        }
        // Where the pages dragged would go: a bar in the gap before page `g` (after the last one).
        if let Some(g) = r.grid_gap {
            let n = v.viewer.page_count();
            let [x, y, w, h] = v.viewer.page_rect(g.min(n.saturating_sub(1)));
            let bar_x = if g >= n { x + w + 4.0 } else { x - 8.0 };
            marks.push(overlay([bar_x, y, 4.0, h], 12));
        }
    }
    if let Some((page, range)) = &r.sel
        && let Some(t) = r.texts.get(page)
    {
        for rect in t.line_rects(range.clone()) {
            marks.push(overlay(view_rect_to_content(&v.viewer, *page, rect), 3));
        }
    }
    for (i, (page, rects)) in r.search.hits.iter().enumerate() {
        if wanted.contains(page) {
            let kind = if r.search.current == Some(i) { 5 } else { 4 };
            for rect in rects {
                marks.push(overlay(view_rect_to_content(&v.viewer, *page, *rect), kind));
            }
        }
    }
    // The OCR run being reviewed: its scan remains visible beside the correction panel.
    if let Some(row) = r.ocr_review.as_ref().filter(|f| !f.busy).and_then(|f| f.selected())
        && let Some(page) = row.page.checked_sub(1).filter(|&page| page < v.viewer.page_count())
    {
        marks.push(overlay(v.viewer.overlay(page, row.rect), 5));
    }
    // Annotations on their way to the document thread, drawn as they will look.
    let mut prov_ink = String::new();
    let mut provisional = Vec::new();
    for p in &r.provisional {
        if p.kind == PEN {
            for (i, pt) in p.stroke.iter().enumerate() {
                let (x, y) = view_to_content(&v.viewer, p.page, *pt);
                prov_ink.push_str(&format!("{}{x:.1} {y:.1} ", if i == 0 { "M" } else { "L" }));
            }
            continue;
        }
        for rect in &p.rects {
            let c = view_rect_to_content(&v.viewer, p.page, *rect);
            let c = match p.kind {
                UNDERLINE => [c[0], c[1] + c[3] - 1.5, c[2], 1.5],
                STRIKE => [c[0], c[1] + c[3] * 0.55, c[2], 1.5],
                _ => c,
            };
            let (col, alpha) = (p.colour, if p.kind == HIGHLIGHT { 110 } else { 255 });
            provisional.push(ReaderMark {
                x: c[0] as f32,
                y: c[1] as f32,
                w: c[2] as f32,
                h: c[3] as f32,
                colour: Color::from_argb_u8(alpha, (col[0] * 255.0) as u8, (col[1] * 255.0) as u8, (col[2] * 255.0) as u8),
            });
        }
    }
    if r.show_links || r.tool == LINK {
        for l in r.snap.links.iter().filter(|l| wanted.contains(&l.page)) {
            marks.push(overlay(v.viewer.overlay(l.page, l.rect.map(f64::from)), 10));
        }
    }
    // Edit tool: the drawing under the pointer.
    if r.tool == EDIT_TEXT
        && let Some((page, q)) = r.hover
        && !r.picked.as_ref().is_some_and(|(_, l)| l.iter().any(|o| o.1 == q))
    {
        // Just outside it, to show on a filled shape too.
        let c = v.viewer.overlay(page, q);
        marks.push(overlay([c[0] - 3.0, c[1] - 3.0, c[2] + 6.0, c[3] + 6.0], 6));
    }
    // Edit tool: the objects picked together, moved as the page shows them (the offset of the
    // last preview sent); several in one box; one paragraph with corners to pull.
    if let Some((page, list)) = &r.picked {
        let off = if r.picked_drag.is_some() { r.live_offset.unwrap_or([0.0, 0.0]) } else { [0.0, 0.0] };
        let moved: Vec<[f64; 4]> = list.iter().map(|(_, q)| [q[0] + off[0], q[1] + off[1], q[2] + off[0], q[3] + off[1]]).collect();
        // (A paragraph whose side is pulled is drawn below, at its new width.)
        for q in moved.iter().filter(|_| r.para_resize.is_none()) {
            let c = v.viewer.overlay(*page, *q);
            marks.push(overlay([c[0] - 2.0, c[1] - 2.0, c[2] + 4.0, c[3] + 4.0], 6));
        }
        if moved.len() > 1 {
            let u = moved.iter().fold(moved[0], |a, q| [a[0].min(q[0]), a[1].min(q[1]), a[2].max(q[2]), a[3].max(q[3])]);
            let c = v.viewer.overlay(*page, u);
            marks.push(overlay([c[0] - 6.0, c[1] - 6.0, c[2] + 12.0, c[3] + 12.0], 6));
        } else if let Some((pp, _, rect)) = super::pointer::picked_paragraph(r)
            && pp == *page
            && r.picked_drag.is_none()
        {
            // While a side is pulled, the box shows the new width.
            let rect = match r.para_resize {
                Some((corner, from, to)) => {
                    let b = &v.viewer.boxes[pp];
                    let dx = viewer::view_to_pdf(b, b.view_size(), to.0, to.1).0 - viewer::view_to_pdf(b, b.view_size(), from.0, from.1).0;
                    let w = (rect[2] - rect[0] + if corner % 2 == 0 { -dx } else { dx }).max(24.0);
                    // As tall as the same text wrapped to that width.
                    let drawn = r.blocks.get(&pp).and_then(|b| b.iter().find(|b| b.rect == rect)).map(|b| b.drawn.clone()).unwrap_or_default();
                    let h = reader::rewrapped_height(&drawn, rect, w);
                    let x0 = if corner % 2 == 0 { rect[2] - w } else { rect[0] };
                    [x0, rect[3] - h, x0 + w, rect[3]]
                }
                None => rect,
            };
            let c = v.viewer.overlay(pp, rect);
            if r.para_resize.is_some() {
                marks.push(overlay([c[0] - 2.0, c[1] - 2.0, c[2] + 4.0, c[3] + 4.0], 6));
            }
            for (cx, cy) in [(c[0], c[1]), (c[0] + c[2], c[1]), (c[0], c[1] + c[3]), (c[0] + c[2], c[1] + c[3])] {
                marks.push(overlay([cx - 4.5, cy - 4.5, 9.0, 9.0], 11));
            }
        }
    }
    if let Some((page, a, b)) = r.marquee {
        let (x0, y0) = view_to_content(&v.viewer, page, (a.0.min(b.0), a.1.min(b.1)));
        let (x1, y1) = view_to_content(&v.viewer, page, (a.0.max(b.0), a.1.max(b.1)));
        marks.push(overlay([x0, y0, x1 - x0, y1 - y0], 6));
    }
    // The box being linked, while its target is typed.
    if let Some(d) = r.draft.as_ref().filter(|d| matches!(d.kind, 8..=11))
        && let Some((_, _, rect)) = &d.block
    {
        marks.push(overlay(v.viewer.overlay(d.page, *rect), 6));
    }
    if r.tool == SELECT && r.show_fields {
        // Fillable fields, tinted as PDF Expert and Acrobat highlight them.
        for f in r.snap.fields.iter().filter(|f| !f.read_only && f.kind < 4) {
            for (page, rect, _, _) in f.widgets.iter().filter(|w| wanted.contains(&w.0)) {
                marks.push(overlay(v.viewer.overlay(*page, *rect), 10));
            }
        }
    }
    if r.tool == EDIT_TEXT {
        // What is pulled or dragged shows itself live on the page: not its old outline too.
        let moving = r.edit_press.as_ref().filter(|e| e.dragging).map(|e| match e.target {
            EditTarget::Block(i, ..) => (e.page, Some(i), None),
            EditTarget::Image(i, ..) => (e.page, None, Some(i)),
            EditTarget::Empty => (e.page, None, None),
        });
        let moving = moving.or(r.image_resize.and(r.selected_image).map(|(p, i)| (p, None, Some(i))));
        // Picked objects moved or a picked paragraph pulled: their old outlines are not drawn.
        let picked_moving: Vec<[f64; 4]> = match &r.picked {
            Some((_, list)) if r.picked_drag.is_some() || r.para_resize.is_some() => list.iter().map(|o| o.1).collect(),
            _ => Vec::new(),
        };
        let left_behind = |q: [f64; 4]| !picked_moving.is_empty() && !reader::picked_again(&[q], &picked_moving).is_empty();
        // The paragraphs that can be edited, outlined on the pages in view (read when first shown).
        for page in wanted.clone() {
            if let Some(blocks) = r.blocks.get(&page) {
                for (_, b) in blocks.iter().enumerate().filter(|(i, b)| moving != Some((page, Some(*i), None)) && !left_behind(b.rect)) {
                    let c = v.viewer.overlay(page, b.rect);
                    marks.push(overlay([c[0] - 2.0, c[1] - 1.0, c[2] + 4.0, c[3] + 2.0], 7));
                }
            }
            if let Some(images) = r.images.get(&page) {
                for (_, (rc, _)) in images.iter().enumerate().filter(|(i, (rc, _))| moving != Some((page, None, Some(*i))) && !left_behind(*rc)) {
                    marks.push(overlay(v.viewer.overlay(page, *rc), 7));
                }
            }
        }
    }
    // A paragraph or image being dragged: its outline follows the pointer.
    if let Some(e) = r.edit_press.as_ref().filter(|e| e.dragging) {
        let rect = match &e.target {
            EditTarget::Block(_, _, _, rc, _) | EditTarget::Image(_, rc, _) => Some(*rc),
            EditTarget::Empty => None,
        };
        if let Some(rc) = rect {
            let c = v.viewer.overlay(e.page, rc);
            let s = 1.0 / k(&v.viewer, e.page);
            let (dx, dy) = ((e.to.0 - e.from.0) * s, (e.to.1 - e.from.1) * s);
            marks.push(overlay([c[0] + dx, c[1] + dy, c[2], c[3]], 6));
        }
    }
    if let Some(a) = r.selected_annot.and_then(|i| r.annots().get(i)) {
        let c = v.viewer.overlay(a.page, a.rect);
        // While dragged, the outline follows the pointer (content pixels).
        let (dx, dy) = match r.annot_drag {
            Some((_, from, to)) => {
                let s = 1.0 / k(&v.viewer, a.page);
                ((to.0 - from.0) * s, (to.1 - from.1) * s)
            }
            None => (0.0, 0.0),
        };
        let (dw, dh) = match r.annot_resize {
            Some((_, from, to)) => {
                let s = 1.0 / k(&v.viewer, a.page);
                ((to.0 - from.0) * s, (to.1 - from.1) * s)
            }
            None => (0.0, 0.0),
        };
        let (mut w, mut h) = ((c[2] + dw).max(8.0), (c[3] + dh).max(8.0));
        if a.fill_sign && c[2] > 0.0 && c[3] > 0.0 {
            let g = (1.0 + (dw * c[2] + dh * c[3]) / (c[2] * c[2] + c[3] * c[3])).max(8.0 / c[2]);
            (w, h) = (c[2] * g, c[3] * g);
        }
        marks.push(overlay([c[0] - 2.0 + dx, c[1] - 2.0 + dy, w + 4.0, h + 4.0], 6));
        if resizable(a) {
            marks.push(overlay([c[0] + dx + w - 4.0, c[1] + dy + h - 4.0, 9.0, 9.0], 11));
        }
    }
    // Comments selected with it (⇧-click): outlined too.
    if r.selected_annot.is_some() {
        for a in r.annots_also.iter().filter_map(|&i| r.annots().get(i)) {
            let c = v.viewer.overlay(a.page, a.rect);
            marks.push(overlay([c[0] - 2.0, c[1] - 2.0, c[2] + 4.0, c[3] + 4.0], 6));
        }
    }
    // Edit tool: the selected image, its four corners to pull (the box follows a pull).
    if let Some((page, i)) = r.selected_image
        && let Some(&(vis, _)) = r.images.get(&page).and_then(|im| im.get(i))
        && !r.edit_press.as_ref().is_some_and(|e| e.dragging)
    {
        let b = &v.viewer.boxes[page];
        let mut bx = viewer::pdf_rect_to_view(b, b.view_size(), vis);
        if let Some((corner, from, to)) = r.image_resize {
            bx = super::pointer::pulled_box(bx, corner, (to.0 - from.0, to.1 - from.1));
        }
        let c = v.viewer.overlay(page, viewer::view_rect_to_pdf(b, b.view_size(), (bx[0], bx[1]), (bx[0] + bx[2], bx[1] + bx[3])));
        marks.push(overlay([c[0] - 2.0, c[1] - 2.0, c[2] + 4.0, c[3] + 4.0], 6));
        for (cx, cy) in [(c[0], c[1]), (c[0] + c[2], c[1]), (c[0], c[1] + c[3]), (c[0] + c[2], c[1] + c[3])] {
            marks.push(overlay([cx - 4.5, cy - 4.5, 9.0, 9.0], 11));
        }
    }
    let ink = match &r.stroke {
        Some((page, pts)) if pts.len() >= 2 => {
            let mut s = String::new();
            for (i, p) in pts.iter().enumerate() {
                let (x, y) = view_to_content(&v.viewer, *page, *p);
                s.push_str(&format!("{}{x:.1} {y:.1} ", if i == 0 { "M" } else { "L" }));
            }
            s
        }
        _ => prov_ink,
    };
    let current_row = reader::current_section(&r.rows, cur);
    let outline = {
        let key = (r.version, current_row, r.outline_edit);
        let mut c = r.outline_cache.borrow_mut();
        match c.as_ref() {
            Some((k, m)) if *k == key => m.clone(),
            _ => {
                let m = ModelRc::new(VecModel::from(outline_model(r, current_row)));
                *c = Some((key, m.clone()));
                m
            }
        }
    };
    let annots = {
        let key = (r.version, r.selected_annot, r.texts.len(), r.notes_filter, r.notes_sort);
        let mut c = r.annots_cache.borrow_mut();
        match c.as_ref() {
            Some((k, m)) if *k == key => m.clone(),
            _ => {
                let m = ModelRc::new(VecModel::from(annots_model(r, v)));
                *c = Some((key, m.clone()));
                m
            }
        }
    };
    // One model for the whole search: rows are appended as batches arrive and the current one is
    // updated in place, so a click on a result is never lost to a model replaced under it.
    let results = {
        let mut c = r.results_cache.borrow_mut();
        let (model, shown_current) = c.get_or_insert_with(|| (Rc::new(VecModel::default()), None));
        let have = model.row_count();
        if have > r.search.hits.len() {
            *model = Rc::new(VecModel::default());
            *shown_current = None;
        }
        let have = model.row_count();
        // Each page heads its group with its count (Preview's results list): "Page 4 · 3".
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for (p, _) in &r.search.hits {
            *counts.entry(*p).or_default() += 1;
        }
        let heading = |page: usize| -> slint::SharedString {
            match counts.get(&page).copied().unwrap_or(0) {
                1 => format!("Page {}", label(r, page)),
                n => format!("Page {} · {n} results", label(r, page)),
            }
            .into()
        };
        // A page whose results came in two batches: its heading counts them all.
        if let Some(last) = have.checked_sub(1)
            && let Some(&(page, _)) = r.search.hits.get(last)
            && let Some(first) = (0..have).rev().take_while(|&i| r.search.hits[i].0 == page).last()
            && let Some(mut row) = model.row_data(first)
        {
            let h = heading(page);
            if row.page_label != h {
                row.page_label = h;
                model.set_row_data(first, row);
            }
        }
        for (i, ((page, _), (b, m, a))) in r.search.hits.iter().zip(&r.search.snippets).enumerate().skip(have) {
            model.push(ReaderResultRow {
                before: b.clone().into(),
                hit: m.clone().into(),
                after: a.clone().into(),
                page_label: heading(*page),
                current: r.search.current == Some(i),
                first: i == 0 || r.search.hits[i - 1].0 != *page,
            });
        }
        if *shown_current != r.search.current {
            for (i, on) in [(*shown_current, false), (r.search.current, true)] {
                if let Some(i) = i.filter(|i| *i < model.row_count())
                    && let Some(mut row) = model.row_data(i)
                {
                    row.current = on;
                    model.set_row_data(i, row);
                }
            }
            *shown_current = r.search.current;
        }
        (ModelRc::from(model.clone()), counts.len() as i32)
    };
    let (results, search_pages) = results;
    let (form_fields, field_props) = super::prepare::form_overlay(r, v, |p| wanted.contains(&p));
    let form_grids = super::prepare::grid_overlay(r, v, |p| wanted.contains(&p));
    let rulers = super::prepare::ruler_overlay(r, v, |p| wanted.contains(&p));
    let note = match &r.draft {
        Some(d) => {
            let (mut x, mut y) = view_to_content(&v.viewer, d.page, d.at);
            let (text, w, h) = match &d.block {
                // A form field: the editor sits on the field, its size.
                Some((_, t, rect)) if d.field.is_some() => {
                    let c = v.viewer.overlay(d.page, *rect);
                    {
                    let n = d.field.and_then(|(fi, _)| r.snap.fields.get(fi)).map(|f| f.options.len()).unwrap_or(0) as f64;
                    (t.clone(), c[2].max(160.0), if d.kind == 5 { 34.0 + 28.0 * n } else { c[3].max(28.0) + 34.0 })
                }
                }
                // Link: a one-line editor below the box.
                Some((_, t, _)) if matches!(d.kind, 8..=12) => (t.clone(), 300.0, if r.link_error.is_empty() { 116.0 } else { 140.0 }),
                // Edit text: the paragraph shows on its page as typed, and what it grows over
                // below it; the editor goes above it when the page has room there, else below.
                Some((_, t, rect)) if d.kind == 2 => {
                    let c = v.viewer.overlay(d.page, *rect);
                    let h = (c[3] * 0.5).clamp(60.0, 160.0) + 70.0;
                    let above = c[1] - h - 6.0;
                    (x, y) = (c[0], if above >= v.viewer.page_rect(d.page)[1] { above } else { c[1] + c[3] + 6.0 });
                    (t.clone(), c[2].max(260.0), h)
                }
                Some((_, t, rect)) => {
                    let c = v.viewer.overlay(d.page, *rect);
                    (t.clone(), c[2].max(260.0), c[3].max(60.0) + 70.0)
                }
                None => (String::new(), 260.0, 150.0),
            };
            // A new text box (1) or added text (3): typed in place, 200 pt wide, in its size and font.
            if matches!(d.kind, 1 | 3) && d.block.is_none() {
                let s = 1.0 / k(&v.viewer, d.page);
                let pt = r.text_pt;
                let family = match r.text_family { 1 => "Times New Roman", 2 => "Courier New", _ => "Helvetica" };
                ReaderNote {
                    show: true,
                    x: x as f32,
                    y: y as f32,
                    w: (200.0 * s) as f32,
                    h: ((pt * 1.25 * 3.0 + 8.0) * s) as f32,
                    text: r.inplace_text.clone().into(),
                    kind: d.kind,
                    seq: r.note_seq,
                    inplace: true,
                    font_px: (pt * s) as f32,
                    font_family: family.into(),
                    bold: r.text_bold,
                    italic: r.text_italic,
                    ink: colour_of(ink_at(r.ink_colour)),
                }
            } else {
                ReaderNote { show: true, x: x as f32, y: y as f32, w: w as f32, h: h as f32, text: text.into(), kind: d.kind, seq: r.note_seq, ..Default::default() }
            }
        }
        None => ReaderNote { seq: r.note_seq, ..Default::default() },
    };
    let n = r.search.hits.len();
    let search_count = if r.search.query.is_empty() {
        String::new()
    } else if n == 0 && r.search.busy {
        "…".into()
    } else if n == 0 {
        "None".into()
    } else {
        format!("{}/{}{}", r.search.current.map(|c| c + 1).unwrap_or(0), n, if r.search.busy { "+" } else { "" })
    };
    if !r.page_sel.is_empty() {
        let thumbs: Vec<sheaf_app::ViewerThumb> = slint::Model::iter(&tab.viewer_thumbs)
            .map(|mut t| {
                t.selected = r.page_sel.contains(&(t.page as usize));
                t
            })
            .collect();
        tab.viewer_thumbs = ModelRc::new(VecModel::from(thumbs));
    }
    tab.name = r.name.clone().into();
    tab.is_reader = true;
    tab.reader = ReaderState {
        // When opened ▸ Show the document title (Acrobat's): its Title, not the file's name.
        title: if r.snap.opening.display_title && !r.snap.props.title.trim().is_empty() { r.snap.props.title.clone().into() } else { r.name.clone().into() },
        open_full_screen: r.opened.0,
        open_bare: r.opened.1,
        open_seq: r.opened.2,
        dirty: r.dirty(),
        read_only: r.snap.read_only.clone().into(),
        page_count: v.viewer.page_count() as i32,
        // The page's label when it has one ("iv"), as Acrobat and PDF Expert show it.
        page_text: label(r, cur).into(),
        page_counter: if label(r, cur) == (cur + 1).to_string() { format!("/ {}", v.viewer.page_count()) } else { format!("({} / {})", cur + 1, v.viewer.page_count()) }.into(),
        page_label: label(r, cur).into(),
        panel_open: r.panel_open,
        panel: r.panel,
        outline_empty: r.rows.is_empty(),
        outline,
        annots,
        tool: r.tool,
        colour: if uses_light(r.tool) { r.light as i32 } else { r.ink_colour as i32 },
        custom_ink: colour_of(custom_colour(false)),
        custom_light: colour_of(custom_colour(true)),
        theme: r.theme.index(),
        backdrop: reader::backdrop(r.theme).or_else(crate::home::backdrop).map(|c| Color::from_rgb_u8(c[0], c[1], c[2])).unwrap_or_default(),
        marks: ModelRc::new(VecModel::from(marks)),
        provisional: ModelRc::new(VecModel::from(provisional)),
        has_selection: r.sel.is_some(),
        selected_text: r.sel.as_ref().and_then(|(p, range)| r.texts.get(p).map(|t| reader::selected_text(t, range.clone()))).unwrap_or_default().into(),
        selected_annot: r.selected_annot.is_some(),
        annot_light: r.selected_annot.and_then(|i| r.annots().get(i)).is_some_and(|a| matches!(a.subtype.as_str(), "Highlight" | "Text")),
        annot_colour: r.selected_annot.and_then(|i| r.annots().get(i)).and_then(|a| a.color).map(palette_index).unwrap_or(-1),
        ink: ink.into(),
        // The eraser leaves a grey trail.
        ink_colour: if r.tool == ERASER { Color::from_argb_u8(140, 128, 128, 128) } else { colour_of(ink_at(r.ink_colour)) },
        note,
        search_count: search_count.into(),
        search_busy: r.search.busy,
        can_undo: r.snap.can_undo && r.in_flight == 0,
        can_redo: r.snap.can_redo && r.in_flight == 0,
        zoom_mode: r.zoom_mode,
        spread: v.viewer.spread() as i32,
        rtl: v.viewer.rtl(),
        pen_width: r.pen_width,
        shape_kind: r.shape_kind,
        shape_fill: r.shape_fill,
        arrow_head: reader::ARROW_HEADS[r.arrow_head.min(reader::ARROW_HEADS.len() - 1)].1.into(),
        arrow_both: r.arrow_both,
        callout: r.callout,
        measure_decimals: r.measure_decimals as i32,
        measure_scale: match r.measure_custom {
            Some((_, unit)) => format!("Calibrated · {unit}"),
            None => {
                let (ratio, unit) = reader::MEASURE_SCALES[r.measure_scale.min(reader::MEASURE_SCALES.len() - 1)];
                if ratio == 1.0 { format!("True size · {unit}") } else { format!("1:{ratio} · {unit}") }
            }
        }
        .into(),
        calibrating: r.calibrating,
        auto_scroll: r.auto_scroll,
        reflow: r.reflow.is_some(),
        suggestions: ModelRc::new(VecModel::from(if r.draft.as_ref().is_some_and(|d| d.kind == 4) { r.suggestions.iter().map(|s| s.as_str().into()).collect::<Vec<slint::SharedString>>() } else { Vec::new() })),
        reflow_size: r.reflow.unwrap_or(16),
        reflow_text: r.reflow.and_then(|_| r.texts.get(&v.current())).map(|t| reader::reflow_text(&t.plain_text())).unwrap_or_default().into(),
        loupe: r.loupe,
        loupe_image: r.loupe_image.clone().unwrap_or_default(),
        dashed: r.dashed,
        replace_ask: r.replace_pending.as_ref().map_or_else(Default::default, |(name, _, count, pages)| {
            format!("Replace the {} page{} with pages of {name} ({count}) from page", pages.len(), if pages.len() == 1 { "" } else { "s" }).into()
        }),
        page_at_a_time: r.page_at_a_time,
        page_top: v.viewer.page_rect(v.current())[1] as f32,
        page_bottom: { let [_, y, _, h] = v.viewer.page_rect(v.current()); (y + h) as f32 },
        page_advance: r.snap.durations.get(v.current()).copied().flatten().unwrap_or(0.0) as f32,
        page_effect: r.snap.transitions.get(v.current()).cloned().flatten().map_or(0, |(s, _)| reader::effect_code(&s)),
        page_effect_seconds: r.snap.transitions.get(v.current()).cloned().flatten().map_or(0.0, |(_, d)| d) as f32,
        view_turn: i32::from(v.viewer.turn()),
        opacity: (super::tools::opacity_of(r) * 100.0).round() as i32,
        fill_colour: r.fill_colour.map_or(-1, |k| k as i32),
        selected_stroke: r.selected_annot.and_then(|i| r.annots().get(i)).is_some_and(|a| super::tools::stroked(&a.subtype)),
        selected_fillable: r.selected_annot.and_then(|i| r.annots().get(i)).is_some_and(|a| super::tools::fillable(&a.subtype)),
        selected_turnable: r.selected_annot.and_then(|i| r.annots().get(i)).is_some_and(|a| super::tools::turnable(&a.subtype)),
        // Dynamic stamps share their names with standard ones: they say they are dated.
        stamp_image: r.custom_stamp.as_ref().map(|(n, _, _)| n.as_str()).unwrap_or_default().into(),
        stamps: ModelRc::new(VecModel::from(
            MARKS
                .iter()
                .map(|(_, l)| slint::SharedString::from(*l))
                .chain(StampKind::ALL.iter().map(|k| match k.group() {
                    sheaf_core::pdfcraft::engine::StampGroup::Dynamic => slint::SharedString::from(format!("{} · DATED", k.label())),
                    _ => slint::SharedString::from(k.label()),
                }))
                // The pictures kept as stamps.
                .chain(crate::home::stamp_library().iter().map(|p| slint::SharedString::from(format!("Picture · {}", p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()))))
                .collect::<Vec<_>>(),
        )),
        stamp: r.stamp as i32,
        comments_hidden: r.comments_hidden,
        // The Link tool shows where the links are, as PDF Expert's does.
        show_links: r.show_links || r.tool == LINK,
        show_fields: r.show_fields,
        field_kind: r.field_kind,
        form_fields: ModelRc::new(VecModel::from(form_fields)),
        form_grid: r.form_grid,
        form_grids: ModelRc::new(VecModel::from(form_grids)),
        rulers_on: r.rulers,
        guide_colour: {
            let [r, g, b] = reader::GUIDE_COLOURS[(crate::home::prefs().guide_colour as usize).min(reader::GUIDE_COLOURS.len() - 1)];
            slint::Color::from_rgb_u8(r, g, b)
        },
        rulers: ModelRc::new(VecModel::from(rulers)),
        field_props,
        link_error: r.link_error.clone().into(),
        link_visible: r.link_visible,
        has_picked: r.picked.is_some(),
        picked_count: r.picked.as_ref().map_or(0, |p| p.1.len() as i32),
        annots_group: if r.selected_annot.is_some() { 1 + r.annots_also.len() as i32 } else { 0 },
        link_dashed: r.link_dashed,
        attachments: ModelRc::new(VecModel::from(
            r.snap
                .attachments
                .iter()
                .map(|a| ReaderProp { name: a.name.clone().into(), value: a.size.map(|n| if n >= 1 << 20 { format!("{:.1} MB", n as f64 / 1_048_576.0) } else { format!("{:.0} KB", (n as f64 / 1024.0).max(1.0)) }).unwrap_or_default().into() })
                .collect::<Vec<_>>(),
        )),
        layers: ModelRc::new(VecModel::from(
            r.snap.layers.iter().zip(super::layers::shown(r)).map(|(l, shown)| ReaderLayer { name: l.name.clone().into(), shown }).collect::<Vec<_>>(),
        )),
        layers_changed: r.layers_off.is_some(),
        voices_open: r.voices_open,
        help_open: r.help_open,
        voices: ModelRc::new(VecModel::from(r.voices.iter().map(|v| slint::SharedString::from(v.as_str())).collect::<Vec<_>>())),
        a11y_open: r.a11y_open,
        a11y_busy: r.a11y_busy,
        a11y_summary: r.a11y.as_ref().map_or_else(String::new, |rep| {
            use sheaf_core::pdfcraft::engine::a11y::Status;
            format!("{} failed · {} to check by hand · {} passed", rep.count(Status::Failed), rep.count(Status::Manual), rep.count(Status::Passed))
        }).into(),
        a11y_rows: ModelRc::new(VecModel::from(r.a11y.as_ref().map_or_else(Vec::new, |rep| {
            use sheaf_core::pdfcraft::engine::a11y::Status;
            super::accessibility::rows(rep)
                .into_iter()
                .map(|x| ReaderCheck {
                    name: x.rule.name().into(),
                    status: match x.status { Status::Failed => 0, Status::Manual => 1, _ => 2 },
                    detail: super::accessibility::detail(x).into(),
                    fix: x.status == Status::Failed && super::accessibility::fixable(x.rule),
                })
                .collect()
        }))),
        notes_filter: r.notes_filter,
        notes_sort: r.notes_sort,
        props_open: r.props_open,
        background_open: r.background_open,
        versions_open: r.versions_open,
        signature_manager_open: r.signature_manager.is_some(),
        signature_manager_name: r.signature_manager.as_ref().and_then(|m| m.selected.as_ref()?.1.1.as_deref()).unwrap_or_default().into(),
        signature_manager_commands: r.signature_manager.as_ref().and_then(|m| m.selected.as_ref()).map(|(_, s)| super::signature_manager::commands(s)).unwrap_or_default().into(),
        signature_manager_style: ModelRc::new(VecModel::from(r.signature_manager.as_ref().and_then(|m| m.selected.as_ref()).map(|(_, s)| vec![i32::from(s.3), i32::from(s.2.0), i32::from(s.2.1)]).unwrap_or_else(|| vec![0, 0, 1]))),
        signature_manager_status: r.signature_manager.as_ref().map(|m| m.status.as_str()).unwrap_or_default().into(),
        signature_manager_seq: r.signature_manager.as_ref().map_or(0, |m| m.seq),
        policy_open: r.policy_panel.is_some(),
        policy_busy: r.policy_panel.as_ref().is_some_and(|p| p.busy),
        policy_names: ModelRc::new(VecModel::from(r.policy_panel.as_ref().map(|p| p.policies.iter().map(|p| slint::SharedString::from(p.name.as_str())).collect::<Vec<_>>()).unwrap_or_default())),
        policy_name: r.policy_panel.as_ref().map(|p| p.draft.name.as_str()).unwrap_or_default().into(),
        policy_flags: ModelRc::new(VecModel::from(r.policy_panel.as_ref().map(|p| { let f = p.draft.permissions; vec![f.printing, f.copying, f.editing, f.commenting, f.filling_forms, p.draft.encrypt_metadata] }).unwrap_or_default())),
        policy_choices: ModelRc::new(VecModel::from(r.policy_panel.as_ref().map(|p| {
            use sheaf_core::security::{PrintQuality, ChangeLevel, EncryptionAlgorithm};
            let options = p.draft.options;
            vec![if options.print_quality == PrintQuality::Low { 1 } else { 0 },
                match options.changes { ChangeLevel::Any => 0, ChangeLevel::None => 1, ChangeLevel::Assemble => 2, ChangeLevel::FillForms => 3, ChangeLevel::CommentForms => 4 },
                if options.algorithm == EncryptionAlgorithm::Aes128 { 1 } else { 0 }]
        }).unwrap_or_default())),
        policy_status: r.policy_panel.as_ref().map(|p| p.status.as_str()).unwrap_or_default().into(),
        policy_seq: r.policy_panel.as_ref().map_or(0, |p| p.seq),
        catalog_open: r.catalog_panel.is_some(),
        catalog_busy: r.catalog_panel.as_ref().is_some_and(|p| p.busy),
        catalog_status: r.catalog_panel.as_ref().map(|p| p.status.as_str()).unwrap_or_default().into(),
        catalog_files: ModelRc::new(VecModel::from(r.catalog_panel.as_ref().map(|p| p.files.iter().map(|f| slint::SharedString::from(f.to_string_lossy().as_ref())).collect::<Vec<_>>()).unwrap_or_default())),
        catalog_hits: ModelRc::new(VecModel::from(r.catalog_panel.as_ref().map(|p| p.results.iter().map(|h| FolderHit { file: h.path.to_string_lossy().as_ref().into(), page: format!("p. {}", h.page + 1).into(), line: h.snippet.as_str().into() }).collect::<Vec<_>>()).unwrap_or_default())),
        ocr_review_open: r.ocr_review.is_some(),
        ocr_review_busy: r.ocr_review.as_ref().is_some_and(|f| f.busy),
        ocr_review_text: r.ocr_review.as_ref().and_then(|f| f.selected()).map(|row| row.text.as_str()).unwrap_or_default().into(),
        ocr_review_position: r.ocr_review.as_ref().map(|f| f.position()).unwrap_or_default().into(),
        ocr_review_has_row: r.ocr_review.as_ref().and_then(|f| f.selected()).is_some(),
        ocr_review_suspects_only: r.ocr_review.as_ref().is_some_and(|f| f.suspects_only),
        ocr_review_suspect: r.ocr_review.as_ref().and_then(|f| f.selected()).is_some_and(|row| row.suspect),
        ocr_review_seq: r.ocr_review.as_ref().map_or(0, |f| f.seq),
        folder_open: r.folder_search.is_some(),
        folder_busy: r.folder_search.as_ref().is_some_and(|f| f.busy),
        folder_title: r.folder_search.as_ref().map(|f| format!("“{}” in {}", f.query, f.folder)).unwrap_or_default().into(),
        folder_results: ModelRc::new(VecModel::from(
            r.folder_search
                .as_ref()
                .map(|f| f.results.iter().map(|(p, page, line)| FolderHit { file: file_name(p).into(), page: format!("p. {}", page + 1).into(), line: line.as_str().into() }).collect::<Vec<_>>())
                .unwrap_or_default(),
        )),
        versions: r.snap.versions as i32,
        undo_label: r.snap.undo_label.clone().into(),
        redo_label: r.snap.redo_label.clone().into(),
        props_title: r.snap.props.title.clone().into(),
        props_author: r.snap.props.author.clone().into(),
        props_subject: r.snap.props.subject.clone().into(),
        props_keywords: r.snap.props.keywords.clone().into(),
        props_signed: ModelRc::new(VecModel::from(r.snap.props.signed.iter().map(|(who, ..)| slint::SharedString::from(format!("Open the version signed by {who}"))).collect::<Vec<_>>())),
        props_trust: ModelRc::new(VecModel::from(r.snap.props.signed.iter().map(|(who, _, _, trusted)| slint::SharedString::from(if *trusted { String::new() } else { format!("Trust {who}'s certificate") })).collect::<Vec<_>>())),
        props_opening: {
            let (p, l, z, n, w) = super::properties::opening_controls(&r.snap.opening);
            ModelRc::new(VecModel::from(vec![p, l, z, n, w]))
        },
        props_custom: r.facts.as_ref().map(|f| super::properties::custom_text(&f.custom)).unwrap_or_default().into(),
        props_about: ModelRc::new(VecModel::from(super::properties::about_rows(&r.snap.props.about, r.facts.as_ref()).into_iter().map(|(n, v)| ReaderProp { name: n.into(), value: v.into() }).collect::<Vec<_>>())),
        text_size: r.text_pt.round() as i32,
        text_family: r.text_family,
        text_bold: r.text_bold,
        text_underline: r.text_underline,
        text_bullets: r.text_bullets,
        text_italic: r.text_italic,
        text_align: r.text_align,
        selected_image: r.selected_image.is_some(),
        selected_text_box: r.selected_annot.and_then(|i| r.annots().get(i)).is_some_and(|a| a.subtype == "FreeText" && !a.fill_sign),
        pages_selected: r.page_sel.len() as i32,
        sign_open: r.sign_open,
        print_open: r.print_open,
        print_preview: r.print_preview.clone().unwrap_or_default(),
        has_print_preview: r.print_preview.is_some(),
        printers: ModelRc::new(VecModel::from(r.printers.iter().map(slint::SharedString::from).collect::<Vec<_>>())),
        printer: r.printer as i32,
        has_signature: r.tool == SIGN && !r.initials && saved_signature().is_some(),
        initials: r.initials,
        sign_every_page: r.sign_every_page,
        signatures: ModelRc::new(VecModel::from(if r.tool == SIGN || r.signature_manager.is_some() { signature_thumbs() } else { Vec::new() })),
        has_form: r.snap.fields.iter().any(|f| f.kind < 4),
        choices: ModelRc::new(VecModel::from(
            r.draft
                .as_ref()
                .filter(|d| d.kind == 5)
                .and_then(|d| d.field)
                .and_then(|(fi, _)| r.snap.fields.get(fi))
                .map(|f| f.options.iter().map(|(e, t)| slint::SharedString::from(if t.is_empty() { e } else { t })).collect::<Vec<_>>())
                .unwrap_or_default(),
        )),
        results,
        search_pages,
        search_history: ModelRc::new(VecModel::from(super::search::recent_searches().into_iter().map(Into::into).collect::<Vec<slint::SharedString>>())),
        case_sensitive: r.search.case_sensitive,
        whole_words: r.search.whole_words,
    };
    if reader::backdrop(r.theme).or_else(crate::home::backdrop).is_none() {
        tab.reader.backdrop = Color::from_argb_u8(0, 0, 0, 0);
    }
}

pub(super) fn outline_model(r: &ReaderUi, current_row: Option<usize>) -> Vec<ReaderOutlineRow> {
    let outline: Vec<ReaderOutlineRow> = r
        .rows
        .iter()
        .enumerate()
        .map(|(i, x)| ReaderOutlineRow {
            title: x.title.clone().into(),
            depth: x.depth as i32,
            page_label: x.page.map(|p| label(r, p)).unwrap_or_default().into(),
            has_children: x.has_children,
            expanded: x.expanded,
            current: current_row == Some(i),
            editing: r.outline_edit == Some(i),
            ..{
                let s = r.snap.outline_styles.iter().find(|s| s.0 == x.path);
                let c = s.and_then(|s| s.3);
                ReaderOutlineRow {
                    bold: s.is_some_and(|s| s.1),
                    italic: s.is_some_and(|s| s.2),
                    has_colour: c.is_some(),
                    colour: c.map_or_else(Default::default, |c| colour_of(c.map(f64::from))),
                    ..Default::default()
                }
            }
        })
        .collect();
    outline
}

/// While the Notes panel is open, the text of the pages holding a highlight saved without a note
/// is read, so the list shows the words it covers (Acrobat), whether or not that page was shown.
/// Read on a thread of its own: a long annotated file kept the window waiting (Sol, 25/10); the
/// rows fill in as it ends.
pub(super) fn quote_texts(r: &mut ReaderUi, tab_id: i32) {
    if !(r.panel_open && r.panel == 2) || r.quotes_reading {
        return;
    }
    let pages: Vec<usize> = r
        .annots()
        .iter()
        .filter(|a| !a.quads.is_empty() && a.contents.is_empty())
        .map(|a| a.page)
        .filter(|p| !r.texts.contains_key(p))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if pages.is_empty() {
        return;
    }
    r.quotes_reading = true;
    let bytes = r.snap.bytes.clone();
    let config = render::RenderConfig { password: r.snap.password.as_deref().map(Arc::from), ..Default::default() };
    std::thread::spawn(move || {
        let mut rr = render::PageRenderer::new(bytes.clone(), config);
        let read: Vec<(usize, Arc<PageText>)> = pages
            .into_iter()
            .filter_map(|page| rr.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() }).text.map(|t| (page, t)))
            .collect();
        let _ = slint::invoke_from_event_loop(move || {
            update(tab_id, |r, _| {
                r.quotes_reading = false;
                // Texts of the file as it is: an edit since then has its own.
                if Arc::ptr_eq(&r.snap.bytes, &bytes) {
                    for (page, t) in read {
                        r.texts.entry(page).or_insert(t);
                    }
                }
            });
        });
    });
}

pub(super) fn annots_model(r: &ReaderUi, v: &ViewerUi) -> Vec<ReaderAnnotRow> {
    let mut rows: Vec<(&AnnotEntry, ReaderAnnotRow)> = r
        .annots()
        .iter()
        .enumerate()
        .filter(|(_, a)| reader::notes_filter(r.notes_filter, &a.subtype, a.fill_sign, &a.status, a.checked))
        .map(|(i, a)| {
            let text = if a.quads.is_empty() || !a.contents.is_empty() {
                a.contents.clone()
            } else {
                r.texts
                    .get(&a.page)
                    .map(|t| {
                        let size = v.viewer.boxes[a.page].view_size();
                        let rects: Vec<[f32; 4]> = a
                            .quads
                            .iter()
                            .map(|q| {
                                let [x, y, w, h] = viewer::pdf_rect_to_view(&v.viewer.boxes[a.page], size, sheaf_core::redact::bounds(std::slice::from_ref(q)));
                                [x as f32, y as f32, (x + w) as f32, (y + h) as f32]
                            })
                            .collect();
                        reader::text_under(t, &rects)
                    })
                    .unwrap_or_default()
            };
            let row = ReaderAnnotRow {
                kind: if a.fill_sign { "Signature".into() } else { reader::kind_label(&a.subtype).into() },
                text: text.chars().take(300).collect::<String>().into(),
                meta: [a.author.trim(), a.modified.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ").into(),
                page_label: format!("Page {}", label(r, a.page)).into(),
                colour: a.color.map(|c| colour_of([c[0] as f64, c[1] as f64, c[2] as f64])).unwrap_or(Color::from_rgb_u8(255, 214, 10)),
                selected: r.selected_annot == Some(i),
                index: i as i32,
                replies: ModelRc::new(VecModel::from(a.replies.iter().map(|(t, p, i)| ReaderReply { text: t.into(), page: *p as i32, index: *i as i32 }).collect::<Vec<_>>())),
                status: a.status.clone().into(),
            };
            (a, row)
        })
        .collect();
    // In page order as listed; else newest first, by author, by kind (page order within each).
    match r.notes_sort {
        1 => rows.sort_by(|x, y| y.0.modified.cmp(&x.0.modified)),
        2 => rows.sort_by_key(|x| x.0.author.to_lowercase()),
        3 => rows.sort_by(|x, y| x.1.kind.cmp(&y.1.kind)),
        _ => {}
    }
    rows.into_iter().map(|(_, row)| row).collect()
}

/// The page's label ("iv") or number, 1-based.
pub(super) fn label(r: &ReaderUi, p: usize) -> String {
    if crate::home::physical_numbers() {
        return (p + 1).to_string();
    }
    r.snap.labels.get(p).cloned().filter(|l| !l.is_empty()).unwrap_or_else(|| (p + 1).to_string())
}
