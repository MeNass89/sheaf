//! How applied redactions look (Acrobat's Redaction Properties ▸ Appearance).
mod common;
use common::render;
use pdfium_render::prelude::*;
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::pages::Input;
use sheaf_core::redact::{self, Query};
use std::path::{Path, PathBuf};

/// Two pages: "Keep this secret word" on the first, "Page two" on the second.
fn two_pages(dir: &Path) -> PathBuf {
    let path = dir.join("look.pdf");
    let _g = pdfium_lock();
    let mut doc = new_document();
    let font = doc.fonts_mut().helvetica();
    for t in ["Keep this secret word", "Page two"] {
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(700.0), t, font, PdfPoints::new(24.0)).unwrap();
    }
    doc.save_to_file(&path).unwrap();
    path
}

#[test]
fn redactions_take_the_colour_and_the_text_chosen() {
    let t = tempfile::tempdir().unwrap();
    let pdf = Input::new(two_pages(t.path()));
    let mut marks = redact::mark_pages(&pdf, &[1]).unwrap();
    let hits = redact::find(&pdf, &Query::Literal { text: "secret".into(), case_insensitive: true, whole_word: false }).unwrap();
    marks.extend(redact::mark_text(&hits));
    let look = redact::Appearance { fill: [1.0, 1.0, 1.0], overlay: "Redacted".into(), repeat: false };
    let r = redact::apply_with(&pdf, &marks, &[], &look, &t.path().join("out.pdf")).unwrap();
    let pages = common::page_texts(&r.path);
    // The word is gone and the overlay text is in its place; page 2 is all white, not black.
    assert!(!pages[0].contains("secret") && pages[0].contains("Keep") && pages[0].contains("Redacted"), "{pages:?}");
    assert_eq!(common::dark_in(&render(&r.path, 1), 0.0, 1.0, 0.0, 1.0), 0, "page 2 white");
    // The independent check passes with "Redacted" on the boxes: the overlay is no leak.
    redact::verify_with(&r.path, &marks, "Redacted").unwrap();
}
