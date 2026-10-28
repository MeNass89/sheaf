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

/// A text box in rich text with mixed runs (a bold word, a larger one), as Acrobat writes it:
/// recoloured, its runs keep their weight and size, only their colours change (Sol, 27/10: the
/// rich text was rewritten in one style from /Contents).
#[test]
fn a_recoloured_rich_text_box_keeps_its_mixed_runs() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("rich.pdf");
    let mut doc = lopdf::Document::with_version("1.7");
    let rc = "<?xml version=\"1.0\"?><body xmlns=\"http://www.w3.org/1999/xhtml\" style=\"font-size:12pt;color:#000000\"><p>A <span style=\"font-weight:bold;color:#000000\">bold</span> and <span style=\"font-size:18pt;background-color:#FFFF00\">large</span> word</p></body>";
    // Drawn by that program: a bold run and an 18-point run, each in its own font and size.
    let helv = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let bold = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding" });
    let ap = doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![72.into(), 650.into(), 372.into(), 700.into()], "Resources" => dictionary! { "Font" => dictionary! { "Helv" => helv, "HeBo" => bold } } },
        b"1 1 0 rg 160 660 40 20 re f BT /Helv 12 Tf 0 g 74 680 Td (A ) Tj /HeBo 12 Tf (bold) Tj /Helv 12 Tf ( and ) Tj /Helv 18 Tf (large) Tj /Helv 12 Tf ( word) Tj ET".to_vec(),
    ));
    let annot = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "FreeText", "Rect" => vec![72.into(), 650.into(), 372.into(), 700.into()],
        "Contents" => Object::string_literal("A bold and large word"), "DA" => Object::string_literal("0 0 0 rg /Helv 12 Tf"),
        "RC" => Object::string_literal(rc), "DS" => Object::string_literal("font: 12pt Helvetica; color:#000000"), "AP" => dictionary! { "N" => ap },
    });
    let pages = doc.new_object_id();
    let content = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Contents" => content, "Annots" => vec![annot.into()] });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    doc.save(&path).unwrap();
    let mut d = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(&path)).unwrap();
    d.apply(Edit::StyleAnnotation { page: 0, index: 0, color: Some([1.0, 0.0, 0.0]), opacity: None, width: None, endings: None }).unwrap();
    let out = d.save(&t.path().join("rich-out.pdf")).unwrap();
    let saved = lopdf::Document::load(&out).unwrap();
    let page = *saved.get_pages().values().next().unwrap();
    let annots = saved.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
    let a = saved.get_dictionary(annots[0].as_reference().unwrap()).unwrap();
    let text = |key: &[u8]| {
        let Object::String(s, _) = a.get(key).unwrap() else { panic!("not a string") };
        // UTF-16 with a byte-order mark, or PDFDocEncoding.
        if s.starts_with(&[0xFE, 0xFF]) { String::from_utf16(&s[2..].chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>()).unwrap() } else { String::from_utf8_lossy(s).into_owned() }
    };
    let (rc, ds) = (text(b"RC"), text(b"DS"));
    assert!(rc.contains("font-weight:bold") && rc.contains("font-size:18pt"), "its runs are kept: {rc}");
    assert!(rc.contains("color:#FF0000") && !rc.contains("color:#000000"), "every text colour is the new one: {rc}");
    assert!(rc.contains("background-color:#FFFF00"), "a background colour is not a text colour: {rc}");
    assert!(ds.contains("color:#FF0000"), "{ds}");
    // What shows: the same runs, in their own fonts and sizes, now red; the yellow behind kept
    // (Sol, 27/10: the box was redrawn from its plain text in one font and size).
    let n = a.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
    let Object::Stream(st) = saved.get_object(n).unwrap() else { panic!("no appearance stream") };
    let drawn = String::from_utf8_lossy(&st.decompressed_content().unwrap_or_else(|_| st.content.clone())).into_owned();
    assert!(drawn.contains("/HeBo 12 Tf") && drawn.contains("/Helv 18 Tf"), "its runs are drawn as before: {drawn}");
    assert!(drawn.contains("1 0 0 rg") && !drawn.contains(" 0 g "), "its text red: {drawn}");
    assert!(drawn.contains("1 1 0 rg 160 660 40 20 re f"), "the background as it was: {drawn}");
}

#[test]
fn pattern_and_type3_hairlines_are_private_per_use_and_leave_shared_pages_unchanged() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"q /Pattern cs /Tiles scn 30 40 100 100 re f Q BT /Glyphs 10 Tf 1 0 0 1 40 100 Tm (A) Tj /Glyphs 20 Tf (A) Tj ET"]);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let pattern = pdf.add_object(Stream::new(dictionary! {
        "Type" => "Pattern", "PatternType" => 1, "PaintType" => 1, "TilingType" => 1,
        "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()], "XStep" => 10, "YStep" => 10,
        "Matrix" => vec![0.5.into(), 0.into(), 0.into(), 0.5.into(), 0.into(), 0.into()], "Resources" => dictionary! {}
    }, b"0 w 0 0 m 10 10 l S".to_vec()));
    let glyph = pdf.add_object(Stream::new(dictionary! {}, b"500 0 0 0 500 500 d1 0 w 0 0 m 500 500 l S".to_vec()));
    let font = pdf.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type3", "FontBBox" => vec![0.into(), 0.into(), 500.into(), 500.into()],
        "FontMatrix" => vec![0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into()],
        "FirstChar" => 65, "LastChar" => 65, "Widths" => vec![500.into()],
        "Encoding" => dictionary! { "Type" => "Encoding", "Differences" => vec![65.into(), Object::Name(b"A".to_vec())] },
        "CharProcs" => dictionary! { "A" => glyph }, "Resources" => dictionary! {}
    });
    let resources = dictionary! { "Pattern" => dictionary! { "Tiles" => pattern }, "Font" => dictionary! { "Glyphs" => font } };
    let page = pdf.get_pages()[&1];
    pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Resources", resources.clone());
    let parent = pdf.get_dictionary(page).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    let content = pdf.get_dictionary(page).unwrap().get(b"Contents").unwrap().clone();
    let other = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => parent, "Contents" => content, "Resources" => resources });
    let pages = pdf.get_object_mut(parent).unwrap().as_dict_mut().unwrap();
    pages.set("Kids", vec![page.into(), other.into()]); pages.set("Count", 2);
    let before_other = pdf.get_page_content(other);
    pdf.save(&path).unwrap();
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    doc.apply(Edit::FixPageHairlines { pages: vec![0], minimum: 0.25 }).unwrap();
    let out = doc.save(&t.path().join("fixed-artwork.pdf")).unwrap();
    let pdf = lopdf::Document::load(out).unwrap();
    assert_eq!(pdf.get_page_content(pdf.get_pages()[&2]), before_other);
    let res = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
    let patterns = res.get(b"Pattern").unwrap().as_dict().unwrap();
    let copied = patterns.iter().find(|(name, _)| name.starts_with(b"SheafHairlinePattern")).unwrap().1.as_reference().unwrap();
    let width_before_stroke = |data: &[u8]| {
        let ops = lopdf::content::Content::decode(data).unwrap().operations;
        let stroke = ops.iter().position(|o| o.operator == "S").unwrap();
        assert_eq!(ops[stroke - 1].operator, "w");
        ops[stroke - 1].operands[0].as_float().unwrap()
    };
    assert!((width_before_stroke(&pdf.get_object(copied).unwrap().as_stream().unwrap().get_plain_content().unwrap()) - 0.5).abs() < 1e-5);
    let fonts = res.get(b"Font").unwrap().as_dict().unwrap();
    let mut widths: Vec<_> = fonts.iter().filter(|(n, _)| n.starts_with(b"SheafHairlineFont")).map(|(_, f)| {
        let font = pdf.get_dictionary(f.as_reference().unwrap()).unwrap();
        let charprocs = font.get(b"CharProcs").unwrap().as_dict().unwrap();
        let glyph = pdf.get_object(charprocs.get(b"A").unwrap().as_reference().unwrap()).unwrap().as_stream().unwrap();
        width_before_stroke(&glyph.get_plain_content().unwrap())
    }).collect();
    widths.sort_by(f32::total_cmp);
    assert_eq!(widths.len(), 2);
    assert!((widths[0] - 12.5).abs() < 1e-4 && (widths[1] - 25.0).abs() < 1e-4);
    let old = pdf.get_dictionary(pdf.get_pages()[&2]).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
    let old_font = old.get(b"Font").unwrap().as_dict().unwrap().get(b"Glyphs").unwrap().as_reference().unwrap();
    let old_font = pdf.get_dictionary(old_font).unwrap();
    let old_glyph = old_font.get(b"CharProcs").unwrap().as_dict().unwrap().get(b"A").unwrap().as_reference().unwrap();
    assert_eq!(pdf.get_object(old_glyph).unwrap().as_stream().unwrap().get_plain_content().unwrap(), b"500 0 0 0 500 500 d1 0 w 0 0 m 500 500 l S");
    let id = doc.id(); doc.session_mut().undo(id).unwrap();
    let undone = doc.save(&t.path().join("undone-artwork.pdf")).unwrap();
    let undone = lopdf::Document::load(undone).unwrap();
    assert_eq!(undone.get_page_content(undone.get_pages()[&1]), before_other);
}

#[test]
fn printer_marks_include_all_plate_registration_process_bars_and_embedded_page_info() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"0 g 0 0 500 500 re f"]);
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    let edit = || Edit::AddPrinterMarks { pages: vec![0], registration: true, colour_bars: true, page_information: true, width: 0.5 };
    doc.apply(edit()).unwrap();
    let out = doc.save(&t.path().join("production-marks.pdf")).unwrap();
    let pdf = lopdf::Document::load(&out).unwrap();
    let page = pdf.get_pages()[&1];
    let dict = pdf.get_dictionary(page).unwrap();
    let content = pdf.get_page_content(page);
    let text = String::from_utf8_lossy(&content);
    assert!(text.contains("0.5 w") && text.contains("1 SCN") && text.contains("/Artifact BMC"));
    let resources = dict.get(b"Resources").unwrap().as_dict().unwrap();
    let separation = resources.get(b"ColorSpace").unwrap().as_dict().unwrap().iter().find(|(name, _)| name.starts_with(b"SheafRegistration")).unwrap().1.as_array().unwrap();
    assert_eq!(separation[1].as_name().unwrap(), b"All");
    assert_eq!(dict.get(b"TrimBox").unwrap().as_array().unwrap()[0].as_float().unwrap(), 30.0);
    let ops = lopdf::content::Content::decode(&content).unwrap().operations;
    assert_eq!(ops.iter().filter(|o| o.operator == "k").count(), 7);
    assert!(common::shown_texts(&out)[0].contains("Page 1 / 1"));
    assert!(doc.document().print_readiness().fonts_not_embedded.is_empty());
    doc.apply(edit()).unwrap();
    let again = doc.save(&t.path().join("again-marks.pdf")).unwrap();
    let again = lopdf::Document::load(again).unwrap();
    assert_eq!(again.get_page_content(again.get_pages()[&1]), content);
    let id = doc.id(); doc.session_mut().undo(id).unwrap(); doc.session_mut().undo(id).unwrap();
    let undone = doc.save(&t.path().join("undone-marks.pdf")).unwrap();
    let undone = lopdf::Document::load(undone).unwrap();
    assert_eq!(undone.get_page_content(undone.get_pages()[&1]), b"0 g 0 0 500 500 re f\n");
}

#[test]
fn paddle_preprocessing_matches_reference_bilinear_pixels_and_rejects_invalid_images() {
    use sheaf_core::pdfcraft::engine::ocr::recognition_input;
    let (width, height) = (37usize, 19usize);
    let rgb: Vec<u8> = (0..width * height * 3).map(|n| (n * 17 % 256) as u8).collect();
    for line in [[0.0, 0.0, 37.0, 19.0], [4.5, 3.0, 28.0, 14.5], [-3.0, -2.0, 15.0, 8.0]] {
        let input = recognition_input(&rgb, width, height, line).unwrap();
        let pad = (line[3] - line[1]).max(1.0) * 0.25;
        let x0 = (line[0] - pad).floor().max(0.0) as usize;
        let y0 = (line[1] - pad).floor().max(0.0) as usize;
        let x1 = ((line[2] + pad).ceil() as usize).min(width);
        let y1 = ((line[3] + pad).ceil() as usize).min(height);
        let (cw, ch) = (x1 - x0, y1 - y0);
        let w = ((cw as f32 * 48.0 / ch as f32).round() as usize).clamp(8, 4000);
        let (sx, sy) = (cw as f32 / w as f32, ch as f32 / 48.0);
        assert_eq!((input.width, input.crop_left, input.crop_width), (w, x0, cw));
        assert_eq!(input.pixels.len(), 3 * 48 * w);
        for y in 0..48 {
            let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (ch - 1) as f32);
            let (ya, dy) = (fy.floor() as usize, fy.fract());
            let yb = (ya + 1).min(ch - 1);
            for x in 0..w {
                let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (cw - 1) as f32);
                let (xa, dx) = (fx.floor() as usize, fx.fract());
                let xb = (xa + 1).min(cw - 1);
                for channel in 0..3 {
                    let at = |px: usize, py: usize| f32::from(rgb[((y0 + py) * width + x0 + px) * 3 + channel]);
                    let v = at(xa, ya) * (1.0 - dx) * (1.0 - dy) + at(xb, ya) * dx * (1.0 - dy) + at(xa, yb) * (1.0 - dx) * dy + at(xb, yb) * dx * dy;
                    assert_eq!(input.pixels[(2 - channel) * 48 * w + y * w + x].to_bits(), (v / 127.5 - 1.0).to_bits());
                }
            }
        }
    }
    for pixels in [&[][..], &rgb[..rgb.len() - 1]] { assert!(recognition_input(pixels, width, height, [0.0, 0.0, 20.0, 10.0]).is_none()); }
    assert!(recognition_input(&rgb, width, height, [f32::NAN, 0.0, 20.0, 10.0]).is_none());
    assert!(recognition_input(&rgb, width, height, [50.0, 20.0, 60.0, 25.0]).is_none());
}

#[test]
fn paddle_installations_do_not_require_the_unused_english_recognition_file() {
    use sheaf_core::pdfcraft::engine::ocr::Models;
    let dir = tempfile::tempdir().unwrap();
    for name in ["text-detection.rten", "latin-recognition.onnx", "latin-dict.txt"] {
        std::fs::write(dir.path().join(name), []).unwrap();
    }
    assert!(Models::in_dir(dir.path()).unwrap().reads_accents());
    std::fs::remove_file(dir.path().join("latin-dict.txt")).unwrap();
    assert!(Models::in_dir(dir.path()).is_none(), "incomplete Paddle data must not be offered");
    std::fs::write(dir.path().join("text-recognition.rten"), []).unwrap();
    assert!(!Models::in_dir(dir.path()).unwrap().reads_accents());
}

#[test]
fn wizard_page_ranges_and_mark_settings_edit_only_selected_pages() {
    use sheaf_core::{pages::Input, pdfcraft::{Doc, engine::actions::{self, Step}}};
    let dir = tempfile::tempdir().unwrap();
    let input = common::fixture_pdf(dir.path(), "pages.pdf", 3);
    let mut doc = Doc::open(&Input::new(&input)).unwrap();
    let step = Step::from_id("watermark_options", "24|0.25|0|10|-20|Courier|2|REVIEW").unwrap();
    doc.apply(actions::command_edit(&step, 3).unwrap()).unwrap();
    let output = doc.save(&dir.path().join("marked.pdf")).unwrap();
    let text = common::page_texts(&output);
    assert!(!text[0].contains("REVIEW")); assert!(text[1].contains("REVIEW")); assert!(!text[2].contains("REVIEW"));
    doc.apply(actions::command_edit(&Step::KeepPages("1,3".into()), 3).unwrap()).unwrap();
    let output = doc.save(&dir.path().join("kept.pdf")).unwrap();
    let text = common::page_texts(&output);
    assert_eq!(text.len(), 2); assert!(text[0].contains("Page 1")); assert!(text[1].contains("Page 3"));
    assert!(actions::command_edit(&Step::DeletePages("all".into()), 2).is_err());
    for bad in ["0", "4", "2-1", "1,", ""] { assert!(actions::page_selection(bad, 3).is_err()); }
    assert!(Step::from_id("watermark_options", "24|NaN|0|0|0|Courier|all|Bad").is_none());
    let header = Step::HeaderFooterOptions("0|12|Times-Roman|1|Header".into());
    assert!(matches!(actions::command_edit(&header, 3).unwrap(), Edit::AddHeaderFooter { pages, settings, .. } if pages == [0] && settings.text[0] == "Header" && settings.font_size == 12.0));
}

#[test]
fn wizard_metadata_steps_replace_clear_and_remove_native_metadata() {
    use sheaf_core::{pages::Input, pdfcraft::{Doc, engine::actions::{self, Step}}};
    let dir = tempfile::tempdir().unwrap();
    let input = common::fixture_pdf(dir.path(), "metadata.pdf", 1);
    let mut doc = Doc::open(&Input::new(input)).unwrap();
    doc.apply(actions::command_edit(&Step::SetMetadata("Title|New title".into()), 1).unwrap()).unwrap();
    assert_eq!(doc.document().info.title.as_deref(), Some("New title"));
    doc.apply(actions::command_edit(&Step::SetMetadata("Title|".into()), 1).unwrap()).unwrap();
    assert!(doc.document().info.title.is_none());
    doc.apply(actions::command_edit(&Step::SetMetadata("Author|Owner".into()), 1).unwrap()).unwrap();
    doc.apply(actions::command_edit(&Step::RemoveMetadata, 1).unwrap()).unwrap();
    let out = doc.save(&dir.path().join("clean.pdf")).unwrap();
    let pdf = lopdf::Document::load(out).unwrap();
    assert!(pdf.trailer.get(b"Info").is_err() || pdf.get_dictionary(pdf.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap().get(b"Author").is_err());
}

#[test]
fn accessibility_suggestion_tags_text_and_preserves_existing_tags_by_refusal() {
    use sheaf_core::pdfcraft::engine::{Session, actions::{self, Action, Step}};
    let bytes = Session::new().create_from_text("Accessible title", "Readable text").unwrap();
    let action = Action { name: "Prepare".into(), description: String::new(), steps: vec![Step::PrepareAccessibility("fr-FR".into())], builtin: false };
    let result = actions::run_on(&action, "source.pdf", bytes, |_, _| {}).unwrap();
    let mut session = Session::new();
    let id = session.open("prepared.pdf", None, result.bytes.clone(), None).unwrap();
    let doc = session.get(id).unwrap();
    assert_eq!(doc.initial_view().language.as_deref(), Some("fr-FR"));
    assert!(doc.initial_view().display_title); assert!(!doc.tags().is_empty());
    assert!(actions::run_on(&action, "prepared.pdf", result.bytes, |_, _| {}).is_err());
    assert!(Step::from_id("prepare_accessibility", "fr--FR").is_none());
}

#[test]
fn console_edits_page_and_link_scripts_preserving_other_events_chains_and_undo() {
    use sheaf_core::{pages::Input, pdfcraft::{Doc, engine::js::ActionScriptTarget}};
    let dir = tempfile::tempdir().unwrap();
    let path = prepress_pdf(dir.path(), &[b"BT ET"]);
    let mut pdf = lopdf::Document::load(&path).unwrap(); let page = pdf.get_pages()[&1];
    let close = pdf.add_object(Stream::new(dictionary! {}, b"console.println('close');".to_vec()));
    let link = pdf.add_object(dictionary! {"Type" => "Annot", "Subtype" => "Link", "Rect" => vec![10.into(),20.into(),100.into(),60.into()],
        "A" =>  dictionary! {"S" => "JavaScript", "JS" => Object::string_literal("console.println('link');"), "Next" => dictionary! {"S" => "URI","URI" => Object::string_literal("https://example.test")}},
        "AA" => dictionary! {"E" => dictionary! {"S" => "JavaScript","JS" => Object::string_literal("console.println('enter');")}}
    });
    let pd = pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();
    pd.set("AA", dictionary! {"O" => dictionary! {"S" => "JavaScript","JS" => Object::string_literal("console.println('open');")}, "C" => dictionary! {"S" => "JavaScript","JS" => close}});
    pd.set("Annots", vec![link.into()]); pdf.save(&path).unwrap();
    let mut doc = Doc::open(&Input::new(path)).unwrap();
    let scripts = doc.document().console_scripts(); assert_eq!(scripts.iter().filter(|s| s.target.is_some()).count(), 4);
    assert!(scripts.iter().any(|s| s.source.contains("close")));
    let open = ActionScriptTarget {page:0, link:None, event:"O".into()};
    doc.apply(Edit::SetActionScript {target:open.clone(), script:Some("console.println('updated');".into())}).unwrap();
    assert!(doc.document().console_scripts().iter().any(|s| s.target.as_ref() == Some(&open) && s.source.contains("updated")));
    let activate = ActionScriptTarget {page:0, link:Some(0), event:"A".into()};
    doc.apply(Edit::SetActionScript {target:activate.clone(), script:Some("console.println('é');".into())}).unwrap();
    let output = doc.save(&dir.path().join("edited.pdf")).unwrap();
    let saved = lopdf::Document::load(output).unwrap();
    let action = saved.get_dictionary(link).unwrap().get(b"A").unwrap().as_dict().unwrap();
    assert!(action.get(b"Next").is_ok(), "editing a source preserves its chained action");
    assert!(doc.apply(Edit::SetActionScript {target:activate.clone(), script:None}).is_err());
    let id = doc.id(); doc.session_mut().undo(id).unwrap();
    assert!(doc.document().console_scripts().iter().any(|s| s.target.as_ref() == Some(&activate) && s.source.contains("link")));
    doc.apply(Edit::SetActionScript {target:open.clone(), script:None}).unwrap();
    assert!(!doc.document().console_scripts().iter().any(|s| s.target.as_ref() == Some(&open)));
    assert!(doc.document().console_scripts().iter().any(|s| s.source.contains("close")));
    assert!(doc.document().console_scripts().iter().any(|s| s.source.contains("enter")));
}

#[test]
fn wizard_protection_encrypts_the_final_output_with_only_the_prompted_password() {
    use sheaf_core::pdfcraft::engine::{Session, actions::{self, Action, Step}};
    let bytes = Session::new().create_from_text("Title", "Body").unwrap();
    let action = Action { name: "Protect".into(), description: String::new(), builtin: false, steps: vec![Step::Protect(String::new())] };
    assert!(actions::run_on(&action, "input.pdf", bytes.clone(), |_, _| {}).is_err());
    let output = actions::run_guided(&action, "input.pdf", bytes, None, |_, _| Some(Step::Protect("runtime secret".into()))).unwrap();
    let mut session = Session::new();
    assert!(session.open("output.pdf", None, output.bytes.clone(), None).is_err());
    assert!(session.open("output.pdf", None, output.bytes, Some("runtime secret")).is_ok());
}

/// A paragraph set in an embedded font (Great Vibes), copied with the Edit tool and pasted on
/// another page, is written there in that very font, its lines as they broke: the font is copied
/// in with its program, the text reads back and is page text again (PDF Expert's paste; Sol,
/// 27/10: pasted paragraphs became Helvetica, Times or Courier).
#[test]
fn a_pasted_paragraph_keeps_its_embedded_font() {
    use sheaf_core::pages::Input;
    use sheaf_core::pdfcraft::engine::{AddedText, ObjectKind};
    use sheaf_core::pdfcraft::Doc;
    let dir = tempfile::tempdir().unwrap();
    let pdf = common::fixture_pdf(dir.path(), "two.pdf", 2);
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    let vibes = std::sync::Arc::new(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/GreatVibes-Regular.ttf")).unwrap());
    let text = AddedText { rect: [72.0, 500.0, 500.0, 560.0], text: "Kind regards\nAda Lovelace".into(), size: 24.0, ..Default::default() };
    doc.apply(Edit::AddTextInFont { page: 0, text, font: vibes }).unwrap();
    let made = doc.save(&dir.path().join("vibes.pdf")).unwrap();
    let doc = Doc::open(&Input::new(&made)).unwrap();
    let d = doc.document();
    let objects = d.editable_objects(0).unwrap();
    let para: Vec<_> = objects.iter().filter(|o| o.target.kind == ObjectKind::Text && o.text.as_deref().is_some_and(|t| t.contains("regards"))).collect();
    assert_eq!(para.len(), 1, "{:?}", objects.iter().map(|o| &o.text).collect::<Vec<_>>());
    let targets = [para[0].target];
    let fonts = d.objects_text_fonts(0, &targets);
    let (font, lines) = fonts[0].clone().expect("Great Vibes writes its own text again");
    assert_eq!(lines, "Kind regards\nAda Lovelace");
    let copied = std::sync::Arc::new(d.objects_pdf(0, &targets, para[0].rect).unwrap());
    let mut text = d.objects_texts(0, &targets)[0].clone().unwrap();
    text.text = lines;
    text.rect = [72.0, 300.0, 500.0, 360.0];
    let mut doc = doc;
    doc.apply(Edit::AddTextInCopiedFont { page: 1, text, pdf: copied, font }).unwrap();
    let out = doc.save(&dir.path().join("pasted.pdf")).unwrap();
    common::qpdf_clean(&out);
    // Page 2's fonts: Great Vibes, embedded.
    let l = lopdf::Document::load(&out).unwrap();
    let page2 = l.get_pages()[&2];
    let fonts = l.get_page_fonts(page2).unwrap();
    let vibes: Vec<_> = fonts.values().filter(|f| f.get(b"BaseFont").ok().and_then(|b| b.as_name().ok()).is_some_and(|b| String::from_utf8_lossy(b).contains("GreatVibes"))).collect();
    assert_eq!(vibes.len(), 1, "page 2's fonts: {:?}", fonts.keys().map(|k| String::from_utf8_lossy(k).into_owned()).collect::<Vec<_>>());
    let embedded = |f: &&lopdf::Dictionary| {
        let desc = |d: &lopdf::Dictionary| d.get(b"FontDescriptor").ok().and_then(|r| l.dereference(r).ok()).and_then(|(_, o)| o.as_dict().ok().cloned());
        let own = desc(f).or_else(|| {
            let kids = l.dereference(f.get(b"DescendantFonts").ok()?).ok()?.1.as_array().ok()?.clone();
            desc(l.dereference(kids.first()?).ok()?.1.as_dict().ok()?)
        });
        own.is_some_and(|d| d.has(b"FontFile2") || d.has(b"FontFile3") || d.has(b"FontFile"))
    };
    assert!(vibes.iter().all(|f| embedded(f)), "the program is copied too");
    let shown = common::shown_texts(&out);
    assert!(shown[1].contains("Kind regards") && shown[1].contains("Ada Lovelace"), "{shown:?}");
    // Page text again, a paragraph the Edit tool edits.
    let again = Doc::open(&Input::new(&out)).unwrap();
    assert!(again.document().text_blocks(1).iter().any(|b| b.text.contains("regards") && b.base_font.contains("GreatVibes")), "{:?}", again.document().text_blocks(1).iter().map(|b| (&b.text, &b.base_font)).collect::<Vec<_>>());
}

/// A certificate recipient's permissions (Acrobat's Encrypt with Certificate) hold the decrypted
/// working copy the way a restricted password would, and survive a save.
#[test]
fn a_certificate_recipients_permissions_refuse_edits_printing_and_copying_after_saves_too() {
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"0 0 m 10 10 l S"]);
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    let id = doc.id();
    // Comment and fill forms only: no printing, copying or page changes.
    let comment_only = sheaf_core::certificate_security::Permissions::new(false, false, false, true, true, false);
    assert!(doc.session_mut().restrict(id, comment_only.0 as i32));
    let check = |doc: &Doc| {
        let d = doc.session().get(id).unwrap();
        assert!(!d.allows_printing() && !d.allows_copying());
        assert!(d.permissions().is_some_and(|p| p.annotate() && !p.assemble()));
    };
    check(&doc);
    assert!(doc.apply(Edit::RotatePages { pages: vec![0], degrees: 90 }).unwrap_err().to_string().contains("page changes"));
    let bytes = doc.session().save_bytes(id).unwrap();
    doc.session_mut().mark_saved(id, bytes, None).unwrap();
    check(&doc);
}

/// Acrobat's barcode field: the form's data, tab-delimited under the field names, is drawn as a QR
/// Code in the field and drawn again when a field changes; the error correction chosen sets its
/// density. Read back from the rendered page with a QR reader.
#[test]
fn a_barcode_field_draws_the_forms_data_as_a_qr_code_and_follows_every_change() {
    use sheaf_core::pdfcraft::engine::{barcode::BarcodeSettings, FieldValue, NewField};
    use sheaf_core::{pages::Input, pdfcraft::Doc};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"0 0 m 10 10 l S"]);
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    doc.apply(Edit::AddField { page: 0, rect: [40.0, 300.0, 200.0, 320.0], kind: NewField::Text { multiline: false }, name: Some("Name".into()) }).unwrap();
    doc.apply(Edit::AddField { page: 0, rect: [40.0, 260.0, 60.0, 280.0], kind: NewField::CheckBox, name: Some("Agree".into()) }).unwrap();
    doc.apply(Edit::AddField { page: 0, rect: [100.0, 60.0, 250.0, 210.0], kind: NewField::Text { multiline: false }, name: Some("Code".into()) }).unwrap();
    doc.apply(Edit::SetBarcode { name: "Code".into(), settings: BarcodeSettings { ecc: 1, names: true } }).unwrap();
    let read = |doc: &Doc| {
        let r = doc.render(0, 3.0).unwrap();
        let (w, h) = (r.width as usize, r.height as usize);
        let mut img = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| r.rgba[(y * w + x) * 4]);
        let grids = img.detect_grids();
        assert_eq!(grids.len(), 1, "one QR Code on the page");
        let (meta, text) = grids[0].decode().unwrap();
        (meta.ecc_level, text)
    };
    let (_, first) = read(&doc);
    assert_eq!(first, "Name\tAgree\n\t", "the other fields' names, then their (empty) values");
    doc.apply(Edit::SetFieldValue { name: "Name".into(), value: FieldValue::Text("Ada Lovelace".into()) }).unwrap();
    doc.apply(Edit::SetFieldValue { name: "Agree".into(), value: FieldValue::Check(true) }).unwrap();
    let (ecc, after) = read(&doc);
    assert!(after.starts_with("Name\tAgree\nAda Lovelace\t"), "{after:?}");
    // Saved and reopened, the barcode and its settings stay.
    let out = doc.save(&t.path().join("barcode.pdf")).unwrap();
    let mut doc = Doc::open(&Input::new(&out)).unwrap();
    assert_eq!(read(&doc).1, after);
    doc.apply(Edit::SetBarcode { name: "Code".into(), settings: BarcodeSettings { ecc: 3, names: false } }).unwrap();
    let (high, values) = read(&doc);
    assert!(values.starts_with("Ada Lovelace\t") && !values.contains("Name"), "{values:?}");
    assert_ne!(ecc, high, "error correction chosen is the one drawn");
    // Only a text field can become a barcode field.
    assert!(doc.apply(Edit::SetBarcode { name: "Agree".into(), settings: BarcodeSettings::default() }).is_err());
}

/// A signature carrying its own revocation evidence (Acrobat's `adbe-revocationInfoArchival`
/// signed attribute, the CRL Acrobat embeds when it signs) is judged with it offline: a CRL that
/// revokes the signer makes the signature invalid; one that does not says so. Nothing is fetched.
#[test]
fn revocation_evidence_inside_a_signature_is_checked_offline() {
    use sheaf_core::pdfcraft::engine::sign::{der, pkcs12, DigestAlg, Time};
    use sheaf_core::pdfcraft::engine::{revocation_archive, SignatureStatus};
    use sheaf_core::{pages::Input, pdfcraft::Doc, sign};
    let t = tempfile::tempdir().unwrap();
    let spec = sheaf_core::digital_id::NewId { name: "Archive Signer".into(), ..Default::default() };
    let p12 = sheaf_core::digital_id::create_file(&spec, "secret-pw", &t.path().join("id.p12")).unwrap();
    let id = pkcs12::open(&std::fs::read(&p12).unwrap(), "secret-pw").unwrap();
    let invoice = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf");
    let digital = sign::DigitalSpec { p12: p12.clone(), password: "secret-pw".into(), reason: None, location: None, certify: None };
    let signed = sign::sign(&Input::new(&invoice), &sign::SignSpec { page: 0, rect: [72.0, 72.0, 272.0, 132.0], graphic: sign::Graphic::Typed("Ada".into(), Default::default()), digital: Some(digital) }, &t.path().join("signed.pdf")).unwrap().path;
    let bytes = std::fs::read(&signed).unwrap();

    // The signer's own CRL (self-signed ID), its window around now; `revoked` lists its serial.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let crl = |revoked: bool| {
        let alg = id.key.signature_algorithm(DigestAlg::Sha256);
        let entry = der::seq(&[&der::seq(&[&der::uint(&id.certificate.serial), &Time::from_unix(now - 3600).encode()])]);
        let mut parts: Vec<&[u8]> = vec![&[2, 1, 1][..], &alg, &id.certificate.subject.raw];
        let (this, next) = (Time::from_unix(now - 86_400).encode(), Time::from_unix(now + 86_400).encode());
        parts.extend([this.as_slice(), next.as_slice()]);
        if revoked { parts.push(&entry); }
        let tbs = der::seq(&parts);
        der::seq(&[&tbs, &alg, &der::bit_string(&id.key.sign(DigestAlg::Sha256, &tbs).unwrap())])
    };
    // The signature's CMS rebuilt with the archive attribute among its signed attributes, signed
    // again; the PDF bytes it signs are unchanged.
    let with_archive = |crl: &[u8]| -> Vec<u8> {
        // The placeholder is the gap the /ByteRange leaves: "<" hex ">".
        let (start, end) = {
            let at = bytes.windows(10).position(|w| w == b"/ByteRange").unwrap();
            let open = at + bytes[at..].iter().position(|b| *b == b'[').unwrap();
            let close = open + bytes[open..].iter().position(|b| *b == b']').unwrap();
            let n: Vec<usize> = std::str::from_utf8(&bytes[open + 1..close]).unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
            (n[1] + 1, n[2] - 1)
        };
        let hex = std::str::from_utf8(&bytes[start..end]).unwrap();
        let cms: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
        let (ci, _) = der::Tlv::parse_ber(&cms).unwrap();
        let ci_parts = ci.children().unwrap();
        let sd = ci_parts[1].inner().unwrap();
        let mut sd_parts: Vec<Vec<u8>> = sd.children().unwrap().iter().map(|c| c.raw.to_vec()).collect();
        let signer_set = der::Tlv::parse_all(sd_parts.last().unwrap()).unwrap();
        let signer = signer_set.children().unwrap()[0];
        let mut fields: Vec<Vec<u8>> = signer.children().unwrap().iter().map(|c| c.raw.to_vec()).collect();
        let attrs_at = fields.iter().position(|f| f[0] == 0xA0).unwrap();
        let attrs = der::Tlv::parse_all(&fields[attrs_at]).unwrap();
        let archive = der::seq(&[&der::oid("1.2.840.113583.1.1.8"), &der::set_of(&[&der::seq(&[&der::explicit(0, &der::seq(&[crl]))])])]);
        let mut list: Vec<Vec<u8>> = attrs.children().unwrap().iter().map(|a| a.raw.to_vec()).collect();
        list.push(archive);
        let refs: Vec<&[u8]> = list.iter().map(Vec::as_slice).collect();
        let set = der::set_of(&refs);
        fields[attrs_at] = [&[0xA0][..], &set[1..]].concat();
        let sig_at = fields.iter().rposition(|f| f[0] == der::tag::OCTET_STRING).unwrap();
        fields[sig_at] = der::octets(&id.key.sign(DigestAlg::Sha256, &set).unwrap());
        let refs: Vec<&[u8]> = fields.iter().map(Vec::as_slice).collect();
        let signer = der::seq(&refs);
        *sd_parts.last_mut().unwrap() = der::set_of(&[&signer]);
        let refs: Vec<&[u8]> = sd_parts.iter().map(Vec::as_slice).collect();
        let cms = der::seq(&[ci_parts[0].raw, &der::explicit(0, &der::seq(&refs))]);
        let mut hex: String = cms.iter().map(|b| format!("{b:02X}")).collect();
        assert!(hex.len() <= end - start, "fits the placeholder");
        hex.extend(std::iter::repeat_n('0', end - start - hex.len()));
        let mut out = bytes.clone();
        out[start..end].copy_from_slice(hex.as_bytes());
        out
    };
    let judged = |name: &str, pdf: Vec<u8>| {
        let path = t.path().join(name);
        std::fs::write(&path, pdf).unwrap();
        let doc = Doc::open(&Input::new(&path)).unwrap();
        doc.document().signatures.iter().find(|s| s.signed).cloned().unwrap()
    };
    let plain = judged("plain.pdf", bytes.clone());
    assert_eq!(plain.status, SignatureStatus::Unknown, "{:?}", plain.details);
    let good = judged("good.pdf", with_archive(&crl(false)));
    assert_eq!(good.status, SignatureStatus::Unknown, "a self-signed ID stays untrusted: {:?}", good.details);
    assert!(good.details.iter().any(|d| d.message.contains("has not been revoked")), "{:?}", good.details);
    let revoked = judged("revoked.pdf", with_archive(&crl(true)));
    assert_eq!(revoked.status, SignatureStatus::Invalid, "{:?}", revoked.details);
    assert!(revoked.details.iter().any(|d| d.message.contains("revoked (CRL in the signature")), "{:?}", revoked.details);
    let (crls, ocsps) = revocation_archive::evidence(&[0x30, 0x00]);
    assert!(crls.is_empty() && ocsps.is_empty(), "no archive, no evidence");
}

/// iLovePDF's repeated signature: the copies placed on several pages in one step carry one group
/// in their /NM, which a later move does not change; a single comment, or comments added on one
/// page, have none.
#[test]
fn a_repeated_signature_keeps_its_group_wherever_a_copy_is_moved() {
    use sheaf_core::pdfcraft::engine::comment_group;
    use sheaf_core::pdfcraft::render::PageInfo;
    use sheaf_core::{pages::Input, pdfcraft::Doc, sign};
    let t = tempfile::tempdir().unwrap();
    let path = prepress_pdf(t.path(), &[b"0 0 m 10 10 l S"]);
    let mut doc = Doc::open(&Input::new(&path)).unwrap();
    doc.apply(Edit::DuplicatePages { pages: vec![0] }).unwrap();
    let n = doc.document().info.pages.len();
    assert!(n >= 2, "two pages to repeat on");
    let info: PageInfo = doc.document().info.pages[0].clone();
    let graphic = sign::Graphic::Typed("Ada".into(), Default::default());
    let copies = (0..n).map(|p| sign::signature_edit(&info, p, [140.0, 300.0], 100.0, &graphic).unwrap()).collect();
    doc.apply(Edit::Batch { label: "Signature on several pages".into(), edits: copies }).unwrap();
    doc.apply(sign::signature_edit(&info, 0, [140.0, 80.0], 100.0, &graphic).unwrap()).unwrap();
    let names = |doc: &Doc| doc.document().info.annotations.iter().map(|a| (a.page, a.index, a.name.clone().unwrap_or_default())).collect::<Vec<_>>();
    let all = names(&doc);
    let groups: Vec<_> = all.iter().map(|(_, _, nm)| comment_group(nm).map(str::to_string)).collect();
    let group = groups.iter().flatten().next().cloned().unwrap_or_else(|| panic!("the repeated copies have a group: {all:?}"));
    assert_eq!(groups.iter().filter(|g| g.as_deref() == Some(group.as_str())).count(), n, "{all:?}");
    assert_eq!(groups.iter().filter(|g| g.is_none()).count(), 1, "the lone signature has none: {all:?}");
    // One copy moved far away (and saved): still in the group.
    let (page, index, _) = all.iter().find(|(p, _, nm)| *p == 1 && comment_group(nm).is_some()).cloned().unwrap();
    doc.apply(Edit::MoveAnnotation { page, index, dx: -60.0, dy: -200.0 }).unwrap();
    let out = doc.save(&t.path().join("moved.pdf")).unwrap();
    let doc = Doc::open(&Input::new(&out)).unwrap();
    let moved = names(&doc).into_iter().find(|(p, i, _)| *p == page && *i == index).unwrap();
    assert_eq!(comment_group(&moved.2), Some(group.as_str()));
}

/// A paragraph mixing a regular and a bold font ("Payment is due within" in Helvetica, "thirty days"
/// in Helvetica-Bold, then a second line in Helvetica), copied with the Edit tool: its runs come
/// back each in its own font and weight, and pasted run by run on another page they keep them
/// there, in their places (PDF Expert's formatted paste; Sol, 28/10: the paragraph was pasted in
/// one standard font and lost its bold).
#[test]
fn a_pasted_paragraph_mixing_fonts_keeps_each_runs_font() {
    use sheaf_core::pages::Input;
    use sheaf_core::pdfcraft::engine::ObjectKind;
    use sheaf_core::pdfcraft::Doc;
    let dir = tempfile::tempdir().unwrap();
    let mut pdf = lopdf::Document::with_version("1.7");
    let font = |base: &str| dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => base.to_string(), "Encoding" => "WinAnsiEncoding" };
    let (f1, f2) = (pdf.add_object(font("Helvetica")), pdf.add_object(font("Helvetica-Bold")));
    let content = b"BT /F1 12 Tf 72 700 Td (Payment is due within ) Tj /F2 12 Tf (thirty days) Tj /F1 12 Tf 0 -15 Td (of the invoice date.) Tj ET".to_vec();
    let c = pdf.add_object(Stream::new(dictionary! {}, content));
    let pages = pdf.new_object_id();
    let mk = |pdf: &mut lopdf::Document, contents: Option<lopdf::ObjectId>| {
        let mut d = dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => f1, "F2" => f2 } } };
        if let Some(c) = contents {
            d.set("Contents", c);
        }
        pdf.add_object(d)
    };
    let (p1, p2) = (mk(&mut pdf, Some(c)), mk(&mut pdf, None));
    pdf.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![p1.into(), p2.into()], "Count" => 2 }));
    let cat = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    pdf.trailer.set("Root", cat);
    let src = dir.path().join("mixed.pdf");
    pdf.save(&src).unwrap();

    let mut doc = Doc::open(&Input::new(&src)).unwrap();
    let d = doc.document();
    let objects = d.editable_objects(0).unwrap();
    let para: Vec<_> = objects.iter().filter(|o| o.target.kind == ObjectKind::Text && o.text.as_deref().is_some_and(|t| t.contains("Payment"))).collect();
    assert_eq!(para.len(), 1, "one paragraph: {:?}", objects.iter().map(|o| &o.text).collect::<Vec<_>>());
    assert!(para[0].text.as_deref().is_some_and(|t| t.contains("invoice")), "both lines in it: {:?}", para[0].text);
    let targets = [para[0].target];
    assert_eq!(d.objects_text_fonts(0, &targets), vec![None], "not one font");
    let runs = d.objects_text_runs(0, &targets)[0].clone().expect("a mixed paragraph comes as runs");
    let bold: Vec<_> = runs.iter().filter(|(t, _)| t.bold).map(|(t, _)| t.text.as_str()).collect();
    assert_eq!(bold, ["thirty days"], "{runs:?}");
    assert_eq!(runs.iter().map(|(t, _)| t.text.trim()).collect::<Vec<_>>().join("|"), "Payment is due within|thirty days|of the invoice date.");
    // Pasted 400 pt lower on page 2, run by run, as ⌘V does.
    let copied = std::sync::Arc::new(d.objects_pdf(0, &targets, para[0].rect).unwrap());
    let edits = runs
        .into_iter()
        .map(|(mut text, font)| {
            text.rect = [text.rect[0], text.rect[1] - 400.0, text.rect[2] + 0.04 * (text.rect[2] - text.rect[0]) + 2.0, text.rect[3] - 400.0];
            match font {
                Some(font) => Edit::AddTextInCopiedFont { page: 1, text, pdf: copied.clone(), font },
                None => Edit::AddText { page: 1, text },
            }
        })
        .collect();
    doc.apply(Edit::Batch { label: "Paste objects".into(), edits }).unwrap();
    let out = doc.save(&dir.path().join("pasted.pdf")).unwrap();
    common::qpdf_clean(&out);
    let shown = common::shown_texts(&out);
    for w in ["Payment is due within", "thirty days", "of the invoice date."] {
        assert!(shown[1].contains(w), "{w} on page 2: {shown:?}");
    }
    // Page 2's text: the bold run in a bold font, the others in a regular one, on two lines.
    let again = Doc::open(&Input::new(&out)).unwrap();
    let lines = again.document().text_lines(1);
    let font_of = |w: &str| lines.iter().find(|l| l.text.contains(w)).map(|l| (l.base_font.clone(), l.bold, l.rect[1])).unwrap_or_else(|| panic!("{w} in {:?}", lines.iter().map(|l| &l.text).collect::<Vec<_>>()));
    let (pay, days, inv) = (font_of("Payment"), font_of("thirty"), font_of("invoice"));
    assert!(days.1 && days.0.contains("Bold"), "the bold run stays bold: {days:?}");
    assert!(!pay.1 && !pay.0.contains("Bold") && !inv.1, "the regular runs stay regular: {pay:?} {inv:?}");
    assert!((pay.2 - days.2).abs() < 2.0 && pay.2 - inv.2 > 10.0, "the runs keep their lines: {pay:?} {days:?} {inv:?}");
}

/// A paragraph in one font with a red word and a larger word ("Please pay" black, "250 euros" red,
/// "today." black; "Note:" 12 pt, "IMPORTANT" 18 pt, "terms apply." 12 pt), copied and pasted on
/// another page: the red word stays red and the larger word larger (Sol, 28/10: runs split only on
/// font, bold and italic, so colour and size were lost).
#[test]
fn a_pasted_paragraph_keeps_each_runs_colour_and_size() {
    use sheaf_core::pages::Input;
    use sheaf_core::pdfcraft::engine::ObjectKind;
    use sheaf_core::pdfcraft::Doc;
    let dir = tempfile::tempdir().unwrap();
    let mut pdf = lopdf::Document::with_version("1.7");
    let f1 = pdf.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let content = b"BT /F1 12 Tf 72 700 Td (Please pay ) Tj 1 0 0 rg (250 euros) Tj 0 g ( today.) Tj 0 -16 Td (Note: ) Tj /F1 18 Tf (IMPORTANT) Tj /F1 12 Tf ( terms apply.) Tj ET".to_vec();
    let c = pdf.add_object(Stream::new(dictionary! {}, content));
    let pages = pdf.new_object_id();
    let res = dictionary! { "Font" => dictionary! { "F1" => f1 } };
    let p1 = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Contents" => c, "Resources" => res.clone() });
    let p2 = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Resources" => res });
    pdf.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![p1.into(), p2.into()], "Count" => 2 }));
    let cat = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    pdf.trailer.set("Root", cat);
    let src = dir.path().join("styled.pdf");
    pdf.save(&src).unwrap();

    let mut doc = Doc::open(&Input::new(&src)).unwrap();
    let d = doc.document();
    let objects = d.editable_objects(0).unwrap();
    let para: Vec<_> = objects.iter().filter(|o| o.target.kind == ObjectKind::Text && o.text.as_deref().is_some_and(|t| t.contains("Please"))).collect();
    assert_eq!(para.len(), 1, "{:?}", objects.iter().map(|o| &o.text).collect::<Vec<_>>());
    assert!(para[0].text.as_deref().is_some_and(|t| t.contains("IMPORTANT")), "both lines in it: {:?}", para[0].text);
    let targets = [para[0].target];
    let runs = d.objects_text_runs(0, &targets)[0].clone().expect("a paragraph with a red and a larger word comes as runs");
    let red: Vec<_> = runs.iter().filter(|(t, _)| t.color[0] > 0.9 && t.color[1] < 0.1).map(|(t, _)| t.text.trim()).collect();
    assert_eq!(red, ["250 euros"], "{runs:?}");
    let big: Vec<_> = runs.iter().filter(|(t, _)| t.size > 15.0).map(|(t, _)| t.text.trim()).collect();
    assert_eq!(big, ["IMPORTANT"], "{runs:?}");
    let copied = std::sync::Arc::new(d.objects_pdf(0, &targets, para[0].rect).unwrap());
    let edits = runs
        .into_iter()
        .map(|(mut text, font)| {
            text.rect = [text.rect[0], text.rect[1] - 400.0, text.rect[2] + 0.04 * (text.rect[2] - text.rect[0]) + 2.0, text.rect[3] - 400.0];
            match font {
                Some(font) => Edit::AddTextInCopiedFont { page: 1, text, pdf: copied.clone(), font },
                None => Edit::AddText { page: 1, text },
            }
        })
        .collect();
    doc.apply(Edit::Batch { label: "Paste objects".into(), edits }).unwrap();
    let out = doc.save(&dir.path().join("pasted.pdf")).unwrap();
    common::qpdf_clean(&out);
    let again = Doc::open(&Input::new(&out)).unwrap();
    let lines = again.document().text_lines(1);
    let of = |w: &str| lines.iter().find(|l| l.text.contains(w)).map(|l| (l.color, l.size)).unwrap_or_else(|| panic!("{w} in {:?}", lines.iter().map(|l| &l.text).collect::<Vec<_>>()));
    let (pay, euros, important, terms) = (of("Please"), of("250"), of("IMPORTANT"), of("terms"));
    assert!(euros.0[0] > 0.9 && euros.0[1] < 0.1 && euros.0[2] < 0.1, "the red word stays red: {euros:?}");
    assert!(pay.0.iter().all(|c| *c < 0.1), "the rest black: {pay:?}");
    assert!((important.1 - 18.0).abs() < 0.5 && (terms.1 - 12.0).abs() < 0.5, "the larger word stays larger: {important:?} {terms:?}");
}
