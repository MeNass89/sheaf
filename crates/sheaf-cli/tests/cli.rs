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
