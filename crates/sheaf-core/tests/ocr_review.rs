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

#[test]
fn live_ocr_review_sees_unsaved_corrections_and_undo_and_refuses_missing_pages() {
    let t = tempfile::tempdir().unwrap();
    let source = common::fixture_pdf(t.path(), "source.pdf", 1);
    let scan = t.path().join("hidden.pdf");
    common::add_invisible_text(&source, "T?xt", &scan);
    let mut doc = sheaf_core::pdfcraft::Doc::open(&Input::new(&scan)).unwrap();
    let before = ocr_review::review_document(doc.document());
    let row = &before[0];
    let edit = ocr_review::correction_edit(doc.document(), row.page - 1, row.line - 1, &row.text, "Text").unwrap();
    doc.apply(edit).unwrap();
    assert_eq!(ocr_review::review_document(doc.document())[0].text, "Text");
    assert!(ocr_review::correction_edit(doc.document(), 0, row.line - 1, &row.text, "Stale").is_err());
    assert!(ocr_review::correction_edit(doc.document(), 99, 0, "", "Missing").is_err());
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    assert_eq!(ocr_review::review_document(doc.document()), before);
    assert_eq!(ocr_review::review(&Input::new(&scan)).unwrap(), before, "review does not save over the source");
}

#[test]
fn word_confidence_survives_save_and_corrections_leave_neighboring_words_and_scan_intact() {
    use sheaf_core::pdfcraft::{Doc, engine::{Edit, ocr::PlacedWord}};
    let t = tempfile::tempdir().unwrap();
    let source = common::fixture_pdf(t.path(), "source.pdf", 2);
    let mut doc = Doc::open(&Input::new(&source)).unwrap();
    let word = |text: &str, x: f64, confidence| PlacedWord {
        text: text.into(), confidence, origin: [x, 500.0], across: [40.0, 0.0], up: [0.0, 12.0],
    };
    doc.apply(Edit::AddOcrText { page: 0, words: vec![word("Clear", 100.0, Some(0.99)), word("Errar", 145.0, Some(0.42))] }).unwrap();
    doc.apply(Edit::AddOcrText { page: 1, words: vec![word("Other", 100.0, Some(0.95))] }).unwrap();
    let path = doc.save_unprotected(&t.path().join("ocr.pdf")).unwrap().0;
    let pixels = common::render(&path, 0);
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    let mut cache = ocr_review::ReviewCache::default();
    let rows = cache.review(doc.document());
    assert_eq!(rows.len(), 3, "adjacent OCR words retain separate edit targets");
    assert_eq!(rows.iter().filter(|r| r.suspect).count(), 1);
    let suspect = rows.iter().find(|r| r.suspect).unwrap();
    assert_eq!(suspect.text.trim(), "Errar");
    assert!((suspect.confidence.unwrap() - 0.42).abs() < 0.001);
    let generation = doc.document().edit_generation();
    let edit = ocr_review::correction_edit(doc.document(), 0, suspect.line - 1, &suspect.text, "Error").unwrap();
    doc.apply(edit).unwrap();
    cache.corrected(doc.document(), generation, 0);
    let corrected = cache.review(doc.document());
    assert_eq!(corrected[0], rows[0], "neighbor remains untouched");
    assert_eq!(corrected[2], rows[2], "other page remains untouched");
    assert_eq!(corrected[1].text.trim(), "Error");
    assert!(!corrected[1].suspect, "manual correction clears the recognition suspect");
    let fixed = doc.save_unprotected(&t.path().join("fixed.pdf")).unwrap().0;
    assert_eq!(common::render(&fixed, 0), pixels);
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    assert_eq!(cache.review(doc.document()), rows, "undo invalidates cached confidence too");
}
