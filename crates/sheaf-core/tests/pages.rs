mod common;
use common::{fixture_pdf, page_texts};
use sheaf_core::{pages::*, OpdfError};

#[test]
fn merge_concatenates_in_order() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let b = fixture_pdf(t.path(), "b.pdf", 1);
    let out = merge(&[Input::new(&b), Input::new(&a)], &t.path().join("m.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 1", "Page 1", "Page 2"]);
}
#[test]
fn split_ranges_writes_one_file_per_range() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 5);
    let outs = split_ranges(&Input::new(&a), "1-2,5", t.path()).unwrap();
    assert_eq!(outs.len(), 2);
    assert!(outs[0].ends_with("a-1-2.pdf"));
    assert_eq!(page_texts(&outs[1]), ["Page 5"]);
}
#[test]
fn split_every_chunks() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 5);
    let outs = split_every(&Input::new(&a), 2, t.path()).unwrap();
    assert_eq!(outs.iter().map(|p| page_texts(p).len()).collect::<Vec<_>>(), [2, 2, 1]);
}
#[test]
fn remove_drops_pages() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 4);
    let out = remove(&Input::new(&a), "2-3", &t.path().join("r.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 1", "Page 4"]);
}
#[test]
fn remove_all_pages_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let e = remove(&Input::new(&a), "1-2", &t.path().join("r.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::WouldBeEmpty { pages: 2 }), "{e:?}");
    assert_eq!(e.to_string(), "cannot remove every page (the document has 2 pages)");
}
#[test]
fn organize_reorders_duplicates_and_inserts_blank() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let out = organize(&Input::new(&a), &[3, 0, 1, 1], &t.path().join("o.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 3", "", "Page 1", "Page 1"]);
}
#[test]
fn load_encrypted_without_password_is_password_required() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let enc = t.path().join("e.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "u", "u", "256", "--"]).arg(&a).arg(&enc).status().unwrap().success());
    let e = merge(&[Input::new(&enc)], &t.path().join("m.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::PasswordRequired(_)));
    let ok = merge(&[Input { path: enc, password: Some("u".into()) }], &t.path().join("m.pdf")).unwrap();
    assert_eq!(page_texts(&ok), ["Page 1"]);
}

#[test]
fn concurrent_merges_do_not_crash() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    std::thread::scope(|s| {
        for th in 0..8 {
            let (a, dir) = (&a, t.path());
            s.spawn(move || {
                for i in 0..20 {
                    let out = merge(&[Input::new(a), Input::new(a)], &dir.join(format!("m-{th}-{i}.pdf"))).unwrap();
                    assert_eq!(lopdf::Document::load(&out).unwrap().get_pages().len(), 4);
                }
            });
        }
    });
}

#[test]
fn split_every_huge_n_gives_one_file() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let outs = split_every(&Input::new(&a), u32::MAX, t.path()).unwrap();
    assert_eq!(outs.len(), 1);
    assert_eq!(page_texts(&outs[0]).len(), 3);
}
#[test]
fn encryption_is_detected_including_owner_only() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    assert!(!Input::new(&a).is_encrypted());
    let owner_only = t.path().join("o.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "", "owner", "256", "--"]).arg(&a).arg(&owner_only).status().unwrap().success());
    assert!(Input::new(&owner_only).is_encrypted());
    let user = t.path().join("u.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "u", "u", "128", "--use-aes=y", "--"]).arg(&a).arg(&user).status().unwrap().success());
    assert!(Input::new(&user).is_encrypted());
    assert!(Input { path: user, password: Some("u".into()) }.is_encrypted());
}
