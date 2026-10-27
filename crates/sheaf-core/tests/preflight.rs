use lopdf::{dictionary, Object, Stream};
use sheaf_core::{pages::Input, pdfcraft::Doc, preflight};

#[test]
fn print_preflight_reports_low_resolution_and_keeps_the_document_unchanged() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("press.pdf");
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let image = pdf.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 144, "Height" => 144, "BitsPerComponent" => 8, "ColorSpace" => "DeviceRGB" }, vec![128; 144 * 144 * 3]));
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
    let large_format = preflight::Profile { fail_ppi: 50.0, warn_ppi: 60.0, min_bleed_mm: 0.0 };
    let custom = doc.document().print_readiness_with(large_format);
    assert!(!custom.findings.iter().any(|f| f.id == "image-ppi" || f.id == "page-bleed"));
    let audit = preflight::report_with_profile(&doc, "press.pdf", "earlier", large_format).unwrap();
    assert!(audit.contains("fail below 50 ppi; warn below 60 ppi"));
    assert!(!audit.contains("below 150"));
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

#[test]
fn preflight_profiles_reject_thresholds_that_would_hide_failures() {
    for profile in [
        preflight::Profile { fail_ppi: f64::NAN, ..Default::default() },
        preflight::Profile { warn_ppi: 100.0, ..Default::default() },
        preflight::Profile { min_bleed_mm: -1.0, ..Default::default() },
        preflight::Profile { warn_ppi: f64::INFINITY, ..Default::default() },
    ] {
        assert!(preflight::validate(profile).is_err());
    }
    assert!(preflight::validate(Default::default()).is_ok());
}

#[test]
fn preflight_inventories_nested_press_resources_without_claiming_ink_simulation() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("resources.pdf");
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let colour = pdf.add_object(Object::Array(vec![Object::Name(b"Separation".to_vec()), Object::Name(b"BrandOrange".to_vec()), Object::Name(b"DeviceCMYK".to_vec()), dictionary! { "FunctionType" => 2, "Domain" => vec![0.into(), 1.into()], "C0" => vec![0.into(), 0.into(), 0.into(), 0.into()], "C1" => vec![0.into(), 0.5.into(), 1.into(), 0.into()], "N" => 1 }.into()]));
    let group = pdf.add_object(Stream::new(dictionary! {
        "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
        "Group" => dictionary! { "S" => "Transparency" },
        "Resources" => dictionary! { "ColorSpace" => dictionary! { "Brand" => colour }, "ExtGState" => dictionary! { "Press" => dictionary! { "OP" => true, "ca" => 0.5 } } }
    }, Vec::new()));
    let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 300.into(), 400.into()], "Resources" => dictionary! { "XObject" => dictionary! { "Group" => group } } });
    let plain = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 300.into(), 400.into()], "Resources" => dictionary! {} });
    pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 2, "Kids" => vec![page.into(), plain.into()] }.into());
    let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    pdf.trailer.set("Root", root);
    pdf.save(&source).unwrap();
    let doc = Doc::open(&Input::new(&source)).unwrap();
    let before = doc.full_bytes().unwrap();
    let readiness = doc.document().print_readiness();
    for id in ["spot-colour", "overprint", "transparency"] {
        assert!(readiness.findings.iter().any(|f| f.id == id && f.page == Some(1)), "{id}");
        assert!(!readiness.findings.iter().any(|f| f.id == id && f.page == Some(2)), "{id} leaked to the plain page");
    }
    assert!(readiness.findings.iter().any(|f| f.id == "spot-colour" && f.message.contains("BrandOrange") && f.message.contains("unused")));
    let report = preflight::report(&doc, "resources.pdf", "2026-10-27 UTC");
    assert!(report.contains("Profile: Close-up print"));
    assert!(report.contains("overprint simulation") && report.contains("separations preview"));
    assert_eq!(doc.full_bytes().unwrap().as_slice(), before.as_slice());
    let report = preflight::report_with_profile(&doc, "resources.pdf", "now", preflight::Profile { fail_ppi: 72.0, warn_ppi: 150.0, min_bleed_mm: 3.0 }).unwrap();
    assert!(report.contains("Profile: Large format"));
}
