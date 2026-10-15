mod common;
use common::fixture_pdf;
use pdfium_render::prelude::*;
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::export::{self, ExportOptions, Office};
use sheaf_core::images::{images_to_pdf, PageLayout};
use sheaf_core::pages::Input;
use sheaf_core::security::protect;
use std::path::{Path, PathBuf};

/// One member of a zip file (.docx, .pptx, .xlsx), as text.
pub fn unzip(file: &Path, member: &str) -> String {
    let o = std::process::Command::new("unzip").arg("-p").arg(file).arg(member).output().unwrap();
    assert!(o.status.success(), "{member} in {}: {}", file.display(), String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn no_cancel() -> impl FnMut(f32) -> bool {
    |_| true
}

#[test]
fn word_export_holds_the_text() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let r = export::to_office(&Input::new(&a), Office::Word, &ExportOptions::default(), &t.path().join("a.docx"), &mut no_cancel()).unwrap();
    assert_eq!(r.path, t.path().join("a.docx"));
    let xml = unzip(&r.path, "word/document.xml");
    assert!(xml.contains("Page 1") && xml.contains("Page 2"), "{xml}");
}

#[test]
fn powerpoint_export_has_one_slide_per_page() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let r = export::to_office(&Input::new(&a), Office::PowerPoint, &ExportOptions::default(), &t.path().join("a.pptx"), &mut no_cancel()).unwrap();
    assert!(unzip(&r.path, "ppt/slides/slide1.xml").contains("Page 1"));
    assert!(unzip(&r.path, "ppt/slides/slide2.xml").contains("Page 2"));
}

#[test]
fn an_existing_file_is_never_overwritten() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    std::fs::write(t.path().join("a.docx"), b"keep").unwrap();
    let r = export::to_office(&Input::new(&a), Office::Word, &ExportOptions::default(), &t.path().join("a.docx"), &mut no_cancel()).unwrap();
    assert_eq!(r.path, t.path().join("a (1).docx"));
    assert_eq!(std::fs::read(t.path().join("a.docx")).unwrap(), b"keep");
}

#[test]
fn a_protected_pdf_exports_with_its_password() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let p = protect(&Input::new(&a), "pw", None, &t.path().join("p.pdf")).unwrap();
    let input = Input { path: p, password: Some("pw".into()) };
    let r = export::to_office(&input, Office::Word, &ExportOptions::default(), &t.path().join("p.docx"), &mut no_cancel()).unwrap();
    assert!(unzip(&r.path, "word/document.xml").contains("Page 1"));
}

/// A picture of the words "Invoice 2026" (no text layer).
fn scanned_pdf(dir: &Path) -> PathBuf {
    let png = dir.join("scan.png");
    {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let size = PdfPagePaperSize::Custom(PdfPoints::new(400.0), PdfPoints::new(120.0));
        let mut page = doc.pages_mut().create_page_at_end(size).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(30.0), PdfPoints::new(50.0), "Invoice 2026", font, PdfPoints::new(36.0)).unwrap();
        let cfg = PdfRenderConfig::new().set_target_width(1667).set_clear_color(PdfColor::WHITE);
        page.render_with_config(&cfg).unwrap().as_image().unwrap().to_rgb8().save(&png).unwrap();
    }
    images_to_pdf(&[png], PageLayout::FitImage, &dir.join("scan.pdf")).unwrap()
}

#[test]
fn ocr_first_recovers_the_text_of_a_scan() {
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    let plain = export::to_office(&Input::new(&scan), Office::Word, &ExportOptions::default(), &t.path().join("plain.docx"), &mut no_cancel()).unwrap();
    assert!(!unzip(&plain.path, "word/document.xml").contains("Invoice"));
    let opts = ExportOptions { ocr: true, ..Default::default() };
    let r = export::to_office(&Input::new(&scan), Office::Word, &opts, &t.path().join("scan.docx"), &mut no_cancel()).unwrap();
    assert!(unzip(&r.path, "word/document.xml").contains("Invoice"));
    // Cancelled during OCR: nothing written.
    let out = t.path().join("c.docx");
    let c = export::to_office(&Input::new(&scan), Office::Word, &opts, &out, &mut |_| false);
    assert!(matches!(c, Err(sheaf_core::OpdfError::Cancelled)), "{c:?}");
    assert!(!out.exists());
}
