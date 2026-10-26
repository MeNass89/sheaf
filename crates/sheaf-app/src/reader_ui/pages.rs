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
            // The pages selected (two or more: from the first to the last), else all, back to front.
            8 => {
                let (a, b) = if pages.len() >= 2 { (pages[0], *pages.last().unwrap_or(&0)) } else { (0, n.saturating_sub(1)) };
                if b <= a {
                    return Err("One page has no order to reverse".to_string());
                }
                // The last of the block goes to the block's front, then to just after it, and so on.
                let edits = (0..b - a).map(|i| Edit::MovePages { pages: vec![b], to: a + i }).collect();
                Edit::Batch { label: "Reverse pages".into(), edits }
            }
            6 | 7 | 9 | 10 | 11 => return Ok(None),
            _ => return Err(String::new()),
        };
        // Page numbers shift: the selection follows what is left.
        r.page_sel.clear();
        Ok(Some(edit))
    });
    match made {
        Some(Ok(Some(e))) => apply_structure(ui, tab_id, e),
        Some(Ok(None)) if op >= 10 => from_clipboard_or_scanner(ui, tab_id, op == 11),
        Some(Ok(None)) => from_file(ui, tab_id, op),
        Some(Err(m)) if !m.is_empty() => crate::flow::toast(ui, &m),
        _ => {}
    }
}

/// A thumbnail's Insert from clipboard (PDF files copied, a picture, text) or Insert from scanner
/// (Acrobat's Insert ▸ From Clipboard, From Scanner): the pages made go after the selected page
/// (or the one shown), one undo step. The scan waits off the UI thread.
fn from_clipboard_or_scanner(ui: &AppWindow, tab_id: i32, scanner: bool) {
    let made = std::env::temp_dir().join(format!("sheaf-insert-{}-{tab_id}.pdf", std::process::id()));
    let insert = move |ui: &AppWindow, name: &str, made: &std::path::Path| {
        let bytes = std::fs::read(made);
        let _ = std::fs::remove_file(made);
        let Ok(bytes) = bytes else { return crate::flow::toast(ui, "Those pages could not be made") };
        let Some(at) = update(tab_id, |r, v| {
            let at = op_pages(r, v).last().map_or(0, |p| p + 1);
            r.page_sel.clear();
            at
        }) else {
            return;
        };
        apply_structure(ui, tab_id, Edit::InsertPagesFrom { name: name.into(), bytes: Arc::new(bytes), pages: None, at, field_conflicts: Default::default() });
    };
    if !scanner {
        let Some(clip) = crate::run::clipboard_pages() else { return crate::flow::toast(ui, "The clipboard holds no PDF, picture or text") };
        return match crate::run::clipboard_to_pdf(&clip, &made) {
            Ok(p) => insert(ui, "Clipboard", &p),
            Err(e) => crate::flow::toast(ui, &e.to_string()),
        };
    }
    crate::flow::toast(ui, "Scanning…");
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let png = made.with_extension("png");
        let r = crate::run::scan_page(&png).and_then(|()| {
            let p = sheaf_core::images::images_to_pdf(std::slice::from_ref(&png), sheaf_core::images::PageLayout::FitImage, &made).map_err(|e| e.to_string());
            let _ = std::fs::remove_file(&png);
            p
        });
        let _ = weak.upgrade_in_event_loop(move |ui| match r {
            Ok(p) => insert(&ui, "Scan", &p),
            Err(e) => crate::flow::toast(&ui, &e),
        });
    });
}

/// More ▸ More tools ▸ Turn pages upright (Acrobat's Auto-Rotate): asks the document which pages'
/// text runs sideways or upside down.
pub fn turn_upright(tab_id: i32) {
    update(tab_id, |r, _| r.send(Op::UprightTurns));
}

/// Those pages turned, one undo step; or said that every page reads upright.
pub(super) fn upright(ui: &AppWindow, tab_id: i32, turns: Vec<(usize, i64)>) {
    if turns.is_empty() {
        return crate::flow::toast(ui, "Every page with text reads upright");
    }
    let n = turns.len();
    let edits: Vec<Edit> = [90, 180, 270]
        .into_iter()
        .filter_map(|d| {
            let pages: Vec<usize> = turns.iter().filter(|t| t.1 == d).map(|t| t.0).collect();
            (!pages.is_empty()).then_some(Edit::RotatePages { pages, degrees: d })
        })
        .collect();
    apply_structure(ui, tab_id, Edit::Batch { label: "Turn pages upright".into(), edits });
    crate::flow::toast(ui, &format!("{n} page{} turned upright", if n == 1 { "" } else { "s" }));
}

/// Insert pages from file (Acrobat's): a PDF picked goes after the selected page (or the one
/// shown), all its pages; `op` 9 before it (Acrobat's Before, so a file can go before page 1; Sol,
/// 26/10). `op` 7: its pages take the selected pages' places instead, from its first page, one for
/// one (Acrobat's Replace Pages). One undo step either way.
fn from_file(ui: &AppWindow, tab_id: i32, op: i32) {
    let replace = op == 7;
    let d = crate::dialog::new(ui).set_title(if replace { "Replace pages with a file" } else { "Insert pages from a file" }).add_filter("PDF", &["pdf"]);
    crate::dialog::pick_file(ui, d, move |ui, path| {
        let bytes = match std::fs::read(&path) {
            Ok(b) => Arc::new(b),
            Err(e) => return crate::flow::toast(ui, &format!("{} could not be read: {e}", file_name(&path))),
        };
        let name = file_name(&path);
        let Some(edit) = update(tab_id, |r, v| {
            let pages = op_pages(r, v);
            r.page_sel.clear();
            if replace {
                let count = sheaf_core::pages::count_in(&bytes);
                if count < pages.len() {
                    return Err(format!("{name} has {count} page{}; {} are selected", if count == 1 { "" } else { "s" }, pages.len()));
                }
                // More pages than needed: which ones, asked under the pages (Acrobat's Replace Pages ▸
                // pages … to … of the file; Sol, 26/10: it always took the first).
                if count > pages.len() {
                    r.replace_pending = Some((name.clone(), bytes.clone(), count, pages));
                    return Err(String::new());
                }
                Ok(Edit::ReplacePages { src_pages: (0..pages.len()).collect(), pages, name: name.clone(), bytes: bytes.clone() })
            } else {
                let at = if op == 9 { pages.first().copied().unwrap_or(0) } else { pages.last().map_or(0, |p| p + 1) };
                Ok(Edit::InsertPagesFrom { name: name.clone(), bytes: bytes.clone(), pages: None, at, field_conflicts: Default::default() })
            }
        }) else {
            return;
        };
        match edit {
            Ok(e) => apply_structure(ui, tab_id, e),
            Err(m) if !m.is_empty() => crate::flow::toast(ui, &m),
            Err(_) => {}
        }
    });
}

/// Replace Pages, the file's pages chosen: from page `from` (1-based, typed) as many as are
/// selected; "" cancels.
pub fn replace_from(ui: &AppWindow, tab_id: i32, from: &str) {
    let edit = update(tab_id, |r, _| {
        let (name, bytes, count, pages) = r.replace_pending.take()?;
        let from = from.trim();
        if from.is_empty() {
            return None;
        }
        let n = pages.len();
        Some(match from.parse::<usize>() {
            Ok(f) if f >= 1 && f + n - 1 <= count => Ok(Edit::ReplacePages { src_pages: (f - 1..f - 1 + n).collect(), pages, name, bytes }),
            _ => Err(format!("From page: 1 to {} ({name} has {count} pages, {n} are needed)", count + 1 - n)),
        })
    })
    .flatten();
    match edit {
        Some(Ok(e)) => apply_structure(ui, tab_id, e),
        Some(Err(m)) => crate::flow::toast(ui, &m),
        None => {}
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
            // The folder's own files are never written over: a name taken gets " (1)" (no dialog asked
            // about these names, unlike a Save dialog's; Sol, 25/10).
            start(pages.iter().map(|p| (vec![*p], sheaf_core::paths::unique_path(&folder.join(format!("{stem} - page {}.pdf", p + 1))))).collect());
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
pub fn number_pages(ui: &AppWindow, tab_id: i32, style: i32, prefix: &str, start: &str, through: &str) {
    use sheaf_core::pdfcraft::engine::LabelStyle as S;
    let first = match start.trim() {
        "" => 1,
        t => match t.parse::<u32>() {
            Ok(n) if n >= 1 => n,
            _ => return crate::flow::toast(ui, &format!("Start: \"{t}\" is not a page number (1 or more)")),
        },
    };
    let style = [S::Decimal, S::LowerRoman, S::UpperRoman, S::LowerAlpha, S::UpperAlpha, S::None][style.clamp(0, 5) as usize];
    // Through page N (1-based), when typed: Acrobat's Pages from … to … (Sol, 25/10).
    let through = match through.trim() {
        "" => None,
        t => match t.parse::<usize>() {
            Ok(n) if n >= 1 => Some(n - 1),
            _ => return crate::flow::toast(ui, &format!("Through: \"{t}\" is not a page number (1 or more)")),
        },
    };
    let Some((from, to)) = update(tab_id, |r, v| {
        // Several pages selected: those; one: it and the rest; through page N when typed.
        let pages = op_pages(r, v);
        let last = if pages.len() > 1 { *pages.last().unwrap_or(&0) } else { v.viewer.page_count().saturating_sub(1) };
        let from = *pages.first().unwrap_or(&0);
        (from, through.map_or(last, |t| t.clamp(from, v.viewer.page_count().saturating_sub(1))))
    }) else {
        return;
    };
    apply(ui, tab_id, Edit::NumberPages { from, to, style, prefix: prefix.to_string(), first }, &[], None);
}

/// ⌘C in the Pages panel: the selected pages are kept to paste into another document (PDF
/// Expert's and Acrobat's page copy), as the document is now, unsaved edits included.
pub fn copy_pages(ui: &AppWindow, tab_id: i32) {
    // A document opened with its password: its pages are written out as a new, unprotected PDF on
    // the document's thread, and copied once they are (Sol, 25/10: pasting needed the password).
    let protected = update(tab_id, |r, v| {
        r.snap.password.is_some().then(|| {
            let pages = op_pages(r, v);
            r.send(Op::CopyPages { pages });
        })
    });
    if protected.flatten().is_some() {
        return;
    }
    let Some(clip) = update(tab_id, |r, v| (r.name.clone(), r.snap.bytes.clone(), op_pages(r, v))) else { return };
    pages_copied(ui, clip);
}

/// The pages copied, ready for ⌘V elsewhere.
pub(super) fn pages_copied(ui: &AppWindow, clip: (String, Arc<Vec<u8>>, Vec<usize>)) {
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
