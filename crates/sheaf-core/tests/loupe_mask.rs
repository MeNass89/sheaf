//! Preview's Loupe and Mask (Tools ▸ Annotate): pdfcraft-annot's `loupe_mask.rs`, through the
//! engine's `Edit::AddLoupe` and `Edit::AddMask`.

mod common;
use lopdf::{dictionary, Object, Stream};
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::engine::Edit;
use sheaf_core::pdfcraft::Doc;
use std::path::{Path, PathBuf};

/// A Letter page drawing `content`.
fn page(dir: &Path, content: &[u8]) -> PathBuf {
    let path = dir.join("page.pdf");
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let content = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Contents" => content, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()] });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    doc.save(&path).unwrap();
    path
}

/// Dark pixels (red < 100) in the page box x0..x1, y0..y1 (points, y up) once saved and drawn.
fn dark(doc: &Doc, dir: &Path, [x0, y0, x1, y1]: [f64; 4]) -> usize {
    let out = doc.save(&dir.join("drawn.pdf")).unwrap();
    let img = common::render(&out, 0);
    common::dark_in(&img, (x0 / 612.0) as f32, (x1 / 612.0) as f32, ((792.0 - y1) / 792.0) as f32, ((792.0 - y0) / 792.0) as f32)
}

fn red_at(doc: &Doc, dir: &Path, x: usize, y: usize) -> u8 {
    let out = doc.save(&dir.join("red.pdf")).unwrap();
    let (w, h, px) = common::render(&out, 0);
    px[((h as usize - y) * w as usize + x) * 4]
}

/// A loupe over a small black square shows it three times as large, inside its circle; the rest
/// of the page is as it was, and the loupe is a comment ("Loupe") saved in the file.
#[test]
fn a_loupe_shows_the_page_under_it_magnified() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = page(dir.path(), b"0 g 300 400 10 10 re f");
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    let around = [255.0, 355.0, 355.0, 455.0];
    let before = dark(&doc, dir.path(), around);
    assert!((80..=120).contains(&before), "the square alone: {before}");
    doc.apply(Edit::AddLoupe { page: 0, centre: [305.0, 405.0], radius: 45.0, magnify: 3.0, color: [1.0, 0.0, 0.0], author: "Ada".into() }).unwrap();
    let after = dark(&doc, dir.path(), around);
    assert!(after > 700, "magnified three times: {after}");
    assert_eq!(dark(&doc, dir.path(), [0.0, 0.0, 200.0, 200.0]), 0, "nothing else changes");
    let list = &doc.document().info.annotations;
    assert!(list.iter().any(|a| a.contents.as_deref() == Some("Loupe")), "a comment of its own");
    common::qpdf_clean(&doc.save(&dir.path().join("loupe.pdf")).unwrap());
}

/// A mask dims the page outside its opening, and leaves the opening as it was.
#[test]
fn a_mask_dims_the_page_outside_its_opening() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = page(dir.path(), b"");
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    doc.apply(Edit::AddMask { page: 0, hole: [100.0, 100.0, 300.0, 300.0], oval: false, author: String::new() }).unwrap();
    let outside = red_at(&doc, dir.path(), 50, 50);
    let inside = red_at(&doc, dir.path(), 200, 200);
    assert!((60..=140).contains(&outside), "dimmed outside: {outside}");
    assert!(inside > 240, "the opening stays white: {inside}");
    common::qpdf_clean(&doc.save(&dir.path().join("mask.pdf")).unwrap());
    // An oval opening: its corner is dimmed, its middle is not.
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    doc.apply(Edit::AddMask { page: 0, hole: [350.0, 350.0, 550.0, 550.0], oval: true, author: String::new() }).unwrap();
    assert!(red_at(&doc, dir.path(), 355, 355) < 200, "an oval's corner is outside it");
    assert!(red_at(&doc, dir.path(), 450, 450) > 240, "its middle is open");
}
