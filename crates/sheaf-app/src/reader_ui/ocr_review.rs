//! Acrobat's Correct Recognized Text: step through hidden runs with the scan in view.
use super::*;
use sheaf_core::ocr_review::ReviewLine;

#[derive(Default)]
pub(super) struct Review {
    rows: Vec<ReviewLine>,
    at: usize,
    pub(super) busy: bool,
    pub(super) seq: i32,
    pub(super) suspects_only: bool,
    panel_was_open: bool,
    advance_pending: Option<bool>,
}

impl Review {
    fn count(&self) -> usize { self.rows.iter().filter(|row| !self.suspects_only || row.suspect).count() }
    pub(super) fn selected(&self) -> Option<&ReviewLine> { self.rows.iter().filter(|row| !self.suspects_only || row.suspect).nth(self.at) }
    pub(super) fn position(&self) -> String {
        if self.busy { return "Reading the OCR layer…".into(); }
        match self.selected() {
            Some(row) => format!("Run {} of {} · Page {}", self.at + 1, self.count(), row.page),
            None if self.rows.is_empty() => "No hidden OCR text. Run OCR first.".into(),
            None if self.suspects_only && self.count() == 0 => "No possible decoding errors. Turn off the filter to review every run.".into(),
            None => format!("Review complete · {} runs", self.count()),
        }
    }
}

pub fn review_ocr(ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| {
        let panel_was_open = r.ocr_review.as_ref().map_or(r.panel_open, |review| review.panel_was_open);
        r.panel_open = false;
        r.ocr_review = Some(Review { panel_was_open, ..Default::default() });
        refresh(r);
    });
    crate::viewer_ui::request(ui, tab_id);
}

pub(super) fn refresh(r: &mut ReaderUi) {
    if let Some(review) = r.ocr_review.as_mut() {
        review.busy = true;
        r.send(Op::ReviewOcr);
    }
}

fn show(r: &ReaderUi, v: &mut ViewerUi) {
    if let Some(row) = r.ocr_review.as_ref().filter(|f| !f.busy).and_then(|f| f.selected())
        && let Some(page) = row.page.checked_sub(1).filter(|&page| page < v.viewer.page_count())
    {
        v.show_page(page, None);
    }
}

pub(super) fn reviewed(ui: &AppWindow, tab_id: i32, rows: Vec<ReviewLine>) {
    update(tab_id, |r, v| {
        if let Some(review) = r.ocr_review.as_mut() {
            review.rows = rows;
            review.at = review.at.min(review.count());
            review.busy = false;
            review.seq = review.seq.wrapping_add(1);
        }
        show(r, v);
    });
    crate::viewer_ui::request(ui, tab_id);
}

pub fn ocr_review_action(ui: &AppWindow, tab_id: i32, action: i32, text: &str) {
    let error = update(tab_id, |r, v| {
        if action < 0 { if let Some(review) = r.ocr_review.take() { r.panel_open = review.panel_was_open; } return None; }
        let review = r.ocr_review.as_mut()?;
        if review.busy || r.edits_in_flight > 0 { return None; }
        if action == 4 {
            review.suspects_only = !review.suspects_only;
            review.at = 0;
            review.seq = review.seq.wrapping_add(1);
            show(r, v);
            return None;
        }
        if matches!(action, 0 | 1) {
            review.at = if action == 0 { review.at.saturating_sub(1) } else { (review.at + 1).min(review.count()) };
            review.seq = review.seq.wrapping_add(1);
            show(r, v);
            return None;
        }
        let row = review.selected()?.clone();
        if !r.snap.read_only.is_empty() { return Some(r.snap.read_only.clone()); }
        let replacement = if action == 3 { "" } else { text };
        if replacement.contains(['\n', '\r']) { return Some("Correct one run without line breaks".into()); }
        if action != 3 && replacement.trim().is_empty() { return Some("Use Not text to remove this OCR run".into()); }
        if row.text == replacement { return None; }
        review.busy = true;
        // Replacing keeps the index; removing makes the next run take this index.
        let remains_suspect = replacement.chars().any(|c| c == '?' || c == '\u{fffd}' || (c.is_control() && !c.is_whitespace()));
        review.advance_pending = Some(action != 3 && (!review.suspects_only || remains_suspect));
        r.undo_pages.push(vec![row.page - 1]);
        r.redo_pages.clear();
        r.send(Op::CorrectOcr { page: row.page - 1, line: row.line - 1, expected: row.text, text: replacement.into() });
        None
    }).flatten();
    if let Some(error) = error { crate::flow::toast(ui, &error); }
    crate::viewer_ui::request(ui, tab_id);
}

/// Failed corrections keep their place; successful replacements advance only after applying.
pub(super) fn changed(r: &mut ReaderUi, applied: bool, failed: bool) {
    if applied && let Some(review) = r.ocr_review.as_mut() && let Some(advance) = review.advance_pending.take() && !failed && advance {
        review.at += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_completion_and_empty_layer_are_distinct() {
        let mut review = Review::default();
        assert!(review.position().contains("Run OCR first"));
        review.rows.push(ReviewLine { page: 3, line: 1, rect: [0.0; 4], text: "Scan".into(), suspect: false });
        assert_eq!(review.position(), "Run 1 of 1 · Page 3");
        review.at = 1;
        assert!(review.selected().is_none());
        assert!(review.position().contains("Review complete"));
    }
    #[test]
    fn suspect_filter_preserves_the_original_page_and_line_targets() {
        let mut review = Review { rows: vec![
            ReviewLine { page: 1, line: 1, rect: [0.0; 4], text: "Fine".into(), suspect: false },
            ReviewLine { page: 4, line: 7, rect: [0.0; 4], text: "T?xt".into(), suspect: true },
        ], ..Default::default() };
        assert_eq!(review.count(), 2);
        review.suspects_only = true;
        assert_eq!(review.count(), 1);
        assert_eq!((review.selected().unwrap().page, review.selected().unwrap().line), (4, 7));
        review.rows[1].suspect = false;
        assert!(review.position().contains("No possible decoding errors"));
    }

}
