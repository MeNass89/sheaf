mod common;
use common::fixture_pdf;
use sheaf_core::{facts, pages::Input, pdfa::{to_pdfa, PdfALevel}, security::*};

#[test]
fn properties_read_restrictions_pdfa_and_dates_from_the_file() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let plain = facts::read(&std::fs::read(&a).unwrap(), None);
    assert!(plain.restricted.is_empty() && plain.pdfa.is_none(), "{plain:?}");
    // An ordinary page: its media and crop boxes.
    assert_eq!(plain.boxes.iter().map(|b| b.0).collect::<Vec<_>>(), ["Media box", "Crop box"]);
    // Its one font, one of the standard fourteen: not embedded.
    assert_eq!(plain.fonts, [("Helvetica".to_string(), false)]);

    let perms = ProtectPermissions { printing: false, copying: false, ..Default::default() };
    let p = protect_with_permissions(&Input::new(&a), "user", Some("owner"), perms, &t.path().join("p.pdf")).unwrap();
    let f = facts::read(&std::fs::read(&p).unwrap(), Some("user"));
    assert_eq!(f.restricted, ["Printing", "Copying text and images"]);
    // No open password, only an owner one: read as it opens, restrictions kept.
    let q = t.path().join("q.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "", "owner", "256", "--print=none", "--"]).arg(&a).arg(&q).status().unwrap().success());
    assert_eq!(facts::read(&std::fs::read(&q).unwrap(), None).restricted, ["Printing"]);

    let pa = to_pdfa(&Input::new(&a), PdfALevel::A2b, &t.path().join("pa.pdf")).unwrap().path;
    let f = facts::read(&std::fs::read(&pa).unwrap(), None);
    assert_eq!(f.pdfa.as_deref(), Some("PDF/A-2b"));
    assert!(f.modified.is_some() || f.created.is_some(), "{f:?}");
}

#[test]
fn custom_properties_are_read_past_the_standard_ones() {
    let t = tempfile::tempdir().unwrap();
    let a = common::fixture_pdf(t.path(), "a.pdf", 1);
    let mut d = lopdf::Document::load(&a).unwrap();
    let info = d.add_object(lopdf::dictionary! { "Title" => lopdf::Object::string_literal("T"), "Client" => lopdf::Object::string_literal("Acme") });
    d.trailer.set("Info", info);
    d.save(&a).unwrap();
    let f = sheaf_core::facts::read(&std::fs::read(&a).unwrap(), None);
    assert_eq!(f.custom, [("Client".to_string(), "Acme".to_string())]);
}

/// Acrobat's Audit Space Usage: a page with a picture spends most of its bytes on the image, and
/// the parts add up to the file.
#[test]
fn the_space_a_file_uses_is_told_by_what_takes_it() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("p.png");
    image::RgbImage::from_fn(400, 300, |x, y| image::Rgb([(x * 7 % 251) as u8, (y * 13 % 241) as u8, ((x ^ y) % 239) as u8])).save(&png).unwrap();
    let pdf = sheaf_core::images::images_to_pdf(&[png], sheaf_core::images::PageLayout::FitImage, &t.path().join("p.pdf")).unwrap();
    let bytes = std::fs::read(&pdf).unwrap();
    let doc = lopdf::Document::load_mem(&bytes).unwrap();
    let space = sheaf_core::facts::space_of(&doc, bytes.len() as u64);
    assert_eq!(space[0].0, "Images", "{space:?}");
    assert_eq!(space.iter().map(|s| s.1).sum::<u64>(), bytes.len() as u64);
}
