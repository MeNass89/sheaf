//! Article threads (Acrobat's Article tool): pdfcraft-engine's `articles.rs`, through
//! `Edit::AddArticleBox` and `Edit::DeleteArticle`.

mod common;
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::engine::articles::Article;
use sheaf_core::pdfcraft::engine::Edit;
use sheaf_core::pdfcraft::Doc;

/// Two columns on page 1 and one on page 2 chained into one article: read back in that order,
/// saved as a ring of beads every reader follows (the last box leads back to the first), each
/// page listing its boxes; deleted, the article and its boxes go; one undo step each.
#[test]
fn boxes_are_chained_into_an_article_across_columns_and_pages() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = common::fixture_pdf(dir.path(), "story.pdf", 2);
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    let (left, right, next) = ([72.0, 100.0, 290.0, 700.0], [322.0, 100.0, 540.0, 700.0], [72.0, 400.0, 540.0, 700.0]);
    doc.apply(Edit::AddArticleBox { thread: None, title: "Story".into(), page: 0, rect: left }).unwrap();
    doc.apply(Edit::AddArticleBox { thread: Some(0), title: String::new(), page: 0, rect: right }).unwrap();
    doc.apply(Edit::AddArticleBox { thread: Some(0), title: String::new(), page: 1, rect: next }).unwrap();
    let story = Article { title: "Story".into(), beads: vec![(0, left), (0, right), (1, next)] };
    assert_eq!(doc.document().articles(), [story.clone()]);
    // A second article, of one box.
    doc.apply(Edit::AddArticleBox { thread: None, title: "Sidebar".into(), page: 1, rect: [72.0, 100.0, 200.0, 300.0] }).unwrap();
    assert_eq!(doc.document().articles().len(), 2);
    let out = doc.save(&dir.path().join("articles.pdf")).unwrap();
    common::qpdf_clean(&out);
    // Read again from the file: the same, and the ring closes.
    let again = Doc::open(&Input::new(&out)).unwrap();
    assert_eq!(again.document().articles()[0], story);
    let saved = lopdf::Document::load(&out).unwrap();
    let catalog = saved.catalog().unwrap();
    let threads = saved.dereference(catalog.get(b"Threads").unwrap()).unwrap().1.as_array().unwrap().clone();
    let thread = saved.get_dictionary(threads[0].as_reference().unwrap()).unwrap();
    let first = thread.get(b"F").unwrap().as_reference().unwrap();
    let mut at = first;
    for _ in 0..3 {
        at = saved.get_dictionary(at).unwrap().get(b"N").unwrap().as_reference().unwrap();
    }
    assert_eq!(at, first, "three boxes, then back to the first");
    let page2 = saved.get_pages()[&2];
    let on_page2 = saved.get_dictionary(page2).unwrap().get(b"B").unwrap().as_array().unwrap().len();
    assert_eq!(on_page2, 2, "page 2 lists its two boxes");
    doc.apply(Edit::DeleteArticle { thread: 0 }).unwrap();
    assert_eq!(doc.document().articles().iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), ["Sidebar"]);
    let id = doc.id();
    doc.session_mut().undo(id).unwrap();
    assert_eq!(doc.document().articles().len(), 2, "one undo brings it back");
}
