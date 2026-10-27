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

/// A text box another program wrote, in font `font` (named `/Own` by its `/DA` and appearance,
/// with `to_unicode` as its `/ToUnicode` CMap when given), showing `contents` with the appearance's
/// `shown` string; saved to `path`.
fn text_box_in_own_font(path: &std::path::Path, mut font: lopdf::Dictionary, to_unicode: Option<Vec<u8>>, contents: &str, shown: &str) {
    let mut doc = lopdf::Document::with_version("1.7");
    if let Some(cmap) = to_unicode {
        font.set("ToUnicode", doc.add_object(Stream::new(dictionary! {}, cmap)));
    }
    let font = doc.add_object(font);
    let rect = || vec![72.into(), 700.into(), 272.into(), 730.into()];
    let ap = doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => rect(), "Resources" => dictionary! { "Font" => dictionary! { "Own" => font } } },
        format!("BT /Own 12 Tf 74 712 Td {shown} Tj ET").into_bytes(),
    ));
    let mut text = vec![0xFE, 0xFF];
    text.extend(contents.encode_utf16().flat_map(u16::to_be_bytes));
    let annot = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "FreeText", "Rect" => rect(), "Contents" => Object::String(text, lopdf::StringFormat::Hexadecimal),
        "DA" => Object::string_literal("0 0 0 rg /Own 12 Tf"), "AP" => dictionary! { "N" => ap },
    });
    let pages = doc.new_object_id();
    let content = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Contents" => content, "Annots" => vec![annot.into()] });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    doc.save(path).unwrap();
}

/// The first annotation's normal appearance in `path`: (its content, its font `/Own`'s `/BaseFont`).
fn own_font_appearance(path: &std::path::Path) -> (String, String) {
    let doc = lopdf::Document::load(path).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let annots = doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
    let annot = doc.get_dictionary(annots[0].as_reference().unwrap()).unwrap();
    let ap = annot.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
    let Object::Stream(s) = doc.get_object(ap).unwrap() else { panic!("no appearance stream") };
    let content = String::from_utf8_lossy(&s.decompressed_content().unwrap_or_else(|_| s.content.clone())).into_owned();
    let fonts = s.dict.get(b"Resources").and_then(Object::as_dict).unwrap().get(b"Font").and_then(Object::as_dict).unwrap();
    let own = fonts.get(b"Own").unwrap();
    let own = match own {
        Object::Reference(r) => doc.get_dictionary(*r).unwrap(),
        o => o.as_dict().unwrap(),
    };
    (content, String::from_utf8_lossy(own.get(b"BaseFont").unwrap().as_name().unwrap()).into_owned())
}

/// A text box in an embedded Type0 font (Japanese, as Acrobat writes it) or a simple font with its
/// own `/Differences` (Greek) keeps that font through a new size and a new colour: the text is
/// written in the font's codes (Sol, 26/10: it was refused or redrawn in Helvetica).
#[test]
fn a_text_box_in_a_type0_or_custom_encoded_font_keeps_it_through_a_new_size_and_colour() {
    let t = tempfile::tempdir().unwrap();
    let cmap = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CMapName /Adobe-Identity-UCS def /CMapType 2 def\n1 begincodespacerange <0000> <FFFF> endcodespacerange\n3 beginbfchar <0001> <65E5> <0002> <672C> <0003> <8A9E> endbfchar\nendcmap CMapName currentdict /CMap defineresource pop end end".to_vec();
    let cid = dictionary! {
        "Type" => "Font", "Subtype" => "CIDFontType2", "BaseFont" => "ABCDEF+KozMin",
        "CIDSystemInfo" => dictionary! { "Registry" => Object::string_literal("Adobe"), "Ordering" => Object::string_literal("Identity"), "Supplement" => 0 },
        "DW" => 1000, "W" => vec![1.into(), vec![1000.into(), 1000.into(), 1000.into()].into()],
    };
    let type0 = dictionary! {
        "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "ABCDEF+KozMin", "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Dictionary(cid)],
    };
    let greek = dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "GHIJKL+Greek", "FirstChar" => 65, "Widths" => vec![700.into(), 650.into()],
        "Encoding" => dictionary! { "Type" => "Encoding", "Differences" => vec![65.into(), Object::Name(b"uni0391".to_vec()), Object::Name(b"uni0392".to_vec())] },
    };
    for (font, to_unicode, contents, shown, base, codes) in [
        (type0, Some(cmap), "日本語", "<000100020003>", "ABCDEF+KozMin", "<000100020003>"),
        (greek, None, "ΑΒ", "(AB)", "GHIJKL+Greek", "<4142>"),
    ] {
        let path = t.path().join(format!("{base}.pdf"));
        text_box_in_own_font(&path, font, to_unicode, contents, shown);
        let mut d = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(&path)).unwrap();
        // A new size (the format row passes the box's own font on as ""), then a new colour.
        d.apply(Edit::SetTextFont { page: 0, index: 0, font: String::new(), size: Some(18.0), align: None }).unwrap();
        d.apply(Edit::StyleAnnotation { page: 0, index: 0, color: Some([0.8, 0.1, 0.1]), opacity: None, width: None, endings: None }).unwrap();
        let out = d.save(&t.path().join(format!("{base}-out.pdf"))).unwrap();
        let (content, font) = own_font_appearance(&out);
        assert_eq!(font, base, "drawn in the file's font");
        assert!(content.contains("/Own 18 Tf") && content.contains("0.8 0.1 0.1 rg"), "new size and colour: {content}");
        assert!(content.contains(codes) && !content.contains('?'), "written in the font's codes: {content}");
    }
}
