mod common;
use sheaf_core::{pages::Input, search_index::Catalog, security, OpdfError};

#[test]
fn a_catalog_build_reads_real_pdfs_and_reuses_only_unchanged_pages() {
    let t = tempfile::tempdir().unwrap();
    let a = common::fixture_pdf(t.path(), "a.pdf", 2);
    let b = common::fixture_pdf(t.path(), "b.pdf", 1);
    let catalog = Catalog::build(&[Input::new(&a), Input::new(&b), Input::new(&a)], &mut |_, _| true).unwrap();
    assert_eq!(catalog.search("page 1").unwrap().len(), 2, "the duplicate input is indexed once");
    assert_eq!(catalog.search("page 2").unwrap().len(), 1);
    assert_eq!(catalog.texts(&a).unwrap().len(), 2);
    let saved = catalog.save(&t.path().join("catalog.json")).unwrap();
    assert_eq!(Catalog::load(&saved).unwrap().search("page 2").unwrap().len(), 1);
    std::fs::remove_file(&a).unwrap();
    assert_eq!(Catalog::load(&saved).unwrap().search("page 1").unwrap().len(), 1);
}

#[test]
fn protected_sources_and_cancelled_catalog_builds_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let source = common::fixture_pdf(t.path(), "source.pdf", 1);
    assert!(matches!(Catalog::build(&[Input::new(&source)], &mut |_, _| false), Err(OpdfError::Cancelled)));
    let protected = security::protect(&Input::new(&source), "secret", None, &t.path().join("protected.pdf")).unwrap();
    assert!(Catalog::build(&[Input { path: protected, password: Some("secret".into()) }], &mut |_, _| true).unwrap_err().to_string().contains("protected text"));
}

#[test]
fn catalog_manager_can_reopen_sources_and_rebuild_after_a_removal() {
    let t = tempfile::tempdir().unwrap();
    let a = common::fixture_pdf(t.path(), "a.pdf", 2);
    let b = common::fixture_pdf(t.path(), "b.pdf", 1);
    let catalog = Catalog::build(&[Input::new(&a), Input::new(&b)], &mut |_, _| true).unwrap();
    let path = catalog.save(&t.path().join("catalog.json")).unwrap();
    let reopened = Catalog::load(&path).unwrap();
    assert_eq!(reopened.sources(), [(a.canonicalize().unwrap(), 2), (b.canonicalize().unwrap(), 1)]);
    let remaining: Vec<_> = reopened.sources().into_iter().skip(1).map(|(path, _)| Input::new(path)).collect();
    let rebuilt = Catalog::build(&remaining, &mut |_, _| true).unwrap();
    assert_eq!(rebuilt.search("Page 1").unwrap().len(), 1);
    assert!(rebuilt.search("Page 2").unwrap().is_empty());
    let rebuilt_path = rebuilt.save(&path).unwrap();
    assert_ne!(rebuilt_path, path, "an existing catalog gets a fresh output name");
    assert_eq!(Catalog::load(&rebuilt_path).unwrap().sources().len(), 1);
    assert_eq!(Catalog::load(&path).unwrap().sources().len(), 2);
}

#[test]
fn incremental_folder_catalog_reuses_unchanged_sources_and_purges_removed_ones() {
    let t = tempfile::tempdir().unwrap();
    let folder = t.path().join("collection");
    std::fs::create_dir(&folder).unwrap();
    let a = common::fixture_pdf(&folder, "a.pdf", 1);
    let b = common::fixture_pdf(&folder, "b.pdf", 2);
    let catalog = Catalog::build(&[Input::new(&folder)], &mut |_, _| true).unwrap();
    std::fs::remove_file(&a).unwrap();
    let updated = catalog.update(&[Input::new(&folder)], &mut |_, _| true).unwrap();
    assert_eq!(updated.sources(), [(b.canonicalize().unwrap(), 2)]);
    assert_eq!(updated.search("page 2").unwrap().len(), 1);
    assert_eq!(catalog.search("page 1").unwrap().len(), 1);
    // Catalogs keep canonical paths (macOS: /private/var for /var); `a` is gone, so its folder is resolved.
    assert_eq!(catalog.stale_sources(), [folder.canonicalize().unwrap().join("a.pdf")]);
    assert!(matches!(catalog.update(&[Input::new(&folder)], &mut |_, _| false), Err(OpdfError::Cancelled)));
}
