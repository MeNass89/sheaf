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
    // The random key strings are hex strings, never literal ones (see the loop test below).
    let bytes = std::fs::read(&out).unwrap();
    for key in ["/O", "/U", "/OE", "/UE", "/Perms"] {
        let lit = format!("{key}(");
        assert!(!bytes.windows(lit.len()).any(|w| w == lit.as_bytes()), "{key} written as a literal string");
    }
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
/// Protect's random salts, keys and AES ciphertext sometimes contain two end-of-line bytes in a
/// row. Written raw inside a literal string, PdfCraft's reader folded the run into one byte and
/// refused the file ("InvalidEncryption"); every output must open in PdfCraft and pdfium.
#[test]
fn protect_outputs_always_open_in_pdfcraft_and_pdfium() {
    let t = tempfile::tempdir().unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf");
    let mut failures = Vec::new();
    for i in 0..200 {
        let out = protect(&Input::new(&src), "pw", None, &t.path().join(format!("p{i}.pdf"))).unwrap();
        let input = Input { path: out.clone(), password: Some("pw".into()) };
        if let Err(e) = sheaf_core::pdfcraft::Doc::open(&input) {
            failures.push(format!("{i}: pdfcraft: {e}"));
        }
        let _g = sheaf_core::engine::pdfium_lock();
        if let Err(e) = sheaf_core::engine::open_pdf(&out, Some("pw")) {
            failures.push(format!("{i}: pdfium: {e}"));
        }
        std::fs::remove_file(&out).unwrap();
    }
    assert!(failures.is_empty(), "{} of 200 outputs failed to open:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn permissions_are_independent_and_default_to_allowed() {
    let t = tempfile::tempdir().unwrap();
    let input = Input::new(fixture_pdf(t.path(), "source.pdf", 1));
    let default = ProtectPermissions::default();
    let cases = [
        default,
        ProtectPermissions { printing: false, ..default },
        ProtectPermissions { copying: false, ..default },
        ProtectPermissions { editing: false, ..default },
        ProtectPermissions { commenting: false, ..default },
        ProtectPermissions { filling_forms: false, ..default },
        ProtectPermissions { printing: false, copying: false, editing: false, commenting: false, filling_forms: false },
    ];
    for (i, permissions) in cases.into_iter().enumerate() {
        let out = protect_with_permissions(&input, "user", Some("owner"), permissions, &t.path().join(format!("p{i}.pdf"))).unwrap();
        let result = Command::new("qpdf").args(["--show-encryption", "--password=user"]).arg(&out).output().unwrap();
        assert!(result.status.success(), "{result:?}");
        let info = String::from_utf8(result.stdout).unwrap();
        for (label, allowed) in [
            ("print low resolution", permissions.printing),
            ("print high resolution", permissions.printing),
            ("extract for any purpose", permissions.copying),
            ("modify other", permissions.editing),
            ("modify document assembly", permissions.editing),
            ("modify annotations", permissions.commenting),
            ("modify forms", permissions.filling_forms),
            ("extract for accessibility", true),
        ] {
            let expected = format!("{label}: {}", if allowed { "allowed" } else { "not allowed" });
            assert!(info.lines().any(|line| line == expected), "expected {expected}:\n{info}");
        }
        assert!(qpdf(&["--show-encryption", "--password=owner", out.to_str().unwrap()]).contains("Supplied password is owner password"));
    }
}

/// Without an owner password, the open password must not grant every right: the restrictions hold.
#[test]
fn restrictions_hold_when_no_owner_password_is_given() {
    let t = tempfile::tempdir().unwrap();
    let input = Input::new(fixture_pdf(t.path(), "source.pdf", 1));
    for (i, owner) in [None, Some("")].into_iter().enumerate() {
        let out = protect_with_permissions(&input, "user", owner,
            ProtectPermissions { printing: false, ..Default::default() }, &t.path().join(format!("p{i}.pdf"))).unwrap();
        let info = qpdf(&["--show-encryption", "--password=user", out.to_str().unwrap()]);
        assert!(info.contains("Supplied password is user password"), "{info}");
        assert!(info.contains("print low resolution: not allowed"), "{info}");
        assert!(info.contains("print high resolution: not allowed"), "{info}");
    }
}

#[test]
fn restrictions_alone_open_without_a_password() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let perms = ProtectPermissions { printing: false, ..Default::default() };
    let out = protect_with_permissions(&Input::new(&a), "", Some("owner"), perms, &t.path().join("r.pdf")).unwrap();
    // Opens with no password; printing is not allowed.
    let info = qpdf(&["--show-encryption", out.to_str().unwrap()]);
    assert!(info.contains("print low resolution: not allowed"), "{info}");
    assert_eq!(page_texts(&out), ["Page 1"]);
    assert!(protect_with_permissions(&Input::new(&a), "", None, perms, &t.path().join("x.pdf")).is_err());
}

#[test]
fn protecting_a_protected_file_changes_its_password() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let old = protect(&Input::new(&a), "old", None, &t.path().join("old.pdf")).unwrap();
    let new = protect(&Input { path: old.clone(), password: Some("old".into()) }, "new", None, &t.path().join("new.pdf")).unwrap();
    let opens = |pw: &str| Command::new("qpdf").arg(format!("--password={pw}")).arg("--check").arg(&new).output().unwrap().status.success();
    assert!(opens("new") && !opens("old"), "the new password opens it, the old one no longer does");
}
