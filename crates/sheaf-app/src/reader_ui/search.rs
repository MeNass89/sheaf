//! Search: streamed results, the results panel, stepping through hits.

use super::*;

pub fn search(ui: &AppWindow, tab_id: i32, query: &str) {
    let q = query.trim().to_string();
    let Some((bytes, password, n, generation, case, whole, live)) = update(tab_id, |r, v| {
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
        (r.snap.bytes.clone(), r.password(), v.viewer.page_count(), r.search.generation, r.search.case_sensitive, r.search.whole_words, r.search_live.clone())
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
        let mut rr = render::PageRenderer::new(bytes, config);
        let mut texts = Vec::new();
        let mut hits = Vec::new();
        let mut snippets = Vec::new();
        // Results come back in batches so the first pages show while the rest are read.
        let mut sent = 0;
        for page in 0..n {
            // A newer search, a cleared field, a page edit or a closed tab: stop reading.
            if stale() {
                return;
            }
            let res = rr.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
            if let Some(t) = res.text {
                for m in t.find_opts(&q, case, whole) {
                    snippets.push(snippet(&t, m.clone()));
                    hits.push((page, t.line_rects(m)));
                }
                texts.push((page, t));
            }
            let last = page + 1 == n;
            if last || (hits.len() > sent && (page % 16 == 15)) {
                let batch: Vec<_> = hits[sent..].to_vec();
                let snips: Vec<_> = snippets[sent..].to_vec();
                sent = hits.len();
                let tx = std::mem::take(&mut texts);
                let w = weak.clone();
                let _ = w.upgrade_in_event_loop(move |_| {
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
        }
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
