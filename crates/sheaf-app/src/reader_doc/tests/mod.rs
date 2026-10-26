use super::*;
use pdfium_render::prelude::*;
use sheaf_core::pdfcraft::engine::{Markup, NewAnnotation, NoteIcon, Shape, Style};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

mod clipboard;
mod forms_and_files;

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
fn a_text_box_in_a_font_of_its_own_keeps_it_through_a_new_size_and_colour() {
    use lopdf::{dictionary, Object as L, Stream};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("arial.pdf");
    // A text box another program wrote in Arial (a TrueType font not embedded, WinAnsi).
    let mut d = lopdf::Document::with_version("1.7");
    let arial = d.add_object(dictionary! { "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "Arial", "Encoding" => "WinAnsiEncoding" });
    let ap = d.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![72.into(), 700.into(), 272.into(), 730.into()], "Resources" => dictionary! { "Font" => dictionary! { "Arial" => arial } } },
        b"BT /Arial 12 Tf 74 712 Td (Hello Arial) Tj ET".to_vec(),
    ));
    let annot = d.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "FreeText", "Rect" => vec![72.into(), 700.into(), 272.into(), 730.into()],
        "Contents" => L::string_literal("Hello Arial"), "DA" => L::string_literal("0 0 0 rg /Arial 12 Tf"), "AP" => dictionary! { "N" => ap },
    });
    let pages = d.new_object_id();
    let content = d.add_object(Stream::new(dictionary! {}, Vec::new()));
    let page = d.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Contents" => content, "Annots" => vec![annot.into()] });
    d.objects.insert(pages, L::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    d.trailer.set("Root", catalog);
    d.save(&path).unwrap();
    let (ops, rx, snap) = started(&path);
    let b = snap.annots.iter().find(|a| a.subtype == "FreeText").cloned().unwrap();
    assert_eq!(b.font, Some(("", 12.0, 0)), "a font of the file's own reads as kept");
    // A new size (the row passes the box's own font on), then a new colour.
    ops.send(Op::Apply { edit: Box::new(Edit::SetTextFont { page: 0, index: b.index, font: "".into(), size: Some(18.0), align: None }), pages: vec![0] }).unwrap();
    let (_, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    ops.send(Op::Apply { edit: Box::new(Edit::StyleAnnotation { page: 0, index: b.index, color: Some([0.8, 0.1, 0.1]), opacity: None, width: None, endings: None }), pages: vec![0] }).unwrap();
    let (_, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let out = dir.path().join("out.pdf");
    ops.send(Op::Save { to: Some(out.clone()) }).unwrap();
    next(&rx);
    let q = std::process::Command::new("qpdf").args(["--qdf", "--object-streams=disable"]).arg(&out).arg("-").output().unwrap();
    let text = String::from_utf8_lossy(&q.stdout);
    assert!(text.contains("/Arial 18 Tf") && text.contains("0.8 0.1 0.1 rg /Arial"), "the /DA keeps Arial");
    // Drawn with the file's Arial: the new appearance names it, no Helvetica.
    assert!(text.contains("/BaseFont /Arial") && !text.contains("/BaseFont /Helvetica"), "drawn in Arial");
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

#[test]
fn a_stamp_turns_a_quarter_about_its_centre() {
    use sheaf_core::pdfcraft::engine::StampKind;
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    let shape = Shape::Stamp { rect: [100.0, 400.0, 260.0, 442.0], stamp: StampKind::ALL[0], by: None };
    let style = Style::default_for(&shape);
    ops.send(Op::Apply { edit: Box::new(Edit::AddAnnotation(NewAnnotation { page: 0, shape, style, contents: String::new(), author: String::new() })), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let index = snap.annots.iter().position(|a| a.subtype == "Stamp").unwrap();
    ops.send(Op::Apply { edit: Box::new(Edit::TurnAnnotation { page: 0, index, quarters: 1 }), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    // 160 × 42 becomes 42 × 160 about the same centre (180, 421).
    let r = snap.annots.iter().find(|a| a.subtype == "Stamp").unwrap().rect;
    assert!((r[0] - 159.0).abs() < 0.5 && (r[2] - 201.0).abs() < 0.5 && (r[1] - 341.0).abs() < 0.5 && (r[3] - 501.0).abs() < 0.5, "{r:?}");
    // Its appearance turned too (Sol, 26/10: only the box turned): drawn taller than wide.
    let out = dir.path().join("turned.pdf");
    ops.send(Op::Save { to: Some(out.clone()) }).unwrap();
    next(&rx);
    let d = lopdf::Document::load(&out).unwrap();
    let nums = |o: &lopdf::Object| o.as_array().map(|a| a.iter().map(|v| v.as_float().map(f64::from).or_else(|_| v.as_i64().map(|i| i as f64)).unwrap()).collect::<Vec<f64>>()).unwrap();
    let (w, h) = d
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .find(|a| a.get(b"Subtype").and_then(|s| s.as_name()).ok() == Some(b"Stamp".as_slice()))
        .map(|a| {
            let n = a.get(b"AP").and_then(|ap| d.dereference(ap)).unwrap().1.as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
            let s = d.get_object(n).unwrap().as_stream().unwrap();
            let b = nums(s.dict.get(b"BBox").unwrap());
            let m = s.dict.get(b"Matrix").map(nums).unwrap_or_else(|_| vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            let (bw, bh) = (b[2] - b[0], b[3] - b[1]);
            ((bw * m[0]).abs() + (bh * m[2]).abs(), (bw * m[1]).abs() + (bh * m[3]).abs())
        })
        .unwrap();
    assert!(h > w * 2.0, "the appearance is {w} × {h}");
}

#[test]
fn a_bookmark_keeps_its_view_and_its_look() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 3);
    let (ops, rx, _) = started(&path);
    let send = |e: Edit| ops.send(Op::Apply { edit: Box::new(e), pages: vec![] }).unwrap();
    send(Edit::AddBookmark { parent: vec![], index: 0, title: "Results".into(), page: 1 });
    next(&rx);
    // Acrobat's Set Destination: page 3, its point (72, 500), at 200 %.
    send(Edit::SetBookmarkView { path: vec![0], page: 2, left: Some(72.0), top: Some(500.0), zoom: Some(2.0) });
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(snap.outline[0].page, Some(2));
    assert!(matches!(snap.outline[0].view, render::DestView::Xyz { zoom: Some(z), .. } if (z - 2.0).abs() < 1e-3), "{:?}", snap.outline[0].view);
    send(Edit::SetBookmarkStyle { path: vec![0], bold: true, italic: false, color: Some([0.8, 0.1, 0.1]) });
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    assert!(matches!(&snap.outline_styles[..], [(p, true, false, Some(c))] if p == &vec![0] && (c[0] - 0.8).abs() < 1e-3), "{:?}", snap.outline_styles);
}

#[test]
fn pages_whose_text_runs_sideways_are_found_with_the_turn_that_sets_them_upright() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sideways.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let mut doc = sheaf_core::engine::new_document();
        let font = doc.fonts_mut().helvetica();
        for sideways in [true, false] {
            let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
            for k in 0..3 {
                let mut o = page.objects_mut().create_text_object(PdfPoints::ZERO, PdfPoints::ZERO, "A line of text that reads along", font, PdfPoints::new(14.0)).unwrap();
                if sideways {
                    // Upward, as a landscape table printed on a portrait page.
                    o.rotate_counter_clockwise_degrees(90.0).unwrap();
                    o.translate(PdfPoints::new(100.0 + 30.0 * k as f32), PdfPoints::new(200.0)).unwrap();
                } else {
                    o.translate(PdfPoints::new(72.0), PdfPoints::new(700.0 - 30.0 * k as f32)).unwrap();
                }
            }
        }
        doc.save_to_file(&path).unwrap();
    }
    let (ops, rx, _) = started(&path);
    ops.send(Op::UprightTurns).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Upright(turns) => assert_eq!(turns, [(0, 90)]),
        _ => panic!("expected Upright"),
    }
}

#[test]
fn a_renamed_file_is_saved_under_its_new_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    let (ops, rx, _) = started(&path);
    let to = dir.path().join("renamed.pdf");
    std::fs::rename(&path, &to).unwrap();
    ops.send(Op::Renamed { to: to.clone() }).unwrap();
    ops.send(Op::Apply { edit: Box::new(highlight(0)), pages: vec![0] }).unwrap();
    next(&rx);
    ops.send(Op::Save { to: None }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Saved { to: saved, error, .. } => assert!(error.is_none() && saved == to, "{error:?} {saved:?}"),
        _ => panic!("expected Saved"),
    }
    // Saved there, and the old name not written again (Sol, 26/10).
    assert!(!path.exists());
    assert_eq!(lopdf::Document::load(&to).unwrap().get_pages().len(), 1);
}

#[test]
fn replace_all_leaves_a_scans_words_alone() {
    let dir = tempfile::tempdir().unwrap();
    // A scan (a picture over the page) with its OCR layer: "secret" in invisible text.
    let png = dir.path().join("scan.png");
    image::RgbImage::from_pixel(600, 800, image::Rgb([240, 240, 235])).save(&png).unwrap();
    let scan = sheaf_core::images::images_to_pdf(&[png], sheaf_core::images::PageLayout::FitImage, &dir.path().join("scan.pdf")).unwrap();
    let path = dir.path().join("ocred.pdf");
    {
        // Invisible text (`3 Tr`) over the picture, as an OCR layer is written.
        use lopdf::{dictionary, Object, Stream};
        let mut d = lopdf::Document::load(&scan).unwrap();
        let page = *d.get_pages().get(&1).unwrap();
        let font = d.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
        let stream = d.add_object(Stream::new(dictionary! {}, b"BT 3 Tr /FOcr 12 Tf 40 300 Td (a secret word) Tj ET".to_vec()));
        let p = d.get_object_mut(page).unwrap().as_dict_mut().unwrap();
        let contents = match p.get(b"Contents").unwrap().clone() {
            Object::Array(mut a) => {
                a.push(stream.into());
                a
            }
            one => vec![one, stream.into()],
        };
        p.set("Contents", contents);
        let mut res = match p.get(b"Resources").unwrap().clone() {
            Object::Dictionary(r) => r,
            Object::Reference(r) => d.get_object(r).unwrap().as_dict().unwrap().clone(),
            _ => panic!("no resources"),
        };
        let mut fonts = res.get(b"Font").and_then(|f| f.as_dict()).cloned().unwrap_or_default();
        fonts.set("FOcr", font);
        res.set("Font", fonts);
        d.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Resources", res);
        d.save(&path).unwrap();
    }
    let (ops, rx, _) = started(&path);
    ops.send(Op::ReplaceAll { find: "secret".into(), with: "public".into(), case: false, whole: true }).unwrap();
    let (_, err) = next(&rx);
    assert!(err.as_deref().is_some_and(|e| e.contains("only on scanned pages")), "{err:?}");
}
