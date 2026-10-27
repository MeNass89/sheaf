//! Shapes ▸ Article (Acrobat's Article tool): a box of an article picked by a click, dragged to
//! move it, its bottom-right corner dragged to resize it, deleted with Delete.

use super::*;

/// How near (view points) the bottom-right corner a press resizes the box instead of moving it.
const CORNER: f64 = 10.0;

/// The article box under view point `p` of `page`, the topmost first: (article, box, its view
/// rectangle `[x, y, w, h]`, whether `p` is on its bottom-right corner).
pub(super) fn article_hit(r: &ReaderUi, v: &Viewer, page: usize, p: (f64, f64)) -> Option<(usize, usize, [f64; 4], bool)> {
    let size = v.boxes.get(page)?.view_size();
    let mut hits = Vec::new();
    for (t, a) in r.snap.articles.iter().enumerate() {
        for (k, (pg, rect)) in a.beads.iter().enumerate().filter(|(_, b)| b.0 == page) {
            let [x, y, w, h] = viewer::pdf_rect_to_view(&v.boxes[*pg], size, *rect);
            if p.0 >= x - 3.0 && p.0 <= x + w + 3.0 && p.1 >= y - 3.0 && p.1 <= y + h + 3.0 {
                let corner = (p.0 - (x + w)).abs() <= CORNER && (p.1 - (y + h)).abs() <= CORNER;
                hits.push((t, k, [x, y, w, h], corner));
            }
        }
    }
    // The smallest box under the pointer (one drawn inside another stays reachable).
    hits.into_iter().min_by(|a, b| (a.2[2] * a.2[3]).total_cmp(&(b.2[2] * b.2[3])))
}

/// An article box dragged: its thread, its index, the view rectangle it would take (two corners),
/// the user-space rectangle.
type ArticleDrag = (usize, usize, [(f64, f64); 2], [f64; 4]);

/// A box pressed at `a` and dragged to `b` (view points of `page`): (article, box, the view
/// rectangle it would take as two corners, the user-space rectangle). Moved, or resized from its
/// bottom-right corner; `None` when the press is on no box.
pub(super) fn article_drag(r: &ReaderUi, v: &Viewer, page: usize, a: (f64, f64), b: (f64, f64)) -> Option<ArticleDrag> {
    let (t, k, [x, y, w, h], corner) = article_hit(r, v, page, a)?;
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (p0, p1) = if corner { ((x, y), (x + w + dx, y + h + dy)) } else { ((x + dx, y + dy), (x + w + dx, y + h + dy)) };
    let (p0, p1) = ((p0.0.min(p1.0), p0.1.min(p1.1)), (p0.0.max(p1.0), p0.1.max(p1.1)));
    let bx = &v.boxes[page];
    Some((t, k, [p0, p1], viewer::view_rect_to_pdf(bx, bx.view_size(), p0, p1)))
}

/// A press and release with the Article mode on (view points of `page`, `end` where a drag
/// ended): a click on a box picks it (new boxes then continue its article), a drag from a box
/// moves or resizes it; elsewhere a drag draws a new box (`None` here: the shape path adds it).
pub(super) fn article_release(r: &mut ReaderUi, v: &Viewer, page: usize, a: (f64, f64), end: Option<(f64, f64)>) -> Option<Option<Pending>> {
    let dragged = end.filter(|e| (a.0 - e.0).hypot(a.1 - e.1) >= 4.0);
    match dragged {
        Some(e) => {
            let (t, k, _, rect) = article_drag(r, v, page, a, e)?;
            r.article_box = Some((t, k));
            r.article_thread = Some(t);
            Some(Some(Pending::Move(page, Box::new(Edit::SetArticleBox { thread: t, bead: k, rect }))))
        }
        None => {
            let hit = article_hit(r, v, page, a);
            r.article_box = hit.map(|h| (h.0, h.1));
            if let Some(h) = hit {
                r.article_thread = Some(h.0);
            }
            Some(None)
        }
    }
}

/// The article being drawn (`current`, an index of `Snap::articles`) after a box of article
/// `deleted` goes: unchanged when its article stays (`removed` false); when that box was its
/// article's last, the article goes and those after it move down one place.
pub(super) fn after_delete(current: Option<usize>, deleted: usize, removed: bool) -> Option<usize> {
    match current {
        Some(t) if removed && t == deleted => None,
        Some(t) if removed && t > deleted => Some(t - 1),
        other => other,
    }
}

/// After the document changed (an Undo, a Redo): the article being drawn and the box picked are
/// forgotten when they no longer exist (Sol, 27/10: Undo of an article's first box left the next
/// box aimed at it).
pub(super) fn reconcile(r: &mut ReaderUi) {
    let (thread, boxed) = settle(r.article_thread, r.article_box, &r.snap.articles.iter().map(|a| a.beads.len()).collect::<Vec<_>>());
    r.article_thread = thread;
    r.article_box = boxed;
}

/// [`reconcile`] on the articles' box counts.
fn settle(thread: Option<usize>, boxed: Option<(usize, usize)>, beads: &[usize]) -> (Option<usize>, Option<(usize, usize)>) {
    (thread.filter(|t| *t < beads.len()), boxed.filter(|(t, k)| beads.get(*t).is_some_and(|n| k < n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sol, 27/10: deleting an article's last box left the next box aimed at its old number.
    #[test]
    fn the_article_being_drawn_follows_a_deleted_article() {
        assert_eq!(after_delete(Some(1), 1, true), None, "its own article went: a new one starts");
        assert_eq!(after_delete(Some(2), 1, true), Some(1), "an article after it moves down");
        assert_eq!(after_delete(Some(0), 1, true), Some(0));
        assert_eq!(after_delete(Some(1), 1, false), Some(1), "a box gone, its article stays");
        assert_eq!(after_delete(None, 0, true), None);
    }

    /// Undo after an article's first box: the article is gone, so the next box starts a new one.
    #[test]
    fn an_undone_article_is_no_longer_drawn() {
        assert_eq!(settle(Some(0), Some((0, 0)), &[]), (None, None), "its only box undone");
        assert_eq!(settle(Some(1), Some((1, 2)), &[3, 2]), (Some(1), None), "its third box undone");
        assert_eq!(settle(Some(0), Some((0, 1)), &[2]), (Some(0), Some((0, 1))), "nothing to change");
    }
}
