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
    assert_eq!(doc.document().articles(), std::slice::from_ref(&story));
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

/// A box of an article moved or resized, then the middle one deleted: the article goes on from
/// the box before it to the one after; its first deleted, the article starts at the next; its
/// last, the article goes (Acrobat's Article tool).
#[test]
fn an_article_box_is_moved_resized_and_deleted_alone() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = common::fixture_pdf(dir.path(), "boxes.pdf", 2);
    let mut doc = Doc::open(&Input::new(&pdf)).unwrap();
    let (a, b, c) = ([72.0, 100.0, 290.0, 700.0], [322.0, 100.0, 540.0, 700.0], [72.0, 400.0, 540.0, 700.0]);
    doc.apply(Edit::AddArticleBox { thread: None, title: "Story".into(), page: 0, rect: a }).unwrap();
    doc.apply(Edit::AddArticleBox { thread: Some(0), title: String::new(), page: 0, rect: b }).unwrap();
    doc.apply(Edit::AddArticleBox { thread: Some(0), title: String::new(), page: 1, rect: c }).unwrap();
    let moved = [330.0, 120.0, 500.0, 600.0];
    doc.apply(Edit::SetArticleBox { thread: 0, bead: 1, rect: moved }).unwrap();
    assert_eq!(doc.document().articles()[0].beads, [(0, a), (0, moved), (1, c)]);
    assert!(doc.apply(Edit::SetArticleBox { thread: 0, bead: 1, rect: [0.0, 0.0, 1.0, 1.0] }).is_err(), "too small");
    doc.apply(Edit::DeleteArticleBox { thread: 0, bead: 1 }).unwrap();
    assert_eq!(doc.document().articles()[0].beads, [(0, a), (1, c)]);
    doc.apply(Edit::DeleteArticleBox { thread: 0, bead: 0 }).unwrap();
    assert_eq!(doc.document().articles(), [Article { title: "Story".into(), beads: vec![(1, c)] }]);
    let out = doc.save(&dir.path().join("boxes-out.pdf")).unwrap();
    common::qpdf_clean(&out);
    assert_eq!(Doc::open(&Input::new(&out)).unwrap().document().articles()[0].beads, [(1, c)], "saved so");
    doc.apply(Edit::DeleteArticleBox { thread: 0, bead: 0 }).unwrap();
    assert!(doc.document().articles().is_empty(), "its last box takes the article");
}
