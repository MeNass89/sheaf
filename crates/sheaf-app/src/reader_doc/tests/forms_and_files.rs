//! Forms, signatures and attachments.
use super::*;

#[test]
fn a_field_font_is_known_through_its_subset_tag() {
    assert_eq!(field_font_family("ABCDEF+Courier"), 2);
    assert_eq!(field_font_family("Times-Roman"), 1);
    assert_eq!(field_font_family("Helv"), 0);
    assert_eq!(field_font_family("ABCDEF+TimesNewRomanPSMT"), 1);
}

#[test]
fn a_file_is_attached_listed_and_saved_back() {
    use sheaf_core::pdfcraft::engine::AttachIcon;
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, snap) = started(&path);
    assert!(snap.attachments.is_empty());
    let shape = Shape::Attachment { at: [72.0, 700.0], icon: AttachIcon::Paperclip, file: "notes.txt".into(), data: b"hello attachment".to_vec() };
    let style = Style::default_for(&shape);
    ops.send(Op::Apply { edit: Box::new(Edit::AddAnnotation(NewAnnotation { page: 0, shape, style, contents: String::new(), author: String::new() })), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap.attachments.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["notes.txt"]);
    let out = dir.path().join("out.txt");
    save_attachment(&snap.bytes, None, &snap.attachments[0], &out).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"hello attachment");
}

#[test]
fn document_properties_list_the_signatures_with_their_verdict() {
    use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
    let dir = tempfile::tempdir().unwrap();
    let (key, cert, p12) = (dir.path().join("k.pem"), dir.path().join("c.pem"), dir.path().join("id.p12"));
    let ok = |c: &mut std::process::Command| c.output().map(|o| o.status.success()).unwrap_or(false);
    if !ok(std::process::Command::new("openssl").args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-subj", "/CN=Ada Signer", "-keyout"]).arg(&key).arg("-out").arg(&cert))
        || !ok(std::process::Command::new("openssl").args(["pkcs12", "-export", "-passout", "pass:pw", "-inkey"]).arg(&key).arg("-in").arg(&cert).arg("-out").arg(&p12))
    {
        return eprintln!("openssl missing: skipped");
    }
    let path = fixture(dir.path(), 1);
    let signed = dir.path().join("signed.pdf");
    let digital = DigitalSpec { p12: p12.clone(), password: "pw".into(), reason: Some("Approved".into()), location: None, certify: None };
    sign::sign(&Input::new(&path), &SignSpec { page: 0, rect: [72.0, 72.0, 272.0, 132.0], graphic: Graphic::Typed("Ada".into(), Default::default()), digital: Some(digital) }, &signed).unwrap();
    let (_, _, snap) = started(&signed);
    let row = snap.props.about.iter().find(|(k, _)| k.starts_with("Signature 1")).expect("a row for the signature");
    assert!(row.0.contains("Ada Signer"), "{row:?}");
    assert!(row.1.contains("Signature validity is unknown") && row.1.contains("Approved"), "self-signed: not trusted, its reason shown: {row:?}");
    // Its certificate, and the signed version: the file's bytes up to the signature's end.
    let cert = snap.props.about.iter().find(|(k, _)| k == "Signature 1 · certificate").expect("the certificate's row");
    assert!(cert.1.starts_with("Issued to Ada Signer by Ada Signer, valid "), "{cert:?}");
    // The subject's full name, every attribute.
    let subject = snap.props.about.iter().find(|(k, _)| k == "Signature 1 · subject").expect("the subject's row");
    assert!(subject.1.contains("CN=Ada Signer"), "{subject:?}");
    let len = std::fs::metadata(&signed).unwrap().len() as usize;
    assert!(matches!(&snap.props.signed[..], [(who, n, 0, false)] if who == "Ada Signer" && *n > 0 && *n <= len), "{:?}", snap.props.signed);
}

#[test]
fn a_signers_certificate_once_trusted_makes_the_signature_valid() {
    use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
    let dir = tempfile::tempdir().unwrap();
    let (key, cert, p12) = (dir.path().join("k.pem"), dir.path().join("c.pem"), dir.path().join("id.p12"));
    let ok = |c: &mut std::process::Command| c.output().map(|o| o.status.success()).unwrap_or(false);
    if !ok(std::process::Command::new("openssl").args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-subj", "/CN=Bea Signer", "-keyout"]).arg(&key).arg("-out").arg(&cert))
        || !ok(std::process::Command::new("openssl").args(["pkcs12", "-export", "-passout", "pass:pw", "-inkey"]).arg(&key).arg("-in").arg(&cert).arg("-out").arg(&p12))
    {
        return eprintln!("openssl missing: skipped");
    }
    // The trust folder of this test process (set once): Bea's certificate goes there.
    let trust = Box::leak(Box::new(tempfile::tempdir().unwrap()));
    sheaf_core::pdfcraft::set_trust_dir(trust.path().to_path_buf());
    let path = fixture(dir.path(), 1);
    let signed = dir.path().join("signed.pdf");
    let digital = DigitalSpec { p12, password: "pw".into(), reason: None, location: None, certify: None };
    sign::sign(&Input::new(&path), &SignSpec { page: 0, rect: [72.0, 72.0, 272.0, 132.0], graphic: Graphic::Typed("Bea".into(), Default::default()), digital: Some(digital) }, &signed).unwrap();
    let (ops, rx, snap) = started(&signed);
    let verdict = |s: &Snap| s.props.about.iter().find(|(k, _)| k.starts_with("Signature 1 · Bea")).map(|r| r.1.clone()).unwrap_or_default();
    assert!(verdict(&snap).contains("validity is unknown"), "{}", verdict(&snap));
    ops.send(Op::TrustSignature { index: 0 }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert!(!verdict(&snap).contains("unknown") && snap.props.signed[0].3, "{}", verdict(&snap));
}

#[test]
fn a_form_is_prepared_fields_added_set_up_copied_and_deleted() {
    use sheaf_core::pdfcraft::engine::{FieldProps, NewField};
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 2);
    let (ops, rx, snap) = started(&path);
    assert!(snap.fields.is_empty());
    let add = |kind: NewField, rect: [f64; 4]| Op::Apply { edit: Box::new(Edit::AddField { page: 0, rect, kind, name: None }), pages: vec![0] };
    ops.send(add(NewField::Text { multiline: false }, [72.0, 600.0, 222.0, 622.0])).unwrap();
    next(&rx);
    ops.send(add(NewField::CheckBox, [72.0, 560.0, 86.0, 574.0])).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap.fields.iter().map(|f| (f.name.as_str(), f.exact)).collect::<Vec<_>>(), [("Text1", 0), ("Check Box1", 1)], "Acrobat's names");
    // Renamed, required, with a tooltip.
    let props = FieldProps { name: Some("Full name".into()), tooltip: Some("As on your passport".into()), required: Some(true), ..Default::default() };
    ops.send(Op::Apply { edit: Box::new(Edit::SetFieldProps { name: "Text1".into(), props: Box::new(props) }), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let f = snap.fields.iter().find(|f| f.name == "Full name").expect("renamed");
    assert!(f.required && f.tooltip == "As on your passport" && !f.read_only);
    // On every page: the same field on page 2; then the check box deleted.
    ops.send(Op::Apply { edit: Box::new(Edit::DuplicateField { name: "Full name".into(), pages: vec![1] }), pages: vec![] }).unwrap();
    let (snap, _) = next(&rx);
    assert_eq!(snap.fields.iter().find(|f| f.name == "Full name").map(|f| f.widgets.iter().map(|w| w.0).collect::<Vec<_>>()), Some(vec![0, 1]));
    ops.send(Op::Apply { edit: Box::new(Edit::DeleteField { name: "Check Box1".into() }), pages: vec![0] }).unwrap();
    let (snap, _) = next(&rx);
    assert_eq!(snap.fields.len(), 1);
    // Copies, 3 down × 2 across (Acrobat's Create Multiple Copies): five separate fields side by
    // side with it, each required with its tooltip, one undo step.
    ops.send(Op::Apply { edit: Box::new(Edit::CopyField { name: "Full name".into(), widget: 0, rows: 3, cols: 2 }), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let at = |n: &str| snap.fields.iter().find(|f| f.name == n).map(|f| (f.widgets[0].0, f.widgets[0].1.map(f64::round), f.required, f.tooltip.clone()));
    assert_eq!(snap.fields.len(), 6);
    assert_eq!(at("Full name_1_2"), Some((0, [222.0, 600.0, 372.0, 622.0], true, "As on your passport".to_string())));
    assert_eq!(at("Full name_3_1"), Some((0, [72.0, 556.0, 222.0, 578.0], true, "As on your passport".to_string())));
    assert!(at("Full name_3_2").is_some());
    ops.send(Op::Undo { pages: None }).unwrap();
    let (snap, _) = next(&rx);
    assert_eq!(snap.fields.len(), 1);
}

#[test]
fn a_flat_forms_blanks_become_fields_named_from_their_labels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flat.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let mut doc = sheaf_core::engine::new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(700.0), "Name: ______________________", font, PdfPoints::new(12.0)).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(670.0), "City: ______________________", font, PdfPoints::new(12.0)).unwrap();
        doc.save_to_file(&path).unwrap();
    }
    let (ops, rx, _) = started(&path);
    ops.send(Op::DetectFields).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let names: Vec<&str> = snap.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(names.len() == 2 && names.iter().any(|n| n.contains("Name")) && names.iter().any(|n| n.contains("City")), "{names:?}");
    // Again: nothing new to find, one undo step each time.
    ops.send(Op::DetectFields).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_some_and(|e| e.contains("No fields found")) && snap.fields.len() == 2);
}

#[test]
fn a_fields_font_named_by_a_resource_key_is_read_through_the_forms_resources() {
    let dir = tempfile::tempdir().unwrap();
    let objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R] /DR << /Font << /F1 5 0 R >> >> /DA (/F1 0 Tf 0 g) >> >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R] >>".into(),
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (Name) /Rect [72 700 272 722] /P 3 0 R /DA (/F1 12 Tf 0 g) >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman >>".into(),
    ];
    let path = dir.path().join("times.pdf");
    std::fs::write(&path, hand_pdf(&objs)).unwrap();
    let (_, _, snap) = started(&path);
    assert_eq!(snap.fields.iter().map(|f| (f.name.as_str(), f.font)).collect::<Vec<_>>(), [("Name", 1)], "Times through /DR");
}

#[test]
fn attachments_kept_in_a_nested_name_tree_are_renamed_and_kept() {
    use lopdf::{dictionary, Object as L, Stream};
    use sheaf_core::pdfcraft::engine::doc_attachments::DocAttachment;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested.pdf");
    // Two attachments, each in a leaf of an /EmbeddedFiles tree with /Kids (as large writers make it).
    let mut d = lopdf::Document::with_version("1.7");
    let leaf = |d: &mut lopdf::Document, name: &str, body: &[u8]| {
        let file = d.add_object(Stream::new(dictionary! { "Type" => "EmbeddedFile" }, body.to_vec()));
        let spec = d.add_object(dictionary! { "Type" => "Filespec", "F" => L::string_literal(name), "EF" => dictionary! { "F" => file } });
        d.add_object(dictionary! { "Names" => vec![L::string_literal(name), spec.into()], "Limits" => vec![L::string_literal(name), L::string_literal(name)] })
    };
    let (a, b) = (leaf(&mut d, "a.txt", b"first"), leaf(&mut d, "b.txt", b"second"));
    let tree = d.add_object(dictionary! { "Kids" => vec![a.into(), b.into()] });
    let pages = d.new_object_id();
    let content = d.add_object(Stream::new(dictionary! {}, Vec::new()));
    let page = d.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Contents" => content });
    d.objects.insert(pages, L::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages, "Names" => dictionary! { "EmbeddedFiles" => tree } });
    d.trailer.set("Root", catalog);
    d.save(&path).unwrap();
    let (ops, rx, snap) = started(&path);
    assert_eq!(snap.attachments.len(), 2);
    let rename = DocAttachment::Rename { to: "renamed.txt".into() };
    ops.send(Op::Apply { edit: Box::new(Edit::DocAttachment { key: "b.txt".into(), action: rename }), pages: vec![] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let mut names: Vec<&str> = snap.attachments.iter().map(|a| a.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["a.txt", "renamed.txt"], "the other attachment kept");
}

#[test]
fn javascript_console_prints_errors_and_changes_fields_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(Edit::AddField {
        page: 0, rect: [72.0, 600.0, 222.0, 622.0], kind: NewField::Text { multiline: false }, name: Some("Total".into()),
    }), pages: vec![0] }).unwrap();
    next(&rx);
    ops.send(Op::RunJavaScript { source: "console.println('pages=' + this.numPages); this.getField('Total').value = '42';".into() }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::JavaScript { output, changed } => { assert!(output.contains("pages=1"), "{output}"); assert!(changed); }
        _ => panic!("console output expected"),
    }
    let (snap, error) = next(&rx);
    assert!(error.is_none());
    assert_eq!(snap.fields[0].value, ["42"]);
    ops.send(Op::Undo { pages: None }).unwrap();
    let (snap, error) = next(&rx);
    assert!(error.is_none());
    assert!(snap.fields[0].value.is_empty() || snap.fields[0].value == [""]);
    ops.send(Op::RunJavaScript { source: "throw new Error('console failure');".into() }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::JavaScript { output, changed } => { assert!(output.contains("console failure"), "{output}"); assert!(!changed); }
        _ => panic!("console error expected"),
    }
    next(&rx);
}
