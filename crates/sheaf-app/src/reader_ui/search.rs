//! Search: streamed results, the results panel, stepping through hits.

use super::*;

thread_local! {
    /// The session's searches, newest first (Acrobat's and PDF Expert's recent searches).
    static HISTORY: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Most searches kept in the history.
const HISTORY_LEN: usize = 10;

pub(super) fn recent_searches() -> Vec<String> {
    HISTORY.with(|h| h.borrow().clone())
}

/// `q` put first in `history`, once, at most [`HISTORY_LEN`] kept.
fn remember(history: &mut Vec<String>, q: &str) {
    history.retain(|h| h != q);
    history.insert(0, q.to_string());
    history.truncate(HISTORY_LEN);
}

pub fn search(ui: &AppWindow, tab_id: i32, query: &str) {
    let q = query.trim().to_string();
    if !q.is_empty() {
        HISTORY.with(|h| remember(&mut h.borrow_mut(), &q));
    }
    let Some((bytes, password, n, generation, case, whole, live, known, (turn, sizes))) = update(tab_id, |r, v| {
        r.search.generation += 1;
        r.search.query = q.clone();
        r.search.hits.clear();
        r.search.snippets.clear();
        r.search.current = None;
        r.search.busy = !q.is_empty();
        if !q.is_empty() {
            // The results show in the side panel, as in PDF Expert (evidence 09).
            r.panel_before_search = Some(r.panel);
            r.panel = 3;
            r.panel_open = true;
        }
        r.search_live.store(r.search.generation, std::sync::atomic::Ordering::Relaxed);
        // Pages read before (an earlier search, selection) are not read again.
        let known = r.texts.clone();
        // With Rotate view on, the pages read are turned as the known ones are (Sol, 26/10: a search
        // after a turn put its hits on the unturned text).
        let turn = (r.turn, r.snap.boxes.iter().map(|b| { let (w, h) = b.view_size(); (w as f32, h as f32) }).collect::<Vec<_>>());
        (r.snap.bytes.clone(), r.password(), v.viewer.page_count(), r.search.generation, r.search.case_sensitive, r.search.whole_words, r.search_live.clone(), known, turn)
    }) else {
        return;
    };
    if q.is_empty() {
        return;
    }
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let stale = || live.load(std::sync::atomic::Ordering::Relaxed) != generation;
        let config = render::RenderConfig { password: password.as_deref().map(Arc::from), ..Default::default() };
        let (tx, rx) = std::sync::mpsc::channel::<(usize, Option<Arc<PageText>>)>();
        std::thread::scope(|scope| {
            // The sender goes with the readers: when they are done, the loop below ends.
            let (b, c, k, st) = (&bytes, &config, &known, &stale);
            scope.spawn(move || reader::read_texts(b, c, n, k, st, |p, t| tx.send((p, t)).is_ok()));
            // Texts come back in any order; they are taken in page order, so results stay in order.
            let mut waiting = std::collections::BTreeMap::new();
            let mut page = 0;
            let mut texts = Vec::new();
            let mut hits = Vec::new();
            let mut snippets = Vec::new();
            // Results come back in batches so the first pages show while the rest are read; the first
            // result as soon as it is found.
            let mut sent = 0;
            for (p, t) in rx {
                waiting.insert(p, t);
                while let Some(t) = waiting.remove(&page) {
                    if stale() {
                        return;
                    }
                    let t = match t {
                        Some(t) if turn != 0 && !known.contains_key(&page) => sizes.get(page).map(|&s| Arc::new(reader::turned_text(&t, s, turn))).or(Some(t)),
                        t => t,
                    };
                    if let Some(t) = t {
                        for m in t.find_opts(&q, case, whole) {
                            snippets.push(snippet(&t, m.clone()));
                            hits.push((page, t.line_rects(m)));
                        }
                        if !known.contains_key(&page) {
                            texts.push((page, t));
                        }
                    }
                    let last = page + 1 == n;
                    if send_now(sent, hits.len(), page, last) {
                        let batch: Vec<_> = hits[sent..].to_vec();
                        let snips: Vec<_> = snippets[sent..].to_vec();
                        sent = hits.len();
                        let tx = std::mem::take(&mut texts);
                        let _ = weak.upgrade_in_event_loop(move |_| {
                            update(tab_id, |r, v| {
                                if r.search.generation != generation {
                                    return;
                                }
                                for (p, t) in tx {
                                    r.texts.entry(p).or_insert(t);
                                }
                                let first = r.search.hits.is_empty() && !batch.is_empty();
                                r.search.hits.extend(batch);
                                r.search.snippets.extend(snips);
                                r.version += 1;
                                r.search.busy = !last;
                                if first {
                                    r.search.current = Some(0);
                                    let (page, rects) = &r.search.hits[0];
                                    let rect = union(rects);
                                    show_view_rect(v, *page, rect);
                                }
                            });
                        });
                    }
                    page += 1;
                }
            }
        });
    });
}

/// Whether the results found so far go to the window now: the first one at once (it is shown and
/// selected: a long file waited for a batch of 16 pages, Sol 25/10), then every 16 pages, and at the end.
fn send_now(sent: usize, found: usize, page: usize, last: bool) -> bool {
    last || (found > sent && (sent == 0 || page % 16 == 15))
}

pub(super) fn union(rects: &[[f32; 4]]) -> [f32; 4] {
    rects.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, r| [a[0].min(r[0]), a[1].min(r[1]), a[2].max(r[2]), a[3].max(r[3])])
}

/// Up to 40 characters of context on each side of a match, on one line.
pub(super) fn snippet(t: &PageText, m: Range<usize>) -> (String, String, String) {
    let n = t.glyphs.len();
    let flat = |s: String| s.replace('\n', " ");
    let before = flat(t.text_of(m.start.saturating_sub(40)..m.start));
    let after = flat(t.text_of(m.end..(m.end + 40).min(n)));
    let before = if m.start > 40 { format!("…{}", before.trim_start()) } else { before };
    let after = if m.end + 40 < n { format!("{}…", after.trim_end()) } else { after };
    (before, flat(t.text_of(m)), after)
}

pub fn search_options(ui: &AppWindow, tab_id: i32, case: bool, whole: bool, query: &str) {
    update(tab_id, |r, _| {
        r.search.case_sensitive = case;
        r.search.whole_words = whole;
    });
    if !query.trim().is_empty() {
        search(ui, tab_id, query);
    }
}

pub fn result_clicked(tab_id: i32, k: i32) {
    update(tab_id, |r, v| {
        let k = k.max(0) as usize;
        let Some((page, rects)) = r.search.hits.get(k) else { return };
        let (page, rect) = (*page, union(rects));
        r.search.current = Some(k);
        show_view_rect(v, page, rect);
    });
}

/// Scrolls so the view rectangle `r` of `page` is a third down the viewport.
pub(super) fn show_view_rect(v: &mut ViewerUi, page: usize, r: [f32; 4]) {
    if page >= v.viewer.page_count() {
        return;
    }
    let c = view_rect_to_content(&v.viewer, page, r);
    v.scroll = (c[1] - v.height / 3.0).clamp(0.0, (v.viewer.content_height() - v.height).max(0.0));
    // Across too, on a page wider than the view, when the result is out of sight (Sol, 25/10).
    let width = v.viewer.view_width();
    v.scroll_x = (v.viewer.max_scroll_x() > 0.0 && (c[0] < v.left || c[0] + c[2] > v.left + width)).then(|| (c[0] - width / 3.0).clamp(0.0, v.viewer.max_scroll_x()));
    v.scroll_seq += 1;
}

pub fn search_step(tab_id: i32, dir: i32) {
    update(tab_id, |r, v| {
        let n = r.search.hits.len();
        if n == 0 {
            return;
        }
        let next = match r.search.current {
            None => 0,
            Some(c) => ((c as i64 + dir.signum() as i64).rem_euclid(n as i64)) as usize,
        };
        r.search.current = Some(next);
        let (page, rects) = &r.search.hits[next];
        let rect = union(rects);
        show_view_rect(v, *page, rect);
    });
}

pub fn search_clear(tab_id: i32) {
    update(tab_id, |r, _| {
        r.search.generation += 1;
        r.search_live.store(r.search.generation, std::sync::atomic::Ordering::Relaxed);
        r.search.query.clear();
        r.search.hits.clear();
        r.search.snippets.clear();
        r.search.current = None;
        r.search.busy = false;
        r.version += 1;
        if r.panel == 3 {
            r.panel = r.panel_before_search.take().unwrap_or(0);
        }
    });
}

/// The search panel's Replace all (Acrobat's Edit PDF ▸ Find and Replace): every place of the search's
/// words in the pages' text is rewritten, with its case and whole-word options, one undo step.
pub fn replace_all(ui: &AppWindow, tab_id: i32, find: &str, with: &str) {
    let find = find.trim().to_string();
    if find.is_empty() {
        return crate::flow::toast(ui, &sheaf_app::i18n::tr("Type the words to find first"));
    }
    let refused = update(tab_id, |r, _| {
        if !r.snap.read_only.is_empty() {
            return Some(r.snap.read_only.clone());
        }
        r.send(Op::ReplaceAll { find: find.clone(), with: with.to_string(), case: r.search.case_sensitive, whole: r.search.whole_words });
        r.undo_pages.push(Vec::new());
        None
    })
    .flatten();
    match refused {
        Some(why) => crate::flow::toast(ui, &why),
        // The results named the old words: they go.
        None => search_clear(tab_id),
    }
}

/// The search panel's Replace (Acrobat's Find ▸ Replace): the match in view only, rewritten with
/// `with`; the search runs again for the next one.
pub fn replace_one(ui: &AppWindow, tab_id: i32, find: &str, with: &str) {
    let find = find.trim().to_string();
    let refused = update(tab_id, |r, _| {
        if !r.snap.read_only.is_empty() {
            return Some(r.snap.read_only.clone());
        }
        let Some(cur) = r.search.current.filter(|_| !find.is_empty()) else {
            return Some("Find the words first, then Replace takes the one in view".to_string());
        };
        let page = r.search.hits.get(cur)?.0;
        // Its place among the page's matches, in reading order.
        let nth = r.search.hits[..cur].iter().filter(|h| h.0 == page).count();
        r.send(Op::ReplaceOne { page, nth, find: find.clone(), with: with.to_string(), case: r.search.case_sensitive, whole: r.search.whole_words });
        r.undo_pages.push(vec![page]);
        None
    })
    .flatten();
    match refused {
        Some(why) => crate::flow::toast(ui, &why),
        None => search_clear(tab_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_goes_first_in_the_history_once_and_the_oldest_drop_off() {
        let mut h = Vec::new();
        for q in ["a", "b", "a"] {
            remember(&mut h, q);
        }
        assert_eq!(h, ["a", "b"]);
        for k in 0..12 {
            remember(&mut h, &k.to_string());
        }
        assert_eq!(h.len(), HISTORY_LEN);
        assert_eq!(h[0], "11");
    }

    #[test]
    fn the_first_result_is_shown_at_once_then_every_sixteen_pages() {
        // The first hit, on page 3: sent with page 3, not with page 15.
        assert!(super::send_now(0, 1, 3, false));
        assert!(!super::send_now(0, 0, 3, false));
        // Then batches: nothing new on page 4, the next batch on page 15, the rest at the end.
        assert!(!super::send_now(1, 2, 4, false) && super::send_now(1, 2, 15, false) && super::send_now(2, 2, 40, true));
    }
}
