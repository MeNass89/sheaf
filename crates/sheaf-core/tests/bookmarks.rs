mod common;
use common::fixture_pdf;
use sheaf_core::bookmarks::add;
use sheaf_core::pages::Input;

/// The top-level bookmarks: title and the page object each opens.
fn outline(path: &std::path::Path) -> Vec<(String, lopdf::ObjectId)> {
    let d = lopdf::Document::load(path).unwrap();
    let root = d.catalog().unwrap().get(b"Outlines").unwrap().as_reference().unwrap();
    let mut next = d.get_dictionary(root).unwrap().get(b"First").ok().map(|o| o.as_reference().unwrap());
    let mut out = Vec::new();
    while let Some(id) = next {
        let item = d.get_dictionary(id).unwrap();
        let title = lopdf::decode_text_string(item.get(b"Title").unwrap()).unwrap();
        let page = item.get(b"Dest").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        out.push((title, page));
        next = item.get(b"Next").ok().map(|o| o.as_reference().unwrap());
    }
    out
}

// `sheaf bookmark` (Preview's Shortcuts actions, Acrobat's bookmarkRoot.createChild): a bookmark
// added to a file without an outline, then a second one after it, each opening its page.
#[test]
fn bookmarks_are_added_after_the_last_one_at_their_pages() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let one = add(&Input::new(&a), "Introduction", 1, &t.path().join("one.pdf")).unwrap();
    let two = add(&Input::new(&one), "  Résumé  ", 3, &t.path().join("two.pdf")).unwrap();
    let d = lopdf::Document::load(&two).unwrap();
    let pages = d.get_pages();
    assert_eq!(outline(&two), [("Introduction".to_string(), pages[&1]), ("Résumé".to_string(), pages[&3])]);
    // The outline is well formed: two items counted, linked both ways.
    let root = d.catalog().unwrap().get(b"Outlines").unwrap().as_reference().unwrap();
    let r = d.get_dictionary(root).unwrap();
    assert_eq!(r.get(b"Count").unwrap().as_i64().unwrap(), 2);
    let last = d.get_dictionary(r.get(b"Last").unwrap().as_reference().unwrap()).unwrap();
    assert_eq!(last.get(b"Prev").unwrap().as_reference().unwrap(), r.get(b"First").unwrap().as_reference().unwrap());
    assert_eq!(pages.len(), 3);
}

#[test]
fn a_bookmark_needs_a_page_of_the_document_and_a_title() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    assert!(add(&Input::new(&a), "Far", 9, &t.path().join("x.pdf")).is_err());
    assert!(add(&Input::new(&a), "   ", 1, &t.path().join("y.pdf")).is_err());
    assert!(!t.path().join("x.pdf").exists());
}
