//! Sheaf's own pieces of PdfCraft (third_party/pdfcraft-engine), whose crate tests are left out.

mod common;
use lopdf::{dictionary, Object, Stream};
use sheaf_core::pdfcraft::engine::{copy_image, sfn, Edit};

fn prepress_pdf(dir: &std::path::Path, contents: &[&[u8]]) -> std::path::PathBuf {
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let streams: Vec<Object> = contents.iter().map(|c| Object::Reference(pdf.add_object(Stream::new(dictionary! {}, c.to_vec())))).collect();
    let page = pdf.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "Contents" => streams,
        "TrimBox" => vec![30.into(), 40.into(), 290.into(), 400.into()],
        "BleedBox" => vec![25.into(), 35.into(), 295.into(), 405.into()],
        "Resources" => dictionary! { "ExtGState" => dictionary! { "Thin" => dictionary! { "LW" => 0.1 } } }
    });
    pdf.objects.insert(pages, Object::Dictionary(dictionary! {
        "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()],
        "MediaBox" => vec![10.into(), 20.into(), 310.into(), 420.into()],
        "CropBox" => vec![20.into(), 30.into(), 300.into(), 410.into()]
    }));
    let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    pdf.trailer.set("Root", root);
    let path = dir.join("prepress.pdf");
    pdf.save(&path).unwrap();
    path
}

#[test]
fn page_hairlines_are_measured_after_transforms_and_undone_in_one_step() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    // A path and its graphics state cross a content-stream boundary.
    let path = prepress_pdf(t.path(), &[
        b"q 0.2 w 0.5 0 0 0.5 0 0 cm 30 50 m",
        b"100 50 l S Q 0 w 30 60 m 100 60 l S 2 w 30 70 m 100 70 l S q /Thin gs 30 80 m 100 80 l S Q",
    ]);
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    doc.apply(Edit::FixPageHairlines { pages: vec![0], minimum: 0.25 }).unwrap();
    let out = doc.save(&t.path().join("fixed.pdf")).unwrap();
    let pdf = lopdf::Document::load(&out).unwrap();
    let ops = lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1])).unwrap().operations;
    let strokes: Vec<usize> = ops.iter().enumerate().filter_map(|(i, op)| (op.operator == "S").then_some(i)).collect();
    assert_eq!(strokes.len(), 4);
    let width = |i: usize| ops[i].operands[0].as_float().unwrap();
    assert_eq!(ops[strokes[0] - 1].operator, "w");
    assert!((width(strokes[0] - 1) - 0.5).abs() < 1e-6, "0.5 local points makes 0.25 on the page");
    assert!((width(strokes[0] + 1) - 0.2).abs() < 1e-6, "the original width is restored");
    assert!((width(strokes[1] - 1) - 0.25).abs() < 1e-6, "a zero-width hairline is made printable");
    assert_eq!(ops[strokes[2] - 1].operator, "l", "a thick stroke was not touched");
    assert!((width(strokes[3] - 1) - 0.25).abs() < 1e-6, "ExtGState line widths are read too");
    common::qpdf_clean(&out);
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    let undone = doc.save(&t.path().join("undone.pdf")).unwrap();
    let original = lopdf::Document::load(&path).unwrap();
    let undone = lopdf::Document::load(undone).unwrap();
    assert_eq!(undone.get_page_content(undone.get_pages()[&1]), original.get_page_content(original.get_pages()[&1]));
    assert!(doc.apply(Edit::FixPageHairlines { pages: vec![0], minimum: f64::NAN }).is_err());
}

#[test]
fn page_hairline_repairs_honour_user_units_and_nonuniform_scaling() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"0.1 w 2 0 0 0.5 0 0 cm 30 50 m 100 50 l S"]);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let page = pdf.get_pages()[&1];
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("UserUnit", 2);
    pdf.save(&path).unwrap();
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    doc.apply(Edit::FixPageHairlines { pages: vec![0], minimum: 0.25 }).unwrap();
    let out = doc.save(&t.path().join("scaled.pdf")).unwrap();
    let pdf = lopdf::Document::load(out).unwrap();
    let ops = lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1])).unwrap().operations;
    let paint = ops.iter().position(|o| o.operator == "S").unwrap();
    assert_eq!(ops[paint - 1].operator, "w");
    assert!((ops[paint - 1].operands[0].as_float().unwrap() - 0.25).abs() < 1e-6, "the narrow scale 0.5 times UserUnit 2 is one point per unit");
}

#[test]
fn crop_marks_keep_inherited_page_boxes_and_do_not_invent_bleed() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"0 g 0 0 500 500 re f"]);
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    doc.apply(Edit::AddCropMarks { pages: vec![0] }).unwrap();
    let out = doc.save(&t.path().join("marks.pdf")).unwrap();
    let pdf = lopdf::Document::load(&out).unwrap();
    let page = pdf.get_pages()[&1];
    let dict = pdf.get_dictionary(page).unwrap();
    let rect = |key: &[u8]| dict.get(key).unwrap().as_array().unwrap().iter().map(|v| v.as_float().unwrap() as f64).collect::<Vec<_>>();
    assert_eq!(rect(b"TrimBox"), [30.0, 40.0, 290.0, 400.0]);
    assert_eq!(rect(b"BleedBox"), [25.0, 35.0, 295.0, 405.0]);
    assert_eq!(rect(b"MediaBox"), [7.0, 17.0, 313.0, 423.0]);
    assert_eq!(rect(b"CropBox"), rect(b"MediaBox"), "the added marks are visible on the expanded sheet");
    let content = pdf.get_page_content(page);
    let text = String::from_utf8_lossy(&content);
    assert!(text.contains("20 30 280 380 re W n"), "expanding the paper cannot reveal previously cropped artwork");
    let ops = lopdf::content::Content::decode(&content).unwrap().operations;
    assert_eq!(ops.iter().filter(|o| o.operator == "S").count(), 8, "two crop marks per corner");
    assert!(text.contains("/Artifact BMC"));
    common::qpdf_clean(&out);
    doc.apply(Edit::AddCropMarks { pages: vec![0] }).unwrap();
    let again = doc.save(&t.path().join("again.pdf")).unwrap();
    let again = lopdf::Document::load(again).unwrap();
    assert_eq!(again.get_page_content(again.get_pages()[&1]), content, "repeated clicks add no duplicate marks");
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    doc.session_mut().undo(id).unwrap();
    let undone = doc.save(&t.path().join("unmarked.pdf")).unwrap();
    let undone = lopdf::Document::load(undone).unwrap();
    assert!(!undone.get_dictionary(undone.get_pages()[&1]).unwrap().has(b"SheafCropMarks"));
}

#[test]
fn print_box_margins_use_each_media_origin_and_failed_changes_leave_it_intact() {
    use sheaf_core::{pages::Input, pdfcraft::{Doc, engine::{BoxSpec, PageBox}}};
    let t = tempfile::tempdir().unwrap();
    let source = prepress_pdf(t.path(), &[b"0 g 30 50 60 70 re f"]);
    let mut doc = Doc::open(&Input::new(&source)).unwrap();
    let before = doc.document().page_boxes();
    doc.apply(Edit::SetPageBox { pages: vec![0], which: PageBox::Trim, spec: BoxSpec::Margins([5.0, 10.0, 15.0, 20.0]) }).unwrap();
    let boxes = doc.document().page_boxes();
    assert_eq!(boxes[0][3], [15.0, 30.0, 295.0, 400.0]);
    assert_eq!(boxes[0][0], before[0][0]);
    assert_eq!(boxes[0][1], before[0][1]);
    assert!(doc.apply(Edit::SetPageBox { pages: vec![0], which: PageBox::Trim, spec: BoxSpec::Margins([500.0; 4]) }).is_err());
    assert_eq!(doc.document().page_boxes(), boxes);
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    assert_eq!(doc.document().page_boxes(), before);
}

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

/// A text box another program wrote, in font `font` (named `/{name}` by its `/DA` and appearance,
/// with `to_unicode` as its `/ToUnicode` CMap when given), showing `contents` with the appearance's
/// `shown` string; saved to `path`.
fn text_box_in_own_font(path: &std::path::Path, name: &str, mut font: lopdf::Dictionary, to_unicode: Option<Vec<u8>>, contents: &str, shown: &str) {
    let mut doc = lopdf::Document::with_version("1.7");
    if let Some(cmap) = to_unicode {
        font.set("ToUnicode", doc.add_object(Stream::new(dictionary! {}, cmap)));
    }
    let font = doc.add_object(font);
    let rect = || vec![72.into(), 700.into(), 272.into(), 730.into()];
    let ap = doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => rect(), "Resources" => dictionary! { "Font" => dictionary! { name => font } } },
        format!("BT /{name} 12 Tf 74 712 Td {shown} Tj ET").into_bytes(),
    ));
    let mut text = vec![0xFE, 0xFF];
    text.extend(contents.encode_utf16().flat_map(u16::to_be_bytes));
    let annot = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "FreeText", "Rect" => rect(), "Contents" => Object::String(text, lopdf::StringFormat::Hexadecimal),
        "DA" => Object::string_literal(format!("0 0 0 rg /{name} 12 Tf")), "AP" => dictionary! { "N" => ap },
    });
    let pages = doc.new_object_id();
    let content = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Contents" => content, "Annots" => vec![annot.into()] });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    doc.save(path).unwrap();
}

/// The first annotation's normal appearance in `path`: (its content, its font `/{name}`'s `/BaseFont`).
fn own_font_appearance(path: &std::path::Path, name: &str) -> (String, String) {
    let doc = lopdf::Document::load(path).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let annots = doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
    let annot = doc.get_dictionary(annots[0].as_reference().unwrap()).unwrap();
    let ap = annot.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
    let Object::Stream(s) = doc.get_object(ap).unwrap() else { panic!("no appearance stream") };
    let content = String::from_utf8_lossy(&s.decompressed_content().unwrap_or_else(|_| s.content.clone())).into_owned();
    let fonts = s.dict.get(b"Resources").and_then(Object::as_dict).unwrap().get(b"Font").and_then(Object::as_dict).unwrap();
    let own = fonts.get(name.as_bytes()).unwrap();
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
        "DW" => 1000, "W" => vec![1.into(), Object::Array(vec![1000.into(), 1000.into(), 1000.into()])],
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
        text_box_in_own_font(&path, "Own", font, to_unicode, contents, shown);
        let mut d = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(&path)).unwrap();
        // A new size (the format row passes the box's own font on as ""), then a new colour.
        d.apply(Edit::SetTextFont { page: 0, index: 0, font: String::new(), size: Some(18.0), align: None }).unwrap();
        d.apply(Edit::StyleAnnotation { page: 0, index: 0, color: Some([0.8, 0.1, 0.1]), opacity: None, width: None, endings: None }).unwrap();
        let out = d.save(&t.path().join(format!("{base}-out.pdf"))).unwrap();
        let (content, font) = own_font_appearance(&out, "Own");
        assert_eq!(font, base, "drawn in the file's font");
        assert!(content.contains("/Own 18 Tf") && content.contains("0.8 0.1 0.1 rg"), "new size and colour: {content}");
        assert!(content.contains(codes) && !content.contains('?'), "written in the font's codes: {content}");
    }
}

/// A text box whose own font (Japanese, Type0) another program put under a standard resource
/// name, `/Helv`: recoloured, it keeps that font and its Japanese text (Sol, 27/10: it became
/// Helvetica, or the recolour was refused for the text).
#[test]
fn a_text_box_whose_own_font_is_named_helv_keeps_it_through_a_new_colour() {
    let t = tempfile::tempdir().unwrap();
    let cmap = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CMapName /Adobe-Identity-UCS def /CMapType 2 def\n1 begincodespacerange <0000> <FFFF> endcodespacerange\n3 beginbfchar <0001> <65E5> <0002> <672C> <0003> <8A9E> endbfchar\nendcmap CMapName currentdict /CMap defineresource pop end end".to_vec();
    let cid = dictionary! {
        "Type" => "Font", "Subtype" => "CIDFontType2", "BaseFont" => "ABCDEF+KozMin",
        "CIDSystemInfo" => dictionary! { "Registry" => Object::string_literal("Adobe"), "Ordering" => Object::string_literal("Identity"), "Supplement" => 0 },
        "DW" => 1000, "W" => vec![1.into(), Object::Array(vec![1000.into(), 1000.into(), 1000.into()])],
    };
    let type0 = dictionary! {
        "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "ABCDEF+KozMin", "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Dictionary(cid)],
    };
    let path = t.path().join("helv.pdf");
    text_box_in_own_font(&path, "Helv", type0, Some(cmap), "日本語", "<000100020003>");
    let mut d = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(&path)).unwrap();
    d.apply(Edit::StyleAnnotation { page: 0, index: 0, color: Some([0.8, 0.1, 0.1]), opacity: None, width: None, endings: None }).unwrap();
    let out = d.save(&t.path().join("helv-out.pdf")).unwrap();
    let (content, font) = own_font_appearance(&out, "Helv");
    assert_eq!(font, "ABCDEF+KozMin", "still the file's font, not Helvetica");
    assert!(content.contains("0.8 0.1 0.1 rg") && content.contains("<000100020003>"), "recoloured, in its codes: {content}");
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

#[test]
fn an_action_stops_on_a_script_error_before_later_steps() {
    use sheaf_core::pdfcraft::engine::actions::{Action, Step, run_on};
    let t = tempfile::tempdir().unwrap();
    let path = common::fixture_pdf(t.path(), "action.pdf", 1);
    let a = Action { name: "Stop on failure".into(), description: String::new(), builtin: false,
        steps: vec![Step::RunJavaScript("throw new Error('stop here');".into()), Step::SetTitle("Must not run".into())] };
    let mut visited = Vec::new();
    let result = run_on(&a, "action.pdf", std::sync::Arc::new(std::fs::read(path).unwrap()), |i, _| visited.push(i));
    assert!(result.unwrap_err().contains("stop here"));
    assert_eq!(visited, [0]);
}

#[test]
fn a_guided_action_opens_encrypted_input_and_keeps_its_protection() {
    use sheaf_core::pdfcraft::engine::{Session, Protection, actions::{Action, Step, run_guided}};
    let mut session = Session::new();
    let bytes = session.create_from_text("Original", "Keep").unwrap();
    let id = session.open("encrypted.pdf", None, bytes, None).unwrap();
    session.apply(id, Edit::Protect(Protection { open_password: Some("secret".into()), ..Default::default() })).unwrap();
    let bytes = session.save_full_bytes(id).unwrap();
    let action = Action { name: "Title".into(), description: String::new(), builtin: false,
        steps: vec![Step::SetTitle("Saved".into())] };
    assert!(run_guided(&action, "encrypted.pdf", bytes.clone(), None, |_, s| Some(s.clone())).is_err());
    let out = run_guided(&action, "encrypted.pdf", bytes, Some("secret"), |_, s| Some(s.clone())).unwrap();
    let mut check = Session::new();
    assert!(check.open("out.pdf", None, out.bytes.clone(), None).is_err());
    let id = check.open("out.pdf", None, out.bytes, Some("secret")).unwrap();
    assert_eq!(check.get(id).unwrap().info.title.as_deref(), Some("Saved"));
}

#[test]
fn custom_commands_round_trip_and_apply_rotation_and_metadata() {
    use sheaf_core::pdfcraft::engine::{Session, actions::{Action, Step, run_on}};
    for step in Step::all() {
        assert_eq!(Step::from_id(step.id(), step.arg().unwrap_or_default()), Some(step));
    }
    let bytes = Session::new().create_from_text("Original", "Keep").unwrap();
    let action = Action { name: "Custom".into(), description: String::new(), builtin: false,
        steps: vec![Step::RotateClockwise, Step::SetAuthor("A. Writer".into()), Step::SetSubject("Review".into()), Step::SetKeywords("local".into()), Step::RemoveLinks] };
    let out = run_on(&action, "in.pdf", bytes, |_, _| {}).unwrap();
    let mut session = Session::new();
    let id = session.open("out.pdf", None, out.bytes, None).unwrap();
    assert_eq!(session.get(id).unwrap().info.author.as_deref(), Some("A. Writer"));
    assert_eq!(session.get(id).unwrap().info.pages[0].rotation, 90);
    assert_eq!(out.log.len(), 5);
}

#[test]
fn console_page_context_matches_the_reader_and_clamps_out_of_range_pages() {
    use sheaf_core::pdfcraft::engine::Session;
    let bytes = Session::new().create_from_text("Original", "Keep").unwrap();
    let mut session = Session::new(); let id = session.open("in.pdf", None, bytes, None).unwrap();
    let out = session.run_javascript_at(id, "console.println(this.pageNum); this.pageNum = 0;", None, 999).unwrap();
    assert_eq!(out.console, ["0"]);
    assert!(out.requests.contains(&sheaf_core::pdfcraft::engine::js::Request::GoToPage(0)));
}

#[test]
fn grouped_hairlines_use_private_forms_per_scale_and_leave_unselected_pages_unchanged() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"q 0.5 0 0 0.5 0 0 cm /Shared Do Q q 2 0 0 2 100 0 cm /Shared Do Q"]);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let bounds = || -> Vec<Object> { vec![0.into(), 0.into(), 100.into(), 100.into()] };
    let leaf = pdf.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => bounds() }, b"0 w 0 0 m 100 0 l S".to_vec()));
    let shared = pdf.add_object(Stream::new(dictionary! {
        "Type" => "XObject", "Subtype" => "Form", "BBox" => bounds(),
        "Matrix" => vec![0.5.into(), 0.into(), 0.into(), 0.5.into(), 0.into(), 0.into()],
        "Resources" => dictionary! { "XObject" => dictionary! { "Leaf" => leaf } }
    }, b"/Leaf Do".to_vec()));
    let page = pdf.get_pages()[&1];
    let parent = pdf.get_dictionary(page).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    let resources = dictionary! { "XObject" => dictionary! { "Shared" => shared } };
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Resources", resources.clone());
    let original_contents = pdf.get_dictionary(page).unwrap().get(b"Contents").unwrap().clone();
    let unselected = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => parent, "Resources" => resources, "Contents" => original_contents });
    let pages = pdf.get_object_mut(parent).unwrap().as_dict_mut().unwrap();
    pages.set("Kids", vec![page.into(), unselected.into()]);
    pages.set("Count", 2);
    pdf.save(&path).unwrap();
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    doc.apply(Edit::FixPageHairlines { pages: vec![0], minimum: 0.25 }).unwrap();
    let out = doc.save(&t.path().join("forms.pdf")).unwrap();
    let pdf = lopdf::Document::load(&out).unwrap();
    fn forms(resource: &lopdf::Dictionary, data: &[u8]) -> Vec<lopdf::ObjectId> {
        let x = resource.get(b"XObject").unwrap().as_dict().unwrap();
        lopdf::content::Content::decode(data).unwrap().operations.iter().filter(|o| o.operator == "Do")
            .map(|o| x.get(o.operands[0].as_name().unwrap()).unwrap().as_reference().unwrap()).collect()
    }
    let fixed_page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let resources = fixed_page.get(b"Resources").unwrap().as_dict().unwrap();
    let copies = forms(resources, &pdf.get_page_content(pdf.get_pages()[&1]));
    assert_eq!(copies.len(), 2);
    assert_ne!(copies[0], copies[1]);
    for (id, expected) in copies.into_iter().zip([1.0, 0.25]) {
        let group = pdf.get_object(id).unwrap().as_stream().unwrap();
        let leaf = forms(group.dict.get(b"Resources").unwrap().as_dict().unwrap(), &group.get_plain_content().unwrap())[0];
        let leaf = pdf.get_object(leaf).unwrap().as_stream().unwrap();
        let ops = lopdf::content::Content::decode(&leaf.get_plain_content().unwrap()).unwrap().operations;
        let paint = ops.iter().position(|o| o.operator == "S").unwrap();
        assert!((ops[paint - 1].operands[0].as_float().unwrap() - expected).abs() < 1e-6);
    }
    // The originals stay as they were for the page not chosen (the saved file numbers its objects
    // anew: they are found by their content).
    let streams: Vec<Vec<u8>> = pdf.objects.values().filter_map(|o| o.as_stream().ok()).filter_map(|s| s.get_plain_content().ok()).collect();
    assert!(streams.iter().any(|c| c == b"0 w 0 0 m 100 0 l S"), "the original hairline form is kept");
    assert!(streams.iter().any(|c| c == b"/Leaf Do"), "the original group is kept");
    let _ = (leaf, shared);
    assert_eq!(pdf.get_page_content(pdf.get_pages()[&2]), b"q 0.5 0 0 0.5 0 0 cm /Shared Do Q q 2 0 0 2 100 0 cm /Shared Do Q\n");
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    let undone = doc.save(&t.path().join("undone-forms.pdf")).unwrap();
    let original = lopdf::Document::load(path).unwrap();
    let undone = lopdf::Document::load(undone).unwrap();
    assert_eq!(undone.get_page_content(undone.get_pages()[&1]), original.get_page_content(original.get_pages()[&1]));
}

#[test]
fn cyclic_grouped_artwork_fails_without_publishing_partial_hairline_repairs() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"0 w 30 50 m 100 50 l S /Loop Do"]);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let form = pdf.new_object_id();
    pdf.objects.insert(form, Stream::new(dictionary! {
        "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
        "Resources" => dictionary! { "XObject" => dictionary! { "Loop" => form } }
    }, b"/Loop Do".to_vec()).into());
    let page = pdf.get_pages()[&1];
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Resources", dictionary! { "XObject" => dictionary! { "Loop" => form } });
    pdf.save(&path).unwrap();
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    let before = doc.full_bytes().unwrap();
    assert!(doc.apply(Edit::FixPageHairlines { pages: vec![0], minimum: 0.25 }).unwrap_err().to_string().contains("cyclic"));
    assert_eq!(doc.full_bytes().unwrap().as_slice(), before.as_slice());
}

#[test]
fn unreadable_page_streams_cannot_be_silently_skipped_by_hairline_repair() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"0 w 30 50 m 100 50 l S"]);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let broken = pdf.add_object(Stream::new(dictionary! { "Filter" => "FlateDecode" }, vec![0xff, 0x00]));
    let page = pdf.get_pages()[&1];
    let contents = pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().get_mut(b"Contents").unwrap().as_array_mut().unwrap();
    contents.push(broken.into());
    pdf.save(&path).unwrap();
    let mut doc = Doc::open(&Input::new(path)).unwrap();
    let before = doc.full_bytes().unwrap();
    assert!(doc.apply(Edit::FixPageHairlines { pages: vec![0], minimum: 0.25 }).is_err());
    assert_eq!(doc.full_bytes().unwrap().as_slice(), before.as_slice());
}

#[test]
fn wizard_native_options_validate_and_number_pages_before_compression() {
    use sheaf_core::pdfcraft::engine::{Session, actions::{Action, Step, number_settings, run_on}};
    for value in ["6|1|8|<<1>>", "4|0|8|<<1>>", "4|1|NaN|<<1>>", "4|1|3|<<1>>", "bad"] {
        assert!(number_settings(value).is_err(), "{value}");
        assert!(Step::from_id("page_numbers", value).is_none());
    }
    assert!(Step::from_id("compress", "unknown").is_none());
    assert!(Step::from_id("pdfa", "1a").is_none());
    let settings = number_settings("5|12|14|Page <<1>> of <<n>>").unwrap();
    assert_eq!(settings.start_number, 12);
    assert_eq!(settings.font_size, 14.0);
    assert_eq!(settings.text[5], "Page <<1>> of <<n>>");
    let bytes = Session::new().create_from_text("Original", "Keep text").unwrap();
    let dir = tempfile::tempdir().unwrap();
    for level in ["low", "recommended", "extreme", "lossless"] {
        let action = Action { name: "Native".into(), description: String::new(), builtin: false,
            steps: vec![Step::PageNumbers("4|7|12|Page <<1>>".into()), Step::Compress(level.into())] };
        let out = run_on(&action, "input.pdf", bytes.clone(), |_, _| {}).unwrap();
        let path = dir.path().join(format!("{level}.pdf"));
        std::fs::write(&path, out.bytes.as_slice()).unwrap();
        assert!(common::shown_texts(&path)[0].contains("Page 7"));
        let mut check = Session::new();
        let id = check.open("out.pdf", None, out.bytes, None).unwrap();
        assert_eq!(check.get(id).unwrap().info.pages.len(), 1);
        assert_eq!(out.log.len(), 2);
    }
}

#[test]
fn custom_tool_commands_edit_the_current_document_and_undo() {
    use sheaf_core::pdfcraft::engine::{Session, actions::{Step, command_edit}};
    let mut s = Session::new();
    let bytes = s.create_from_text("Unsaved title", "Keep").unwrap();
    let id = s.open("current.pdf", None, bytes, None).unwrap();
    s.apply(id, Edit::SetInfo { key: "Author".into(), value: "Already editing".into() }).unwrap();
    s.apply(id, command_edit(&Step::SetTitle("Custom command".into()), 1).unwrap()).unwrap();
    assert_eq!(s.get(id).unwrap().info.title.as_deref(), Some("Custom command"));
    s.undo(id).unwrap();
    assert_eq!(s.get(id).unwrap().info.title.as_deref(), Some("Unsaved title"));
    assert_eq!(s.get(id).unwrap().info.author.as_deref(), Some("Already editing"));
    assert!(command_edit(&Step::Compress("recommended".into()), 1).is_err());
}

#[test]
fn console_manager_lists_and_edits_document_and_field_scripts_with_undo() {
    use sheaf_core::pdfcraft::engine::{Session, NewField};
    let mut s = Session::new();
    let bytes = s.create_from_text("Scripts", "Keep").unwrap();
    let id = s.open("scripts.pdf", None, bytes, None).unwrap();
    s.apply(id, Edit::AddField { page: 0, rect: [72.0, 600.0, 210.0, 625.0], kind: NewField::Text { multiline: false }, name: Some("Amount".into()) }).unwrap();
    s.apply(id, Edit::SetDocumentScript { name: "Helpers".into(), script: Some("function twice(x) { return x * 2; }".into()) }).unwrap();
    s.apply(id, Edit::SetFieldScript { name: "Amount".into(), event: "keystroke".into(), script: Some("console.println('field');".into()) }).unwrap();
    let scripts = s.get(id).unwrap().console_scripts();
    assert!(scripts.iter().any(|s| s.name == "Helpers" && s.event.is_none()));
    assert!(scripts.iter().any(|s| s.name == "Amount" && s.event.as_deref() == Some("keystroke") && s.source.contains("field")));
    s.apply(id, Edit::SetFieldScript { name: "Amount".into(), event: "keystroke".into(), script: None }).unwrap();
    assert!(!s.get(id).unwrap().console_scripts().iter().any(|s| s.name == "Amount"));
    s.undo(id).unwrap();
    assert!(s.get(id).unwrap().console_scripts().iter().any(|s| s.name == "Amount"));
}
