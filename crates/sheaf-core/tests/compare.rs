mod common;
use common::{fixture_pdf, page_texts, qpdf_clean};
use pdfium_render::prelude::*;
use sheaf_core::compare::{self, CompareMode};
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::pages::Input;
use sheaf_core::security::protect;
use std::path::{Path, PathBuf};

/// One A4 page per line of `lines`, each drawn at the top in Helvetica 14.
fn text_pdf(dir: &Path, name: &str, lines: &[&str]) -> PathBuf {
    let _g = pdfium_lock();
    let mut doc = new_document();
    let font = doc.fonts_mut().helvetica();
    let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
    for (i, l) in lines.iter().enumerate() {
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(760.0 - 24.0 * i as f32), *l, font, PdfPoints::new(14.0)).unwrap();
    }
    let path = dir.join(name);
    doc.save_to_file(&path).unwrap();
    path
}

fn pair(dir: &Path) -> (PathBuf, PathBuf) {
    let old = text_pdf(dir, "old.pdf", &["The quick brown fox jumps", "over the lazy dog today"]);
    let new = text_pdf(dir, "new.pdf", &["The quick red fox jumps", "over the lazy dog today"]);
    (old, new)
}

#[test]
fn one_changed_word_is_one_replacement_in_the_report() {
    let t = tempfile::tempdir().unwrap();
    let (old, new) = pair(t.path());
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Text, &t.path().join("report.pdf")).unwrap();
    // PdfCraft groups a deleted word followed by an inserted one as one "Replaced" change.
    assert_eq!((r.replaced, r.added, r.removed), (1, 0, 0));
    assert!(!r.identical());
    qpdf_clean(&r.path);
    let text = page_texts(&r.path).join("\n");
    assert!(text.contains("Compare Report") && text.contains("brown") && text.contains("red"), "{text}");
    assert!(text.contains("old.pdf") && text.contains("new.pdf"), "{text}");
}

#[test]
fn added_and_removed_words_are_counted_apart() {
    let t = tempfile::tempdir().unwrap();
    let old = text_pdf(t.path(), "old.pdf", &["alpha beta gamma delta", "one two three four"]);
    let new = text_pdf(t.path(), "new.pdf", &["alpha beta gamma delta extra", "one three four"]);
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Text, &t.path().join("r.pdf")).unwrap();
    assert_eq!((r.replaced, r.added, r.removed), (0, 1, 1), "{r:?}");
}

#[test]
fn identical_documents_are_reported_identical() {
    let t = tempfile::tempdir().unwrap();
    let (old, _) = pair(t.path());
    let r = compare::compare(&Input::new(&old), &Input::new(&old), CompareMode::Text, &t.path().join("r.pdf")).unwrap();
    assert!(r.identical());
    assert!(page_texts(&r.path).join(" ").contains("identical"));
}

#[test]
fn overlay_marks_the_differences_on_the_new_document() {
    let t = tempfile::tempdir().unwrap();
    let (old, new) = pair(t.path());
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Overlay, &t.path().join("overlay.pdf")).unwrap();
    assert_eq!((r.replaced, r.added, r.removed), (1, 0, 0));
    // The only visual difference is the changed word, already highlighted.
    assert_eq!(r.regions, 0, "{r:?}");
    qpdf_clean(&r.path);
    // The new document, with highlight annotations over the change.
    assert_eq!(page_texts(&r.path), page_texts(&new));
    let d = lopdf::Document::load(&r.path).unwrap();
    let page = d.get_dictionary(d.get_pages()[&1]).unwrap();
    let annots = page.get(b"Annots").and_then(|a| match a {
        lopdf::Object::Reference(id) => d.get_object(*id).and_then(|o| o.as_array().cloned()),
        o => o.as_array().cloned(),
    });
    let subtypes: Vec<String> = annots
        .unwrap()
        .iter()
        .map(|a| {
            let dict = d.get_dictionary(a.as_reference().unwrap()).unwrap();
            String::from_utf8_lossy(dict.get(b"Subtype").unwrap().as_name().unwrap()).into_owned()
        })
        .collect();
    assert!(subtypes.iter().any(|s| s == "Highlight"), "{subtypes:?}");
}

#[test]
fn protected_inputs_open_with_their_password_and_the_output_is_not_protected() {
    let t = tempfile::tempdir().unwrap();
    let (old, new) = pair(t.path());
    let p = protect(&Input::new(&new), "pw", None, &t.path().join("p.pdf")).unwrap();
    let locked = Input { path: p, password: Some("pw".into()) };
    for mode in [CompareMode::Text, CompareMode::Overlay] {
        let r = compare::compare(&Input::new(&old), &locked, mode, &t.path().join("r.pdf")).unwrap();
        assert_eq!(r.replaced, 1);
        assert_eq!(r.was_encrypted, mode == CompareMode::Overlay, "only the overlay is a copy of the protected file");
        let o = std::process::Command::new("qpdf").arg("--show-encryption").arg(&r.path).output().unwrap();
        assert!(String::from_utf8_lossy(&o.stdout).contains("File is not encrypted"), "{mode:?}");
    }
}

#[test]
fn overlay_outlines_a_changed_picture() {
    use sheaf_core::images::{images_to_pdf, PageLayout};
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a.png");
    let b = t.path().join("b.png");
    image::RgbImage::from_pixel(300, 200, image::Rgb([255, 255, 255])).save(&a).unwrap();
    image::RgbImage::from_fn(300, 200, |x, y| if (100..160).contains(&x) && (60..120).contains(&y) { image::Rgb([200, 20, 20]) } else { image::Rgb([255, 255, 255]) }).save(&b).unwrap();
    let old = images_to_pdf(&[a], PageLayout::FitImage, &t.path().join("old.pdf")).unwrap();
    let new = images_to_pdf(&[b], PageLayout::FitImage, &t.path().join("new.pdf")).unwrap();
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Overlay, &t.path().join("o.pdf")).unwrap();
    assert!(r.identical(), "no words changed");
    assert!(r.regions >= 1, "{r:?}");
    qpdf_clean(&r.path);
}

/// Overlay of files encrypted by other producers (qpdf, MuPDF: owner ≠ user password), both
/// with differences and identical (the protection is then the only thing removed).
#[test]
fn foreign_encrypted_inputs_compare() {
    let t = tempfile::tempdir().unwrap();
    let (old, new) = pair(t.path());
    for (path, pw) in common::foreign_encrypted(&new, t.path()) {
        let locked = Input { path: path.clone(), password: Some(pw.into()) };
        for (other, changed) in [(&old, 1), (&new, 0)] {
            let r = compare::compare(&Input::new(other), &locked, CompareMode::Overlay, &t.path().join("f.pdf")).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(r.replaced, changed, "{}", path.display());
            assert!(r.was_encrypted);
            common::unprotected_and_clean(&r.path);
        }
    }
}

/// One page: a filled square of colour `rgb` with the caption `caption` just under it.
fn picture_with_caption(dir: &Path, name: &str, rgb: &str, caption: &str) -> PathBuf {
    let content = format!("{rgb} rg 100 500 200 150 re f BT /F1 14 Tf 100 486 Td ({caption}) Tj ET");
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj {o} endobj\n", i + 1).bytes());
    }
    let x = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(format!("trailer << /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n", objs.len() + 1).bytes());
    let path = dir.join(name);
    std::fs::write(&path, out).unwrap();
    path
}

#[test]
fn a_changed_picture_touching_a_changed_caption_is_still_outlined() {
    let t = tempfile::tempdir().unwrap();
    let old = picture_with_caption(t.path(), "old.pdf", "0 0 1", "Figure one");
    let new = picture_with_caption(t.path(), "new.pdf", "1 0 0", "Figure two");
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Overlay, &t.path().join("o.pdf")).unwrap();
    assert_eq!(r.replaced, 1, "{r:?}");
    assert_eq!(r.regions, 1, "the picture's change is not hidden by the caption's: {r:?}");
    qpdf_clean(&r.path);
}

#[test]
fn the_report_lists_a_changed_picture_whose_words_are_the_same() {
    let t = tempfile::tempdir().unwrap();
    let old = picture_with_caption(t.path(), "old.pdf", "0 0 1", "Figure one");
    let new = picture_with_caption(t.path(), "new.pdf", "1 0 0", "Figure one");
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Text, &t.path().join("r.pdf")).unwrap();
    assert!(r.identical(), "no words changed: {r:?}");
    assert_eq!(r.regions, 1, "{r:?}");
    qpdf_clean(&r.path);
    let texts = page_texts(&r.path);
    let last = texts.last().unwrap();
    assert!(last.contains("Changes outside the text") && last.contains("Page 1: 1 area"), "{texts:?}");
}

#[test]
fn a_page_added_or_removed_at_the_end_is_reported() {
    let t = tempfile::tempdir().unwrap();
    let one = text_pdf(t.path(), "one.pdf", &["The same words"]);
    // The same page, then a blank (scanned-like, no words) page: nothing for the text to see.
    let two = sheaf_core::pages::merge(&[Input::new(&one), Input::new(sheaf_core::pages::blank(false, &t.path().join("b.pdf")).unwrap())], &t.path().join("two.pdf")).unwrap();
    let r = compare::compare(&Input::new(&one), &Input::new(&two), CompareMode::Text, &t.path().join("r1.pdf")).unwrap();
    assert_eq!((r.pages_added, r.pages_removed), (1, 0), "{r:?}");
    assert!(r.summary().contains("1 page added at the end"), "{}", r.summary());
    assert!(page_texts(&r.path).join(" ").contains("Page 2 is only in the changed document"));
    let r = compare::compare(&Input::new(&two), &Input::new(&one), CompareMode::Text, &t.path().join("r2.pdf")).unwrap();
    assert_eq!((r.pages_added, r.pages_removed), (0, 1));
    assert!(page_texts(&r.path).join(" ").contains("Page 2 of the original is not in the changed document"));
}

#[test]
fn side_by_side_puts_each_original_page_beside_the_marked_one() {
    let t = tempfile::tempdir().unwrap();
    let old = fixture_pdf(t.path(), "old.pdf", 2);
    let new = fixture_pdf(t.path(), "new.pdf", 3);
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::SideBySide, &t.path().join("s.pdf")).unwrap();
    let doc = lopdf::Document::load(&r.path).unwrap();
    // Three sheets (the longer document's pages), each wider than tall, as wide as two pages.
    assert_eq!(doc.get_pages().len(), 3);
    let media = doc.get_dictionary(doc.get_pages()[&1]).unwrap().get(b"MediaBox").unwrap().as_array().unwrap().iter().map(|o| o.as_float().unwrap_or(0.0)).collect::<Vec<f32>>();
    assert!(media[2] > 2.0 * 595.0 && media[3] < media[2], "{media:?}");
    // Page 1's words are on sheet 1 twice: the original's and the copy's.
    assert_eq!(common::shown_texts(&r.path)[0].matches("Page 1").count(), 2, "{:?}", common::shown_texts(&r.path));
}

#[test]
fn a_line_moved_with_the_same_words_is_reported() {
    let t = tempfile::tempdir().unwrap();
    let at = |name: &str, y: f32| {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(y), "The same words here", font, PdfPoints::new(14.0)).unwrap();
        let path = t.path().join(name);
        doc.save_to_file(&path).unwrap();
        path
    };
    let (old, new) = (at("old.pdf", 760.0), at("new.pdf", 500.0));
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Text, &t.path().join("r.pdf")).unwrap();
    assert_eq!((r.added, r.removed, r.replaced, r.moved), (0, 0, 0, 1), "{r:?}");
    assert!(r.summary().contains("1 line moved"), "{}", r.summary());
    assert!(page_texts(&r.path).join(" ").contains("Page 1: 1 line moved"));
    // The same with a word changed in another column at the new line's height: still moved.
    let two = |name: &str, y: f32, right: &str| {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(y), "The same words here", font, PdfPoints::new(14.0)).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(360.0), PdfPoints::new(500.0), right, font, PdfPoints::new(14.0)).unwrap();
        let path = t.path().join(name);
        doc.save_to_file(&path).unwrap();
        path
    };
    let (old, new) = (two("old2.pdf", 760.0, "Column two old"), two("new2.pdf", 500.0, "Column two new"));
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Text, &t.path().join("r2.pdf")).unwrap();
    assert_eq!(r.moved, 1, "{r:?}");
}

#[test]
fn documents_that_share_almost_no_words_are_said_to() {
    let t = tempfile::tempdir().unwrap();
    let doc = |name: &str, word: &str| {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        for line in 0..6 {
            let text = (0..5).map(|k| format!("{word}{}", line * 5 + k)).collect::<Vec<_>>().join(" ");
            page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(760.0 - line as f32 * 20.0), &text, font, PdfPoints::new(12.0)).unwrap();
        }
        let path = t.path().join(name);
        doc.save_to_file(&path).unwrap();
        path
    };
    let (old, new) = (doc("a.pdf", "apple"), doc("b.pdf", "zebra"));
    let r = compare::compare(&Input::new(&old), &Input::new(&new), CompareMode::Text, &t.path().join("r.pdf")).unwrap();
    assert!(page_texts(&r.path).join(" ").contains("differ almost everywhere"), "{:?}", page_texts(&r.path));
}
