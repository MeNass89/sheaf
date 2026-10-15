mod common;
use common::{page_texts, qpdf_clean};
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
