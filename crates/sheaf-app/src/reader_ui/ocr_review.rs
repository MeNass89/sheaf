//! Acrobat's Correct Recognized Text: step through hidden runs with the scan in view.
use super::*;
use sheaf_core::ocr_review::ReviewLine;

#[derive(Default)]
pub(super) struct Review {
    rows: Vec<ReviewLine>,
    at: usize,
    pub(super) busy: bool,
    pub(super) seq: i32,
}

impl Review {
    pub(super) fn selected(&self) -> Option<&ReviewLine> { self.rows.get(self.at) }
    pub(super) fn position(&self) -> String {
        if self.busy { return "Reading the OCR layer…".into(); }
        match self.selected() {
            Some(row) => format!("Run {} of {} · Page {}", self.at + 1, self.rows.len(), row.page),
            None if self.rows.is_empty() => "No hidden OCR text. Run OCR first.".into(),
            None => format!("Review complete · {} runs", self.rows.len()),
        }
    }
}

pub fn review_ocr(ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| {
        r.ocr_review = Some(Review::default());
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
    if let Some(row) = r.ocr_review.as_ref().and_then(|f| f.selected()) {
        v.show_page(row.page - 1, None);
    }
}

pub(super) fn reviewed(ui: &AppWindow, tab_id: i32, rows: Vec<ReviewLine>) {
    update(tab_id, |r, v| {
        if let Some(review) = r.ocr_review.as_mut() {
            review.rows = rows;
            review.at = review.at.min(review.rows.len());
            review.busy = false;
            review.seq = review.seq.wrapping_add(1);
        }
        show(r, v);
    });
    crate::viewer_ui::request(ui, tab_id);
}

pub fn ocr_review_action(ui: &AppWindow, tab_id: i32, action: i32, text: &str) {
    let error = update(tab_id, |r, v| {
        if action < 0 { r.ocr_review = None; return None; }
        let review = r.ocr_review.as_mut()?;
        if review.busy || r.edits_in_flight > 0 { return None; }
        if matches!(action, 0 | 1) {
            review.at = if action == 0 { review.at.saturating_sub(1) } else { (review.at + 1).min(review.rows.len()) };
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
        if action != 3 { review.at += 1; }
        r.undo_pages.push(vec![row.page - 1]);
        r.redo_pages.clear();
        r.send(Op::CorrectOcr { page: row.page - 1, line: row.line - 1, expected: row.text, text: replacement.into() });
        None
    }).flatten();
    if let Some(error) = error { crate::flow::toast(ui, &error); }
    crate::viewer_ui::request(ui, tab_id);
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
}
