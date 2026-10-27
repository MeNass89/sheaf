//! The Edit tool's Arrange (Acrobat's Bring to Front, Bring Forward, Send Backward, Send to Back):
//! pdfcraft-edit's `restack.rs`, through the engine's `Edit::RestackObjects`.

mod common;
use lopdf::{dictionary, Object, Stream};
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::edit::Restack;
use sheaf_core::pdfcraft::engine::{Edit, ObjectKind, ObjectTarget};
use sheaf_core::pdfcraft::Doc;
use std::path::{Path, PathBuf};

/// A Letter page drawing `content`, with Helvetica as F1.
fn page(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let content = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "Contents" => content, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
    });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    doc.save(&path).unwrap();
    path
}

/// The colour (r, g, b) at page point (x, y) once saved and drawn.
fn colour_at(doc: &Doc, dir: &Path, x: usize, y: usize) -> [u8; 3] {
    let out = doc.save(&dir.join("arranged.pdf")).unwrap();
    let (w, h, px) = common::render(&out, 0);
    let i = ((h as usize - y) * w as usize + x) * 4;
    [px[i], px[i + 1], px[i + 2]]
}

fn path_at(doc: &Doc, x0: f64) -> ObjectTarget {
    let objects = doc.document().editable_objects(0).unwrap();
    objects.iter().find(|o| o.target.kind == ObjectKind::Path && (o.rect[0] - x0).abs() < 0.5).unwrap_or_else(|| panic!("a drawing at {x0}: {objects:?}")).target
}

const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];

/// A red square under a blue one: brought to the front it shows over the blue, sent to the back
/// it goes under again; forward and backward step past the one it overlaps. Each is one undo step
/// and the objects keep their places.
#[test]
fn a_drawing_is_brought_to_the_front_and_sent_to_the_back() {
    let dir = tempfile::tempdir().unwrap();
    // A line width and a dash set before the squares, so the red one is drawn again with them.
    let pdf = page(dir.path(), "squares.pdf", b"3 w [4 2] 0 d 1 0 0 rg 100 100 100 100 re f 0 0 1 rg 150 150 100 100 re f");
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    let before = doc.document().editable_objects(0).unwrap();
    assert_eq!(colour_at(&doc, dir.path(), 175, 175), BLUE, "blue on top at first");
    doc.apply(Edit::RestackObjects { page: 0, objects: vec![path_at(&doc, 100.0)], how: Restack::Front }).unwrap();
    assert_eq!(colour_at(&doc, dir.path(), 175, 175), RED, "red brought to the front");
    assert_eq!(colour_at(&doc, dir.path(), 225, 225), BLUE, "the blue's own corner stays blue");
    assert_eq!(colour_at(&doc, dir.path(), 125, 125), RED, "the red's own corner stays red");
    let mut after: Vec<_> = doc.document().editable_objects(0).unwrap().iter().map(|o| o.rect).collect();
    let mut rects: Vec<_> = before.iter().map(|o| o.rect).collect();
    after.sort_by(|a, b| a.partial_cmp(b).unwrap());
    rects.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(after, rects, "nothing moved");
    doc.apply(Edit::RestackObjects { page: 0, objects: vec![path_at(&doc, 100.0)], how: Restack::Back }).unwrap();
    assert_eq!(colour_at(&doc, dir.path(), 175, 175), BLUE, "red sent to the back");
    doc.apply(Edit::RestackObjects { page: 0, objects: vec![path_at(&doc, 100.0)], how: Restack::Forward }).unwrap();
    assert_eq!(colour_at(&doc, dir.path(), 175, 175), RED, "red one step forward, past the blue");
    // Nothing above it overlaps it any more: refused, the page as it was.
    assert!(doc.apply(Edit::RestackObjects { page: 0, objects: vec![path_at(&doc, 100.0)], how: Restack::Forward }).is_err());
    doc.apply(Edit::RestackObjects { page: 0, objects: vec![path_at(&doc, 100.0)], how: Restack::Backward }).unwrap();
    assert_eq!(colour_at(&doc, dir.path(), 175, 175), BLUE, "red one step backward");
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    assert_eq!(colour_at(&doc, dir.path(), 175, 175), RED, "one undo per step");
    let out = doc.save(&dir.path().join("checked.pdf")).unwrap();
    common::qpdf_clean(&out);
}

/// The blue square is drawn inside a clipped group: the red one can't be drawn after it inside
/// that clip, so the blue one is moved under the red instead; the clip still cuts the blue.
#[test]
fn a_drawing_steps_past_one_drawn_in_a_clipped_group() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = page(dir.path(), "clipped.pdf", b"1 0 0 rg 100 100 100 100 re f q 0 0 230 230 re W n 0 0 1 rg 150 150 100 100 re f Q");
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    assert_eq!(colour_at(&doc, dir.path(), 175, 175), BLUE);
    doc.apply(Edit::RestackObjects { page: 0, objects: vec![path_at(&doc, 100.0)], how: Restack::Forward }).unwrap();
    assert_eq!(colour_at(&doc, dir.path(), 175, 175), RED, "red over blue");
    assert_eq!(colour_at(&doc, dir.path(), 240, 240), [255, 255, 255], "the blue is still clipped at 230");
    assert_eq!(colour_at(&doc, dir.path(), 220, 220), BLUE);
}

/// White words under a black box: brought to the front they show on it; the box behind them
/// keeps its place, and the words stay where they were.
#[test]
fn words_are_brought_in_front_of_a_box() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = page(dir.path(), "words.pdf", b"BT 1 g /F1 30 Tf 72 700 Td (Hidden words) Tj ET 0 g 60 680 300 60 re f");
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    let words = |doc: &Doc| doc.document().editable_objects(0).unwrap().into_iter().find(|o| o.target.kind == ObjectKind::Text).expect("the words");
    let light = |doc: &Doc| {
        let out = doc.save(&dir.path().join("words-out.pdf")).unwrap();
        let img = common::render(&out, 0);
        // Inside the box (x 60–360, y 680–740 on the page) by 2 points, as fractions of the
        // image, y down: its light pixels.
        let area = 296 * 56;
        area - common::dark_in(&img, 62.0 / 612.0, 358.0 / 612.0, 54.0 / 792.0, 110.0 / 792.0).min(area)
    };
    assert!(light(&doc) < 50, "the words are hidden at first");
    let before = words(&doc);
    doc.apply(Edit::RestackObjects { page: 0, objects: vec![before.target], how: Restack::Front }).unwrap();
    assert!(light(&doc) > 300, "the words show on the box");
    let after = words(&doc);
    assert_eq!(after.text, before.text);
    assert!(after.rect.iter().zip(before.rect).all(|(a, b)| (a - b).abs() < 0.5), "{:?} → {:?}", before.rect, after.rect);
    doc.apply(Edit::RestackObjects { page: 0, objects: vec![after.target], how: Restack::Backward }).unwrap();
    assert!(light(&doc) < 50, "one step backward: under the box again");
}
