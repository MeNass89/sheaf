use lopdf::{dictionary, Object, Stream};
use sheaf_core::{pages::Input, pdfcraft::Doc, preflight};

#[test]
fn print_preflight_reports_low_resolution_and_keeps_the_document_unchanged() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("press.pdf");
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let image = pdf.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1, "BitsPerComponent" => 8, "ColorSpace" => "DeviceRGB" }, vec![255, 0, 0]));
    let content = pdf.add_object(Stream::new(dictionary! {}, b"q 144 0 0 144 20 20 cm /Im Do Q".to_vec()));
    let page = pdf.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 300.into(), 400.into()],
        "TrimBox" => vec![10.into(), 10.into(), 290.into(), 390.into()],
        "BleedBox" => vec![9.into(), 9.into(), 291.into(), 391.into()],
        "Contents" => content, "Resources" => dictionary! { "XObject" => dictionary! { "Im" => image } }
    });
    pdf.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()] }));
    let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    pdf.trailer.set("Root", root);
    pdf.save(&source).unwrap();
    let bytes = std::fs::read(&source).unwrap();
    let doc = Doc::open(&Input::new(&source)).unwrap();
    let before = doc.full_bytes().unwrap();
    let readiness = doc.document().print_readiness();
    assert!(readiness.findings.iter().any(|f| f.id == "image-ppi" && f.severity == "fail"));
    assert!(readiness.findings.iter().any(|f| f.id == "page-bleed" && f.page == Some(1)));
    let text = preflight::report(&doc, "press.pdf", "2026-10-27 10:30 UTC");
    assert!(text.contains("press.pdf") && text.contains("2026-10-27 10:30 UTC"));
    assert!(text.contains("150 ppi") && text.contains("300 ppi"));
    assert!(text.contains("Not checked: PDF/X, overprint"));
    let out = t.path().join("report.txt");
    std::fs::write(&out, "earlier report").unwrap();
    let written = preflight::save_report(&doc, "press.pdf", "now", &out).unwrap();
    assert_ne!(written, out);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "earlier report");
    assert!(std::fs::read_to_string(written).unwrap().contains("below 150"));
    assert_eq!(doc.full_bytes().unwrap().as_slice(), before.as_slice());
    assert_eq!(std::fs::read(source).unwrap(), bytes);
}
