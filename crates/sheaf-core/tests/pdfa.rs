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
    assert!(matches!(pdf.reference_table.cross_reference_type, lopdf::xref::XrefType::CrossReferenceTable));
    assert_eq!(pdf.get_pages().len(), 2);
    assert!(verify(&Input::new(&out), PdfALevel::A1b).unwrap().is_empty());
    // Identification may be in RDF attributes, not only the converter's element form.
    let metadata = pdf.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
    let stream = pdf.get_object(metadata).unwrap().as_stream().unwrap();
    let text = String::from_utf8(stream.get_plain_content().unwrap()).unwrap()
        .replace("<pdfaid:part>1</pdfaid:part>", "")
        .replace("<pdfaid:conformance>B</pdfaid:conformance>", "")
        .replace("rdf:about=\"\"", "rdf:about=\"\" pdfaid:part=\"1\" pdfaid:conformance=\"B\"");
    let mut dict = stream.dict.clone(); dict.remove(b"Filter"); dict.remove(b"DecodeParms");
    pdf.objects.insert(metadata, lopdf::Object::Stream(lopdf::Stream::new(dict, text.into_bytes())));
    pdf.save(&out).unwrap();
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

/// iLovePDF offers all eight identifiers, including a/u. Missing tags are findings, not a
/// silent downgrade to b; strict publication refuses them without replacing existing files.
#[test]
fn every_archival_level_keeps_its_identifier_and_strict_a_levels_require_tags() {
    use sheaf_core::pdfa::to_pdfa_with_policy;
    let t = tempfile::tempdir().unwrap();
    let source = picture_pdf(t.path());
    for level in PdfALevel::ALL {
        let path = t.path().join(format!("{}.pdf", level.label().replace('/', "")));
        let report = to_pdfa_with_policy(&Input::new(&source), level, true, &path).unwrap();
        let doc = sheaf_core::pdfcraft::Doc::open(&Input::new(&path)).unwrap();
        assert_eq!(doc.document().standards().pdfa, Some((level.part(), level.conformance().into())));
        assert_eq!(lopdf::Document::load(&path).unwrap().get_pages().len(), 2);
        if level.conformance() == "A" {
            assert!(report.issues.iter().any(|i| i.clause.starts_with("PDF/A-a")), "{:?}", report.issues);
            let strict = path.with_file_name(format!("strict-{}.pdf", level.part()));
            assert!(to_pdfa_with_policy(&Input::new(&source), level, false, &strict).is_err());
            assert!(!strict.exists());
        } else { assert!(report.issues.is_empty(), "{}: {:?}", level.label(), report.issues); }
        qpdf_clean(&path);
    }
}

/// A proper text-only tag tree and Unicode survive conversion to each a-level; it is not
/// enough just to reject untagged input. Use Sheaf's Markdown producer and native tagging.
#[test]
fn tagged_unicode_text_can_be_archived_at_all_a_levels_without_losing_tags() {
    use sheaf_core::{office, pdfa::to_pdfa_with_policy, pdfcraft::{Doc, engine::Edit}};
    if office::find_soffice().is_none() { eprintln!("skip: no LibreOffice"); return; }
    let t = tempfile::tempdir().unwrap();
    let md = t.path().join("source.md");
    std::fs::write(&md, "# Archival title\n\nCafé and Unicode text.\n").unwrap();
    let input = office::office_to_pdf(&md, &t.path().join("source.pdf")).unwrap();
    let mut doc = Doc::open(&Input::new(&input)).unwrap();
    if doc.document().tags().is_empty() { doc.apply(Edit::AutoTagText).unwrap(); }
    let mut view = doc.document().initial_view();
    view.language = Some("en-US".into());
    doc.apply(Edit::SetInitialView(Box::new(view))).unwrap();
    let tagged = doc.save(&t.path().join("tagged.pdf")).unwrap();
    let before = doc.document().tags().len();
    assert!(before > 0);
    for level in [PdfALevel::A1a, PdfALevel::A2a, PdfALevel::A3a] {
        let path = t.path().join(format!("ready-{}.pdf", level.part()));
        let report = to_pdfa_with_policy(&Input::new(&tagged), level, false, &path).unwrap();
        assert!(report.issues.is_empty());
        let reopened = Doc::open(&Input::new(&path)).unwrap();
        assert_eq!(reopened.document().tags().len(), before);
        assert_eq!(reopened.document().initial_view().language.as_deref(), Some("en-US"));
        assert!(page_texts(&path).join(" ").contains("Café and Unicode text."));
        qpdf_clean(&path);
    }
}

#[test]
fn unicode_archive_refuses_symbolic_subset_fonts_without_maps_even_when_b_checks_pass() {
    use sheaf_core::{office, pdfa::to_pdfa_with_policy};
    if office::find_soffice().is_none() { eprintln!("skip: no LibreOffice"); return; }
    let t = tempfile::tempdir().unwrap();
    let md = t.path().join("source.md");
    std::fs::write(&md, "# Unicode archive\n\nCafé and several text codes.\n").unwrap();
    let input = office::office_to_pdf(&md, &t.path().join("source.pdf")).unwrap();
    let mut pdf = lopdf::Document::load(&input).unwrap();
    let mut removed = 0;
    for object in pdf.objects.values_mut() {
        if let Ok(font) = object.as_dict_mut()
            && font.get(b"Subtype").and_then(lopdf::Object::as_name).ok() == Some(b"TrueType")
            && font.remove(b"ToUnicode").is_some()
        { removed += 1; }
    }
    assert!(removed > 0, "LibreOffice's canonical producer emitted mapped TrueType subsets");
    let without_maps = t.path().join("without-maps.pdf");
    pdf.save(&without_maps).unwrap();
    to_pdfa_with_policy(&Input::new(&without_maps), PdfALevel::A2b, false, &t.path().join("basic.pdf")).unwrap();
    let strict = t.path().join("unicode.pdf");
    assert!(to_pdfa_with_policy(&Input::new(&without_maps), PdfALevel::A2u, false, &strict).is_err());
    assert!(!strict.exists());
    let report = to_pdfa_with_policy(&Input::new(&without_maps), PdfALevel::A2u, true, &strict).unwrap();
    assert!(report.issues.iter().any(|i| i.clause == "PDF/A Unicode mapping"), "{:?}", report.issues);
    assert_eq!(sheaf_core::pdfcraft::Doc::open(&Input::new(&strict)).unwrap().document().standards().pdfa, Some((2, "U".into())));
}

/// PDF/E-1 (Acrobat's Save as PDF/E, C-convert-019): written as PDF 1.6 with the PDF/E identification in its
/// metadata and checked clean; a file whose fonts are not embedded is refused with nothing saved, as PDF/A is.
#[test]
fn pdfe_writes_pdf16_identified_as_pdfe1_and_verifies_clean() {
    use sheaf_core::pdfe::{to_pdfe, verify as verify_e};
    let t = tempfile::tempdir().unwrap();
    let source = picture_pdf(t.path());
    let report = to_pdfe(&Input::new(&source), false, &t.path().join("drawing-e.pdf")).unwrap();
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let pdf = lopdf::Document::load(&report.path).unwrap();
    assert_eq!(pdf.version, "1.6");
    let xmp = pdf.get_object(pdf.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap()).unwrap().as_stream().unwrap().get_plain_content().unwrap();
    assert!(String::from_utf8_lossy(&xmp).contains("<pdfe:ISO_PDFEVersion>PDF/E-1</pdfe:ISO_PDFEVersion>"));
    assert_eq!(pdf.get_pages().len(), 2);
    qpdf_clean(&report.path);
    // The source is not PDF/E: its check names the missing identification.
    assert!(verify_e(&Input::new(&source)).unwrap().iter().any(|i| i.message.contains("PDF/E-1")));
    // Helvetica not embedded: refused, nothing written.
    let text = fixture_pdf(t.path(), "text.pdf", 1);
    let refused = to_pdfe(&Input::new(&text), false, &t.path().join("text-e.pdf")).unwrap_err();
    assert!(refused.to_string().contains("Helvetica") && !t.path().join("text-e.pdf").exists(), "{refused}");
}

/// PDF/E keeps what engineering documents need (Sol rev36: the PDF/A-2b step removed 3D annotations): a page's
/// 3D model annotation and its U3D stream come through with their subtype, and the file still checks clean.
#[test]
fn pdfe_keeps_a_3d_annotation_and_its_model() {
    use lopdf::{dictionary, Object, Stream};
    let t = tempfile::tempdir().unwrap();
    let source = picture_pdf(t.path());
    let mut pdf = lopdf::Document::load(&source).unwrap();
    let model = pdf.add_object(Stream::new(dictionary! { "Type" => "3D", "Subtype" => "U3D" }, b"U3D\0 not a real model".to_vec()));
    let poster = pdf.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()] }, b"0 0 1 rg 0 0 100 100 re f".to_vec()));
    let page = *pdf.get_pages().get(&1).unwrap();
    let annot = pdf.add_object(dictionary! { "Type" => "Annot", "Subtype" => "3D", "Rect" => vec![0.into(), 0.into(), 100.into(), 100.into()], "F" => 4,
        "3DD" => model, "AP" => dictionary! { "N" => poster }, "P" => page });
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Annots", vec![Object::Reference(annot)]);
    let with_3d = t.path().join("model.pdf");
    pdf.save(&with_3d).unwrap();
    let report = sheaf_core::pdfe::to_pdfe(&Input::new(&with_3d), false, &t.path().join("model-e.pdf")).unwrap();
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let out = lopdf::Document::load(&report.path).unwrap();
    let kept = out.get_page_annotations(*out.get_pages().get(&1).unwrap()).unwrap().into_iter()
        .find(|a| a.get(b"Subtype").and_then(Object::as_name).is_ok_and(|n| n == b"3D")).expect("the 3D annotation is kept");
    let stream = out.dereference(kept.get(b"3DD").unwrap()).unwrap().1.as_stream().unwrap().clone();
    assert!(stream.get_plain_content().unwrap().starts_with(b"U3D"));
    assert!(!String::from_utf8_lossy(&std::fs::read(&report.path).unwrap()).contains("SheafHeld3D"));
}
