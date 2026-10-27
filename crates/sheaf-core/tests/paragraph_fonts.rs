//! A paragraph edited with the Edit tool set in a font of the computer's (Acrobat's Edit PDF font
//! list: `sheaf_core::system_fonts`, the engine's `Edit::EditTextBlockInFont`), underlined.

mod common;
use lopdf::{dictionary, Object, Stream};
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::edit::BlockStyle;
use sheaf_core::pdfcraft::engine::{Edit, ObjectKind};
use sheaf_core::pdfcraft::Doc;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Great Vibes (SIL OFL, its name table says so), bundled for typed signatures.
fn great_vibes() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/GreatVibes-Regular.ttf")
}

/// The fonts of a folder that may be embedded are listed by name; a file that is no font is not.
#[test]
fn the_computers_open_fonts_are_listed_by_name() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("truetype/greatvibes")).unwrap();
    std::fs::copy(great_vibes(), dir.path().join("truetype/greatvibes/GreatVibes-Regular.ttf")).unwrap();
    std::fs::write(dir.path().join("broken.ttf"), b"not a font").unwrap();
    let fonts = sheaf_core::system_fonts::list_fonts(&[dir.path().to_path_buf()]);
    assert_eq!(fonts.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Great Vibes"], "{fonts:?}");
    assert!(fonts[0].path.ends_with("GreatVibes-Regular.ttf"));
}

/// A Helvetica paragraph set in Great Vibes and underlined: the font is embedded and named on the
/// page, the text reads back, the underline is a drawing of its own; one undo step.
#[test]
fn a_paragraph_is_set_in_a_font_of_the_computers_and_underlined() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("para.pdf");
    let mut d = lopdf::Document::with_version("1.7");
    let pages = d.new_object_id();
    let font = d.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let content = d.add_object(Stream::new(dictionary! {}, b"BT /F1 14 Tf 72 700 Td (A paragraph in Helvetica) Tj ET".to_vec()));
    let page = d.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "Contents" => content, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
    });
    d.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    d.trailer.set("Root", catalog);
    d.save(&path).unwrap();
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    let bytes = Arc::new(std::fs::read(great_vibes()).unwrap());
    let style = BlockStyle { underline: Some(true), ..Default::default() };
    doc.apply(Edit::EditTextBlockInFont { page: 0, block: 0, text: "A paragraph in Great Vibes".into(), style, font: bytes }).unwrap();
    let paths = doc.document().editable_objects(0).unwrap().iter().filter(|o| o.target.kind == ObjectKind::Path).count();
    assert_eq!(paths, 1, "the underline");
    let out = doc.save(&dir.path().join("out.pdf")).unwrap();
    let saved = lopdf::Document::load(&out).unwrap();
    let bases: Vec<String> = saved
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter_map(|d| d.get(b"BaseFont").ok().and_then(|b| b.as_name().ok()).map(|b| String::from_utf8_lossy(b).into_owned()))
        .collect();
    assert!(bases.iter().any(|b| b.contains("GreatVibes")), "{bases:?}");
    assert!(common::shown_texts(&out)[0].contains("Great Vibes"), "{:?}", common::shown_texts(&out));
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    assert!(doc.document().editable_objects(0).unwrap().iter().all(|o| o.target.kind != ObjectKind::Path), "one undo takes it all back");
}
