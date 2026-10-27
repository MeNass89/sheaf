//! Sheaf's own pieces of PdfCraft (third_party/pdfcraft-engine), whose crate tests are left out.

mod common;
use lopdf::{dictionary, Object, Stream};
use sheaf_core::pdfcraft::engine::{copy_image, sfn, Edit};

#[test]
fn notation_reads_each_field_as_a_number() {
    assert_eq!(
        sfn::notation_js("(Price + Tax\\ rate) * Qty.1"),
        "/** BVCALC (Price + Tax\\ rate) * Qty.1 EVCALC **/ event.value = (AFMakeNumber(getField(\"Price\").value) + AFMakeNumber(getField(\"Tax rate\").value)) * AFMakeNumber(getField(\"Qty.1\").value);"
    );
}

#[test]
fn a_drawing_beside_a_picture_is_told_from_the_picture_alone() {
    assert!(copy_image::paints_paths(b"q 100 0 0 50 0 0 cm /Im0 Do Q 1 0 0 RG 2 w 0 0 m 100 50 l S"));
    assert!(!copy_image::paints_paths(b"q 100 0 0 50 0 0 cm /Im0 Do Q"));
    // Text with "S" in its strings, or an inline image's bytes, is no path.
    assert!(!copy_image::paints_paths(b"BT /F1 12 Tf (S f) Tj ET BI /W 1 /H 1 ID f S EI"));
    assert!(!copy_image::paints_paths(b"0 0 10 10 re W n /Im0 Do"));
}

/// Remove all images on a page whose picture sits in a group with text (Acrobat's): the picture
/// goes, the text stays (Sol, 27/10: such groups kept their pictures).
#[test]
fn a_groups_picture_goes_and_its_text_stays() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("group.pdf");
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let image = doc.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8 }, vec![0u8]));
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let group = doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => image }, "Font" => dictionary! { "F1" => font } } },
        b"q 100 0 0 100 72 560 cm /Im0 Do Q BT /F1 14 Tf 72 500 Td (Kept text) Tj ET".to_vec(),
    ));
    let content = doc.add_object(Stream::new(dictionary! {}, b"/Fm0 Do".to_vec()));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Contents" => content, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Resources" => dictionary! { "XObject" => dictionary! { "Fm0" => group } } });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    doc.save(&path).unwrap();
    // The picture: 72..172 across, 560..660 up, on a 612 × 792 page.
    let picture = |p: &std::path::Path| common::dark_in(&common::render(p, 0), 0.13, 0.27, 0.18, 0.28);
    assert!(picture(&path) > 100, "the black picture shows first");
    let mut d = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(&path)).unwrap();
    d.apply(Edit::RemoveImages { pages: vec![0] }).unwrap();
    let out = d.save(&t.path().join("out.pdf")).unwrap();
    assert_eq!(picture(&out), 0, "its picture went");
    assert!(common::shown_texts(&out)[0].contains("Kept text"), "its text stayed");
}

#[test]
fn an_offline_ocr_adapter_keeps_skips_cancellation_and_unicode_placement() {
    use sheaf_core::pdfcraft::engine::ocr::{Line, OcrSettings, Word};
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let source = common::fixture_pdf(t.path(), "ocr-source.pdf", 1);
    let mut doc = Doc::open(&Input::new(source)).unwrap();
    let id = doc.id();
    let skipped = doc.session().ocr_job(id, &[], OcrSettings::default()).unwrap().run_with(|_, _, _| {
        Err(sheaf_core::pdfcraft::engine::ocr::OcrError::Recognize("must not run on a skipped page".into()))
    }, |_, _| true);
    assert_eq!(skipped[0].skipped.as_deref(), Some("the page already contains text"));
    let settings = OcrSettings { dpi: 72.0, skip_text_pages: false, language: "ja".into() };
    let cancelled = doc.session().ocr_job(id, &[], settings.clone()).unwrap().run_with(|_, _, _| Ok(vec![]), |_, _| false);
    assert!(cancelled.is_empty());
    let found = doc.session().ocr_job(id, &[], settings).unwrap().run_with(|_, _, _| {
        Ok(vec![Line { words: vec![Word { text: "日本語".into(), rect: [20.0, 50.0, 120.0, 75.0], ..Default::default() }] }])
    }, |_, _| true);
    assert_eq!(found[0].words.len(), 1);
    assert_eq!(doc.session_mut().apply_ocr(id, &found).unwrap(), 1);
    let out = doc.save(&t.path().join("with-japanese.pdf")).unwrap();
    let text = common::shown_texts(&out).join(" ");
    assert!(text.contains("日本語") && text.contains("Page 1"), "{text}");
}
