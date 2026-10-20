use std::process::Command;
fn bin() -> Command { Command::new(env!("CARGO_BIN_EXE_sheaf")) }
#[test]
fn merge_then_protect_then_unlock() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a.pdf");
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    assert!(bin().arg("from-images").arg(&png).arg("-o").arg(&a).status().unwrap().success());
    let m = t.path().join("m.pdf");
    assert!(bin().arg("merge").arg(&a).arg(&a).arg("-o").arg(&m).status().unwrap().success());
    let p = t.path().join("p.pdf");
    assert!(bin().arg("protect").arg(&m).args(["--user-password", "pw", "-o"]).arg(&p).status().unwrap().success());
    let no = bin().arg("unlock").arg(&p).arg("-o").arg(t.path().join("u.pdf")).status().unwrap();
    assert_eq!(no.code(), Some(2));
    let ok = bin().arg("unlock").arg(&p).args(["--password", "pw", "-o"]).arg(t.path().join("u.pdf")).output().unwrap();
    assert!(ok.status.success());
    assert!(String::from_utf8_lossy(&ok.stdout).trim().ends_with("u.pdf"));
}

#[test]
fn usage_errors_exit_1_not_2() {
    let t = tempfile::tempdir().unwrap();
    let out = bin().args(["organize", "x.pdf", "--order", "3,a,1", "-o"]).arg(t.path().join("o.pdf")).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid"));
}

#[test]
fn split_every_prints_one_path_per_line() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg(&png).arg(&png).arg("--a4").arg("-o").arg(&a).status().unwrap().success());
    let d = t.path().join("parts");
    std::fs::create_dir(&d).unwrap();
    let out = bin().arg("split").arg(&a).args(["--every", "1", "-o"]).arg(&d).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let lines: Vec<_> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines.iter().all(|l| std::path::Path::new(l).is_file()));
}

#[test]
fn rotate_accepts_negative_degrees() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg("-o").arg(&a).status().unwrap().success());
    let r = bin().arg("rotate").arg(&a).args(["--degrees", "-90", "-o"]).arg(t.path().join("r.pdf")).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let bad = bin().arg("rotate").arg(&a).args(["--degrees", "45", "-o"]).arg(t.path().join("b.pdf")).status().unwrap();
    assert_eq!(bad.code(), Some(1));
}

#[test]
fn missing_output_dirs_are_created() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("deep/er/a.pdf");
    let r = bin().arg("from-images").arg(&png).arg(&png).arg("-o").arg(&a).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(a.is_file());
    let d = t.path().join("new/sub");
    let out = bin().arg("split").arg(&a).args(["--every", "1", "-o"]).arg(&d).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let lines: Vec<_> = String::from_utf8_lossy(&out.stdout).lines().map(std::path::PathBuf::from).collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().all(|l| l.is_file() && l.parent() == Some(d.as_path())));
}

fn protected_pdf(dir: &std::path::Path) -> std::path::PathBuf {
    let png = dir.join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = dir.join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg(&png).arg("-o").arg(&a).status().unwrap().success());
    let p = dir.join("p.pdf");
    assert!(bin().arg("protect").arg(&a).args(["--user-password", "pw", "-o"]).arg(&p).status().unwrap().success());
    p
}

const NOTE: &str = "note: the output is not password-protected";

#[test]
fn tools_on_a_protected_input_note_that_the_output_is_not() {
    let t = tempfile::tempdir().unwrap();
    let p = protected_pdf(t.path());
    let o = |n: &str| t.path().join(n);
    let runs: Vec<Vec<std::ffi::OsString>> = vec![
        vec!["rotate".into(), p.clone().into(), "--degrees".into(), "90".into(), "-o".into(), o("r.pdf").into()],
        vec!["merge".into(), p.clone().into(), p.clone().into(), "-o".into(), o("m.pdf").into()],
        vec!["extract".into(), p.clone().into(), "--ranges".into(), "1".into(), "-o".into(), o("e.pdf").into()],
        vec!["remove".into(), p.clone().into(), "--pages".into(), "1".into(), "-o".into(), o("x.pdf").into()],
        vec!["organize".into(), p.clone().into(), "--order".into(), "2,1".into(), "-o".into(), o("g.pdf").into()],
        vec!["split".into(), p.clone().into(), "--every".into(), "1".into(), "-o".into(), o("parts").into()],
        vec!["repair".into(), p.clone().into(), "-o".into(), o("f.pdf").into()],
        vec!["compress".into(), p.clone().into(), "-o".into(), o("c.pdf").into()],
        vec!["to-jpg".into(), p.clone().into(), "-o".into(), o("jpgs").into()],
        vec!["number".into(), p.clone().into(), "-o".into(), o("n.pdf").into()],
        vec!["watermark".into(), p.clone().into(), "--text".into(), "DRAFT".into(), "-o".into(), o("w.pdf").into()],
        vec!["crop".into(), p.clone().into(), "--top".into(), "1".into(), "-o".into(), o("k.pdf").into()],
        vec!["pdfa".into(), p.clone().into(), "-o".into(), o("pa.pdf").into()],
        vec!["ocr".into(), p.clone().into(), "-o".into(), o("oc.pdf").into()],
    ];
    for args in runs {
        let r = bin().args(&args).args(["--password", "pw"]).output().unwrap();
        let err = String::from_utf8_lossy(&r.stderr);
        assert!(r.status.success(), "{args:?}: {err}");
        assert_eq!(err.matches(NOTE).count(), 1, "{args:?}: {err}");
    }
    // Unlock removes protection on purpose, Protect adds it: no note.
    let u = bin().arg("unlock").arg(&p).args(["--password", "pw", "-o"]).arg(o("u.pdf")).output().unwrap();
    assert!(u.status.success() && !String::from_utf8_lossy(&u.stderr).contains(NOTE));
    let pp = bin().arg("protect").arg(&p).args(["--password", "pw", "--user-password", "new", "-o"]).arg(o("pp.pdf")).output().unwrap();
    assert!(pp.status.success() && !String::from_utf8_lossy(&pp.stderr).contains(NOTE));
    // Unprotected input: no note.
    let plain = bin().arg("rotate").arg(t.path().join("a.pdf")).args(["--degrees", "90", "-o"]).arg(o("r2.pdf")).output().unwrap();
    assert!(plain.status.success() && !String::from_utf8_lossy(&plain.stderr).contains(NOTE));
}

#[test]
fn protect_rejects_an_empty_password() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg("-o").arg(&a).status().unwrap().success());
    let r = bin().arg("protect").arg(&a).args(["--user-password", "", "-o"]).arg(t.path().join("p.pdf")).output().unwrap();
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stderr).contains("password must not be empty"));
    assert!(!t.path().join("p.pdf").exists());
}

fn text_of(p: &std::path::Path, page: u32) -> String {
    lopdf::Document::load(p).unwrap().extract_text(&[page]).unwrap()
}

#[test]
fn number_watermark_and_crop_subcommands() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg(&png).arg(&png).arg("--a4").arg("-o").arg(&a).status().unwrap().success());
    let o = |n: &str| t.path().join(n);
    let run = |args: &[&str], out: &std::path::Path| {
        let r = bin().arg(args[0]).arg(&a).args(&args[1..]).arg("-o").arg(out).output().unwrap();
        assert!(r.status.success(), "{args:?}: {}", String::from_utf8_lossy(&r.stderr));
        assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
    };
    run(&["number", "--cover", "--text", "custom", "--custom", "p{n}/{p}", "--position", "top-right", "--start", "5"], &o("n.pdf"));
    assert!(!text_of(&o("n.pdf"), 1).contains("p5"));
    assert!(text_of(&o("n.pdf"), 2).contains("p5/3") && text_of(&o("n.pdf"), 3).contains("p6/3"));
    run(&["watermark", "--text", "SECRET", "--from", "2", "--transparency", "50", "--rotate", "-45", "--below", "--position", "mosaic"], &o("w.pdf"));
    assert!(!text_of(&o("w.pdf"), 1).contains("SECRET") && text_of(&o("w.pdf"), 2).contains("SECRET"));
    run(&["watermark", "--image", png.to_str().unwrap(), "--scale", "0.3"], &o("wi.pdf"));
    run(&["crop", "--top", "20", "--pages", "2"], &o("c.pdf"));
    let d = lopdf::Document::load(o("c.pdf")).unwrap();
    let pages = d.get_pages();
    assert!(d.get_dictionary(pages[&1]).unwrap().get(b"CropBox").is_err());
    let b = d.get_dictionary(pages[&2]).unwrap().get(b"CropBox").unwrap().as_array().unwrap().clone();
    let top = b[3].as_float().unwrap();
    let media = d.get_dictionary(pages[&2]).unwrap().get(b"MediaBox").unwrap().as_array().unwrap()[3].as_float().unwrap();
    assert!((media - top - 20.0 * 72.0 / 25.4).abs() < 0.1, "{media} {top}");
    run(&["crop", "--remove"], &o("r.pdf"));
    // A watermark needs text or an image; a number position must be in the top or bottom row.
    assert_eq!(bin().arg("watermark").arg(&a).arg("-o").arg(o("bad.pdf")).status().unwrap().code(), Some(1));
    assert_eq!(bin().arg("number").arg(&a).args(["--position", "middle-left", "-o"]).arg(o("bad.pdf")).status().unwrap().code(), Some(1));
}

#[test]
fn bad_ranges_and_bounds_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg(&png).arg("-o").arg(&a).status().unwrap().success());
    let out = t.path().join("o.pdf");
    let fails = |args: &[&str], msg: &str| {
        let r = bin().arg(args[0]).arg(&a).args(&args[1..]).arg("-o").arg(&out).output().unwrap();
        let err = String::from_utf8_lossy(&r.stderr);
        assert_eq!(r.status.code(), Some(1), "{args:?}: {err}");
        assert!(err.contains(msg), "{args:?}: {err}");
    };
    fails(&["number", "--from", "0"], "invalid page range");
    fails(&["watermark", "--text", "X", "--from", "0"], "invalid page range");
    fails(&["number", "--text", "custom", "--custom", "<<m/d/yyyy>>"], "custom text cannot contain << or >>");
    fails(&["watermark", "--image", png.to_str().unwrap(), "--scale", "1.5"], "0.05 to 1");
    fails(&["watermark", "--image", png.to_str().unwrap(), "--scale", "0"], "0.05 to 1");
    fails(&["watermark", "--text", "X", "--font-size", "900"], "1 to 500");
    fails(&["watermark", "--text", "X", "--font-size", "big"], "font size");
    assert!(!out.exists());
    let r = bin().arg("watermark").arg(&a).args(["--text", "X", "--font-size", "auto", "-o"]).arg(&out).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
}

#[test]
fn pdfa_subcommand_reports_conformance() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg("-o").arg(&a).status().unwrap().success());
    for (level, label, part) in [("2b", "PDF/A-2b", "2"), ("3b", "PDF/A-3b", "3")] {
        let out = t.path().join(format!("a-{level}.pdf"));
        let r = bin().arg("pdfa").arg(&a).args(["--level", level, "-o"]).arg(&out).output().unwrap();
        let err = String::from_utf8_lossy(&r.stderr);
        assert!(r.status.success(), "{err}");
        assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
        assert!(err.contains(&format!("Conforms to {label}")), "{err}");
        let bytes = std::fs::read(&out).unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains(&format!("pdfaid:part>{part}<")));
    }
    assert_eq!(bin().arg("pdfa").arg(&a).args(["--level", "1a", "-o"]).arg(t.path().join("x.pdf")).status().unwrap().code(), Some(1));
}

#[test]
fn pdfa_exits_1_when_rules_are_still_broken_but_writes_the_file() {
    // Helvetica, not embedded: the converter cannot fix it (clause 6.2.11.4).
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("text.pdf");
    let body = "%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >> endobj\n4 0 obj << /Length 37 >>\nstream\nBT /F1 12 Tf 20 100 Td (Hello) Tj ET\nendstream endobj\n5 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n";
    std::fs::write(&a, body).unwrap();
    let out = t.path().join("text-pdfa.pdf");
    let r = bin().arg("pdfa").arg(&a).arg("-o").arg(&out).output().unwrap();
    let err = String::from_utf8_lossy(&r.stderr);
    assert_eq!(r.status.code(), Some(1), "{err}");
    assert!(err.contains("warning: PDF/A-2b (6.2.11.4)") && err.contains("does not fully conform"), "{err}");
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
    assert!(out.exists());
    let help = bin().args(["pdfa", "--help"]).output().unwrap();
    assert!(String::from_utf8_lossy(&help.stdout).contains("the exit status is 1"));
}

#[test]
fn ocr_subcommand_reports_what_it_read() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg("-o").arg(&a).status().unwrap().success());
    let out = t.path().join("a-ocr.pdf");
    let r = bin().arg("ocr").arg(&a).args(["--language", "en", "-o"]).arg(&out).output().unwrap();
    let err = String::from_utf8_lossy(&r.stderr);
    assert!(r.status.success(), "{err}");
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
    // The same line the app shows (sheaf_core::ocr::summary).
    assert!(err.contains("1 page read, 0 words added"), "{err}");
    let bad = bin().arg("ocr").arg(&a).args(["--language", "xx", "-o"]).arg(t.path().join("b.pdf")).output().unwrap();
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("OCR language"));
}

fn unzip(file: &std::path::Path, member: &str) -> String {
    let o = Command::new("unzip").arg("-p").arg(file).arg(member).output().unwrap();
    assert!(o.status.success(), "{member}");
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// A PDF made by the CLI itself whose page carries the text "Sheaf 2026" (page numbers).
fn text_pdf(dir: &std::path::Path) -> std::path::PathBuf {
    let png = dir.join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = dir.join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg("--a4").arg("-o").arg(&a).status().unwrap().success());
    let n = dir.join("n.pdf");
    let r = bin().arg("number").arg(&a).args(["--text", "custom", "--custom", "Sheaf 2026", "-o"]).arg(&n).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    n
}

#[test]
fn word_powerpoint_and_excel_subcommands() {
    let t = tempfile::tempdir().unwrap();
    let n = text_pdf(t.path());
    let run = |args: &[&str], out: &std::path::Path| {
        let r = bin().arg(args[0]).arg(&n).args(&args[1..]).arg("-o").arg(out).output().unwrap();
        assert!(r.status.success(), "{args:?}: {}", String::from_utf8_lossy(&r.stderr));
        assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
        String::from_utf8_lossy(&r.stderr).into_owned()
    };
    run(&["to-word"], &t.path().join("n.docx"));
    assert!(unzip(&t.path().join("n.docx"), "word/document.xml").contains("Sheaf 2026"));
    run(&["to-powerpoint"], &t.path().join("n.pptx"));
    assert!(unzip(&t.path().join("n.pptx"), "ppt/slides/slide1.xml").contains("Sheaf 2026"));
    let err = run(&["to-excel", "--layout", "sheet-per-table"], &t.path().join("n.xlsx"));
    assert!(err.contains("No tables found: the text is in column A"), "{err}");
    assert!(unzip(&t.path().join("n.xlsx"), "xl/sharedStrings.xml").contains("Sheaf 2026"));
    run(&["to-word", "--ocr"], &t.path().join("o.docx"));
}

#[test]
fn compare_subcommand_prints_the_counts() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg("--a4").arg("-o").arg(&a).status().unwrap().success());
    let v = |text: &str, name: &str| {
        let out = t.path().join(name);
        assert!(bin().arg("number").arg(&a).args(["--text", "custom", "--custom", text, "-o"]).arg(&out).status().unwrap().success());
        out
    };
    let old = v("Draft one of the plan", "old.pdf");
    let new = v("Draft two of the plan", "new.pdf");
    for mode in ["text", "overlay"] {
        let out = t.path().join(format!("{mode}.pdf"));
        let r = bin().args(["compare"]).arg(&old).arg(&new).args(["--mode", mode, "-o"]).arg(&out).output().unwrap();
        let err = String::from_utf8_lossy(&r.stderr);
        assert!(r.status.success(), "{err}");
        assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
        assert!(err.contains("0 added, 0 removed, 1 changed"), "{mode}: {err}");
    }
    let r = bin().arg("compare").arg(&old).arg(&old).arg("-o").arg(t.path().join("same.pdf")).output().unwrap();
    assert!(String::from_utf8_lossy(&r.stderr).contains("No differences: the documents' text is identical."));
}

#[test]
fn redact_subcommand_counts_the_areas_and_removes_the_text() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = t.path().join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg(&png).arg("--a4").arg("-o").arg(&a).status().unwrap().success());
    let src = t.path().join("src.pdf");
    let text = "Write to ada@example.com or call (555) 123-4567 about the secret";
    assert!(bin().arg("number").arg(&a).args(["--text", "custom", "--custom", text, "-o"]).arg(&src).status().unwrap().success());
    let out = t.path().join("out.pdf");
    let r = bin().arg("redact").arg(&src).args(["--text", "secret", "--pattern", "email", "--pattern", "phone", "--pages", "2", "-o"]).arg(&out).output().unwrap();
    let err = String::from_utf8_lossy(&r.stderr);
    assert!(r.status.success(), "{err}");
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
    // 3 matches on each of the 2 pages; page 2 is also marked whole.
    assert!(err.contains("7 areas redacted"), "{err}");
    assert!(err.contains("No text remains under the black boxes. Matches also removed from metadata, bookmarks and comments."), "{err}");
    let d = lopdf::Document::load(&out).unwrap();
    let p1 = d.extract_text(&[1]).unwrap();
    assert!(p1.contains("Write to") && !p1.contains("ada@") && !p1.contains("secret") && !p1.contains("4567"), "{p1}");
    assert!(!d.extract_text(&[2]).unwrap().contains("Write"));
    // Nothing found and nothing marked: an error, no file.
    let none = t.path().join("none.pdf");
    let r = bin().arg("redact").arg(&src).args(["--text", "absent", "-o"]).arg(&none).output().unwrap();
    assert!(!r.status.success() && !none.exists());
    assert!(String::from_utf8_lossy(&r.stderr).contains("nothing to redact"), "{}", String::from_utf8_lossy(&r.stderr));
}

#[test]
fn sign_subcommand_visual_and_digital() {
    let t = tempfile::tempdir().unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sheaf-core/tests/fixtures/invoice.pdf");
    // Visual, typed.
    let out = t.path().join("typed.pdf");
    let r = bin().arg("sign").arg(&src).args(["--at", "72,600", "--text", "Ada Lovelace", "-o"]).arg(&out).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(String::from_utf8_lossy(&r.stderr).trim(), "Signed");
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
    // Visual, drawn from a JSON file.
    let strokes = t.path().join("s.json");
    std::fs::write(&strokes, "[[[0,0],[0.5,0.3],[1,0]]]").unwrap();
    let out = t.path().join("drawn.pdf");
    assert!(bin().arg("sign").arg(&src).args(["--page", "1", "--at", "300,100", "--width", "120", "--strokes"]).arg(&strokes).arg("-o").arg(&out).status().unwrap().success());
    // Digital, with a throwaway openssl identity.
    let (key, cert, p12) = (t.path().join("k.pem"), t.path().join("c.pem"), t.path().join("id.p12"));
    assert!(Command::new("openssl").args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-subj", "/CN=Sheaf CLI Test", "-keyout"]).arg(&key).arg("-out").arg(&cert).output().unwrap().status.success());
    assert!(Command::new("openssl").args(["pkcs12", "-export", "-passout", "pass:pw", "-inkey"]).arg(&key).arg("-in").arg(&cert).arg("-out").arg(&p12).output().unwrap().status.success());
    let out = t.path().join("digital.pdf");
    let r = bin().arg("sign").arg(&src).args(["--at", "72,600", "--text", "Ada", "--p12"]).arg(&p12).args(["--p12-password", "pw", "--reason", "OK", "-o"]).arg(&out).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(String::from_utf8_lossy(&r.stderr).trim(), "Signed digitally (certificate: Sheaf CLI Test)");
    // Wrong certificate password: exit 1 (2 is for PDF passwords), its own message, no file.
    let bad = t.path().join("bad.pdf");
    let r = bin().arg("sign").arg(&src).args(["--at", "72,600", "--text", "Ada", "--p12"]).arg(&p12).args(["--p12-password", "nope", "-o"]).arg(&bad).output().unwrap();
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stderr).contains("wrong certificate password"), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(!bad.exists());
    // No graphic: a usage error.
    let r = bin().arg("sign").arg(&src).args(["--at", "72,600", "-o"]).arg(&bad).output().unwrap();
    assert_eq!(r.status.code(), Some(1));
    // Protected input: refused.
    let locked = t.path().join("locked.pdf");
    assert!(Command::new("qpdf").args(["--encrypt", "u", "o", "256", "--"]).arg(&src).arg(&locked).status().unwrap().success());
    let r = bin().arg("sign").arg(&locked).args(["--password", "u", "--at", "72,600", "--text", "Ada", "-o"]).arg(&bad).output().unwrap();
    assert!(!r.status.success() && !bad.exists());
    assert!(String::from_utf8_lossy(&r.stderr).contains("unlock the PDF first"), "{}", String::from_utf8_lossy(&r.stderr));
}

#[test]
fn edit_subcommand_adds_text_and_images_in_order() {
    let t = tempfile::tempdir().unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sheaf-core/tests/fixtures/invoice.pdf");
    let png = t.path().join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let out = t.path().join("edited.pdf");
    let r = bin()
        .arg("edit")
        .arg(&src)
        .args(["--text", "Approved", "--page", "1", "--at", "72,40", "--size", "18", "--color", "#C00000"])
        .args(["--text", "Second", "--at", "72,90"])
        .arg("--image")
        .arg(&png)
        .args(["--at", "400,40", "--width", "60", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(String::from_utf8_lossy(&r.stderr).trim(), "3 items added");
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), out.display().to_string());
    let bbox = Command::new("pdftotext").args(["-cropbox", "-bbox"]).arg(&out).arg("-").output().unwrap();
    let s = String::from_utf8_lossy(&bbox.stdout);
    let word = |w: &str| s.lines().find(|l| l.ends_with(&format!(">{w}</word>"))).unwrap_or_else(|| panic!("{w} missing"));
    assert!(word("Approved").contains("xMin=\"72.000000\""), "{}", word("Approved"));
    assert!(word("Second").contains("xMin=\"72.000000\""), "{}", word("Second"));
    let page = lopdf::Document::load(&out).unwrap();
    let content = String::from_utf8_lossy(&page.get_page_content(page.get_pages()[&1])).into_owned();
    assert!(content.contains(" 18 Tf 0.752941 0 0 rg") || content.contains(" 18 Tf 0.75"), "{content}");
    assert!(content.contains(" 12 Tf 0 0 0 rg"));

    // Usage errors: an option before any item, an item without --at, a bad colour, an image as text.
    let bad = t.path().join("bad.pdf");
    for args in [
        vec!["--page", "1", "--text", "x", "--at", "1,1"],
        vec!["--text", "x"],
        vec!["--text", "x", "--at", "1,1", "--color", "red"],
        vec!["--text", "x", "--at", "1,1", "--width", "50"],
        vec!["--at", "1,1"],
    ] {
        let r = bin().arg("edit").arg(&src).args(&args).arg("-o").arg(&bad).output().unwrap();
        assert_eq!(r.status.code(), Some(1), "{args:?}: {}", String::from_utf8_lossy(&r.stderr));
        assert!(!bad.exists(), "{args:?}");
    }
    // Off the page: the core's error, exit 1, nothing written.
    let r = bin().arg("edit").arg(&src).args(["--text", "x", "--at", "5000,10", "-o"]).arg(&bad).output().unwrap();
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stderr).contains("outside the page"), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(!bad.exists());
    // Not an image: refused, nothing written.
    let r = bin().arg("edit").arg(&src).arg("--image").arg(&src).args(["--at", "1,1", "--width", "50", "-o"]).arg(&bad).output().unwrap();
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stderr).contains("not a PNG or JPEG"), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(!bad.exists());
    // Protected input: edited, with the note.
    let locked = t.path().join("locked.pdf");
    assert!(Command::new("qpdf").args(["--encrypt", "u", "o", "256", "--"]).arg(&src).arg(&locked).status().unwrap().success());
    let r = bin().arg("edit").arg(&locked).args(["--password", "u", "--text", "x", "--at", "10,10", "-o"]).arg(&bad).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(String::from_utf8_lossy(&r.stderr).contains("note: the output is not password-protected"), "{}", String::from_utf8_lossy(&r.stderr));
}

/// A one-page PDF from the PNG fixture, in `dir`.
fn one_page(dir: &std::path::Path) -> std::path::PathBuf {
    let png = dir.join("x.png");
    std::fs::write(&png, include_bytes!("fixtures/8x8.png")).unwrap();
    let a = dir.join("a.pdf");
    assert!(bin().arg("from-images").arg(&png).arg("--a4").arg("-o").arg(&a).status().unwrap().success());
    a
}

fn json_of(out: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

#[test]
fn json_success_and_errors() {
    let t = tempfile::tempdir().unwrap();
    let a = one_page(t.path());
    let out = bin().arg("--json").arg("compress").arg(&a).arg("-o").arg(t.path().join("c.pdf")).output().unwrap();
    let v = json_of(&out);
    assert_eq!(v["ok"], true);
    assert!(v["outputs"][0].as_str().unwrap().ends_with("c.pdf"));
    assert!(v["notes"][0].as_str().unwrap().contains("bytes"), "{v}");
    assert!(out.stderr.is_empty(), "notes go to the JSON, not stderr");

    let p = t.path().join("p.pdf");
    assert!(bin().arg("protect").arg(&a).args(["--user-password", "pw", "-o"]).arg(&p).status().unwrap().success());
    let out = bin().arg("unlock").arg(&p).args(["--json", "-o"]).arg(t.path().join("u.pdf")).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(json_of(&out)["code"], "password_required");

    let out = bin().args(["--json", "organize", "x.pdf", "--order", "3,a", "-o", "o.pdf"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(json_of(&out)["code"], "usage");
}

#[test]
fn info_text_and_render() {
    let t = tempfile::tempdir().unwrap();
    let a = one_page(t.path());
    let m = t.path().join("m.pdf");
    assert!(bin().arg("merge").arg(&a).arg(&a).arg("-o").arg(&m).status().unwrap().success());
    let v = json_of(&bin().arg("--json").arg("info").arg(&m).output().unwrap());
    assert_eq!(v["data"]["page_count"], 2);
    assert_eq!(v["data"]["encrypted"], false);
    assert!((v["data"]["pages"][0]["width"].as_f64().unwrap() - 595.0).abs() < 1.0);

    let v = json_of(&bin().arg("--json").arg("text").arg(&m).args(["--pages", "2"]).output().unwrap());
    assert_eq!(v["data"].as_array().unwrap().len(), 1);

    let out = bin().arg("--json").arg("render").arg(&m).args(["--page", "2", "--dpi", "36", "-o"]).arg(t.path().join("p.png")).output().unwrap();
    let png = json_of(&out)["outputs"][0].as_str().unwrap().to_string();
    let img = image::open(&png).unwrap();
    assert_eq!(img.width(), 298); // 595 pt at 36 dpi
    let out = bin().arg("--json").arg("render").arg(&m).args(["--page", "3", "-o"]).arg(t.path().join("q.png")).output().unwrap();
    assert_eq!(json_of(&out)["code"], "unsupported");
}

/// One MCP session: requests in, one reply per request out.
fn mcp(requests: &[serde_json::Value], dir: &std::path::Path) -> Vec<serde_json::Value> {
    use std::io::Write;
    let mut child = bin().arg("mcp").current_dir(dir).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for r in requests {
        writeln!(stdin, "{r}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn mcp_lists_and_calls_tools() {
    use serde_json::json;
    let t = tempfile::tempdir().unwrap();
    one_page(t.path());
    let replies = mcp(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "rotate", "arguments": {"input": "a.pdf", "degrees": -90, "out": "r.pdf"}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "edit", "arguments": {"input": "a.pdf", "items": [{"text": "Approved", "at": "72,72", "color": "#c00000"}, {"image": "x.png", "page": 1, "at": "300,300", "width": 50}], "out": "e.pdf"}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "render", "arguments": {"input": "e.pdf", "dpi": 30, "out": "e.png"}}}),
            json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "merge", "arguments": {"inputs": ["a.pdf", "r.pdf"], "out": "m.pdf"}}}),
            json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "compress", "arguments": {"input": "a.pdf", "out": "c.pdf", "nope": 1}}}),
            json!({"jsonrpc": "2.0", "id": 8, "method": "nope"}),
        ],
        t.path(),
    );
    assert_eq!(replies.len(), 8, "no reply to a notification");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "sheaf");
    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["merge", "split", "redact", "sign", "edit", "to_word", "info", "text", "render"] {
        assert!(names.contains(&n), "{n} missing from {names:?}");
    }
    assert!(!names.contains(&"mcp") && !names.contains(&"tools"));
    let ok = |i: usize| {
        let r = &replies[i]["result"];
        assert_eq!(r["isError"], false, "{r}");
        r["structuredContent"].clone()
    };
    assert!(ok(2)["outputs"][0].as_str().unwrap().ends_with("r.pdf"));
    assert_eq!(ok(3)["notes"][0], "2 items added");
    ok(4);
    assert_eq!(replies[4]["result"]["content"][1]["mimeType"], "image/png");
    ok(5);
    let v = json_of(&bin().arg("--json").arg("info").arg(t.path().join("m.pdf")).output().unwrap());
    assert_eq!(v["data"]["pages"][1]["rotation"], 270);
    assert_eq!(replies[6]["result"]["isError"], true);
    assert_eq!(replies[7]["error"]["code"], -32601);
}

/// Every tool's schema turns back into a command line clap accepts (no drift between the two).
#[test]
fn every_tool_schema_parses() {
    let v = json_of(&bin().arg("--json").arg("tools").output().unwrap());
    let tools = v["data"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 28);
    for t in tools {
        let s = &t["inputSchema"];
        assert_eq!(s["type"], "object");
        for r in s["required"].as_array().unwrap() {
            assert!(s["properties"].get(r.as_str().unwrap()).is_some(), "{}: required {r} not in properties", t["name"]);
        }
    }
}
