use super::*;
use pdfium_render::prelude::*;
use sheaf_core::pdfcraft::engine::{Markup, NewAnnotation, NoteIcon, Shape, Style};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

#[test]
fn a_check_mark_is_not_a_review_status() {
    assert_eq!(review_status("Accepted"), Some("Accepted"));
    assert_eq!(review_status("Marked"), None);
    assert_eq!(review_status("Unmarked"), None);
    assert_eq!(review_status("None"), Some(""));
}
#[test]
fn wraps_become_spaces_and_typed_breaks_stay() {
    let soft = [true, false, true];
    assert_eq!(super::reflowed("one two\nthree\n- item pseudo-\nmarisques", &soft), "one two three\n- item pseudo-marisques");
    assert_eq!(super::reflowed("ends with space \nnext", &[true]), "ends with space next");
    // Rows and flags that disagree: left as drawn.
    assert_eq!(super::reflowed("a\nb", &[]), "a\nb");
}


/// A PDF whose page i (1-based) says "Page i".
fn fixture(dir: &Path, pages: u32) -> PathBuf {
    let _g = sheaf_core::engine::pdfium_lock();
    let mut doc = sheaf_core::engine::new_document();
    let font = doc.fonts_mut().helvetica();
    for i in 1..=pages {
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(720.0), format!("Page {i}"), font, PdfPoints::new(24.0)).unwrap();
    }
    let path = dir.join("doc.pdf");
    doc.save_to_file(&path).unwrap();
    path
}

fn started(path: &Path) -> (Sender<Op>, Receiver<Reply>, Snap) {
    let (tx, rx) = channel();
    let ops = start(Input::new(path), move |r| {
        let _ = tx.send(r);
    });
    let snap = match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Opened(s) => *s,
        _ => panic!("expected Opened"),
    };
    (ops, rx, snap)
}

/// The next Changed or Saved reply: its snapshot and error.
fn next(rx: &Receiver<Reply>) -> (Snap, Option<String>) {
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Changed { snap, error, .. } | Reply::Saved { snap, error, .. } => (*snap, error),
        Reply::Extracted { error, .. } => panic!("unexpected Extracted ({error:?})"),
        _ => panic!("unexpected reply"),
    }
}

/// A PDF written from its objects (1 the catalog), with its cross-reference table.
fn hand_pdf(objs: &[String]) -> Vec<u8> {
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).bytes());
    }
    let xref = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    offsets.iter().for_each(|o| pdf.extend(format!("{o:010} 00000 n \n").bytes()));
    pdf.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).bytes());
    pdf
}

fn highlight(page: usize) -> Edit {
    Edit::AddAnnotation(NewAnnotation {
        page,
        shape: Shape::TextMarkup { kind: Markup::Highlight, quads: vec![[72.0, 744.0, 160.0, 744.0, 72.0, 718.0, 160.0, 718.0]] },
        style: Style::default(),
        contents: "Page 1".into(),
        author: String::new(),
    })
}

fn qpdf_clean(path: &Path) {
    let o = std::process::Command::new("qpdf").arg("--check").arg(path).output().unwrap();
    let s = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(o.status.success() && !s.contains("WARNING"), "qpdf --check: {s}");
}

fn texts(path: &Path) -> Vec<String> {
    let d = lopdf::Document::load(path).unwrap();
    (1..=d.get_pages().len() as u32).map(|i| d.extract_text(&[i]).unwrap().trim().to_string()).collect()
}

/// What an edit costs on a large file (`SHEAF_BENCH_PDF`): run with --ignored --nocapture,
/// and SHEAF_READER_DEBUG=1 for the engine / snapshot split.
#[test]
#[ignore]
fn edit_cost_on_a_big_file() {
    let Some(path) = std::env::var_os("SHEAF_BENCH_PDF") else { return };
    let t = std::time::Instant::now();
    let (ops, rx, snap) = started(Path::new(&path));
    println!("open: {:?} ({} pages)", t.elapsed(), snap.boxes.len());
    // SHEAF_BENCH_BOXES=n: n text boxes first, one a page (each snapshot reads their fonts).
    let boxes = std::env::var("SHEAF_BENCH_BOXES").ok().and_then(|n| n.parse().ok()).unwrap_or(0).min(snap.boxes.len());
    for page in 0..boxes {
        let shape = Shape::TextBox { rect: [72.0, 600.0, 272.0, 640.0], font_size: 12.0 };
        let style = Style::default_for(&shape);
        let annotation = NewAnnotation { page, shape, style, contents: "Bench".into(), author: String::new() };
        ops.send(Op::Apply { edit: Box::new(Edit::AddTextBox { annotation, font: "TiRo".into(), align: 0 }), pages: vec![page] }).unwrap();
        next(&rx);
    }
    let edits = std::env::var("SHEAF_BENCH_EDITS").ok().and_then(|n| n.parse().ok()).unwrap_or(3);
    for i in 0..edits {
        let t = std::time::Instant::now();
        ops.send(Op::Apply { edit: Box::new(highlight(i)), pages: vec![i] }).unwrap();
        let (_, err) = next(&rx);
        println!("highlight {i}: {:?} {err:?}", t.elapsed());
    }
    let t = std::time::Instant::now();
    ops.send(Op::Undo { pages: Some(vec![2]) }).unwrap();
    next(&rx);
    println!("undo: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let copy = std::env::temp_dir().join("sheaf-bench-save.pdf");
    ops.send(Op::Save { to: Some(copy.clone()) }).unwrap();
    let (_, err) = next(&rx);
    println!("save as: {:?} {err:?}", t.elapsed());
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    next(&rx);
    let t = std::time::Instant::now();
    ops.send(Op::Save { to: None }).unwrap();
    let (_, err) = next(&rx);
    println!("save (incremental): {:?} {err:?}", t.elapsed());
    let _ = std::fs::remove_file(copy);
}

/// What a search reads on a large file (`SHEAF_BENCH_PDF`): the text layer of every page,
/// as the reader's search thread does. Run with --ignored --nocapture.
#[test]
#[ignore]
fn search_cost_on_a_big_file() {
    use sheaf_core::pdfcraft::render;
    let Some(path) = std::env::var_os("SHEAF_BENCH_PDF") else { return };
    let bytes = Arc::new(std::fs::read(path).unwrap());
    let n = render::PageRenderer::new(bytes.clone(), render::RenderConfig::default()).page_count();
    let texts = std::sync::Mutex::new(std::collections::HashMap::new());
    let t = std::time::Instant::now();
    sheaf_app::reader::read_texts(&bytes, &render::RenderConfig::default(), n, &Default::default(), &|| false, |p, x| {
        texts.lock().unwrap().insert(p, x);
        true
    });
    let first = t.elapsed();
    let known: std::collections::HashMap<usize, Arc<render::PageText>> = texts.into_inner().unwrap().into_iter().filter_map(|(p, x)| Some((p, x?))).collect();
    let hits: usize = known.values().map(|x| x.find_opts("tumor", false, false).len()).sum();
    let t = std::time::Instant::now();
    sheaf_app::reader::read_texts(&bytes, &render::RenderConfig::default(), n, &known, &|| false, |_, _| true);
    println!("{n} pages, {hits} hits: first search {first:?}, again {:?}", t.elapsed());
}

/// Search on a 300-page file of full text pages, as the reader's search thread reads it: the
/// first page's text comes at once (its results show while the rest is read), every page is
/// read, and the whole pass stays within seconds even in a debug build.
#[test]
fn a_search_shows_its_first_page_at_once_on_a_long_file() {
    use sheaf_core::pdfcraft::render;
    // Written by hand: 13,500 text objects added one by one through the engine took minutes in a
    // debug build (the unit layer hung on it, 25/10).
    let line = "(The quick brown fox jumps over the lazy dog, a tumor of words for the search to read.) Tj 0 -17 Td ";
    let content = format!("BT /F1 10 Tf 40 800 Td {} ET", line.repeat(45));
    let mut objs = vec!["<< /Type /Catalog /Pages 2 0 R >>".to_string(), String::new(), "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into()];
    objs.push(format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()));
    let kids: Vec<String> = (0..300).map(|i| format!("{} 0 R", 5 + i)).collect();
    objs[1] = format!("<< /Type /Pages /Kids [{}] /Count 300 >>", kids.join(" "));
    objs.extend((0..300).map(|_| "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 3 0 R >> >> >>".to_string()));
    let bytes = Arc::new(hand_pdf(&objs));
    let (first, read) = (std::sync::Mutex::new(None), std::sync::atomic::AtomicUsize::new(0));
    let t = std::time::Instant::now();
    sheaf_app::reader::read_texts(&bytes, &render::RenderConfig::default(), 300, &Default::default(), &|| false, |_, x| {
        first.lock().unwrap().get_or_insert(t.elapsed());
        assert!(x.is_some_and(|x| !x.find_opts("tumor", false, false).is_empty()));
        read.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        true
    });
    let (first, all) = (first.into_inner().unwrap().unwrap(), t.elapsed());
    println!("300 pages: first page {first:?}, all {all:?}");
    assert_eq!(read.into_inner(), 300);
    assert!(first < Duration::from_millis(500), "first page after {first:?}");
    assert!(all < Duration::from_secs(20), "300 pages read in {all:?}");
}

#[test]
fn a_summary_of_comments_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 2);
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(highlight(1)), pages: vec![1] }).unwrap();
    next(&rx);
    let out = dir.path().join("summary.pdf");
    ops.send(Op::Summary { to: out.clone() }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Extracted { to, error } => assert!(error.is_none() && to == out, "{error:?}"),
        _ => panic!("expected Extracted"),
    }
    let text = texts(&out).join("\n");
    assert!(text.contains("Summary of Comments") && text.contains("Highlight"), "{text}");
}

#[test]
fn a_reply_is_listed_under_its_comment() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    let (snap, _) = next(&rx);
    let index = snap.annots[0].index;
    ops.send(Op::Apply { edit: Box::new(Edit::ReplyToAnnotation { page: 0, index, text: "Agreed".into(), author: "Ada".into() }), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap.annots.len(), 1, "a reply is not a comment of its own");
    let (text, page, index) = snap.annots[0].replies[0].clone();
    assert_eq!(text, "Ada: Agreed");
    // Deleted alone, the comment stays.
    ops.send(Op::Apply { edit: Box::new(Edit::DeleteAnnotation { page, index }), pages: vec![page] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap.annots.len(), 1);
    assert!(snap.annots[0].replies.is_empty());
}

#[test]
fn a_review_status_shows_on_its_comment() {
    use sheaf_core::pdfcraft::engine::ReviewState;
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    let (snap, _) = next(&rx);
    let index = snap.annots[0].index;
    ops.send(Op::Apply { edit: Box::new(Edit::SetAnnotationStatus { page: 0, index, state: ReviewState::Accepted, author: "Ada".into() }), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!((snap.annots.len(), snap.annots[0].status.as_str()), (1, "Accepted"));
}

#[test]
fn bookmarks_are_added_renamed_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 3);
    let (ops, rx, snap) = started(&path);
    assert!(snap.outline.is_empty());
    let send = |e: Edit| ops.send(Op::Apply { edit: Box::new(e), pages: vec![] }).unwrap();
    send(Edit::AddBookmark { parent: vec![], index: 0, title: "Page 2".into(), page: 1 });
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!((snap.outline[0].title.as_str(), snap.outline[0].page), ("Page 2", Some(1)));
    send(Edit::RenameBookmark { path: vec![0], title: "Methods".into() });
    let (snap, _) = next(&rx);
    assert_eq!(snap.outline[0].title, "Methods");
    send(Edit::DeleteBookmark { path: vec![0] });
    let (snap, _) = next(&rx);
    assert!(snap.outline.is_empty());
}

#[test]
fn a_text_box_takes_a_font_grows_to_fit_and_undoes_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (ops, rx, _) = started(&fixture(dir.path(), 1));
    let send = |e: Edit| ops.send(Op::Apply { edit: Box::new(e), pages: vec![0] }).unwrap();
    // 100 × 20 pt is one line of 24 pt text: the box grows down to keep all of it.
    let shape = Shape::TextBox { rect: [72.0, 700.0, 172.0, 720.0], font_size: 24.0 };
    let style = Style { color: [0.0; 3], width: 0.0, ..Style::default_for(&shape) };
    let annotation = NewAnnotation { page: 0, shape, style, contents: "A text box that wraps".into(), author: String::new() };
    send(Edit::AddTextBox { annotation, font: "TiBo".into(), align: 1 });
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let boxes = |s: &Snap| s.annots.iter().filter(|a| a.subtype == "FreeText").cloned().collect::<Vec<_>>();
    let b = &boxes(&snap)[0];
    assert_eq!(b.font, Some(("TiBo", 24.0, 1)));
    assert!(b.rect[3] - b.rect[1] > 60.0 && (b.rect[3] - 720.0).abs() < 0.01, "grew down from its top: {:?}", b.rect);
    send(Edit::SetTextFont { page: 0, index: b.index, font: "CoBO".into(), size: Some(10.0), align: Some(2) });
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(boxes(&snap).len(), 1, "restyled, not added");
    assert_eq!(boxes(&snap)[0].font, Some(("CoBO", 10.0, 2)));
    // One undo per step: back to Times Bold, then no box at all.
    ops.send(Op::Undo { pages: None }).unwrap();
    assert_eq!(boxes(&next(&rx).0)[0].font, Some(("TiBo", 24.0, 1)));
    ops.send(Op::Undo { pages: None }).unwrap();
    assert!(boxes(&next(&rx).0).is_empty());
}

#[test]
fn a_file_is_attached_listed_and_saved_back() {
    use sheaf_core::pdfcraft::engine::AttachIcon;
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, snap) = started(&path);
    assert!(snap.attachments.is_empty());
    let shape = Shape::Attachment { at: [72.0, 700.0], icon: AttachIcon::Paperclip, file: "notes.txt".into(), data: b"hello attachment".to_vec() };
    let style = Style::default_for(&shape);
    ops.send(Op::Apply { edit: Box::new(Edit::AddAnnotation(NewAnnotation { page: 0, shape, style, contents: String::new(), author: String::new() })), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap.attachments.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["notes.txt"]);
    let out = dir.path().join("out.txt");
    save_attachment(&snap.bytes, None, &snap.attachments[0], &out).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"hello attachment");
}

#[test]
fn comments_are_exported_and_imported() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    next(&rx);
    let xfdf = dir.path().join("c.xfdf");
    ops.send(Op::ExportComments { to: xfdf.clone() }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Exported { error, .. } => assert!(error.is_none(), "{error:?}"),
        _ => panic!("expected Exported"),
    }
    let text = std::fs::read_to_string(&xfdf).unwrap();
    assert!(text.contains("<xfdf") && text.contains("highlight"), "{text}");
    // Into another copy of the document, without comments: the highlight comes in.
    let other = dir.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let path2 = fixture(&other, 1);
    let (ops2, rx2, snap2) = started(&path2);
    assert!(snap2.annots.is_empty());
    ops2.send(Op::Apply { edit: Box::new(Edit::ImportData { name: "c.xfdf".into(), bytes: Arc::new(std::fs::read(&xfdf).unwrap()) }), pages: vec![0] }).unwrap();
    let (snap2, err) = next(&rx2);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap2.annots.iter().map(|a| a.subtype.as_str()).collect::<Vec<_>>(), ["Highlight"]);
}

/// ⌘S twice with an edit between: each save appends, and the light refresh after an
/// appended save (patched engine) keeps the comment list and undo in step with the file.
#[test]
fn incremental_saves_in_a_row_keep_every_edit() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 3);
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    next(&rx);
    ops.send(Op::Save { to: None }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none() && !snap.dirty, "{err:?}");
    let first = std::fs::metadata(&path).unwrap().len();
    ops.send(Op::Apply { edit: Box::new(highlight(2)), pages: vec![2] }).unwrap();
    let (snap, _) = next(&rx);
    assert_eq!(snap.annots.iter().map(|a| a.page).collect::<Vec<_>>(), [0, 2]);
    ops.send(Op::Save { to: None }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none() && !snap.dirty && snap.annots.len() == 2, "{err:?}");
    assert!(std::fs::metadata(&path).unwrap().len() > first);
    // Undo after the save still works on the saved state.
    ops.send(Op::Undo { pages: Some(vec![2]) }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none() && snap.dirty && snap.annots.len() == 1, "{err:?}");
    qpdf_clean(&path);
    let (_, _, reopened) = started(&path);
    assert_eq!(reopened.annots.iter().map(|a| a.page).collect::<Vec<_>>(), [0, 2]);
}

#[test]
fn an_annotation_is_saved_into_the_file_and_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 2);
    let before = std::fs::metadata(&path).unwrap().len();
    let (ops, rx, snap) = started(&path);
    assert!(snap.annots.is_empty() && !snap.dirty && snap.boxes.len() == 2);
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert!(snap.dirty && snap.can_undo);
    assert_eq!(snap.annots.iter().map(|a| a.subtype.as_str()).collect::<Vec<_>>(), ["Highlight"]);
    ops.send(Op::Save { to: None }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none() && !snap.dirty, "{err:?}");
    // An incremental update: the original bytes, then the change.
    assert!(std::fs::metadata(&path).unwrap().len() > before);
    qpdf_clean(&path);
    let (_, _, again) = started(&path);
    assert_eq!(again.annots.len(), 1);
    assert_eq!(again.annots[0].subtype, "Highlight");
}

#[test]
fn a_link_is_added_changed_and_removed_and_saved() {
    use sheaf_core::pdfcraft::engine::{LinkAction, LinkStyle};
    use sheaf_core::pdfcraft::render::LinkTarget;
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 3);
    let (ops, rx, _) = started(&path);
    let add = Edit::AddLink { page: 0, rect: [72.0, 700.0, 200.0, 740.0], action: LinkAction::Page(2), style: LinkStyle::default() };
    ops.send(Op::Apply { edit: Box::new(add), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let l = snap.links.iter().find(|l| l.page == 0).expect("the link is read back");
    assert!(matches!(l.target, LinkTarget::Page(2, _)), "{:?}", l.target);
    let index = l.index;
    let set = Edit::SetLink { page: 0, index, rect: None, action: Some(LinkAction::Uri("https://example.org".into())), style: None };
    ops.send(Op::Apply { edit: Box::new(set), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert!(snap.links.iter().any(|l| matches!(&l.target, LinkTarget::Uri(u) if u == "https://example.org")), "{:?}", snap.links);
    ops.send(Op::Save { to: None }).unwrap();
    let (_, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    qpdf_clean(&path);
    let (ops, rx, again) = started(&path);
    assert_eq!(again.links.len(), 1);
    ops.send(Op::Apply { edit: Box::new(Edit::DeleteLink { page: 0, index: again.links[0].index }), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none() && snap.links.is_empty(), "{err:?}");
}

#[test]
fn a_visible_link_is_drawn_by_its_appearance() {
    use sheaf_core::pdfcraft::engine::{LinkAction, LinkHighlight, LinkStyle};
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    let style = LinkStyle { visible: true, color: [0.85, 0.2, 0.15], width: 2.0, dashed: true, underline: false, highlight: LinkHighlight::Invert };
    ops.send(Op::Apply { edit: Box::new(Edit::AddLink { page: 0, rect: [72.0, 700.0, 200.0, 740.0], action: LinkAction::Page(0), style }), pages: vec![0] }).unwrap();
    assert!(next(&rx).1.is_none());
    ops.send(Op::Save { to: None }).unwrap();
    assert!(next(&rx).1.is_none());
    qpdf_clean(&path);
    let qdf = std::process::Command::new("qpdf").args(["--qdf", "--object-streams=disable"]).arg(&path).arg("-").output().unwrap().stdout;
    let qdf = String::from_utf8_lossy(&qdf);
    // Its border as an appearance: dashed, 2 pt, in its colour (readers that draw only
    // appearances showed nothing).
    assert!(qdf.contains("/AP") && qdf.contains("0.850 0.200 0.150 RG 2.00 w [3] 0 d"), "{qdf}");
}

#[test]
fn web_addresses_in_the_text_are_linked_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("urls.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let mut doc = sheaf_core::engine::new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(720.0), "Visit https://example.org today", font, PdfPoints::new(18.0)).unwrap();
        doc.save_to_file(&path).unwrap();
    }
    let (ops, rx, snap) = started(&path);
    assert!(snap.links.is_empty());
    let style = sheaf_core::pdfcraft::engine::LinkStyle::default();
    ops.send(Op::LinkUrls { style }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert!(matches!(&snap.links[..], [l] if matches!(&l.target, render::LinkTarget::Uri(u) if u.starts_with("https://example.org"))), "{:?}", snap.links);
    // Again: the address is linked already.
    ops.send(Op::LinkUrls { style }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_some_and(|e| e.contains("No web addresses")) && snap.links.len() == 1);
}

#[test]
fn an_address_under_a_link_is_not_linked_again() {
    let link = render::Link { page: 0, index: 0, rect: [100.0, 700.0, 200.0, 720.0], target: render::LinkTarget::Uri("x".into()) };
    let found = vec![(0, vec![[105.0, 702.0, 190.0, 718.0], [100.0, 600.0, 150.0, 620.0]], "https://a.b".to_string()), (1, vec![[105.0, 702.0, 190.0, 718.0]], "https://c.d".to_string())];
    let left = unlinked(found, &[link]);
    assert_eq!(left.iter().map(|(p, r, _)| (*p, r.len())).collect::<Vec<_>>(), [(0, 1), (1, 1)], "{left:?}");
}

#[test]
fn an_edit_that_draws_a_character_as_a_question_mark_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    let boxed = |contents: &str| {
        Edit::AddAnnotation(NewAnnotation {
            page: 0,
            shape: Shape::TextBox { rect: [100.0, 400.0, 300.0, 440.0], font_size: 12.0 },
            style: Style::default(),
            contents: contents.into(),
            author: String::new(),
        })
    };
    ops.send(Op::Apply { edit: Box::new(boxed("Plain")), pages: vec![0] }).unwrap();
    let Reply::Changed { note, .. } = rx.recv_timeout(Duration::from_secs(30)).unwrap() else { panic!("expected Changed") };
    assert_eq!(note, None);
    ops.send(Op::Apply { edit: Box::new(boxed("Ωmega")), pages: vec![0] }).unwrap();
    let Reply::Changed { note, .. } = rx.recv_timeout(Duration::from_secs(30)).unwrap() else { panic!("expected Changed") };
    assert!(note.as_deref().is_some_and(|n| n.starts_with("A comment on page 1 has \"Ω\"")), "{note:?}");
}

#[test]
fn a_preview_is_taken_back_without_a_trace_and_a_commit_after_it_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    let edit = |text: &str| Box::new(Edit::EditTextBlock { page: 0, block: 0, text: text.into(), style: Default::default() });
    ops.send(Op::Preview { edit: Some(edit("Page one draft")), pages: vec![0] }).unwrap();
    let (s, e) = next(&rx);
    assert!(e.is_none() && !s.dirty && !s.can_undo, "a preview is not a change to save or undo");
    ops.send(Op::Preview { edit: Some(edit("Page one, second draft")), pages: vec![0] }).unwrap();
    next(&rx);
    ops.send(Op::Preview { edit: None, pages: vec![0] }).unwrap();
    let (s, _) = next(&rx);
    assert!(!s.dirty && !s.can_undo && !s.can_redo, "taken back: clean, nothing to undo or redo");
    // Previewed, then committed: the commit takes the preview back first, one undo step.
    ops.send(Op::Preview { edit: Some(edit("Page one draft")), pages: vec![0] }).unwrap();
    next(&rx);
    ops.send(Op::Apply { edit: edit("Page one final"), pages: vec![0] }).unwrap();
    let (s, e) = next(&rx);
    assert!(e.is_none() && s.dirty && s.can_undo && !s.can_redo);
    ops.send(Op::Undo { pages: Some(vec![0]) }).unwrap();
    let (s, _) = next(&rx);
    assert!(!s.can_undo && s.can_redo, "one step: the preview left none");
    // Something to redo, a preview, Escape: it can still be redone.
    ops.send(Op::Preview { edit: Some(edit("Page one draft")), pages: vec![0] }).unwrap();
    let (s, _) = next(&rx);
    assert!(s.can_redo, "shown as before the preview");
    ops.send(Op::Preview { edit: None, pages: vec![0] }).unwrap();
    let (s, _) = next(&rx);
    assert!(s.can_redo && !s.can_undo, "the redo list is as it was");
    ops.send(Op::Redo { pages: Some(vec![0]) }).unwrap();
    let (s, e) = next(&rx);
    assert!(e.is_none() && s.can_undo && !s.can_redo);
}

#[test]
fn undo_and_redo_take_the_edit_away_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    let note = Edit::AddAnnotation(NewAnnotation {
        page: 0,
        shape: Shape::Note { at: [100.0, 500.0], icon: NoteIcon::Comment },
        style: Style::default(),
        contents: "Check".into(),
        author: String::new(),
    });
    ops.send(Op::Apply { edit: Box::new(note), pages: vec![0] }).unwrap();
    assert_eq!(next(&rx).0.annots.len(), 1);
    ops.send(Op::Undo { pages: Some(vec![0]) }).unwrap();
    let (s, _) = next(&rx);
    assert!(s.annots.is_empty() && s.can_redo);
    ops.send(Op::Redo { pages: Some(vec![0]) }).unwrap();
    let (s, _) = next(&rx);
    assert_eq!(s.annots.len(), 1);
    assert_eq!(s.annots[0].contents, "Check");
}

#[test]
fn page_edits_reorder_turn_and_delete_pages_in_the_saved_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 4);
    let (ops, rx, _) = started(&path);
    // Page 1 moved after page 3: 2 3 1 4.
    ops.send(Op::Apply { edit: Box::new(Edit::MovePages { pages: vec![0], to: 2 }), pages: vec![] }).unwrap();
    assert!(next(&rx).1.is_none());
    ops.send(Op::Apply { edit: Box::new(Edit::RotatePages { pages: vec![0], degrees: 90 }), pages: vec![] }).unwrap();
    let (s, _) = next(&rx);
    assert_eq!(s.boxes[0].rotation, 90);
    ops.send(Op::Apply { edit: Box::new(Edit::DeletePages { pages: vec![3] }), pages: vec![] }).unwrap();
    assert_eq!(next(&rx).0.boxes.len(), 3);
    ops.send(Op::Save { to: None }).unwrap();
    assert!(next(&rx).1.is_none());
    qpdf_clean(&path);
    assert_eq!(texts(&path), ["Page 2", "Page 3", "Page 1"]);
}

#[test]
fn extract_writes_the_chosen_pages_to_a_new_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 3);
    let (ops, rx, _) = started(&path);
    let out = dir.path().join("extract.pdf");
    ops.send(Op::Extract { parts: vec![(vec![1, 2], out.clone())] }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Extracted { to, error } => assert!(error.is_none() && to == out, "{error:?}"),
        _ => panic!("expected Extracted"),
    }
    qpdf_clean(&out);
    assert_eq!(texts(&out), ["Page 2", "Page 3"]);
    // The document itself is untouched.
    assert_eq!(texts(&path).len(), 3);
}

#[test]
fn extract_one_file_per_page_writes_each_and_names_their_folder() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 3);
    let (ops, rx, _) = started(&path);
    let (a, b) = (dir.path().join("x - page 1.pdf"), dir.path().join("x - page 3.pdf"));
    ops.send(Op::Extract { parts: vec![(vec![0], a.clone()), (vec![2], b.clone())] }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Extracted { to, error } => assert!(error.is_none() && to == dir.path(), "{error:?} {to:?}"),
        _ => panic!("expected Extracted"),
    }
    qpdf_clean(&a);
    qpdf_clean(&b);
    assert_eq!((texts(&a), texts(&b)), (vec!["Page 1".to_string()], vec!["Page 3".to_string()]));
}

#[test]
fn save_as_writes_a_full_copy_and_follows_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    next(&rx);
    let copy = dir.path().join("copy.pdf");
    ops.send(Op::Save { to: Some(copy.clone()) }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Saved { to, error, snap } => assert!(error.is_none() && to == copy && !snap.dirty, "{error:?}"),
        _ => panic!("expected Saved"),
    }
    qpdf_clean(&copy);
    let (_, _, s) = started(&copy);
    assert_eq!(s.annots.len(), 1);
    // The original keeps no annotation.
    let (_, _, orig) = started(&path);
    assert!(orig.annots.is_empty());
}

#[test]
fn replace_keeps_the_old_file_until_the_new_one_is_complete() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.pdf");
    std::fs::write(&p, b"old").unwrap();
    replace(&p, b"new bytes").unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), b"new bytes");
    // No temporary file left behind.
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn a_comment_is_copied_and_pasted_as_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 2);
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    let (snap, _) = next(&rx);
    let nm = snap.annots[0].nm.clone();
    assert!(!nm.is_empty(), "Sheaf's comments are named");
    ops.send(Op::CopyComment { nm, page: 0, rect: snap.annots[0].rect }).unwrap();
    let el = match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Copied(Some(el), None) => el,
        _ => panic!("expected Copied"),
    };
    assert!(el.starts_with("<highlight "), "{el}");
    // Pasted on page 2 under a new name: a second comment, the first kept.
    let xfdf = sheaf_app::reader::paste_xfdf(&el, 1, (0.0, 0.0), "pasted-1");
    ops.send(Op::Apply { edit: Box::new(Edit::ImportData { name: "Paste".into(), bytes: Arc::new(xfdf.into_bytes()) }), pages: vec![1] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap.annots.iter().map(|a| (a.page, a.subtype.as_str())).collect::<Vec<_>>(), [(0, "Highlight"), (1, "Highlight")]);
}

#[test]
fn document_properties_list_the_signatures_with_their_verdict() {
    use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
    let dir = tempfile::tempdir().unwrap();
    let (key, cert, p12) = (dir.path().join("k.pem"), dir.path().join("c.pem"), dir.path().join("id.p12"));
    let ok = |c: &mut std::process::Command| c.output().map(|o| o.status.success()).unwrap_or(false);
    if !ok(std::process::Command::new("openssl").args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-subj", "/CN=Ada Signer", "-keyout"]).arg(&key).arg("-out").arg(&cert))
        || !ok(std::process::Command::new("openssl").args(["pkcs12", "-export", "-passout", "pass:pw", "-inkey"]).arg(&key).arg("-in").arg(&cert).arg("-out").arg(&p12))
    {
        return eprintln!("openssl missing: skipped");
    }
    let path = fixture(dir.path(), 1);
    let signed = dir.path().join("signed.pdf");
    let digital = DigitalSpec { p12: p12.clone(), password: "pw".into(), reason: Some("Approved".into()), location: None };
    sign::sign(&Input::new(&path), &SignSpec { page: 0, rect: [72.0, 72.0, 272.0, 132.0], graphic: Graphic::Typed("Ada".into(), Default::default()), digital: Some(digital) }, &signed).unwrap();
    let (_, _, snap) = started(&signed);
    let row = snap.props.about.iter().find(|(k, _)| k.starts_with("Signature 1")).expect("a row for the signature");
    assert!(row.0.contains("Ada Signer"), "{row:?}");
    assert!(row.1.contains("Signature validity is unknown") && row.1.contains("Approved"), "self-signed: not trusted, its reason shown: {row:?}");
    // Its certificate, and the signed version: the file's bytes up to the signature's end.
    let cert = snap.props.about.iter().find(|(k, _)| k == "Signature 1 · certificate").expect("the certificate's row");
    assert!(cert.1.starts_with("Issued to Ada Signer by Ada Signer, valid "), "{cert:?}");
    let len = std::fs::metadata(&signed).unwrap().len() as usize;
    assert!(matches!(&snap.props.signed[..], [(who, n, 0, false)] if who == "Ada Signer" && *n > 0 && *n <= len), "{:?}", snap.props.signed);
}

#[test]
fn a_signers_certificate_once_trusted_makes_the_signature_valid() {
    use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
    let dir = tempfile::tempdir().unwrap();
    let (key, cert, p12) = (dir.path().join("k.pem"), dir.path().join("c.pem"), dir.path().join("id.p12"));
    let ok = |c: &mut std::process::Command| c.output().map(|o| o.status.success()).unwrap_or(false);
    if !ok(std::process::Command::new("openssl").args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-subj", "/CN=Bea Signer", "-keyout"]).arg(&key).arg("-out").arg(&cert))
        || !ok(std::process::Command::new("openssl").args(["pkcs12", "-export", "-passout", "pass:pw", "-inkey"]).arg(&key).arg("-in").arg(&cert).arg("-out").arg(&p12))
    {
        return eprintln!("openssl missing: skipped");
    }
    // The trust folder of this test process (set once): Bea's certificate goes there.
    let trust = Box::leak(Box::new(tempfile::tempdir().unwrap()));
    sheaf_core::pdfcraft::set_trust_dir(trust.path().to_path_buf());
    let path = fixture(dir.path(), 1);
    let signed = dir.path().join("signed.pdf");
    let digital = DigitalSpec { p12, password: "pw".into(), reason: None, location: None };
    sign::sign(&Input::new(&path), &SignSpec { page: 0, rect: [72.0, 72.0, 272.0, 132.0], graphic: Graphic::Typed("Bea".into(), Default::default()), digital: Some(digital) }, &signed).unwrap();
    let (ops, rx, snap) = started(&signed);
    let verdict = |s: &Snap| s.props.about.iter().find(|(k, _)| k.starts_with("Signature 1 · Bea")).map(|r| r.1.clone()).unwrap_or_default();
    assert!(verdict(&snap).contains("validity is unknown"), "{}", verdict(&snap));
    ops.send(Op::TrustSignature { index: 0 }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert!(!verdict(&snap).contains("unknown") && snap.props.signed[0].3, "{}", verdict(&snap));
}

#[test]
fn a_form_is_prepared_fields_added_set_up_copied_and_deleted() {
    use sheaf_core::pdfcraft::engine::{FieldProps, NewField};
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 2);
    let (ops, rx, snap) = started(&path);
    assert!(snap.fields.is_empty());
    let add = |kind: NewField, rect: [f64; 4]| Op::Apply { edit: Box::new(Edit::AddField { page: 0, rect, kind, name: None }), pages: vec![0] };
    ops.send(add(NewField::Text { multiline: false }, [72.0, 600.0, 222.0, 622.0])).unwrap();
    next(&rx);
    ops.send(add(NewField::CheckBox, [72.0, 560.0, 86.0, 574.0])).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap.fields.iter().map(|f| (f.name.as_str(), f.exact)).collect::<Vec<_>>(), [("Text1", 0), ("Check Box1", 1)], "Acrobat's names");
    // Renamed, required, with a tooltip.
    let props = FieldProps { name: Some("Full name".into()), tooltip: Some("As on your passport".into()), required: Some(true), ..Default::default() };
    ops.send(Op::Apply { edit: Box::new(Edit::SetFieldProps { name: "Text1".into(), props: Box::new(props) }), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let f = snap.fields.iter().find(|f| f.name == "Full name").expect("renamed");
    assert!(f.required && f.tooltip == "As on your passport" && !f.read_only);
    // On every page: the same field on page 2; then the check box deleted.
    ops.send(Op::Apply { edit: Box::new(Edit::DuplicateField { name: "Full name".into(), pages: vec![1] }), pages: vec![] }).unwrap();
    let (snap, _) = next(&rx);
    assert_eq!(snap.fields.iter().find(|f| f.name == "Full name").map(|f| f.widgets.iter().map(|w| w.0).collect::<Vec<_>>()), Some(vec![0, 1]));
    ops.send(Op::Apply { edit: Box::new(Edit::DeleteField { name: "Check Box1".into() }), pages: vec![0] }).unwrap();
    let (snap, _) = next(&rx);
    assert_eq!(snap.fields.len(), 1);
}

#[test]
fn a_flat_forms_blanks_become_fields_named_from_their_labels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flat.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let mut doc = sheaf_core::engine::new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(700.0), "Name: ______________________", font, PdfPoints::new(12.0)).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(670.0), "City: ______________________", font, PdfPoints::new(12.0)).unwrap();
        doc.save_to_file(&path).unwrap();
    }
    let (ops, rx, _) = started(&path);
    ops.send(Op::DetectFields).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let names: Vec<&str> = snap.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(names.len() == 2 && names.iter().any(|n| n.contains("Name")) && names.iter().any(|n| n.contains("City")), "{names:?}");
    // Again: nothing new to find, one undo step each time.
    ops.send(Op::DetectFields).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_some_and(|e| e.contains("No fields found")) && snap.fields.len() == 2);
}

#[test]
fn a_picked_picture_is_copied_as_its_own_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("p.png");
    image::RgbImage::from_pixel(30, 20, image::Rgb([200, 30, 30])).save(&png).unwrap();
    let path = sheaf_core::images::images_to_pdf(&[png], sheaf_core::images::PageLayout::FitImage, &dir.path().join("pic.pdf")).unwrap();
    let (ops, rx, _) = started(&path);
    ops.send(Op::CopyImage { page: 0, index: 0 }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::ImageCopied(Ok((w, h, rgba))) => assert!(w == 30 && h == 20 && rgba[..4] == [200, 30, 30, 255], "{w}×{h} {:?}", &rgba[..4]),
        Reply::ImageCopied(Err(e)) => panic!("{e}"),
        _ => panic!("expected ImageCopied"),
    }
}

#[test]
fn a_picture_stamp_is_copied_with_its_picture() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 2);
    let png = dir.path().join("logo.png");
    image::RgbImage::from_pixel(30, 20, image::Rgb([200, 30, 30])).save(&png).unwrap();
    let file = |name: String, bytes| sheaf_core::pdfcraft::engine::MarkFile { name, bytes, page: 0 };
    let stamp = |page, rect, file| Edit::AddCustomStamp { page, rect, name: "logo".into(), file, author: "Ada".into() };
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(stamp(0, [100.0, 100.0, 250.0, 200.0], file("logo.png".into(), Arc::new(std::fs::read(&png).unwrap())))), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let copy = |page: usize, a: &AnnotEntry| {
        ops.send(Op::CopyComment { nm: a.nm.clone(), page, rect: a.rect }).unwrap();
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            Reply::Copied(Some(el), picture) => (el, picture),
            _ => panic!("expected Copied"),
        }
    };
    let (el, picture) = copy(0, &snap.annots[0]);
    let (name, bytes) = picture.expect("the stamp's picture is copied");
    let pixel = image::load_from_memory(&bytes).unwrap().to_rgb8().get_pixel(0, 0).0;
    assert!(name.starts_with("Stamp.") && pixel == [200, 30, 30], "{name} {pixel:?}");
    // Pasted as the window does: a picture stamp in the element's box, here on page 2.
    let rect = sheaf_app::reader::xfdf_rect(&el).unwrap();
    ops.send(Op::Apply { edit: Box::new(stamp(1, rect, file(name, bytes))), pages: vec![1] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let pasted = snap.annots.iter().find(|a| a.page == 1).expect("a stamp on page 2");
    assert!(copy(1, pasted).1.is_some(), "the pasted stamp has its picture");
}

#[test]
fn sanitize_says_what_it_removed_only_when_it_did() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    let sanitized = |ops: &Sender<Op>| {
        ops.send(Op::Apply { edit: Box::new(Edit::Sanitize), pages: vec![] }).unwrap();
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            Reply::Changed { error, note, .. } => (error, note),
            _ => panic!("expected Changed"),
        }
    };
    let (error, note) = sanitized(&ops);
    assert!(error.is_none() && note.as_deref().is_some_and(|n| n.starts_with("Hidden information removed")), "{error:?} {note:?}");
    // Revert brings back the file as last saved: nothing is left to save (More ▸ Versions…).
    ops.send(Op::Revert).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none() && !snap.dirty, "{err:?}");
}

#[test]
fn an_area_of_a_page_is_copied_as_its_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    ops.send(Op::CopyArea { page: 0, rect: [72.0, 700.0, 172.0, 750.0] }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        // 100 × 50 points at 3 pixels a point.
        Reply::ImageCopied(Ok((w, h, rgba))) => assert!(w.abs_diff(300) <= 3 && h.abs_diff(150) <= 3 && rgba.len() == (w * h * 4) as usize, "{w}×{h}"),
        Reply::ImageCopied(Err(e)) => panic!("{e}"),
        _ => panic!("expected ImageCopied"),
    }
    ops.send(Op::CopyArea { page: 0, rect: [10.0, 10.0, 10.0, 10.0] }).unwrap();
    assert!(matches!(rx.recv_timeout(Duration::from_secs(30)).unwrap(), Reply::ImageCopied(Err(_))), "an empty area is refused");
}

#[test]
fn a_fields_font_named_by_a_resource_key_is_read_through_the_forms_resources() {
    let dir = tempfile::tempdir().unwrap();
    let objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R] /DR << /Font << /F1 5 0 R >> >> /DA (/F1 0 Tf 0 g) >> >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R] >>".into(),
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (Name) /Rect [72 700 272 722] /P 3 0 R /DA (/F1 12 Tf 0 g) >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman >>".into(),
    ];
    let path = dir.path().join("times.pdf");
    std::fs::write(&path, hand_pdf(&objs)).unwrap();
    let (_, _, snap) = started(&path);
    assert_eq!(snap.fields.iter().map(|f| (f.name.as_str(), f.font)).collect::<Vec<_>>(), [("Name", 1)], "Times through /DR");
}

#[test]
fn pages_inserted_from_a_file_keep_their_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 4);
    let other = tempfile::tempdir().unwrap();
    let single = fixture(other.path(), 1);
    let (ops, rx, _) = started(&path);
    let bytes = std::sync::Arc::new(std::fs::read(&single).unwrap());
    ops.send(Op::Apply { edit: Box::new(Edit::InsertPagesFrom { name: "single.pdf".into(), bytes, pages: None, at: 1, field_conflicts: Default::default() }), pages: Vec::new() }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let out = dir.path().join("out.pdf");
    std::fs::write(&out, &*snap.bytes).unwrap();
    assert_eq!(texts(&out), ["Page 1", "Page 1", "Page 2", "Page 3", "Page 4"]);
}

