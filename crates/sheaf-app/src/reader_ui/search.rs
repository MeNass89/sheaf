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
    let Some((bytes, password, n, generation, case, whole, live, known)) = update(tab_id, |r, v| {
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
        (r.snap.bytes.clone(), r.password(), v.viewer.page_count(), r.search.generation, r.search.case_sensitive, r.search.whole_words, r.search_live.clone(), known)
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
            // Results come back in batches so the first pages show while the rest are read.
            let mut sent = 0;
            for (p, t) in rx {
                waiting.insert(p, t);
                while let Some(t) = waiting.remove(&page) {
                    if stale() {
                        return;
                    }
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
                    if last || (hits.len() > sent && (page % 16 == 15)) {
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
}
