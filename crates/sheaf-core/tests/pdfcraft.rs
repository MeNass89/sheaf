mod common;
use common::{fixture_pdf, page_texts};
use sheaf_core::{pages::Input, pdfcraft::{engine::Edit, Doc}, security::protect, OpdfError};
use std::process::Command;

fn with_pw(path: &std::path::Path, pw: Option<&str>) -> Input {
    Input { path: path.to_path_buf(), password: pw.map(Into::into) }
}

/// `qpdf --check` output (stdout + stderr); asserts the exit status is 0.
fn qpdf_check(path: &std::path::Path) -> String {
    let o = Command::new("qpdf").arg("--check").arg(path).output().unwrap();
    let s = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(o.status.success(), "qpdf --check failed: {s}");
    s
}

#[test]
fn opens_plain_pdf() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let doc = Doc::open(&Input::new(&a)).unwrap();
    assert!(!doc.was_encrypted());
    assert_eq!(doc.session().get(doc.id()).unwrap().info.pages.len(), 3);
}

#[test]
fn not_a_pdf_is_an_engine_error() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("x.pdf");
    std::fs::write(&p, b"hello, not a pdf").unwrap();
    assert!(matches!(Doc::open(&Input::new(&p)), Err(OpdfError::Engine(_))));
}

#[test]
fn protected_without_password_is_password_required() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let p = protect(&Input::new(&a), "user", None, &t.path().join("p.pdf")).unwrap();
    match Doc::open(&Input::new(&p)) {
        Err(OpdfError::PasswordRequired(path)) => assert_eq!(path, p),
        other => panic!("expected PasswordRequired, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn protected_with_wrong_password_is_wrong_password() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let p = protect(&Input::new(&a), "user", None, &t.path().join("p.pdf")).unwrap();
    match Doc::open(&with_pw(&p, Some("nope"))) {
        Err(OpdfError::WrongPassword(path)) => assert_eq!(path, p),
        other => panic!("expected WrongPassword, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn protected_with_right_password_opens_and_reports_encrypted() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let p = protect(&Input::new(&a), "user", Some("owner"), &t.path().join("p.pdf")).unwrap();
    for pw in ["user", "owner"] {
        let doc = Doc::open(&with_pw(&p, Some(pw))).unwrap();
        assert!(doc.was_encrypted(), "opened with {pw}");
        assert_eq!(doc.session().get(doc.id()).unwrap().info.pages.len(), 2);
    }
}

#[test]
fn save_writes_a_clean_pdf_with_the_edit() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let mut doc = Doc::open(&Input::new(&a)).unwrap();
    doc.apply(Edit::RotatePages { pages: vec![0], degrees: 90 }).unwrap();
    let out = doc.save(&t.path().join("out.pdf")).unwrap();
    assert_eq!(out, t.path().join("out.pdf"));
    let check = qpdf_check(&out);
    assert!(!check.contains("WARNING"), "{check}");
    let d = lopdf::Document::load(&out).unwrap();
    let first = *d.get_pages().get(&1).unwrap();
    assert_eq!(d.get_object(first).unwrap().as_dict().unwrap().get(b"Rotate").unwrap().as_i64().unwrap(), 90);
    assert_eq!(page_texts(&out), ["Page 1", "Page 2"]);
}

#[test]
fn save_never_overwrites() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let target = t.path().join("out.pdf");
    std::fs::write(&target, b"keep me").unwrap();
    let doc = Doc::open(&Input::new(&a)).unwrap();
    let out = doc.save(&target).unwrap();
    assert_eq!(out, t.path().join("out (1).pdf"));
    assert_eq!(std::fs::read(&target).unwrap(), b"keep me");
    let out2 = doc.save_incremental(&target).unwrap();
    assert_eq!(out2, t.path().join("out (2).pdf"));
    assert!(!std::fs::read_dir(t.path()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".part")));
}

#[test]
fn save_incremental_appends_to_the_original_bytes() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let original = std::fs::read(&a).unwrap();
    let mut doc = Doc::open(&Input::new(&a)).unwrap();
    doc.apply(Edit::RotatePages { pages: vec![0], degrees: 180 }).unwrap();
    let out = doc.save_incremental(&t.path().join("inc.pdf")).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > original.len() && bytes.starts_with(&original), "an incremental save keeps the original bytes as its prefix");
    qpdf_check(&out);
}

#[test]
fn bad_edit_is_an_engine_error() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let mut doc = Doc::open(&Input::new(&a)).unwrap();
    assert!(matches!(doc.apply(Edit::DeletePages { pages: vec![7] }), Err(OpdfError::Engine(_))));
}

