//! iLovePDF's language list: real script recognition and the saved Unicode searchable layer.
mod common;
use sheaf_core::{images::{images_to_pdf, PageLayout}, ocr::{self, OcrOptions}, pages::Input};
use std::path::Path;

#[test]
fn installed_cjk_packs_recognize_and_save_searchable_unicode_without_repainting() {
    let dir = tempfile::tempdir().unwrap();
    let mut checked = 0;
    for (language, image, expected) in [("ja", "japanese", "日本語"), ("zh", "chinese", "中文"), ("ko", "korean", "한국어")] {
        if !ocr::languages().iter().any(|(code, _)| *code == language) { continue; }
        let png = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/ocr-{image}.png"));
        let scan = images_to_pdf(&[png], PageLayout::FitImage, &dir.path().join(format!("{image}.pdf"))).unwrap();
        let report = ocr::ocr(&Input::new(&scan), &OcrOptions { language: language.into(), ..Default::default() }, &dir.path().join(format!("{image}-ocr.pdf")), &mut |_| true).unwrap();
        let text = common::page_texts(&report.path).join("").replace(char::is_whitespace, "");
        assert!(text.contains(expected), "{language}: {text}");
        eprintln!("CJK OCR: {language} read {text}; {} words", report.words);
        assert_eq!((report.pages_read, report.pages_skipped), (1, 0));
        let render = |path: &Path| sheaf_core::pdfcraft::Doc::open(&Input::new(path)).unwrap().render(0, 1.0).unwrap().rgba;
        assert!(render(&scan) == render(&report.path), "{language}: the hidden Unicode layer must not repaint the scan");
        common::qpdf_clean(&report.path);
        checked += 1;
    }
    if checked == 0 { eprintln!("CJK recognition not available: install Tesseract packs in the system folder or SHEAF_TESSDATA"); }
}
