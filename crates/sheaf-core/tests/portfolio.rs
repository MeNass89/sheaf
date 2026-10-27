use sheaf_core::portfolio;
use lopdf::Object;

#[test]
fn a_portfolio_keeps_original_files_and_duplicate_basenames_instead_of_merging() {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join("other")).unwrap();
    let a = t.path().join("résumé.txt");
    let b = t.path().join("other/résumé.txt");
    std::fs::write(&a, b"first original").unwrap();
    std::fs::write(&b, b"second original").unwrap();
    let out = t.path().join("portfolio.pdf");
    std::fs::write(&out, b"existing output").unwrap();
    let written = portfolio::create(&[a.clone(), b.clone()], "Project bundle", &out).unwrap();
    assert_ne!(written, out);
    assert_eq!(std::fs::read(out).unwrap(), b"existing output");
    let doc = lopdf::Document::load(written).unwrap();
    assert_eq!(doc.get_pages().len(), 1);
    let catalog = doc.catalog().unwrap();
    let collection = catalog.get(b"Collection").unwrap().as_dict().unwrap();
    assert_eq!(collection.get(b"View").unwrap().as_name().unwrap(), b"D");
    assert_eq!(catalog.get(b"PageMode").unwrap().as_name().unwrap(), b"UseAttachments");
    let tree = catalog.get(b"Names").unwrap().as_dict().unwrap().get(b"EmbeddedFiles").unwrap().as_reference().unwrap();
    let names = doc.get_dictionary(tree).unwrap().get(b"Names").unwrap().as_array().unwrap();
    assert_eq!(names.len(), 4);
    assert_ne!(names[0], names[2]);
    let mut originals = Vec::new();
    for pair in names.as_chunks::<2>().0 {
        let spec = doc.get_dictionary(pair[1].as_reference().unwrap()).unwrap();
        let stream = spec.get(b"EF").unwrap().as_dict().unwrap().get(b"F").unwrap().as_reference().unwrap();
        let Object::Stream(stream) = doc.get_object(stream).unwrap() else { panic!("embedded file stream") };
        originals.push(stream.decompressed_content().unwrap_or_else(|_| stream.content.clone()));
    }
    originals.sort();
    assert_eq!(originals, [b"first original".to_vec(), b"second original".to_vec()]);
    assert_eq!(std::fs::read(a).unwrap(), b"first original");
    assert_eq!(std::fs::read(b).unwrap(), b"second original");
}

#[test]
fn a_missing_portfolio_input_does_not_publish_a_partial_bundle() {
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("portfolio.pdf");
    assert!(portfolio::create(&[], "", &out).is_err());
    assert!(portfolio::create(&[t.path().join("missing")], "", &out).is_err());
    assert!(!out.exists());
}
