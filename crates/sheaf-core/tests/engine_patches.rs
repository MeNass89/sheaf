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
    for level in ["low", "recommended", "extreme", "lossless"] {
        let action = Action { name: "Native".into(), description: String::new(), builtin: false,
            steps: vec![Step::PageNumbers("4|7|12|Page <<1>>".into()), Step::Compress(level.into())] };
        let out = run_on(&action, "input.pdf", bytes.clone(), |_, _| {}).unwrap();
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
