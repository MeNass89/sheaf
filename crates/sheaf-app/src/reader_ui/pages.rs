//! The Pages panel: selecting, dragging and dropping thumbnails, page operations, extract.

use super::*;

/// A thumbnail clicked: it is shown; ⇧ extends the selection from the last one clicked, ⌘ adds or
/// removes it, a plain click selects it alone (PDF Expert's thumbnails).
pub fn thumb_pressed(tab_id: i32, page: i32, extend: bool, toggle: bool) {
    update(tab_id, |r, v| {
        let p = page.max(0) as usize;
        match (extend, toggle, r.page_anchor) {
            (true, _, Some(a)) => {
                let (lo, hi) = (a.min(p), a.max(p));
                r.page_sel = (lo..=hi).collect();
            }
            (_, true, _) => {
                if !r.page_sel.remove(&p) {
                    r.page_sel.insert(p);
                }
                r.page_anchor = Some(p);
            }
            // A plain press on a page already selected keeps the selection, so it can be dragged
            // whole; it shrinks to this page if the press ends without a drag.
            _ if r.page_sel.contains(&p) => r.pending_reduce = Some(p),
            _ => {
                r.page_sel = [p].into_iter().collect();
                r.page_anchor = Some(p);
            }
        }
        v.show_page(p, None);
    });
}

/// The pages an operation acts on: the selection, else the page shown.
pub(super) fn op_pages(r: &ReaderUi, v: &ViewerUi) -> Vec<usize> {
    if r.page_sel.is_empty() { vec![v.current()] } else { r.page_sel.iter().copied().collect() }
}

pub fn page_op(ui: &AppWindow, tab_id: i32, op: i32) {
    let made = update(tab_id, |r, v| {
        let pages = op_pages(r, v);
        let n = v.viewer.page_count();
        let edit = match op {
            0 | 1 => Edit::RotatePages { pages: pages.clone(), degrees: if op == 0 { -90 } else { 90 } },
            2 => {
                if pages.len() >= n {
                    return Err("A PDF needs at least one page".to_string());
                }
                Edit::DeletePages { pages: pages.clone() }
            }
            3 => Edit::DuplicatePages { pages: pages.clone() },
            5 => {
                let after = *pages.last().unwrap_or(&0);
                let (w, h) = v.viewer.boxes[after].view_size();
                Edit::InsertBlankPage { at: after + 1, width: w, height: h }
            }
            _ => return Err(String::new()),
        };
        // Page numbers shift: the selection follows what is left.
        r.page_sel.clear();
        Ok(edit)
    });
    match made {
        Some(Ok(e)) => apply_structure(ui, tab_id, e),
        Some(Err(m)) if !m.is_empty() => crate::flow::toast(ui, &m),
        _ => {}
    }
}

/// An edit that adds, removes, moves or turns pages: every page renders again.
pub(super) fn apply_structure(ui: &AppWindow, tab_id: i32, edit: Edit) {
    let err = update(tab_id, |r, _| {
        if !r.snap.read_only.is_empty() {
            return Some(r.snap.read_only.clone());
        }
        if r.structure_pending {
            return Some("One moment: the pages are being rearranged".into());
        }
        r.send(Op::Apply { edit: Box::new(edit), pages: Vec::new() });
        r.undo_pages.push(Vec::new());
        r.redo_pages.clear();
        r.structure_pending = true;
        None
    })
    .flatten();
    if let Some(e) = err {
        crate::flow::toast(ui, &e);
    }
}

/// The selected pages (or the one shown) written to a new PDF chosen in a Save dialog, which then
/// opens in its own tab; or, `each`, one file per page ("name - page 3.pdf") in a folder chosen
/// (Acrobat's Extract as separate files). `delete` removes them from the document once written,
/// an edit that can be undone.
pub fn extract(ui: &AppWindow, tab_id: i32, each: bool, delete: bool) {
    let Some((pages, n, dir, stem)) = update(tab_id, |r, v| {
        (op_pages(r, v), v.viewer.page_count(), crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    }) else {
        return;
    };
    if delete && pages.len() >= n {
        return crate::flow::toast(ui, "A PDF needs at least one page: extract without deleting them");
    }
    let to_delete = if delete { pages.clone() } else { Vec::new() };
    let start = move |parts: Vec<(Vec<usize>, PathBuf)>| {
        update(tab_id, |r, _| {
            r.delete_after_extract = to_delete;
            r.send(Op::Extract { parts });
        });
    };
    let d = crate::dialog::new(ui).set_directory(&dir);
    if each {
        let pages = pages.clone();
        return crate::dialog::pick_folder(ui, d, move |_, folder| {
            start(pages.iter().map(|p| (vec![*p], folder.join(format!("{stem} - page {}.pdf", p + 1)))).collect());
        });
    }
    let name = if pages.len() == 1 { format!("{stem} - page {}.pdf", pages[0] + 1) } else { format!("{stem} - {} pages.pdf", pages.len()) };
    let d = d.set_file_name(name).add_filter("PDF", &["pdf"]);
    let parts = pages.clone();
    crate::dialog::save_file(ui, d, move |_, target| start(vec![(parts, target)]));
}

/// Number pages… (Acrobat's Page Labels): from the first selected page (or the one shown) to the
/// end, pages are labelled in `style` from `start` after `prefix` (a front matter in i, ii, then
/// 1 from the first chapter: two steps). An edit that can be undone; the labels show in the page
/// field and Go to.
pub fn number_pages(ui: &AppWindow, tab_id: i32, style: i32, prefix: &str, start: &str) {
    use sheaf_core::pdfcraft::engine::LabelStyle as S;
    let first = match start.trim() {
        "" => 1,
        t => match t.parse::<u32>() {
            Ok(n) if n >= 1 => n,
            _ => return crate::flow::toast(ui, &format!("Start: \"{t}\" is not a page number (1 or more)")),
        },
    };
    let style = [S::Decimal, S::LowerRoman, S::UpperRoman, S::LowerAlpha, S::UpperAlpha, S::None][style.clamp(0, 5) as usize];
    let Some((from, to)) = update(tab_id, |r, v| {
        let pages = op_pages(r, v);
        (*pages.first().unwrap_or(&0), v.viewer.page_count().saturating_sub(1))
    }) else {
        return;
    };
    apply(ui, tab_id, Edit::NumberPages { from, to, style, prefix: prefix.to_string(), first }, &[], None);
}

/// ⌘C in the Pages panel: the selected pages are kept to paste into another document (PDF
/// Expert's and Acrobat's page copy), as the document is now, unsaved edits included.
pub fn copy_pages(ui: &AppWindow, tab_id: i32) {
    let Some(clip) = update(tab_id, |r, v| (r.name.clone(), r.snap.bytes.clone(), op_pages(r, v))) else { return };
    let n = clip.2.len();
    with(|a| a.page_clip = Some(clip));
    crate::flow::toast(ui, &format!("{n} page{} copied: ⌘V in a document's Pages panel puts {} after its selected page", if n == 1 { "" } else { "s" }, if n == 1 { "it" } else { "them" }));
}

/// ⌘V with the Pages panel shown and pages copied: they go after the selected page (or the one
/// shown), an edit that can be undone. False: nothing to paste here.
pub(super) fn paste_pages(ui: &AppWindow, tab_id: i32) -> bool {
    let Some((name, bytes, pages)) = with(|a| a.page_clip.clone()) else { return false };
    let Some(at) = update(tab_id, |r, v| (r.panel_open && r.panel == 0).then(|| op_pages(r, v).last().map_or(0, |p| p + 1))).flatten() else { return false };
    apply_structure(ui, tab_id, Edit::InsertPagesFrom { name, bytes, pages: Some(pages), at, field_conflicts: Default::default() });
    true
}

/// A thumbnail dragged to content y `y` of the panel: the drop line's y (−1: none).
pub fn thumb_drag(tab_id: i32, page: i32, y: f32) -> f32 {
    update(tab_id, |r, v| {
        if r.page_drag.is_none() {
            let p = page.max(0) as usize;
            if !r.page_sel.contains(&p) {
                r.page_sel = [p].into_iter().collect();
            }
            r.page_drag = Some(r.page_sel.iter().copied().collect());
        }
        let (_, line) = drop_gap(&v.viewer, y as f64);
        line as f32
    })
    .unwrap_or(-1.0)
}

/// The page whose thumbnail is at content y of the panel (−1: a gap).
pub fn thumb_at(tab_id: i32, y: f32) -> i32 {
    with(|a| {
        let v = a.tab(tab_id)?.viewer.as_ref()?;
        let y = y as f64;
        v.viewer.thumb_layout().iter().position(|(t, h)| y >= *t && y <= t + h + 18.0).map(|p| p as i32)
    })
    .unwrap_or(-1)
}

/// The gap a drop at content y lands in (0 = before the first page) and the y of its line.
pub(super) fn drop_gap(v: &Viewer, y: f64) -> (usize, f64) {
    let tl = v.thumb_layout();
    let gap = tl.iter().position(|(top, h)| y < top + h / 2.0).unwrap_or(tl.len());
    // Just above the next thumbnail, below the previous one's page number (drawn under it).
    let line = match tl.get(gap) {
        Some((top, _)) => top - 2.0,
        None => tl.last().map(|(t, h)| t + h + 20.0).unwrap_or(0.0),
    };
    (gap, line)
}

/// A press on a thumbnail ended without a drag.
pub fn thumb_released(tab_id: i32) {
    update(tab_id, |r, _| {
        if let Some(p) = r.pending_reduce.take() {
            r.page_sel = [p].into_iter().collect();
            r.page_anchor = Some(p);
        }
    });
}

pub fn thumb_drop(ui: &AppWindow, tab_id: i32, y: f32) {
    let edit = update(tab_id, |r, v| {
        r.pending_reduce = None;
        let pages = r.page_drag.take()?;
        let (gap, _) = drop_gap(&v.viewer, y as f64);
        // `to` is the position in the result: the gap less the moved pages before it.
        let to = gap - pages.iter().filter(|p| **p < gap).count();
        if pages.iter().enumerate().all(|(k, p)| *p == to + k) {
            return None;
        }
        r.page_sel = (to..to + pages.len()).collect();
        Some(Edit::MovePages { pages, to })
    })
    .flatten();
    if let Some(e) = edit {
        apply_structure(ui, tab_id, e);
    }
}
