mod common;
use sheaf_core::{office::*, OpdfError};
use std::time::Duration;

#[test]
fn local_html_prints_on_the_chosen_paper_with_its_relative_picture() {
    if find_soffice().is_none() {
        eprintln!("skip: no LibreOffice");
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("source folder");
    std::fs::create_dir(&source).unwrap();
    image::RgbImage::from_pixel(64, 64, image::Rgb([0, 0, 0])).save(source.join("picture.png")).unwrap();
    let html = source.join("page.html");
    std::fs::write(&html, "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body><p>Local HTML paper</p><img src=\"picture.png\" width=\"64\" height=\"64\"></body></html>").unwrap();
    for landscape in [false, true] {
        let out = sheaf_core::html::to_pdf(&html, sheaf_core::html::Options { paper: sheaf_core::html::Paper::Letter, landscape, margin: 18.0 }, &t.path().join(format!("page-{landscape}.pdf"))).unwrap();
        let doc = lopdf::Document::load(&out).unwrap();
        let id = doc.get_pages()[&1];
        let media = doc.get_dictionary(id).unwrap().get(b"MediaBox").unwrap().as_array().unwrap();
        let number = |i: usize| media[i].as_float().unwrap() as f64;
        let (w, h) = if landscape { (792.0, 612.0) } else { (612.0, 792.0) };
        assert!((number(2) - number(0) - w).abs() < 1.0 && (number(3) - number(1) - h).abs() < 1.0);
        assert!(common::page_texts(&out)[0].contains("Local HTML paper"));
        assert!(doc.objects.values().any(|o| o.as_stream().is_ok_and(|s| s.dict.get(b"Subtype").is_ok_and(|s| s.as_name().is_ok_and(|n| n == b"Image")))));
        common::qpdf_clean(&out);
    }
}

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
#[cfg(unix)]
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
#[cfg(unix)]
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
#[cfg(unix)]
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

/// A Word file with two heading levels and a web link, made by LibreOffice from a flat ODT.
fn headed_docx(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let fodt = dir.join("headed.fodt");
    std::fs::write(&fodt, r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.text">
<office:body><office:text>
<text:h text:outline-level="1">Chapter One</text:h><text:p>Intro with a <text:a xlink:type="simple" xlink:href="https://example.org/">link</text:a>.</text:p>
<text:h text:outline-level="2">Section A</text:h><text:p>Text.</text:p>
<text:h text:outline-level="1">Chapter Two</text:h><text:p>More.</text:p>
</office:text></office:body></office:document>"#).unwrap();
    let st = std::process::Command::new(find_soffice()?)
        .args(["--headless", "--norestore", "--convert-to", "docx", "--outdir"])
        .arg(dir)
        .arg(&fodt)
        .env("HOME", dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    let docx = dir.join("headed.docx");
    (st.success() && docx.exists()).then_some(docx)
}

#[test]
fn a_word_file_keeps_its_headings_as_bookmarks_and_its_links() {
    let t = tempfile::tempdir().unwrap();
    let Some(docx) = headed_docx(t.path()) else {
        eprintln!("skip: no LibreOffice");
        return;
    };
    let out = office_to_pdf(&docx, &t.path().join("headed.pdf")).unwrap();
    let doc = lopdf::Document::load(&out).unwrap();
    let mut titles = Vec::new();
    let mut uris = Vec::new();
    for o in doc.objects.values() {
        if let Ok(d) = o.as_dict() {
            if let Ok(lopdf::Object::String(s, _)) = d.get(b"Title")
                && d.has(b"Parent")
            {
                let t: Vec<u16> = s.chunks(2).skip(1).map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])).collect();
                titles.push(if s.starts_with(&[0xFE, 0xFF]) { String::from_utf16_lossy(&t) } else { String::from_utf8_lossy(s).into_owned() });
            }
            // A link annotation's action, inline or its own object.
            let action = d.get(b"A").ok().and_then(|a| match a {
                lopdf::Object::Reference(r) => doc.get_dictionary(*r).ok(),
                a => a.as_dict().ok(),
            });
            if let Some(Ok(lopdf::Object::String(u, _))) = action.map(|a| a.get(b"URI")) {
                uris.push(String::from_utf8_lossy(u).into_owned());
            }
        }
    }
    titles.sort();
    assert_eq!(titles, ["Chapter One", "Chapter Two", "Section A"], "the headings are the bookmarks");
    assert_eq!(uris, ["https://example.org/"], "the link still goes there");
}

/// Stirling's Markdown and Email to PDF, Preview's PostScript: a Markdown file's heading and
/// table, a mail's subject and text with its attachment attached, a PostScript page's words.
#[test]
fn a_markdown_file_a_mail_and_postscript_become_pdfs() {
    let t = tempfile::tempdir().unwrap();
    if find_soffice().is_some() {
        let md = t.path().join("notes.md");
        std::fs::write(&md, "# Release notes\n\n| Item | Done |\n|---|---|\n| Layers | yes |\n").unwrap();
        let text = common::page_texts(&office_to_pdf(&md, &t.path().join("notes.pdf")).unwrap()).join(" ");
        assert!(text.contains("Release notes") && text.contains("Layers"), "{text}");
        let eml = t.path().join("mail.eml");
        std::fs::write(&eml, "From: Ada <ada@x.org>\r\nSubject: Quarterly figures\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nThe figures are attached.\r\n--b\r\nContent-Disposition: attachment; filename=\"figures.csv\"\r\n\r\na,b\r\n--b--\r\n").unwrap();
        let pdf = office_to_pdf(&eml, &t.path().join("mail.pdf")).unwrap();
        let text = common::page_texts(&pdf).join(" ");
        assert!(text.contains("Quarterly figures") && text.contains("The figures are attached"), "{text}");
        let raw = std::fs::read(&pdf).unwrap();
        assert!(raw.windows(11).any(|w| w == b"figures.csv"), "the attachment is in the PDF");
    } else {
        eprintln!("skip: no LibreOffice for Markdown and mail");
    }
    if std::process::Command::new("ps2pdf").arg("--version").output().is_ok() || std::path::Path::new("/usr/bin/ps2pdf").exists() {
        let ps = t.path().join("page.ps");
        std::fs::write(&ps, "%!PS\n/Helvetica findfont 24 scalefont setfont 72 700 moveto (PostScript page) show showpage\n").unwrap();
        let out = office_to_pdf(&ps, &t.path().join("page.pdf")).unwrap();
        assert!(common::page_texts(&out)[0].contains("PostScript page"));
    }
}
