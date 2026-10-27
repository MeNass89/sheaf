mod common;
use common::{fixture_pdf, page_texts, qpdf_clean};
use sheaf_core::images::{images_to_pdf, PageLayout};
use sheaf_core::pages::Input;
use sheaf_core::pdfa::{to_pdfa, verify, PdfALevel};
use sheaf_core::security::protect;

/// The XMP metadata of `path` declares `pdfaid:part` = `part`.
fn declares_part(path: &std::path::Path, part: u8) -> bool {
    let bytes = std::fs::read(path).unwrap();
    let s = String::from_utf8_lossy(&bytes);
    s.contains(&format!("pdfaid:part>{part}<")) || s.contains(&format!("pdfaid:part=\"{part}\""))
}

/// A two-page PDF of RGB pictures and no fonts: everything in it can be made PDF/A.
fn picture_pdf(dir: &std::path::Path) -> std::path::PathBuf {
    let png = dir.join("pic.png");
    image::RgbImage::from_fn(120, 80, |x, y| image::Rgb([(x * 2) as u8, (y * 3) as u8, 90])).save(&png).unwrap();
    images_to_pdf(&[png.clone(), png], PageLayout::FitImage, &dir.join("pics.pdf")).unwrap()
}

#[test]
fn converts_to_pdfa_2b_and_the_output_verifies_clean() {
    let t = tempfile::tempdir().unwrap();
    let a = picture_pdf(t.path());
    assert!(!verify(&Input::new(&a), PdfALevel::A2b).unwrap().is_empty(), "the input is not PDF/A yet");
    let r = to_pdfa(&Input::new(&a), PdfALevel::A2b, &t.path().join("a-pdfa.pdf")).unwrap();
    assert!(!r.was_encrypted);
    assert!(r.issues.is_empty(), "{:?}", r.issues);
    qpdf_clean(&r.path);
    assert!(declares_part(&r.path, 2));
    assert_eq!(lopdf::Document::load(&r.path).unwrap().get_pages().len(), 2);
    // An independent re-check of the written file agrees.
    assert!(verify(&Input::new(&r.path), PdfALevel::A2b).unwrap().is_empty());
}

#[test]
fn what_the_converter_cannot_fix_is_reported() {
    // The fixture's text is in Helvetica, which is not embedded: PdfCraft cannot embed it.
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let r = to_pdfa(&Input::new(&a), PdfALevel::A2b, &t.path().join("a-pdfa.pdf")).unwrap();
    assert!(r.issues.iter().any(|i| i.clause == "6.2.11.4" && i.message.contains("Helvetica")), "{:?}", r.issues);
    // The fixable parts were still done, and the text is kept.
    assert!(declares_part(&r.path, 2));
    assert_eq!(page_texts(&r.path), ["Page 1", "Page 2"]);
    qpdf_clean(&r.path);
}

#[test]
fn pdfa_3b_declares_part_3() {
    let t = tempfile::tempdir().unwrap();
    let a = picture_pdf(t.path());
    let r = to_pdfa(&Input::new(&a), PdfALevel::A3b, &t.path().join("a-pdfa.pdf")).unwrap();
    assert!(r.issues.is_empty(), "{:?}", r.issues);
    assert!(declares_part(&r.path, 3));
    qpdf_clean(&r.path);
}

#[test]
fn an_unconverted_pdf_has_issues() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let issues = verify(&Input::new(&a), PdfALevel::A2b).unwrap();
    assert!(!issues.is_empty());
    assert!(issues.iter().all(|i| !i.clause.is_empty() && !i.message.is_empty()));
}

#[test]
fn protected_input_gives_an_unprotected_pdfa() {
    let t = tempfile::tempdir().unwrap();
    let a = picture_pdf(t.path());
    let p = protect(&Input::new(&a), "user", None, &t.path().join("p.pdf")).unwrap();
    let input = Input { path: p, password: Some("user".into()) };
    let r = to_pdfa(&input, PdfALevel::A2b, &t.path().join("p-pdfa.pdf")).unwrap();
    assert!(r.was_encrypted);
    assert!(r.issues.is_empty(), "{:?}", r.issues);
    let o = std::process::Command::new("qpdf").arg("--show-encryption").arg(&r.path).output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).contains("File is not encrypted"));
    qpdf_clean(&r.path);
}

/// Files encrypted by other producers (qpdf, MuPDF: owner ≠ user password, AES-256 R6) used
/// to fail with "the edited document could not be reopened".
#[test]
fn foreign_encrypted_inputs_convert() {
    let t = tempfile::tempdir().unwrap();
    let a = picture_pdf(t.path());
    for (path, pw) in common::foreign_encrypted(&a, t.path()) {
        let input = Input { path: path.clone(), password: Some(pw.into()) };
        let r = to_pdfa(&input, PdfALevel::A2b, &t.path().join("f-pdfa.pdf")).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(r.was_encrypted);
        assert!(r.issues.is_empty(), "{:?}", r.issues);
        common::unprotected_and_clean(&r.path);
    }
}

#[test]
fn strict_archival_output_keeps_existing_files_and_publishes_no_failed_copy() {
    use sheaf_core::pdfa::to_pdfa_with_policy;
    let t = tempfile::tempdir().unwrap();
    let source = fixture_pdf(t.path(), "source.pdf", 1);
    let out = t.path().join("archive.pdf");
    std::fs::write(&out, "existing archival copy").unwrap();
    let error = to_pdfa_with_policy(&Input::new(&source), PdfALevel::A2b, false, &out).unwrap_err();
    assert!(error.to_string().contains("Helvetica") && error.to_string().contains("no output saved"));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "existing archival copy");
    let files: Vec<_> = std::fs::read_dir(t.path()).unwrap().map(|f| f.unwrap().file_name()).collect();
    assert_eq!(files.len(), 2, "no temporary or neighbouring failed output remains");
    let permissive = to_pdfa_with_policy(&Input::new(&source), PdfALevel::A2b, true, &out).unwrap();
    assert_ne!(permissive.path, out);
    assert!(!permissive.issues.is_empty());
    assert_eq!(page_texts(&permissive.path), ["Page 1"]);
    let good = picture_pdf(t.path());
    let strict = to_pdfa_with_policy(&Input::new(good), PdfALevel::A3b, false, &t.path().join("good.pdf")).unwrap();
    assert!(strict.issues.is_empty());
    qpdf_clean(&strict.path);
}

#[test]
fn native_pdfa1_writes_pdf14_and_checks_transparency_before_publication() {
    use sheaf_core::pdfa::to_pdfa_with_policy;
    let t = tempfile::tempdir().unwrap();
    let source = picture_pdf(t.path());
    let out = t.path().join("part1.pdf");
    let report = to_pdfa_with_policy(&Input::new(&source), PdfALevel::A1b, false, &out).unwrap();
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    assert!(declares_part(&out, 1));
    let mut pdf = lopdf::Document::load(&out).unwrap();
    assert_eq!(pdf.version, "1.4");
    assert_eq!(pdf.get_pages().len(), 2);
    assert!(verify(&Input::new(&out), PdfALevel::A1b).unwrap().is_empty());
    let alpha = pdf.add_object(lopdf::Object::Real(0.5));
    let page = *pdf.get_pages().get(&1).unwrap();
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Resources", lopdf::dictionary! {
        "ExtGState" => lopdf::dictionary! { "Alpha" => lopdf::dictionary! { "ca" => alpha } }
    });
    let transparent = t.path().join("transparent.pdf");
    pdf.save(&transparent).unwrap();
    let failed = t.path().join("failed.pdf");
    let error = to_pdfa_with_policy(&Input::new(&transparent), PdfALevel::A1b, false, &failed).unwrap_err();
    assert!(error.to_string().contains("Transparency"));
    assert!(!failed.exists());
    let incomplete = to_pdfa_with_policy(&Input::new(&transparent), PdfALevel::A1b, true, &failed).unwrap();
    assert!(incomplete.issues.iter().any(|i| i.message.contains("Transparency")));
    // No downgrade or removal of artwork is silently substituted for the requested level.
    assert!(declares_part(&failed, 1));
}
