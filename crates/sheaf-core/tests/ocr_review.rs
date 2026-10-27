mod common;
use sheaf_core::{ocr_review, pages::Input};

#[test]
fn corrected_and_rejected_ocr_runs_change_search_text_but_never_the_scan() {
    let t = tempfile::tempdir().unwrap();
    let source = common::fixture_pdf(t.path(), "source.pdf", 1);
    let scan = t.path().join("hidden.pdf");
    common::add_invisible_text(&source, "T?xt", &scan);
    let before = common::render(&scan, 0);
    let rows = ocr_review::review(&Input::new(&scan)).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].suspect);
    assert_eq!(rows[0].text, "T?xt");
    let fixed = ocr_review::correct(&Input::new(&scan), 0, rows[0].line - 1, "T?xt", "Text", &t.path().join("fixed.pdf")).unwrap();
    assert_eq!(common::render(&fixed, 0), before);
    assert!(common::shown_texts(&fixed)[0].contains("Text"));
    let rows = ocr_review::review(&Input::new(&fixed)).unwrap();
    assert!(!rows[0].suspect);
    let removed = ocr_review::correct(&Input::new(&fixed), 0, rows[0].line - 1, "Text", "", &t.path().join("not-text.pdf")).unwrap();
    assert_eq!(common::render(&removed, 0), before);
    assert!(!common::shown_texts(&removed)[0].contains("Text"));
    assert!(common::shown_texts(&removed)[0].contains("Page 1"));
}

#[test]
fn ocr_correction_refuses_visible_text_and_stale_review_rows() {
    let t = tempfile::tempdir().unwrap();
    let source = common::fixture_pdf(t.path(), "source.pdf", 1);
    let out = t.path().join("out.pdf");
    assert!(ocr_review::correct(&Input::new(&source), 0, 0, "Page 1", "Changed", &out).unwrap_err().to_string().contains("visible"));
    let scan = t.path().join("hidden.pdf");
    common::add_invisible_text(&source, "Wrong", &scan);
    let rows = ocr_review::review(&Input::new(&scan)).unwrap();
    assert!(ocr_review::correct(&Input::new(scan), 0, rows[0].line - 1, "Old review", "Fixed", &out).unwrap_err().to_string().contains("changed"));
    assert!(!out.exists());
}
