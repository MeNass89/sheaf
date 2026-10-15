mod common;
use common::{fixture_pdf, page_texts};
use sheaf_core::{pages::Input, repair::repair, security::*, OpdfError};
use std::process::Command;

fn qpdf(args: &[&str]) -> String {
    String::from_utf8(Command::new("qpdf").args(args).output().unwrap().stdout).unwrap()
}
#[test]
fn protect_writes_aes256_r6() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let out = protect(&Input::new(&a), "user", None, &t.path().join("p.pdf")).unwrap();
    let info = qpdf(&["--show-encryption", "--password=user", out.to_str().unwrap()]);
    assert!(info.contains("R = 6"), "{info}");
    let check = Command::new("qpdf").args(["--password=user", "--check"]).arg(&out).output().unwrap();
    let all = format!("{}{}", String::from_utf8_lossy(&check.stdout), String::from_utf8_lossy(&check.stderr));
    assert_eq!(check.status.code(), Some(0), "{all}");
    assert!(!all.contains("WARNING"), "{all}");
    let _g = sheaf_core::engine::pdfium_lock();
    assert!(matches!(sheaf_core::engine::open_pdf(&out, None), Err(OpdfError::PasswordRequired(_))));
}
#[test]
fn unlock_roundtrips() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let p = protect(&Input::new(&a), "user", Some("owner"), &t.path().join("p.pdf")).unwrap();
    let u = unlock(&Input { path: p, password: Some("user".into()) }, &t.path().join("u.pdf")).unwrap();
    assert_eq!(page_texts(&u), ["Page 1", "Page 2"]);
    assert!(qpdf(&["--show-encryption", u.to_str().unwrap()]).contains("not encrypted"));
}
#[test]
fn unlock_wrong_password() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let p = protect(&Input::new(&a), "user", None, &t.path().join("p.pdf")).unwrap();
    let e = unlock(&Input { path: p, password: Some("nope".into()) }, &t.path().join("u.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::WrongPassword(_)), "{e:?}");
}
#[test]
fn repair_rebuilds_truncated_xref() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let mut bytes = std::fs::read(&a).unwrap();
    let cut = bytes.windows(4).rposition(|w| w == b"xref").unwrap();
    bytes.truncate(cut); // drop the xref table and trailer
    let broken = t.path().join("broken.pdf");
    std::fs::write(&broken, bytes).unwrap();
    let out = repair(&Input::new(&broken), &t.path().join("fixed.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 1", "Page 2"]);
    assert!(Command::new("qpdf").args(["--check"]).arg(&out).status().unwrap().code() != Some(2));
}
#[test]
fn empty_user_password_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let e = protect(&Input::new(&a), "", None, &t.path().join("p.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::Unsupported(_)), "{e:?}");
    assert!(e.to_string().contains("password must not be empty"));
    assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), 1);
}
#[test]
fn repair_of_protected_file_is_unencrypted() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let p = protect(&Input::new(&a), "u", None, &t.path().join("p.pdf")).unwrap();
    let out = repair(&Input { path: p, password: Some("u".into()) }, &t.path().join("r.pdf")).unwrap();
    assert!(qpdf(&["--show-encryption", out.to_str().unwrap()]).contains("not encrypted"));
    assert_eq!(page_texts(&out), ["Page 1", "Page 2"]);
    let c = Command::new("qpdf").arg("--check").arg(&out).output().unwrap();
    assert_eq!(c.status.code(), Some(0), "{c:?}");
}
