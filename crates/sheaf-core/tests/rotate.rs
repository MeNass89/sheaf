mod common;
use common::{fixture_pdf, page_texts};
use sheaf_core::{pages::Input, rotate::rotate};
#[test]
fn rotate_keeps_text_and_sets_rotate() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let out = rotate(&Input::new(&a), Some("1,3"), 90, &t.path().join("r.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 1", "Page 2", "Page 3"]);
    let d = lopdf::Document::load(&out).unwrap();
    let rot = |n: u32| d.get_dictionary(d.get_pages()[&n]).unwrap().get(b"Rotate").and_then(|o| o.as_i64()).unwrap_or(0);
    assert_eq!((rot(1), rot(2), rot(3)), (90, 0, 90));
}
#[test]
fn rotate_accumulates_mod_360() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let r1 = rotate(&Input::new(&a), None, 270, &t.path().join("r1.pdf")).unwrap();
    let r2 = rotate(&Input::new(&r1), None, 180, &t.path().join("r2.pdf")).unwrap();
    let d = lopdf::Document::load(&r2).unwrap();
    assert_eq!(d.get_dictionary(d.get_pages()[&1]).unwrap().get(b"Rotate").unwrap().as_i64().unwrap(), 90);
}
#[test]
fn rotate_rejects_odd_angle() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    assert!(rotate(&Input::new(&a), None, 45, &t.path().join("r.pdf")).is_err());
}

/// Moves page 1's own /Rotate (if any) to the /Pages root as `root_rotate`; page keeps none.
fn with_inherited_rotate(src: &std::path::Path, out: &std::path::Path, root_rotate: i64) {
    let mut d = lopdf::Document::load(src).unwrap();
    let page = d.get_pages()[&1];
    d.get_dictionary_mut(page).unwrap().remove(b"Rotate");
    let root = d.catalog().unwrap().get(b"Pages").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root).unwrap().set("Rotate", root_rotate);
    d.save(out).unwrap();
}
fn page_rotate(p: &std::path::Path) -> i64 {
    let d = lopdf::Document::load(p).unwrap();
    d.get_dictionary(d.get_pages()[&1]).unwrap().get(b"Rotate").unwrap().as_i64().unwrap()
}
#[test]
fn rotate_adds_to_inherited_rotation() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let inh = t.path().join("inh.pdf");
    with_inherited_rotate(&a, &inh, 90);
    let out = rotate(&Input::new(&inh), None, 90, &t.path().join("r.pdf")).unwrap();
    assert_eq!(page_rotate(&out), 180);
}
#[test]
fn rotate_huge_existing_value_does_not_panic() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let mut d = lopdf::Document::load(&a).unwrap();
    let page = d.get_pages()[&1];
    d.get_dictionary_mut(page).unwrap().set("Rotate", i64::MAX);
    let big = t.path().join("big.pdf");
    d.save(&big).unwrap();
    let out = rotate(&Input::new(&big), None, 90, &t.path().join("r.pdf")).unwrap();
    assert_eq!(page_rotate(&out), (i64::MAX.rem_euclid(360) + 90).rem_euclid(360));
}
#[test]
fn rotate_and_unlock_of_protected_file_pass_qpdf_check_without_warnings() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let p = sheaf_core::security::protect(&Input::new(&a), "u", None, &t.path().join("p.pdf")).unwrap();
    let q = t.path().join("q.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "u", "o", "256", "--"]).arg(&a).arg(&q).status().unwrap().success());
    for src in [p, q] {
        let i = Input { path: src, password: Some("u".into()) };
        let r = rotate(&i, None, 90, &t.path().join("r.pdf")).unwrap();
        let u = sheaf_core::security::unlock(&i, &t.path().join("u.pdf")).unwrap();
        for out in [r, u] {
            let c = std::process::Command::new("qpdf").arg("--check").arg(&out).output().unwrap();
            let all = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
            assert_eq!(c.status.code(), Some(0), "{all}");
            assert!(!all.contains("WARNING"), "{}: {all}", out.display());
        }
    }
}
