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
        // An edit naming a comment, paragraph or image by its index means the index in the last
        // snapshot: with an edit still on its way, it could name another one.
        if r.edits_in_flight > 0 && by_index(&edit) {
            return Some("One moment: the last change is still being applied".into());
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

/// Whether `edit` names something on a page by its index (comments, paragraphs, images).
fn by_index(edit: &Edit) -> bool {
    match edit {
        Edit::Batch { edits, .. } => edits.iter().any(by_index),
        Edit::MoveAnnotation { .. }
        | Edit::ResizeAnnotation { .. }
        | Edit::TurnAnnotation { .. }
        | Edit::DashAnnotation { .. }
        | Edit::DeleteAnnotation { .. }
        | Edit::EraseInk { .. }
        | Edit::StyleAnnotation { .. }
        | Edit::SetTextFont { .. }
        | Edit::SetAnnotationContents { .. }
        | Edit::SetAnnotationStatus { .. }
        | Edit::ReplyToAnnotation { .. }
        | Edit::EditTextBlock { .. }
        | Edit::SetLink { .. }
        | Edit::DeleteLink { .. }
        | Edit::MoveObjects { .. }
        | Edit::ArrangeObjects { .. }
        | Edit::DeleteObjects { .. }
        | Edit::EditPageImage { .. }
        | Edit::RenameBookmark { .. }
        | Edit::DeleteBookmark { .. }
        | Edit::MoveBookmark { .. }
        | Edit::SetBookmarkPage { .. } => true,
        _ => false,
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
    r.selected_image = None;
    r.image_resize = None;
    r.sign_at = None;
    r.provisional.clear();
    r.texts.clear();
    r.blocks.clear();
    r.images.clear();
    r.objects.clear();
    r.picked = None;
    r.repick = None;
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
pub(super) fn after_change(ui: &AppWindow, tab_id: i32, r: &mut ReaderUi, v: &mut ViewerUi, pages: Option<&[usize]>) {
    layers::remake(ui, tab_id, r, v);
    let turn = v.viewer.turn();
    if v.viewer.boxes != viewer::turned(&r.snap.boxes, turn) {
        // A page was added, removed or turned (the view's own turn kept).
        let width = v.viewer.content_width();
        v.viewer = reader_viewer(r.snap.boxes.clone());
        v.viewer.set_turn(i32::from(turn));
        v.viewer.set_width(width);
    }
    match pages {
        Some(ps) => ps.iter().for_each(|&p| {
            v.page_changed(p);
            r.blocks.remove(&p);
            r.images.remove(&p);
            r.objects.remove(&p);
            r.blocks_asked.remove(&p);
        }),
        None => {
            v.all_changed();
            r.blocks.clear();
            r.images.clear();
            r.objects.clear();
            r.blocks_asked.clear();
        }
    }
    // The text of the pages changed is read again when needed (edits of the page content change
    // it; whoever filled the cache, a selection or a search, it must not outlive them).
    match pages {
        Some(ps) => ps.iter().for_each(|p| {
            r.texts.remove(p);
        }),
        None => r.texts.clear(),
    }
    r.version += 1;
    // A selection that survives the change is selected again, so the format row shows the
    // comment as it is now (after an undo, say).
    match r.selected_annot {
        Some(i) if i >= r.snap.annots.len() => r.selected_annot = None,
        Some(i) => select_annot(r, i),
        None => {}
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
        // From the selection bar (another tool active), the markup tool's own saved colour.
        let saved = (r.tool != kind).then(|| crate::home::tool_style(kind)).flatten().map(|(c, ..)| c as usize);
        let (m, colour) = match kind {
            UNDERLINE => (Markup::Underline, ink_at(saved.unwrap_or(r.ink_colour))),
            STRIKE => (Markup::StrikeOut, ink_at(saved.unwrap_or(r.ink_colour))),
            _ => (Markup::Highlight, light_at(saved.unwrap_or(r.light))),
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
    crate::home::author()
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

/// `add`, and when the line is to be dashed (the Dashed option), the mark it adds dashed in the
/// same undo step.
fn maybe_dashed(r: &ReaderUi, page: usize, add: Edit) -> Edit {
    if r.dashed {
        Edit::Batch { label: "Add comment".into(), edits: vec![add, Edit::DashAnnotation { page, index: usize::MAX, dashed: true }] }
    } else {
        add
    }
}

pub(super) fn add_ink(ui: &AppWindow, tab_id: i32, page: usize, pts: Vec<(f64, f64)>) {
    let pts = smooth(&pts);
    let edit = update(tab_id, |r, v| {
        let b = &v.viewer.boxes[page];
        let size = b.view_size();
        let stroke: Vec<[f64; 2]> = pts.iter().map(|p| { let (x, y) = viewer::view_to_pdf(b, size, p.0, p.1); [x, y] }).collect();
        let prov = Provisional { page, rects: Vec::new(), stroke: pts.clone(), colour: ink_at(r.ink_colour), kind: PEN };
        let style = Style { opacity: super::tools::opacity_of(r), ..style(ink_at(r.ink_colour), pen_points(r.pen_width)) };
        (maybe_dashed(r, page, Edit::AddAnnotation(NewAnnotation { page, shape: Shape::Ink { strokes: vec![stroke] }, style, contents: String::new(), author: author() })), prov)
    });
    if let Some((e, prov)) = edit {
        apply(ui, tab_id, e, &[page], Some(prov));
    }
}

/// A rectangle, oval, line or arrow dragged from `a` to `b` (view points of `page`), in the pen's
/// colour, width and opacity.
pub(super) fn add_shape(ui: &AppWindow, tab_id: i32, page: usize, a: (f64, f64), b: (f64, f64)) {
    let edit = update(tab_id, |r, v| {
        let bx = &v.viewer.boxes[page];
        let size = bx.view_size();
        let (pa, pb) = (viewer::view_to_pdf(bx, size, a.0, a.1), viewer::view_to_pdf(bx, size, b.0, b.1));
        let rect = [pa.0.min(pb.0), pa.1.min(pb.1), pa.0.max(pb.0), pa.1.max(pb.1)];
        let shape = match r.shape_kind {
            0 => Shape::Rectangle { rect },
            1 => Shape::Oval { rect },
            // An arrow's head as chosen, at both ends when asked (Acrobat's Line Ends, start and end).
            k => {
                let head = if k == 3 { reader::ARROW_HEADS[r.arrow_head.min(reader::ARROW_HEADS.len() - 1)].0 } else { LineEnding::None };
                Shape::Line { from: [pa.0, pa.1], to: [pb.0, pb.1], start: if r.arrow_both { head } else { LineEnding::None }, end: head }
            }
        };
        let colour = ink_at(r.ink_colour);
        // Its own fill colour (a highlight colour or the sixth), else a tint of the stroke.
        let fill = (r.shape_fill && r.shape_kind < 2).then(|| r.fill_colour.map_or_else(|| reader::fill_tint(colour), light_at));
        let style = Style { color: colour, opacity: super::tools::opacity_of(r), width: pen_points(r.pen_width), fill };
        let prov = Provisional { page, rects: Vec::new(), stroke: reader::shape_outline(r.shape_kind, a, b), colour, kind: PEN };
        (maybe_dashed(r, page, Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: String::new(), author: author() })), prov)
    });
    if let Some((e, prov)) = edit {
        apply(ui, tab_id, e, &[page], Some(prov));
    }
}

/// The Polygon, Cloud, Polygonal line or measure clicked out ends (a double-click, or a distance's
/// second point): two vertices at least for a line, three for a polygon or an area. Acrobat's
/// Polygon, Cloud and Polygonal Line tools, and its Measuring tools (true size, in cm).
pub(super) fn add_poly(ui: &AppWindow, tab_id: i32) {
    let edit = update(tab_id, |r, v| {
        let (page, pts) = r.poly.take()?;
        r.stroke = None;
        let closed = reader::closed_shape(r.shape_kind);
        if pts.len() < if closed { 3 } else { 2 } {
            return None;
        }
        let bx = &v.viewer.boxes[page];
        let size = bx.view_size();
        let vertices: Vec<[f64; 2]> = pts.iter().map(|p| { let (x, y) = viewer::view_to_pdf(bx, size, p.0, p.1); [x, y] }).collect();
        let colour = ink_at(r.ink_colour);
        let fill = (r.shape_fill && closed && r.shape_kind != 9).then(|| r.fill_colour.map_or_else(|| reader::fill_tint(colour), light_at));
        let style = Style { color: colour, opacity: super::tools::opacity_of(r), width: pen_points(r.pen_width), fill };
        let mut outline = pts.clone();
        if closed {
            outline.push(pts[0]);
        }
        let prov = Provisional { page, rects: Vec::new(), stroke: outline, colour, kind: PEN };
        // Calibrate: the line drawn is a length the user knows; its real length is asked for at its end.
        if r.calibrating && r.shape_kind == 7 {
            r.calibration_len = vertices.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
            let end = *pts.last()?;
            r.link_error.clear();
            r.draft = Some(Draft { page, at: (end.0, end.1 + 8.0), kind: 12, block: Some((0, String::new(), [0.0; 4])), field: None });
            r.note_seq += 1;
            return None;
        }
        let edit = match r.shape_kind {
            7..=9 => {
                use sheaf_core::pdfcraft::engine::measure::{Kind, NewMeasurement, Scale};
                let kind = [Kind::Distance, Kind::Perimeter, Kind::Area][(r.shape_kind - 7) as usize];
                let factor = r.measure_custom.unwrap_or_else(|| reader::measure_factor(r.measure_scale, v.viewer.boxes.get(page).map_or(1.0, |b| b.unit)));
                let label = reader::measure_label(r.shape_kind, &vertices, factor, r.measure_decimals);
                let scale = Scale::new(factor.0, factor.1, r.measure_decimals as u8).ok()?;
                Edit::AddMeasurement(NewMeasurement { page, kind, points: vertices, scale, style, label, author: author() })
            }
            k => {
                let shape = if k < 6 { Shape::Polygon { vertices, cloud: k == 5 } } else { Shape::PolyLine { vertices, start: LineEnding::None, end: LineEnding::None } };
                maybe_dashed(r, page, Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: String::new(), author: author() }))
            }
        };
        Some((page, edit, prov))
    })
    .flatten();
    if let Some((page, e, prov)) = edit {
        apply(ui, tab_id, e, &[page], Some(prov));
    }
}

/// The eraser went along `pts` (view points of `page`): every drawing it crossed is rubbed out
/// along the path (PdfCraft's EraseInk), as one undo step. Signatures are left alone.
pub(super) fn erase(ui: &AppWindow, tab_id: i32, page: usize, pts: Vec<(f64, f64)>) {
    let edit = update(tab_id, |r, v| {
        // The three widths of the pen are the eraser's sizes too.
        let radius = [3.0, 6.0, 12.0][r.pen_width.clamp(0, 2) as usize];
        let bx = &v.viewer.boxes[page];
        let size = bx.view_size();
        let path: Vec<[f64; 2]> = pts.iter().map(|p| { let (x, y) = viewer::view_to_pdf(bx, size, p.0, p.1); [x, y] }).collect();
        let (lo, hi) = path.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let crosses = |rc: [f64; 4]| rc[0] <= hi[0] + radius && rc[2] >= lo[0] - radius && rc[1] <= hi[1] + radius && rc[3] >= lo[1] - radius;
        // Highest index first: a drawing rubbed out entirely is removed, which would shift the
        // indices after it.
        let mut hit: Vec<usize> = r.annots().iter().filter(|a| a.page == page && a.subtype == "Ink" && !a.fill_sign && crosses(a.rect)).map(|a| a.index).collect();
        hit.sort_unstable_by(|a, b| b.cmp(a));
        let mut edits: Vec<Edit> = hit.into_iter().map(|index| Edit::EraseInk { page, index, path: path.clone(), radius }).collect();
        match edits.len() {
            0 => None,
            1 => edits.pop(),
            _ => Some(Edit::Batch { label: "Erase".into(), edits }),
        }
    })
    .flatten();
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// The chosen stamp, 160 × 42 pt centred on `at` (view point of `page`), as Acrobat sizes them;
/// a dynamic stamp says who and when ("By Ada at 2:14 pm, Oct 02, 2026").
/// A dated stamp's time in the format chosen in Preferences (0 Acrobat's, 1 day first, 2 ISO).
fn stamp_time(t: chrono::DateTime<chrono::Local>, format: i32) -> String {
    t.format(match format {
        1 => "%d/%m/%Y %H:%M",
        2 => "%Y-%m-%d %H:%M",
        _ => "%-I:%M %P, %b %d, %Y",
    })
    .to_string()
}

pub(super) fn add_stamp(ui: &AppWindow, tab_id: i32, page: usize, at: (f64, f64)) {
    let edit = update(tab_id, |r, v| {
        let bx = &v.viewer.boxes[page];
        let size = bx.view_size();
        // A picture of the user's own, 150 pt wide (less on a small page), centred on the click.
        if let Some((name, bytes, aspect)) = r.custom_stamp.clone() {
            let w = 150.0_f64.min(size.0 * 0.8).min(size.1 * 0.8 / aspect);
            let h = w * aspect;
            let (x, y) = ((at.0 - w / 2.0).clamp(0.0, size.0 - w), (at.1 - h / 2.0).clamp(0.0, size.1 - h));
            let rect = viewer::view_rect_to_pdf(bx, size, (x, y), (x + w, y + h));
            let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s).to_string();
            return Edit::AddCustomStamp { page, rect, name: stem, file: sheaf_core::pdfcraft::engine::MarkFile { name, bytes, page: 0 }, author: author() };
        }
        // A mark (✓ ✗ • —): small, centred on the click, as Fill & Sign places it.
        if let Some((mark, label)) = MARKS.get(r.stamp).copied() {
            let (w, h) = (18.0, 18.0);
            let rect = viewer::view_rect_to_pdf(bx, size, (at.0 - w / 2.0, at.1 - h / 2.0), (at.0 + w / 2.0, at.1 + h / 2.0));
            let shape = Shape::Mark { rect, mark };
            // In the colour picked, as Acrobat's Fill & Sign marks take the text colour.
            let style = Style { color: ink_at(r.ink_colour), ..Style::default_for(&shape) };
            return Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: label.trim_start_matches(|c: char| !c.is_alphabetic()).into(), author: author() });
        }
        let (w, h) = (160.0_f64.min(size.0), 42.0_f64.min(size.1));
        let x = (at.0 - w / 2.0).clamp(0.0, size.0 - w);
        let y = (at.1 - h / 2.0).clamp(0.0, size.1 - h);
        let rect = viewer::view_rect_to_pdf(bx, size, (x, y), (x + w, y + h));
        let stamp = StampKind::ALL[(r.stamp - MARKS.len()).min(StampKind::ALL.len() - 1)];
        let dynamic = matches!(stamp.group(), sheaf_core::pdfcraft::engine::StampGroup::Dynamic);
        // The time is formatted alone: the author's name may hold a % (a format directive).
        let by = dynamic.then(|| format!("By {} at {}", author(), stamp_time(chrono::Local::now(), crate::home::prefs().stamp_date)));
        let shape = Shape::Stamp { rect, stamp, by };
        let style = Style::default_for(&shape);
        Edit::AddAnnotation(NewAnnotation { page, shape, style, contents: stamp.label().into(), author: author() })
    });
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// Stamp ▸ Image…: a PNG or JPEG chosen becomes the stamp, placed at each click until another
/// stamp is picked (Acrobat's Create Custom Stamp, kept for the session).
pub fn stamp_image(ui: &AppWindow, tab_id: i32) {
    let d = crate::dialog::new(ui).set_title("Choose a picture for the stamp").add_filter("Image", &["png", "jpg", "jpeg"]);
    crate::dialog::pick_file(ui, d, move |ui, p| {
        let Some(stamp) = picture_stamp(&p) else {
            return crate::flow::toast(ui, "That picture could not be read (PNG or JPEG)");
        };
        // Kept in the library for next time (Sol, 25/10: it lasted as long as its tab), and chosen.
        let kept = crate::home::keep_stamp(&stamp.0, &stamp.1);
        update(tab_id, |r, _| {
            let first_picture = MARKS.len() + StampKind::ALL.len();
            if let Some(k) = kept.and_then(|f| crate::home::stamp_library().iter().position(|p| *p == f)) {
                r.stamp = first_picture + k;
            }
            r.custom_stamp = Some(stamp);
        });
    });
}

/// A picture file as a stamp: (name, bytes, height / width), `None` when it is no PNG or JPEG.
pub(super) fn picture_stamp(p: &std::path::Path) -> Option<(String, Arc<Vec<u8>>, f64)> {
    let bytes = std::fs::read(p).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "stamp.png".into());
    Some((name, Arc::new(bytes), img.height() as f64 / img.width().max(1) as f64))
}

/// The Notes panel's actions: 0 hide or show every comment, 1 a summary of the comments (a PDF,
/// saved then opened), 2 remove every comment (one undo step), 3 export the comments (XFDF or
/// FDF), 4 import comments from an XFDF or FDF file (one undo step).
pub fn notes_action(ui: &AppWindow, tab_id: i32, what: i32) {
    match what {
        0 => {
            update(tab_id, |r, v| {
                r.comments_hidden = !r.comments_hidden;
                v.hide_comments = r.comments_hidden;
                r.selected_annot = None;
                v.all_changed();
            });
            crate::viewer_ui::request(ui, tab_id);
        }
        1 => {
            let Some((dir, stem)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())) else { return };
            let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(format!("{stem} - comments.pdf")).add_filter("PDF", &["pdf"]);
            crate::dialog::save_file(ui, d, move |_, target| {
                update(tab_id, |r, _| r.send(Op::Summary { to: target }));
            });
        }
        3 => {
            let Some((dir, stem)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())) else { return };
            let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(format!("{stem}.xfdf")).add_filter("XFDF", &["xfdf"]).add_filter("FDF", &["fdf"]);
            crate::dialog::save_file(ui, d, move |_, target| {
                update(tab_id, |r, _| r.send(Op::ExportComments { to: target }));
            });
        }
        4 => {
            let d = crate::dialog::new(ui).add_filter("Comments", &["xfdf", "fdf"]);
            crate::dialog::pick_file(ui, d, move |ui, path| {
                let Ok(bytes) = std::fs::read(&path) else {
                    return crate::flow::toast(ui, "That file could not be read");
                };
                // The comments may land on any page: every page renders again once they are in.
                let all: Vec<usize> = update(tab_id, |_, v| (0..v.viewer.page_count()).collect()).unwrap_or_default();
                apply(ui, tab_id, Edit::ImportData { name: file_name(&path), bytes: Arc::new(bytes) }, &all, None);
            });
        }
        _ => {
            let edit = update(tab_id, |r, _| {
                // Highest index first on each page: deleting shifts the indices after it.
                let mut all: Vec<(usize, usize)> = r.annots().iter().map(|a| (a.page, a.index)).collect();
                all.sort_unstable_by(|a, b| b.cmp(a));
                let mut pages: Vec<usize> = all.iter().map(|a| a.0).collect();
                pages.dedup();
                let edits: Vec<Edit> = all.into_iter().map(|(page, index)| Edit::DeleteAnnotation { page, index }).collect();
                (!edits.is_empty()).then(|| (Edit::Batch { label: "Remove all comments".into(), edits }, pages))
            })
            .flatten();
            if let Some((e, pages)) = edit {
                apply(ui, tab_id, e, &pages, None);
            }
        }
    }
}

/// Review status of comment `k` (Notes panel): 0 none, 1 accepted, 2 rejected, 3 cancelled,
/// 4 completed; a status reply by the author, as Acrobat's Set Status.
pub fn annot_status(ui: &AppWindow, tab_id: i32, k: i32, state: i32) {
    use sheaf_core::pdfcraft::engine::ReviewState as S;
    let state = [S::None, S::Accepted, S::Rejected, S::Cancelled, S::Completed][state.clamp(0, 4) as usize];
    let edit = update(tab_id, |r, _| r.annots().get(k.max(0) as usize).map(|a| (a.page, Edit::SetAnnotationStatus { page: a.page, index: a.index, state, author: author() }))).flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

pub fn note_commit(ui: &AppWindow, tab_id: i32, text: &str) {
    let text = text.trim().to_string();
    let edit = update(tab_id, |r, v| {
        let d = r.draft.take()?;
        let b = &v.viewer.boxes[d.page];
        let size = b.view_size();
        if d.kind == 11 {
            // Insert Text: a caret after the selection's last character, holding the words.
            let last = r.replace_quads.pop()?;
            r.replace_quads.clear();
            if text.is_empty() {
                return None;
            }
            let (x, y0, y1) = (last[2].max(last[6]), last[5].min(last[7]), last[1].max(last[3]));
            let rect = [x - 3.0, y0, x + 3.0, y0 + (y1 - y0) * 0.6];
            let shape = Shape::Caret { rect };
            let style = Style { color: ink_at(r.ink_colour), ..Style::default_for(&shape) };
            return Some((d.page, Edit::AddAnnotation(NewAnnotation { page: d.page, shape, style, contents: text.clone(), author: author() })));
        }
        if d.kind == 10 {
            // Replace Text: the words struck out, a caret after them holding the new ones (in
            // the ink colour, as Strikethrough). Nothing typed proposes nothing.
            let quads = std::mem::take(&mut r.replace_quads);
            if text.is_empty() || quads.is_empty() {
                return None;
            }
            let s = style(ink_at(r.ink_colour), 1.0);
            return Some((d.page, Edit::ReplaceText { page: d.page, quads, text: text.clone(), author: author(), strike: s.clone(), caret: s }));
        }
        if d.kind == 12 {
            // Calibrate: the real length of the line just measured sets the scale; a length not
            // understood keeps the editor open to correct it.
            match reader::calibrated(&text, r.calibration_len) {
                Ok(scale) => {
                    r.measure_custom = Some(scale);
                    r.calibrating = false;
                    r.link_error.clear();
                }
                Err(why) => {
                    r.link_error = why;
                    r.draft = Some(d);
                    r.note_seq += 1;
                }
            }
            return None;
        }
        if d.kind == 8 || d.kind == 9 {
            // Link: a page number or an address; an existing link emptied is removed.
            let (index, old, rect) = d.block.clone()?;
            if d.kind == 9 && text.is_empty() {
                return Some((d.page, Edit::DeleteLink { page: d.page, index }));
            }
            let Some(action) = reader::link_action(&text, v.viewer.page_count()) else {
                // Not a page or an address: the editor stays open to correct it.
                r.link_error = format!("“{text}” is not a page number (1–{}) or a web address", v.viewer.page_count());
                r.draft = Some(d);
                r.note_seq += 1;
                return None;
            };
            r.link_error.clear();
            if d.kind == 9 {
                return (text != old).then_some((d.page, Edit::SetLink { page: d.page, index, rect: None, action: Some(action), style: None }));
            }
            let style = super::tools::link_style(r);
            // Selected text: a box per line, one undo step.
            let rects = std::mem::take(&mut r.link_rects);
            if rects.len() > 1 {
                let edits = rects.into_iter().map(|rect| Edit::AddLink { page: d.page, rect, action: action.clone(), style }).collect();
                return Some((d.page, Edit::Batch { label: "Add link".into(), edits }));
            }
            return Some((d.page, Edit::AddLink { page: d.page, rect: rects.first().copied().unwrap_or(rect), action, style }));
        }
        if d.kind == 7 {
            // A reply to an existing comment (PdfCraft's ReplyToAnnotation: /IRT, the author's name).
            let (index, _, _) = d.block?;
            if text.is_empty() {
                return None;
            }
            return Some((d.page, Edit::ReplyToAnnotation { page: d.page, index, text: text.clone(), author: author() }));
        }
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
            crate::home::remember_entry(&text);
            return Some((d.page, Edit::SetFieldValue { name: f.name.clone(), value: FieldValue::Text(text.clone()) }));
        }
        if d.kind == 2 {
            // The preview, if any, is taken back by the edit itself (the document thread).
            r.preview = None;
            r.preview_timer = None;
            return paragraph_edit(r, &d, &text);
        }
        if text.is_empty() {
            return None;
        }
        if d.kind == 3 {
            // The font, size and weight chosen in the options bar (PDF Expert's text format row).
            let pt = r.text_pt;
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
                Edit::AddText {
                    page: d.page,
                    text: AddedText {
                        rect,
                        text: if r.text_bullets { reader::bulleted(&text) } else { text.clone() },
                        size: pt,
                        color: ink_at(r.ink_colour),
                        family,
                        bold: r.text_bold,
                        italic: r.text_italic,
                        align: TEXT_ALIGNS[r.text_align.clamp(0, 2) as usize],
                        underline: r.text_underline,
                    },
                },
            ));
        }
        let (x, y) = viewer::view_to_pdf(b, size, d.at.0, d.at.1);
        let shape = if d.kind == 0 {
            Shape::Note { at: [x, y], icon: NoteIcon::Comment }
        } else {
            // A box 200 pt wide, tall enough for the lines typed at the chosen size, kept on the page.
            let pt = r.text_pt;
            let lines = text.lines().count().max(1) as f64 + (text.len() as f64 / (38.0 * 12.0 / pt)).floor();
            // 200 pt wide, moved left or up so the whole box is on the page (PDF Expert does the same).
            let (w, h) = (200.0_f64.min(size.0), (lines * pt * 1.25 + 8.0).min(size.1));
            let at = (d.at.0.min(size.0 - w).max(0.0), d.at.1.min(size.1 - h).max(0.0));
            if r.callout {
                // The click is the point; the box stands 60 pt up and right of it (left or down
                // where the page ends), the line bending 18 pt before the box's near side.
                let bx = if d.at.0 + 60.0 + w <= size.0 { d.at.0 + 60.0 } else { (d.at.0 - 60.0 - w).max(0.0) };
                let by = if d.at.1 - 60.0 - h >= 0.0 { d.at.1 - 60.0 - h } else { (d.at.1 + 60.0).min(size.1 - h) };
                let rect = viewer::view_rect_to_pdf(b, size, (bx, by), (bx + w, by + h));
                let near = if bx > d.at.0 { bx } else { bx + w };
                let knee = viewer::view_to_pdf(b, size, if bx > d.at.0 { near - 18.0 } else { near + 18.0 }, by + h / 2.0);
                Shape::Callout { rect, knee: [knee.0, knee.1], point: [x, y], font_size: pt, ending: LineEnding::OpenArrow }
            } else {
                let r = viewer::view_rect_to_pdf(b, size, at, (at.0 + w, at.1 + h));
                Shape::TextBox { rect: r, font_size: pt }
            }
        };
        let colour = if d.kind == 0 { light_at(r.light) } else { ink_at(r.ink_colour) };
        let callout = matches!(shape, Shape::Callout { .. });
        let annotation = NewAnnotation { page: d.page, shape, style: style(colour, if d.kind == 0 || callout { 1.0 } else { 0.0 }), contents: text, author: author() };
        // A text box in the font of the format row; a callout in PdfCraft's own (Helvetica).
        Some((d.page, if d.kind == 0 || callout { Edit::AddAnnotation(annotation) } else { Edit::AddTextBox { annotation, font: super::tools::text_font(r).into(), align: r.text_align as u8 } }))
    })
    .flatten();
    if let Some((page, e)) = edit {
        // A form value can change other pages (widgets, scripts): all render again.
        let pages = if matches!(e, Edit::SetFieldValue { .. }) { vec![] } else { vec![page] };
        apply(ui, tab_id, e, &pages, None);
    }
}

/// Edit tool: the arrow keys nudge the picked objects (1 point, 10 with ⇧; `dy` up the page).
/// Held down, a key waits for the last nudge to be applied (the objects are found again first).
pub fn nudge(ui: &AppWindow, tab_id: i32, dx: f32, dy: f32) {
    let edit = update(tab_id, |r, _| {
        if r.edits_in_flight > 0 || r.repick.is_some() {
            return None;
        }
        let (page, list) = r.picked.as_ref()?;
        let (dx, dy) = (f64::from(dx), f64::from(dy));
        r.repick = Some((*page, list.iter().map(|o| [o.1[0] + dx, o.1[1] + dy, o.1[2] + dx, o.1[3] + dy]).collect()));
        Some((*page, Edit::MoveObjects { page: *page, objects: list.iter().map(|o| o.0).collect(), offset: [dx, dy] }))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// Edit tool, several objects picked ▸ Align or Distribute (`how` as [`reader::arrange`]; Acrobat's
/// Align Objects): lined up on the selection's outer edge or centre, or spaced evenly; one step.
pub fn objects_arrange(ui: &AppWindow, tab_id: i32, how: i32) {
    let edit = update(tab_id, |r, _| {
        if r.edits_in_flight > 0 || r.repick.is_some() {
            return None;
        }
        let (page, list) = r.picked.as_ref()?;
        let rects: Vec<[f64; 4]> = list.iter().map(|o| o.1).collect();
        let arranged = if how == 9 || how == 10 {
            reader::arrange(&rects, how)
        } else {
            // Against the box round them all: the leftmost edge, the topmost, their middle.
            let union = rects.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |u, r| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]);
            reader::arrange(&[&[union][..], &rects].concat(), how).split_off(1)
        };
        let moves = list.iter().zip(&arranged).map(|(o, a)| (o.0, [a[0] - o.1[0], a[1] - o.1[1]])).collect();
        r.repick = Some((*page, arranged));
        Some((*page, Edit::ArrangeObjects { page: *page, moves }))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

/// Comments selected together (⇧-click) ▸ Align or Distribute (`how` as [`reader::arrange`], after
/// the first one selected, as fields are): each moved, one step (Sol, 26/10: comments could only be
/// moved one at a time).
pub fn annots_arrange(ui: &AppWindow, tab_id: i32, how: i32) {
    let edit = update(tab_id, |r, v| {
        let first = r.selected_annot?;
        let all: Vec<reader_doc::AnnotEntry> = std::iter::once(first).chain(r.annots_also.iter().copied()).filter_map(|i| r.annots().get(i).cloned()).collect();
        if all.len() < 2 {
            return None;
        }
        // Arranged as the page shows (Sol, 27/10: on a page turned 90°, Left lined them up along
        // the file's x), y up; each moved box back in the file's space.
        let b = v.viewer.boxes.get(all[0].page)?;
        let size = b.view_size();
        let shown: Vec<[f64; 4]> = all
            .iter()
            .map(|a| {
                let [x, y, w, h] = viewer::pdf_rect_to_view(b, size, a.rect);
                [x, size.1 - (y + h), x + w, size.1 - y]
            })
            .collect();
        let moved = reader::arrange(&shown, how);
        let edits: Vec<Edit> = all
            .iter()
            .zip(moved)
            .map(|(a, d)| (a, viewer::view_rect_to_pdf(b, size, (d[0], size.1 - d[3]), (d[2], size.1 - d[1]))))
            .filter(|(a, m)| (m[0] - a.rect[0]).abs() > 1e-3 || (m[1] - a.rect[1]).abs() > 1e-3)
            .map(|(a, m)| Edit::MoveAnnotation { page: a.page, index: a.index, dx: m[0] - a.rect[0], dy: m[1] - a.rect[1] })
            .collect();
        (!edits.is_empty()).then(|| (all[0].page, Edit::Batch { label: "Arrange comments".into(), edits }))
    })
    .flatten();
    if let Some((page, e)) = edit {
        apply(ui, tab_id, e, &[page], None);
    }
}

pub fn note_cancel(tab_id: i32) {
    update(tab_id, |r, _| {
        r.draft = None;
        r.link_error.clear();
        r.link_rects.clear();
        r.replace_quads.clear();
        r.preview_timer = None;
    });
}

/// Edit text: the paragraph's new text (empty deletes it), rewrapped to its box by PdfCraft, in
/// what of the format row was changed while it was open; `None` when nothing changed.
fn paragraph_edit(r: &ReaderUi, d: &Draft, text: &str) -> Option<(usize, Edit)> {
    let (block, old, _) = d.block.as_ref()?;
    if text == old.trim() && r.para_format == 0 {
        return None;
    }
    use sheaf_core::pdfcraft::engine::FontFamily;
    let f = r.para_format;
    let family = [FontFamily::Helvetica, FontFamily::Times, FontFamily::Courier][r.text_family.clamp(0, 2) as usize];
    let style = sheaf_core::pdfcraft::edit::BlockStyle {
        color: (f & 1 != 0).then(|| ink_at(r.ink_colour)),
        size: (f & 2 != 0).then_some(r.text_pt),
        family: (f & 4 != 0).then_some((family, r.text_bold, r.text_italic)),
        align: (f & 8 != 0).then(|| TEXT_ALIGNS[r.text_align.clamp(0, 2) as usize]),
        ..Default::default()
    };
    Some((d.page, Edit::EditTextBlock { page: d.page, block: *block, text: text.to_string(), style }))
}

/// How long typing pauses before the paragraph is shown on its page as typed.
const PREVIEW_PAUSE: std::time::Duration = std::time::Duration::from_millis(300);

/// Edit text: the paragraph's editor changed. After a pause, the page shows the paragraph as
/// typed (PDF Expert edits on the page; the result, and anything it now covers, is seen before
/// ⌘↩). Other editors (notes, fields) do nothing here.
pub fn note_edited(ui: &AppWindow, tab_id: i32, text: &str) {
    let weak = ui.as_weak();
    update(tab_id, |r, _| {
        // Typed in place: kept, so a click elsewhere on the page commits it.
        if r.draft.as_ref().is_some_and(|d| matches!(d.kind, 1 | 3) && d.block.is_none()) {
            r.inplace_text = text.to_string();
            return;
        }
        // A form text field: the entries remembered that start as typed (Acrobat's Auto-Complete).
        if r.draft.as_ref().is_some_and(|d| d.kind == 4) {
            r.suggestions = sheaf_app::reader::completions(&crate::home::prefs().form_entries, text, 4);
            return;
        }
        let Some(d) = r.draft.as_ref().filter(|d| d.kind == 2) else { return };
        // The paragraph this text is for: the timer previews it only if it is still the one edited.
        let Some(key) = d.block.as_ref().map(|b| (d.page, b.0)) else { return };
        r.preview_text = text.trim().to_string();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::SingleShot, PREVIEW_PAUSE, move || {
            if weak.upgrade().is_some() {
                show_preview(tab_id, key);
            }
        });
        r.preview_timer = Some(timer);
    });
}

fn show_preview(tab_id: i32, (page, block): (usize, usize)) {
    update(tab_id, |r, _| {
        r.preview_timer = None;
        let Some(d) = r.draft.as_ref().filter(|d| d.kind == 2 && d.page == page && d.block.as_ref().is_some_and(|b| b.0 == block)) else { return };
        match paragraph_edit(r, d, &r.preview_text) {
            Some((_, edit)) => {
                r.preview = Some((page, block));
                r.send(Op::Preview { edit: Some(Box::new(edit)), pages: vec![page] });
            }
            // Back to the text as it was: the page too.
            None if r.preview.take().is_some() => r.send(Op::Preview { edit: None, pages: vec![page] }),
            None => {}
        }
    });
}

pub fn delete_selected(ui: &AppWindow, tab_id: i32) {
    // Prepare form: the selected field.
    if update(tab_id, |r, _| r.tool == FORM && r.form_sel.is_some()) == Some(true) {
        return super::prepare::field_delete(ui, tab_id);
    }
    // The page grid: Delete takes the selected pages out.
    if update(tab_id, |r, _| r.grid_prev.is_some() && !r.page_sel.is_empty()) == Some(true) {
        return super::pages::page_op(ui, tab_id, 2);
    }
    // Objects picked with the Edit tool (drawings, paragraphs, images): deleted in one step.
    if let Some((page, objects)) = update(tab_id, |r, _| r.picked.take().map(|(p, l)| (p, l.iter().map(|o| o.0).collect::<Vec<_>>()))).flatten() {
        return apply(ui, tab_id, Edit::DeleteObjects { page, objects }, &[page], None);
    }
    if update(tab_id, |r, _| r.selected_image.is_some()) == Some(true) {
        return image_action(ui, tab_id, 4);
    }
    // Comments selected together: deleted together, one step.
    let group = update(tab_id, |r, _| {
        let first = r.selected_annot?;
        if r.annots_also.is_empty() {
            return None;
        }
        let mut all: Vec<(usize, usize)> = std::iter::once(first).chain(std::mem::take(&mut r.annots_also)).filter_map(|i| r.annots().get(i).map(|a| (a.page, a.index))).collect();
        r.selected_annot = None;
        // Last first on the page: deleting one keeps the indices before it.
        all.sort_by(|a, b| b.cmp(a));
        let page = all[0].0;
        Some((page, Edit::Batch { label: "Delete comments".into(), edits: all.into_iter().map(|(page, index)| Edit::DeleteAnnotation { page, index }).collect() }))
    })
    .flatten();
    if let Some((page, e)) = group {
        return apply(ui, tab_id, e, &[page], None);
    }
    let edit = update(tab_id, |r, _| {
        let i = r.selected_annot.take()?;
        let a = r.annots().get(i)?;
        // A signature or initials placed on several pages at once (Every page): the copies go
        // together, one undo step (Sol, 26/10: only the one selected went).
        // Drawn signatures are ink, picture and typed ones stamps (Sol, 26/10: ink was left).
        let twins: Vec<(usize, usize)> = if a.fill_sign && matches!(a.subtype.as_str(), "Stamp" | "Ink") {
            let same = |b: &reader_doc::AnnotEntry| b.page != a.page && b.subtype == a.subtype && b.fill_sign && b.contents == a.contents && (0..4).all(|k| (b.rect[k] - a.rect[k]).abs() < 0.5);
            r.annots().iter().filter(|b| same(b)).map(|b| (b.page, b.index)).collect()
        } else {
            Vec::new()
        };
        if twins.is_empty() {
            return Some((vec![a.page], Edit::DeleteAnnotation { page: a.page, index: a.index }, 0));
        }
        let mut pages = vec![a.page];
        pages.extend(twins.iter().map(|t| t.0));
        let edits = std::iter::once((a.page, a.index)).chain(twins).map(|(page, index)| Edit::DeleteAnnotation { page, index }).collect();
        let n = pages.len();
        Some((pages, Edit::Batch { label: "Delete signatures".into(), edits }, n))
    })
    .flatten();
    if let Some((pages, e, n)) = edit {
        apply(ui, tab_id, e, &pages, None);
        if n > 1 {
            crate::flow::toast(ui, &format!("Deleted from the {n} pages it was placed on (⌘Z brings them back)"));
        }
    }
}

/// What the system clipboard holds while a comment copied in Sheaf is the latest copy.
pub const COMMENT_ON_CLIPBOARD: &str = "(Sheaf comment)";

/// ⌘C (and ⌘X, `cut`) on a selected comment, Acrobat's copy of a comment: it can be pasted on any
/// page of any document open (⌘V).
pub fn copy_comment(ui: &AppWindow, tab_id: i32, cut: bool) {
    let sent = update(tab_id, |r, _| {
        // The comment selected and those ⇧-clicked with it (Sol, 27/10: a cut copied the first only).
        let first = r.selected_annot?;
        let all: Vec<(String, usize, [f64; 4])> = std::iter::once(first).chain(r.annots_also.iter().copied()).filter_map(|i| r.annots().get(i).map(|a| (a.nm.clone(), a.page, a.rect))).collect();
        r.copy_from = Some(all.first()?.1);
        r.comments_asked = all.len();
        r.comments_copied.clear();
        for (nm, page, rect) in all {
            r.send(Op::CopyComment { nm, page, rect });
        }
        Some(())
    })
    .flatten();
    if sent.is_some() && cut {
        delete_selected(ui, tab_id);
    }
}

/// ⌘C (⌘X, `cut`) on a picture picked with the Edit tool: its pixels go to the clipboard (Acrobat's
/// Copy Image; Sol 25/10: picked objects could not be copied); cut, it is deleted once sent.
pub fn copy_image(ui: &AppWindow, tab_id: i32, cut: bool) {
    let sent = update(tab_id, |r, v| {
        if let Some((page, index)) = r.selected_image {
            r.send(Op::CopyImage { page, index });
            return Some(true);
        }
        // Drawings, paragraphs or several objects picked: copied as objects (pasted in Sheaf still
        // vector and text, Acrobat's) and as what they cover for other apps; cut, they are deleted
        // once sent.
        let (page, list) = r.picked.as_ref()?;
        let rect = list.iter().map(|o| o.1).reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])?;
        let b = v.viewer.boxes.get(*page)?;
        let size = b.view_size();
        let shown_of = |rect| {
            let [x, y, w, h] = viewer::pdf_rect_to_view(b, size, rect);
            [x, size.1 - (y + h), x + w, size.1 - y]
        };
        let shown = shown_of(rect);
        r.copy_from = Some(*page);
        r.copy_layout = list.iter().map(|o| shown_of(o.1)).collect();
        r.send(Op::CopyObjects { page: *page, objects: list.iter().map(|o| o.0).collect(), parts: list.iter().map(|o| o.1).collect(), rect, shown });
        Some(false)
    })
    .flatten();
    match sent {
        Some(true) if cut => image_action(ui, tab_id, 4),
        Some(false) if cut => delete_selected(ui, tab_id),
        _ => {}
    }
}

/// ⌘V with a comment copied last: it goes on the page shown, a little below and right of its
/// place when that is the page it came from. False: nothing to paste.
pub(super) fn paste_comment(ui: &AppWindow, tab_id: i32) -> bool {
    let fresh = arboard::Clipboard::new().and_then(|mut c| c.get_text()).is_ok_and(|t| t == COMMENT_ON_CLIPBOARD);
    let Some(clip) = with(|a| a.comment_clip.clone()).filter(|_| fresh) else { return false };
    let Some((to, same)) = update(tab_id, |r, v| {
        let to = v.current();
        (to, r.path == clip.from && to == clip.page)
    }) else {
        return false;
    };
    let (dx, dy) = if same { (12.0, -12.0) } else { (0.0, 0.0) };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mut edits: Vec<Edit> = clip
        .items
        .into_iter()
        .enumerate()
        .map(|(k, (element, picture))| match (picture, reader::xfdf_rect(&element)) {
            // A picture stamp is placed again from its picture, in the same box (XFDF would bring it back blank).
            (Some((name, bytes)), Some(r)) => {
                let rect = [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy];
                let stem = reader::xfdf_attr(&element, "subject").unwrap_or_else(|| "Stamp".into());
                let author = reader::xfdf_attr(&element, "title").unwrap_or_else(author);
                let file = sheaf_core::pdfcraft::engine::MarkFile { name, bytes, page: 0 };
                Edit::AddCustomStamp { page: to, rect, name: stem, file, author }
            }
            _ => {
                let xfdf = reader::paste_xfdf(&element, to, (dx, dy), &format!("sheaf-{now:x}-{k}"));
                Edit::ImportData { name: "Paste comment".into(), bytes: Arc::new(xfdf.into_bytes()) }
            }
        })
        .collect();
    let edit = match edits.len() {
        0 => return false,
        1 => edits.remove(0),
        _ => Edit::Batch { label: "Paste comments".into(), edits },
    };
    apply(ui, tab_id, edit, &[to], None);
    true
}

/// ⌘V with objects copied last (and their picture still on the clipboard): they go on the page
/// shown as one added item, still vector and text, at their place (a little below and right of it
/// on the page they came from), selected. False: nothing to paste.
pub(super) fn paste_objects(ui: &AppWindow, tab_id: i32) -> bool {
    let size = arboard::Clipboard::new().and_then(|mut c| c.get_image()).ok().map(|i| (i.width, i.height));
    let Some(clip) = with(|a| a.objects_clip.clone()).filter(|c| size == Some(c.pixels)) else { return false };
    let edit = update(tab_id, |r, v| {
        let to = v.current();
        let (dx, dy) = if r.path == clip.from && to == clip.page { (12.0, -12.0) } else { (0.0, 0.0) };
        let moved = |s: [f64; 4]| [s[0] + dx, s[1] + dy, s[2] + dx, s[3] + dy];
        r.select_added = Some((to, r.images.get(&to).map_or(0, |im| im.len())));
        r.tool = EDIT_TEXT;
        if clip.texts.is_empty() {
            return (to, Edit::AddArtwork { page: to, rect: moved(clip.shown), pdf: clip.pdf.clone() });
        }
        // Paragraphs come back as text, editable (Acrobat's); the rest as one item. One undo step.
        let rest = clip.rest.iter().map(|(pdf, s)| Edit::AddArtwork { page: to, rect: moved(*s), pdf: pdf.clone() });
        let texts = clip.texts.iter().map(|t| {
            let mut text = t.clone();
            text.rect = moved(t.rect);
            Edit::AddText { page: to, text }
        });
        (to, Edit::Batch { label: "Paste objects".into(), edits: rest.chain(texts).collect() })
    });
    let Some((to, e)) = edit else { return false };
    apply(ui, tab_id, e, &[to], None);
    true
}

/// More ▸ Overlay a PDF… (PDF Expert's and Acrobat's overlay): a PDF chosen is drawn over every
/// page, its page 1 on page 1 and so on (its last again past its end), each an item the Edit tool
/// moves, resizes or deletes; `behind` (Underlay a PDF…, Acrobat's) under the page's content
/// instead. One undo step.
pub fn overlay_pdf(ui: &AppWindow, tab_id: i32, behind: bool) {
    let d = crate::dialog::new(ui).add_filter("PDF", &["pdf"]);
    crate::dialog::pick_file(ui, d, move |ui, path| {
        let Ok(bytes) = std::fs::read(&path) else {
            return crate::flow::toast(ui, "That file could not be read");
        };
        let Some(pages) = update(tab_id, |_, v| (0..v.viewer.page_count()).collect::<Vec<_>>()) else { return };
        apply(ui, tab_id, Edit::OverlayPdf { name: file_name(&path), bytes: Arc::new(bytes), pages: pages.clone(), behind }, &pages, None);
    });
}

/// A reply deleted from its comment's thread (Acrobat's Delete on a reply); the comment stays.
pub fn reply_delete(ui: &AppWindow, tab_id: i32, page: i32, index: i32) {
    let (page, index) = (page.max(0) as usize, index.max(0) as usize);
    apply(ui, tab_id, Edit::DeleteAnnotation { page, index }, &[page], None);
}

/// The selected page image (Edit tool): 0 turned left, 1 right, 2 flipped, 3 replaced by a file
/// chosen, 4 deleted, 5 saved to a file (Acrobat's Save Image As). It stays selected, except deleted.
pub fn image_action(ui: &AppWindow, tab_id: i32, action: i32) {
    use sheaf_core::pdfcraft::engine::ImageEdit;
    let Some((page, index)) = update(tab_id, |r, _| r.selected_image).flatten() else { return };
    if action == 5 {
        let Some((dir, stem)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())) else { return };
        let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(format!("{stem} - image {}.png", page + 1)).add_filter("Image", &["png", "jpg", "jpeg"]);
        return crate::dialog::save_file(ui, d, move |_, to| {
            update(tab_id, |r, _| r.send(Op::SaveImage { page, index, to }));
        });
    }
    let change = match action {
        0 => ImageEdit::Rotate(-1),
        1 => ImageEdit::Rotate(1),
        2 => ImageEdit::Flip { horizontal: true },
        3 => {
            let d = crate::dialog::new(ui).set_title("Replace the image").add_filter("Image", super::pointer::IMAGE_TYPES);
            return crate::dialog::pick_file(ui, d, move |ui, path| {
                let pic = match sheaf_core::edit::Picture::read(&path) {
                    Ok(p) => p,
                    Err(e) => return crate::flow::toast(ui, &e.to_string()),
                };
                let change = ImageEdit::Replace { name: file_name(&path), bytes: pic.bytes.clone() };
                apply(ui, tab_id, Edit::EditPageImage { page, index, change }, &[page], None);
            });
        }
        _ => {
            update(tab_id, |r, _| r.selected_image = None);
            ImageEdit::Delete
        }
    };
    apply(ui, tab_id, Edit::EditPageImage { page, index, change }, &[page], None);
}

pub fn annot_delete(ui: &AppWindow, tab_id: i32, k: i32) {
    update(tab_id, |r, _| select_annot(r, k.max(0) as usize));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dated_stamps_write_the_time_in_the_format_chosen() {
        use chrono::TimeZone;
        let t = chrono::Local.with_ymd_and_hms(2026, 10, 25, 14, 5, 0).unwrap();
        assert_eq!(stamp_time(t, 0), "2:05 pm, Oct 25, 2026");
        assert_eq!(stamp_time(t, 1), "25/10/2026 14:05");
        assert_eq!(stamp_time(t, 2), "2026-10-25 14:05");
    }
}
