mod common;
use sheaf_core::{office::*, OpdfError};
use std::time::Duration;

#[test]
fn missing_soffice_is_reported() {
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("a.txt");
    std::fs::write(&f, "hello").unwrap();
    let e = office_to_pdf_with(Some(std::path::Path::new("/nonexistent/soffice")), &f, &t.path().join("a.pdf"), Duration::from_secs(5)).unwrap_err();
    assert!(matches!(e, OpdfError::OfficeMissing), "{e:?}");
}

#[test]
fn converts_docx_like_text_and_html() {
    if find_soffice().is_none() {
        eprintln!("skip: no LibreOffice");
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let txt = t.path().join("note.txt");
    std::fs::write(&txt, "Bonjour Sheaf").unwrap();
    let out = office_to_pdf(&txt, &t.path().join("note.pdf")).unwrap();
    assert!(common::page_texts(&out)[0].contains("Bonjour Sheaf"));
    let html = t.path().join("p.html");
    std::fs::write(&html, "<h1>Titre</h1><p>Paragraphe</p>").unwrap();
    let out = office_to_pdf(&html, &t.path().join("p.pdf")).unwrap();
    assert!(common::page_texts(&out)[0].contains("Titre"));
}

#[test]
fn two_conversions_in_parallel() {
    if find_soffice().is_none() {
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let hs: Vec<_> = (0..2)
        .map(|i| {
            let d = t.path().to_path_buf();
            std::thread::spawn(move || {
                let f = d.join(format!("n{i}.txt"));
                std::fs::write(&f, format!("doc {i}")).unwrap();
                office_to_pdf(&f, &d.join(format!("n{i}.pdf"))).unwrap()
            })
        })
        .collect();
    for h in hs {
        assert!(h.join().unwrap().exists());
    }
}

#[test]
fn converts_real_docx() {
    let textutil = std::path::Path::new("/usr/bin/textutil");
    if find_soffice().is_none() || !textutil.exists() {
        eprintln!("skip: no LibreOffice or textutil");
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let txt = t.path().join("memo.txt");
    std::fs::write(&txt, "Memo de service Sheaf").unwrap();
    let docx = t.path().join("memo.docx");
    let st = std::process::Command::new(textutil)
        .args(["-convert", "docx", "-output"])
        .arg(&docx)
        .arg(&txt)
        .status()
        .unwrap();
    assert!(st.success() && docx.exists());
    let out = office_to_pdf(&docx, &t.path().join("memo.pdf")).unwrap();
    assert!(common::page_texts(&out)[0].contains("Memo de service Sheaf"), "{:?}", common::page_texts(&out));
}

#[test]
fn timeout_kills_and_reports() {
    // A fake "soffice" that sleeps forever: the conversion must time out, not hang.
    let t = tempfile::tempdir().unwrap();
    let fake = t.path().join("fake-soffice");
    std::fs::write(&fake, "#!/bin/sh\nsleep 30\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let f = t.path().join("a.txt");
    std::fs::write(&f, "hello").unwrap();
    let start = std::time::Instant::now();
    let e = office_to_pdf_with(Some(&fake), &f, &t.path().join("a.pdf"), Duration::from_secs(1)).unwrap_err();
    assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
    assert!(matches!(&e, OpdfError::Conversion(m) if m.contains("timed out after 1s")), "{e:?}");
    assert!(!t.path().join("a.pdf").exists());
}

#[test]
fn failing_soffice_reports_last_stderr_line() {
    let t = tempfile::tempdir().unwrap();
    let fake = t.path().join("fake-soffice");
    std::fs::write(&fake, "#!/bin/sh\necho 'Fontconfig warning' >&2\necho 'Error: source file could not be loaded' >&2\nexit 1\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let f = t.path().join("a.txt");
    std::fs::write(&f, "hello").unwrap();
    let e = office_to_pdf_with(Some(&fake), &f, &t.path().join("a.pdf"), Duration::from_secs(5)).unwrap_err();
    assert!(matches!(&e, OpdfError::Conversion(m) if m == "Error: source file could not be loaded"), "{e:?}");
}

#[test]
fn converts_leading_dash_unicode_space_name() {
    if find_soffice().is_none() {
        eprintln!("skip: no LibreOffice");
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("-é x.txt");
    std::fs::write(&f, "Nom bizarre Sheaf").unwrap();
    let out = office_to_pdf(&f, &t.path().join("-é x.pdf")).unwrap();
    assert!(out.exists());
    assert!(common::page_texts(&out)[0].contains("Nom bizarre Sheaf"), "{:?}", common::page_texts(&out));
}

#[test]
fn exit_zero_without_pdf_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("a.txt");
    std::fs::write(&f, "hello").unwrap();
    let quiet = t.path().join("quiet-soffice");
    std::fs::write(&quiet, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&quiet, std::fs::Permissions::from_mode(0o755)).unwrap();
    let e = office_to_pdf_with(Some(&quiet), &f, &t.path().join("a.pdf"), Duration::from_secs(5)).unwrap_err();
    assert!(matches!(&e, OpdfError::Conversion(m) if m == "LibreOffice produced no PDF (exit 0)"), "{e:?}");
    let chatty = t.path().join("chatty-soffice");
    std::fs::write(&chatty, "#!/bin/sh\necho 'Error: no export filter' >&2\nexit 0\n").unwrap();
    std::fs::set_permissions(&chatty, std::fs::Permissions::from_mode(0o755)).unwrap();
    let e = office_to_pdf_with(Some(&chatty), &f, &t.path().join("a.pdf"), Duration::from_secs(5)).unwrap_err();
    assert!(matches!(&e, OpdfError::Conversion(m) if m == "LibreOffice produced no PDF (exit 0): Error: no export filter"), "{e:?}");
}
