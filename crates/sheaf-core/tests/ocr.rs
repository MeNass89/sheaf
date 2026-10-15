mod common;
use common::{fixture_pdf, page_texts, qpdf_clean};
use pdfium_render::prelude::*;
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::images::{images_to_pdf, PageLayout};
use sheaf_core::ocr::{self, OcrOptions};
use sheaf_core::pages::Input;
use sheaf_core::security::protect;
use sheaf_core::OpdfError;
use std::path::{Path, PathBuf};

const WORDS: &str = "Sheaf OCR test 2026";

/// A scan-like PDF: a page of text drawn by pdfium, rendered at 300 dpi to a PNG, and that
/// picture as the only content of a page (no text layer).
fn scanned_pdf(dir: &Path) -> PathBuf {
    let png = dir.join("scan.png");
    {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let size = PdfPagePaperSize::Custom(PdfPoints::new(420.0), PdfPoints::new(120.0));
        let mut page = doc.pages_mut().create_page_at_end(size).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(30.0), PdfPoints::new(50.0), WORDS, font, PdfPoints::new(32.0)).unwrap();
        let px = (420.0 * 300.0 / 72.0) as i32;
        let cfg = PdfRenderConfig::new().set_target_width(px).set_clear_color(PdfColor::WHITE);
        page.render_with_config(&cfg).unwrap().as_image().unwrap().to_rgb8().save(&png).unwrap();
    }
    images_to_pdf(&[png], PageLayout::FitImage, &dir.join("scan.pdf")).unwrap()
}

fn words_in(path: &Path) -> usize {
    page_texts(path).iter().map(|t| t.split_whitespace().count()).sum()
}

fn no_cancel() -> impl FnMut(f32) -> bool {
    |_| true
}

#[test]
fn reads_the_words_of_a_scanned_page() {
    assert!(ocr::available(), "run scripts/fetch-models.sh");
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    assert_eq!(words_in(&scan), 0, "the input has no text layer");
    let mut seen = Vec::new();
    let r = ocr::ocr(&Input::new(&scan), &OcrOptions::default(), &t.path().join("scan-ocr.pdf"), &mut |p| {
        seen.push(p);
        true
    })
    .unwrap();
    assert!(!r.was_encrypted);
    assert_eq!((r.pages_read, r.pages_skipped), (1, 0));
    assert!(r.words >= 3, "{r:?}");
    let text = page_texts(&r.path).join(" ");
    assert!(text.contains("Sheaf") && text.contains("OCR"), "{text:?}");
    assert_eq!(seen.last(), Some(&1.0));
    qpdf_clean(&r.path);
}

#[test]
fn pages_that_already_have_text_are_skipped_by_default() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let r = ocr::ocr(&Input::new(&a), &OcrOptions::default(), &t.path().join("a-ocr.pdf"), &mut no_cancel()).unwrap();
    assert_eq!((r.words, r.pages_read, r.pages_skipped), (0, 0, 2));
    assert_eq!(page_texts(&r.path), ["Page 1", "Page 2"]);
    assert_eq!(words_in(&r.path), words_in(&a));
    qpdf_clean(&r.path);
}

#[test]
fn reading_text_pages_never_duplicates_their_text() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let opts = OcrOptions { read_text_pages: true, ..Default::default() };
    let r = ocr::ocr(&Input::new(&a), &opts, &t.path().join("a-ocr.pdf"), &mut no_cancel()).unwrap();
    assert_eq!((r.pages_read, r.pages_skipped), (1, 0));
    assert_eq!(page_texts(&r.path), ["Page 1"]);
}

#[test]
fn a_protected_scan_gives_an_unprotected_searchable_pdf() {
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    let p = protect(&Input::new(&scan), "pw", None, &t.path().join("p.pdf")).unwrap();
    let no_pw = ocr::ocr(&Input::new(&p), &OcrOptions::default(), &t.path().join("x.pdf"), &mut no_cancel());
    assert!(matches!(no_pw, Err(OpdfError::PasswordRequired(_))), "{no_pw:?}");
    let input = Input { path: p, password: Some("pw".into()) };
    let r = ocr::ocr(&input, &OcrOptions::default(), &t.path().join("p-ocr.pdf"), &mut no_cancel()).unwrap();
    assert!(r.was_encrypted);
    let o = std::process::Command::new("qpdf").arg("--show-encryption").arg(&r.path).output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).contains("File is not encrypted"));
    assert!(page_texts(&r.path).join(" ").contains("Sheaf"));
    qpdf_clean(&r.path);
}

#[test]
fn cancelling_writes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    let out = t.path().join("c.pdf");
    let r = ocr::ocr(&Input::new(&scan), &OcrOptions::default(), &out, &mut |_| false);
    assert!(matches!(r, Err(OpdfError::Cancelled)), "{r:?}");
    assert!(!out.exists());
    let left: Vec<_> = std::fs::read_dir(t.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left.len(), 2, "only scan.png and scan.pdf, no .part: {left:?}");
}

#[test]
fn unknown_languages_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let opts = OcrOptions { language: "xx".into(), ..Default::default() };
    let r = ocr::ocr(&Input::new(&a), &opts, &t.path().join("o.pdf"), &mut no_cancel());
    assert!(matches!(r, Err(OpdfError::Unsupported(_))), "{r:?}");
    assert!(ocr::languages().iter().any(|(code, _)| *code == "en"));
}
