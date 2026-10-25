mod common;
use common::fixture_pdf;
use sheaf_core::{facts, pages::Input, pdfa::{to_pdfa, PdfALevel}, security::*};

#[test]
fn properties_read_restrictions_pdfa_and_dates_from_the_file() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let plain = facts::read(&std::fs::read(&a).unwrap(), None);
    assert!(plain.restricted.is_empty() && plain.pdfa.is_none(), "{plain:?}");
    // An ordinary page: its media and crop boxes.
    assert_eq!(plain.boxes.iter().map(|b| b.0).collect::<Vec<_>>(), ["Media box", "Crop box"]);

    let perms = ProtectPermissions { printing: false, copying: false, ..Default::default() };
    let p = protect_with_permissions(&Input::new(&a), "user", Some("owner"), perms, &t.path().join("p.pdf")).unwrap();
    let f = facts::read(&std::fs::read(&p).unwrap(), Some("user"));
    assert_eq!(f.restricted, ["Printing", "Copying text and images"]);
    // No open password, only an owner one: read as it opens, restrictions kept.
    let q = t.path().join("q.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "", "owner", "256", "--print=none", "--"]).arg(&a).arg(&q).status().unwrap().success());
    assert_eq!(facts::read(&std::fs::read(&q).unwrap(), None).restricted, ["Printing"]);

    let pa = to_pdfa(&Input::new(&a), PdfALevel::A2b, &t.path().join("pa.pdf")).unwrap().path;
    let f = facts::read(&std::fs::read(&pa).unwrap(), None);
    assert_eq!(f.pdfa.as_deref(), Some("PDF/A-2b"));
    assert!(f.modified.is_some() || f.created.is_some(), "{f:?}");
}
