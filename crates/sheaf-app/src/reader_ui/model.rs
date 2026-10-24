//! Into the tab model: the ReaderState pushed to Slint, its marks, panels and rows.

use super::*;

/// The palette entry (light or ink) closest to `c`, if one is close.
pub(super) fn palette_index(c: [f32; 3]) -> i32 {
    let d = |p: &[f64; 3]| (0..3).map(|i| (p[i] - c[i] as f64).powi(2)).sum::<f64>();
    COLOURS.iter().chain(INK.iter()).enumerate().map(|(i, p)| (i % 5, d(p))).filter(|(_, d)| *d < 0.01).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i as i32).unwrap_or(-1)
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
    // The box being linked, while its target is typed.
    if let Some(d) = r.draft.as_ref().filter(|d| d.kind == 8 || d.kind == 9)
        && let Some((_, _, rect)) = &d.block
    {
        marks.push(overlay(v.viewer.overlay(d.page, *rect), 6));
    }
    if r.tool == SELECT {
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
        // The paragraphs that can be edited, outlined on the pages in view (read when first shown).
        for page in wanted.clone() {
            if let Some(blocks) = r.blocks.get(&page) {
                for (_, b) in blocks.iter().enumerate().filter(|(i, _)| moving != Some((page, Some(*i), None))) {
                    let c = v.viewer.overlay(page, b.rect);
                    marks.push(overlay([c[0] - 2.0, c[1] - 1.0, c[2] + 4.0, c[3] + 2.0], 7));
                }
            }
            if let Some(images) = r.images.get(&page) {
                for (_, (rc, _)) in images.iter().enumerate().filter(|(i, _)| moving != Some((page, None, Some(*i)))) {
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
        for (i, ((page, _), (b, m, a))) in r.search.hits.iter().zip(&r.search.snippets).enumerate().skip(have).take(2000usize.saturating_sub(have)) {
            model.push(ReaderResultRow {
                before: b.clone().into(),
                hit: m.clone().into(),
                after: a.clone().into(),
                page_label: format!("Page {}", label(r, *page)).into(),
                current: r.search.current == Some(i),
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
        ModelRc::from(model.clone())
    };
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
                Some((_, t, _)) if d.kind == 8 || d.kind == 9 => (t.clone(), 300.0, if r.link_error.is_empty() { 116.0 } else { 140.0 }),
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
            ReaderNote { show: true, x: x as f32, y: y as f32, w: w as f32, h: h as f32, text: text.into(), kind: d.kind, seq: r.note_seq }
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
        title: r.name.clone().into(),
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
        theme: r.theme.index(),
        backdrop: reader::backdrop(r.theme).map(|c| Color::from_rgb_u8(c[0], c[1], c[2])).unwrap_or_default(),
        marks: ModelRc::new(VecModel::from(marks)),
        provisional: ModelRc::new(VecModel::from(provisional)),
        has_selection: r.sel.is_some(),
        selected_text: r.sel.as_ref().and_then(|(p, range)| r.texts.get(p).map(|t| reader::selected_text(t, range.clone()))).unwrap_or_default().into(),
        selected_annot: r.selected_annot.is_some(),
        annot_light: r.selected_annot.and_then(|i| r.annots().get(i)).is_some_and(|a| matches!(a.subtype.as_str(), "Highlight" | "Text")),
        annot_colour: r.selected_annot.and_then(|i| r.annots().get(i)).and_then(|a| a.color).map(palette_index).unwrap_or(-1),
        ink: ink.into(),
        // The eraser leaves a grey trail.
        ink_colour: if r.tool == ERASER { Color::from_argb_u8(140, 128, 128, 128) } else { colour_of(INK[r.ink_colour]) },
        note,
        search_count: search_count.into(),
        search_busy: r.search.busy,
        can_undo: r.snap.can_undo && r.in_flight == 0,
        can_redo: r.snap.can_redo && r.in_flight == 0,
        zoom_mode: r.zoom_mode,
        spread: v.viewer.spread() as i32,
        pen_width: r.pen_width,
        shape_kind: r.shape_kind,
        shape_fill: r.shape_fill,
        opacity: (reader::OPACITIES[r.opacity] * 100.0).round() as i32,
        // Dynamic stamps share their names with standard ones: they say they are dated.
        stamps: ModelRc::new(VecModel::from(
            MARKS
                .iter()
                .map(|(_, l)| slint::SharedString::from(*l))
                .chain(StampKind::ALL.iter().map(|k| match k.group() {
                    sheaf_core::pdfcraft::engine::StampGroup::Dynamic => slint::SharedString::from(format!("{} · DATED", k.label())),
                    _ => slint::SharedString::from(k.label()),
                }))
                .collect::<Vec<_>>(),
        )),
        stamp: r.stamp as i32,
        comments_hidden: r.comments_hidden,
        // The Link tool shows where the links are, as PDF Expert's does.
        show_links: r.show_links || r.tool == LINK,
        link_error: r.link_error.clone().into(),
        link_visible: r.link_visible,
        link_dashed: r.link_dashed,
        attachments: ModelRc::new(VecModel::from(
            r.snap
                .attachments
                .iter()
                .map(|a| ReaderProp { name: a.name.clone().into(), value: a.size.map(|n| if n >= 1 << 20 { format!("{:.1} MB", n as f64 / 1_048_576.0) } else { format!("{:.0} KB", (n as f64 / 1024.0).max(1.0)) }).unwrap_or_default().into() })
                .collect::<Vec<_>>(),
        )),
        notes_filter: r.notes_filter,
        notes_sort: r.notes_sort,
        props_open: r.props_open,
        props_title: r.snap.props.title.clone().into(),
        props_author: r.snap.props.author.clone().into(),
        props_subject: r.snap.props.subject.clone().into(),
        props_keywords: r.snap.props.keywords.clone().into(),
        props_about: ModelRc::new(VecModel::from(r.snap.props.about.iter().map(|(n, v)| ReaderProp { name: n.into(), value: v.into() }).collect::<Vec<_>>())),
        text_size: r.text_pt.round() as i32,
        text_family: r.text_family,
        text_bold: r.text_bold,
        text_italic: r.text_italic,
        text_align: r.text_align,
        selected_image: r.selected_image.is_some(),
        selected_text_box: r.selected_annot.and_then(|i| r.annots().get(i)).is_some_and(|a| a.subtype == "FreeText" && !a.fill_sign),
        pages_selected: r.page_sel.len() as i32,
        sign_open: r.sign_open,
        print_open: r.print_open,
        printers: ModelRc::new(VecModel::from(r.printers.iter().map(slint::SharedString::from).collect::<Vec<_>>())),
        printer: r.printer as i32,
        has_signature: r.tool == SIGN && saved_signature().is_some(),
        signatures: ModelRc::new(VecModel::from(if r.tool == SIGN { signature_thumbs() } else { Vec::new() })),
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
        case_sensitive: r.search.case_sensitive,
        whole_words: r.search.whole_words,
    };
    if reader::backdrop(r.theme).is_none() {
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
        })
        .collect();
    outline
}

pub(super) fn annots_model(r: &ReaderUi, v: &ViewerUi) -> Vec<ReaderAnnotRow> {
    let mut rows: Vec<(&AnnotEntry, ReaderAnnotRow)> = r
        .annots()
        .iter()
        .enumerate()
        .filter(|(_, a)| reader::notes_filter(r.notes_filter, &a.subtype, a.fill_sign))
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
                replies: a.replies.join("\n").into(),
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
    r.snap.labels.get(p).cloned().filter(|l| !l.is_empty()).unwrap_or_else(|| (p + 1).to_string())
}
