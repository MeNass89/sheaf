use sheaf_app::jobs::{run, Job};
use sheaf_core::{engine::{open_pdf, pdfium_lock}, images::{images_to_pdf, PageLayout}, pages::Input, OpdfError};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn two_page_pdf(dir: &Path) -> PathBuf {
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    image::RgbImage::from_pixel(200, 300, image::Rgb([200, 30, 30])).save(&a).unwrap();
    image::RgbImage::from_pixel(300, 200, image::Rgb([30, 30, 200])).save(&b).unwrap();
    images_to_pdf(&[a, b], PageLayout::FitImage, &dir.join("two.pdf")).unwrap()
}

#[test]
fn merge_then_protect_and_unlock_without_password() {
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let no_cancel = AtomicBool::new(false);
    let seen = std::cell::Cell::new(0.0f32);
    let progress = |p: f32| seen.set(p);

    let merged = run(
        &Job::Merge { inputs: vec![Input::new(&src), Input::new(&src)], pages: vec![], seq: vec![], out: dir.path().join("m.pdf") },
        &no_cancel,
        &progress,
    )
    .unwrap();
    assert_eq!(merged.outputs.len(), 1);
    assert_eq!(seen.get(), 1.0);
    {
        let _g = pdfium_lock();
        assert_eq!(open_pdf(&merged.outputs[0], None).unwrap().pages().len(), 4);
    }

    let prot = run(
        &Job::Protect {
            items: vec![(Input::new(&src), dir.path().join("p.pdf"))],
            password: "secret".into(),
            owner: None,
            permissions: Default::default(),
            sanitize: false,
        },
        &no_cancel,
        &progress,
    )
    .unwrap();
    let err = run(
        &Job::Unlock { items: vec![(Input::new(&prot.outputs[0]), dir.path().join("u.pdf"))] },
        &no_cancel,
        &progress,
    )
    .unwrap_err();
    assert!(matches!(err, OpdfError::PasswordRequired(_)), "{err}");
}

#[test]
fn cancel_before_start_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let cancel = AtomicBool::new(true);
    let out = dir.path().join("r.pdf");
    let err = run(
        &Job::Repair { items: vec![(Input::new(&src), out.clone())] },
        &cancel,
        &|_| {},
    )
    .unwrap_err();
    assert!(matches!(err, OpdfError::Cancelled));
    assert!(!out.exists());
}

#[test]
fn jpg_to_pdf_one_page_per_image_and_cancel_midway_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let imgs: Vec<PathBuf> = (0..6)
        .map(|i| {
            let p = dir.path().join(format!("i{i}.jpg"));
            image::RgbImage::from_pixel(120, 80, image::Rgb([i * 40, 90, 160])).save(&p).unwrap();
            p
        })
        .collect();
    let out = dir.path().join("all.pdf");
    let job = Job::JpgToPdf { images: imgs.clone(), layout: PageLayout::FitImage, out: out.clone() };
    let done = run(&job, &AtomicBool::new(false), &|_| {}).unwrap();
    {
        let _g = pdfium_lock();
        assert_eq!(open_pdf(&done.outputs[0], None).unwrap().pages().len(), 6);
    }

    // Cancel once a third of the images are done: no output appears.
    let cancel = AtomicBool::new(false);
    let out2 = dir.path().join("cancelled.pdf");
    let job = Job::JpgToPdf { images: imgs, layout: PageLayout::FitImage, out: out2.clone() };
    let err = run(&job, &cancel, &|p| {
        if p >= 0.3 {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    })
    .unwrap_err();
    assert!(matches!(err, OpdfError::Cancelled));
    assert!(!out2.exists());
}

#[test]
fn finalize_removes_outputs_when_cancel_arrived_during_an_uninterruptible_call() {
    use sheaf_app::jobs::{finalize, JobOutcome};
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    // Merge is one engine call: the cancel flag is only seen after it returned Ok.
    let done = run(
        &Job::Merge { inputs: vec![Input::new(&src)], pages: vec![], seq: vec![], out: dir.path().join("m.pdf") },
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    let out = done.outputs[0].clone();
    assert!(out.exists());
    let kept = finalize(done.clone(), false).unwrap();
    assert_eq!(kept.outputs, vec![out.clone()]);
    assert!(out.exists());
    let err = finalize(done, true).unwrap_err();
    assert!(matches!(err, OpdfError::Cancelled));
    assert!(!out.exists(), "outputs of a cancelled job are deleted");
    // Nothing to delete is fine too.
    assert!(matches!(finalize(JobOutcome::default(), true), Err(OpdfError::Cancelled)));
}

#[test]
fn probing_20_images_runs_at_most_3_at_once_with_small_thumbnails() {
    use sheaf_app::files::{image_thumb, IMAGE_THUMB_PX};
    use sheaf_app::pool::{Pool, PROBE_THREADS};
    let dir = tempfile::tempdir().unwrap();
    let imgs: Vec<PathBuf> = (0..20)
        .map(|i| {
            let p = dir.path().join(format!("i{i}.png"));
            image::RgbImage::from_pixel(1600, 1200, image::Rgb([i * 10, 80, 160])).save(&p).unwrap();
            p
        })
        .collect();
    let pool = Pool::new(PROBE_THREADS);
    let (tx, rx) = std::sync::mpsc::channel();
    for p in imgs {
        let tx = tx.clone();
        pool.submit(move || {
            let t = image_thumb(&p, IMAGE_THUMB_PX).unwrap();
            tx.send((t.width, t.height)).unwrap();
        });
    }
    drop(tx);
    let sizes: Vec<(u32, u32)> = rx.iter().collect();
    assert_eq!(sizes.len(), 20);
    assert!(sizes.iter().all(|&(w, h)| w <= 120 && h <= 120 && w.max(h) == 120), "{sizes:?}");
    assert!(pool.peak() <= 3, "peak {}", pool.peak());
    assert!(pool.peak() >= 2, "the pool should actually run in parallel, peak {}", pool.peak());
}

#[test]
fn compress_batch_cancelled_after_the_first_file_leaves_no_outputs() {
    use sheaf_core::compress::Level;
    use std::sync::atomic::Ordering;
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let outs: Vec<PathBuf> = (0..3).map(|i| dir.path().join(format!("c{i}.pdf"))).collect();
    let items = outs.iter().map(|o| (Input::new(&src), o.clone())).collect();
    let cancel = AtomicBool::new(false);
    let first_done = std::cell::Cell::new(false);
    let err = run(&Job::Compress { items, level: Level::Recommended, advanced: Default::default() }, &cancel, &|p| {
        // After file 1 (a third of the way: progress also comes within each file): it is on
        // disk, then the user cancels.
        if !first_done.get() && p >= 1.0 / 3.0 - 1e-6 {
            first_done.set(true);
            assert!(outs[0].exists(), "first output written before the cancel");
            cancel.store(true, Ordering::Relaxed);
        }
    })
    .unwrap_err();
    assert!(matches!(err, OpdfError::Cancelled), "{err}");
    for o in &outs {
        assert!(!o.exists(), "{} left behind", o.display());
    }
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    assert_eq!(left.len(), 3, "only the fixture files remain: {left:?}"); // a.png, b.png, two.pdf
}

#[test]
fn compress_batch_writes_each_output_in_its_own_inputs_folder() {
    // "Save next to the original file" with inputs in two folders.
    use sheaf_core::compress::Level;
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    let (x, y) = (two_page_pdf(&a), two_page_pdf(&b));
    let items = vec![
        (Input::new(&x), a.join("two-compressed.pdf")),
        (Input::new(&y), b.join("two-compressed.pdf")),
    ];
    let out = run(&Job::Compress { items, level: Level::Recommended, advanced: Default::default() }, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(out.outputs, vec![a.join("two-compressed.pdf"), b.join("two-compressed.pdf")]);
    assert!(out.outputs.iter().all(|p| p.is_file()));
}

fn protect_with_qpdf(src: &Path, out: &Path, user: &str, owner: &str) {
    assert!(std::process::Command::new("qpdf").args(["--encrypt", user, owner, "256", "--"]).arg(src).arg(out).status().unwrap().success());
}

#[test]
fn every_tool_but_protect_and_unlock_reports_an_encrypted_input() {
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let p = dir.path().join("p.pdf");
    protect_with_qpdf(&src, &p, "pw", "pw");
    let owner_only = dir.path().join("o.pdf");
    protect_with_qpdf(&src, &owner_only, "", "owner");
    let enc = Input { path: p.clone(), password: Some("pw".into()) };
    let go = |job: Job| run(&job, &AtomicBool::new(false), &|_| {}).unwrap();
    let o = |n: &str| dir.path().join(n);

    assert!(go(Job::Merge { inputs: vec![Input::new(&src), enc.clone()], pages: vec![], seq: vec![], out: o("m.pdf") }).was_encrypted);
    assert!(!go(Job::Merge { inputs: vec![Input::new(&src)], pages: vec![], seq: vec![], out: o("m2.pdf") }).was_encrypted);
    assert!(go(Job::Merge { inputs: vec![Input::new(&owner_only)], pages: vec![], seq: vec![], out: o("m3.pdf") }).was_encrypted, "owner-password-only counts");
    assert!(go(Job::Rotate { items: vec![(enc.clone(), o("r.pdf"))], degrees: 90, pages: None, filter: Default::default() }).was_encrypted);
    assert!(go(Job::Repair { items: vec![(enc.clone(), o("f.pdf"))] }).was_encrypted);
    assert!(go(Job::ExtractPages { input: enc.clone(), pages: "1".into(), out: o("e.pdf") }).was_encrypted);
    assert!(!go(Job::Unlock { items: vec![(enc.clone(), o("u.pdf"))] }).was_encrypted);
    assert!(!go(Job::Protect { items: vec![(enc, o("pp.pdf"))], password: "x".into(), owner: None, permissions: Default::default(), sanitize: false }).was_encrypted);
}

#[test]
fn a_batch_keeps_the_outputs_written_before_a_failure() {
    use sheaf_core::compress::Level;
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let garbage = dir.path().join("garbage.pdf");
    std::fs::write(&garbage, b"this is not a pdf").unwrap();
    let items = vec![
        (Input::new(&src), dir.path().join("c1.pdf")),
        (Input::new(&garbage), dir.path().join("c2.pdf")),
    ];
    let out = run(&Job::Compress { items, level: Level::Recommended, advanced: Default::default() }, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(out.outputs, vec![dir.path().join("c1.pdf")]);
    assert!(out.outputs[0].is_file());
    assert_eq!(out.failures.len(), 1, "{:?}", out.failures);
    assert_eq!(out.failures[0].0, garbage);
    assert!(!out.failures[0].1.is_empty());
    // Every file failing is still an error, so a single-file run keeps its toast.
    let only = vec![(Input::new(&garbage), dir.path().join("c3.pdf"))];
    assert!(run(&Job::Compress { items: only, level: Level::Recommended, advanced: Default::default() }, &AtomicBool::new(false), &|_| {}).is_err());
}

#[test]
fn merge_takes_images_and_documents_with_pdfs() {
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let png = dir.path().join("c.png");
    image::RgbImage::from_pixel(120, 80, image::Rgb([30, 160, 30])).save(&png).unwrap();
    let mut inputs = vec![Input::new(&png), Input::new(&src)];
    let html = dir.path().join("d.html");
    let office = sheaf_core::office::find_soffice().is_some();
    if office {
        std::fs::write(&html, "<html><body><p>Hello</p></body></html>").unwrap();
        inputs.push(Input::new(&html));
    }
    // The PDF's page 2 only; the image and the document whole.
    let out = run(&Job::Merge { inputs, pages: vec![String::new(), "2".into()], seq: vec![], out: dir.path().join("m.pdf") }, &AtomicBool::new(false), &|_| {}).unwrap();
    let _g = pdfium_lock();
    let doc = open_pdf(&out.outputs[0], None).unwrap();
    assert_eq!(doc.pages().len(), if office { 3 } else { 2 });
    // The image's page has its proportions (120 × 80), then the PDF's page 2 (300 × 200 px image).
    let w = |i: i32| doc.pages().get(i).unwrap().width().value / doc.pages().get(i).unwrap().height().value;
    assert!((w(0) - 1.5).abs() < 0.01 && (w(1) - 1.5).abs() < 0.01, "{} {}", w(0), w(1));
}

#[test]
fn office_batch_keeps_the_first_pdf_when_the_second_fails() {
    if sheaf_core::office::find_soffice().is_none() { return; }
    let dir = tempfile::tempdir().unwrap();
    let html = dir.path().join("a.html");
    std::fs::write(&html, "<html><body><p>Hello</p></body></html>").unwrap();
    let missing = dir.path().join("missing.docx");
    let items = vec![(html, dir.path().join("a.pdf")), (missing.clone(), dir.path().join("b.pdf"))];
    let out = run(&Job::Office { items, html: None, whole_sheets: false }, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(out.outputs, vec![dir.path().join("a.pdf")]);
    assert_eq!(out.failures.iter().map(|f| f.0.clone()).collect::<Vec<_>>(), vec![missing]);
}

#[test]
fn bates_numbers_run_on_across_a_batch() {
    use sheaf_core::numbers::{Bates, NumberOptions, NumberText};
    let dir = tempfile::tempdir().unwrap();
    let a = two_page_pdf(dir.path());
    let b = dir.path().join("b.pdf");
    std::fs::copy(&a, &b).unwrap();
    let mut texts: [String; 6] = Default::default();
    texts[4] = "{bates}".into();
    let options = NumberOptions { text: NumberText::Nothing, texts, bates: Bates { digits: 3, start: 1, prefix: "X-".into(), suffix: String::new() }, ..Default::default() };
    let job = Job::PageNumbers { items: vec![(Input::new(&a), dir.path().join("a-n.pdf")), (Input::new(&b), dir.path().join("b-n.pdf"))], options };
    run(&job, &AtomicBool::new(false), &|_| {}).unwrap();
    // The second file goes on from the first's two pages: X-003.
    let text = |p: &Path| String::from_utf8_lossy(&std::process::Command::new("pdftotext").arg(p).arg("-").output().unwrap().stdout).into_owned();
    assert!(text(&dir.path().join("a-n.pdf")).contains("X-002"));
    assert!(text(&dir.path().join("b-n.pdf")).contains("X-003"), "{}", text(&dir.path().join("b-n.pdf")));
}

#[test]
fn numbers_watermark_and_crop_jobs_warn_on_protected_input() {
    use sheaf_core::{crop::CropSpec, numbers::NumberOptions, watermark::{Mark, WatermarkOptions}};
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let prot = sheaf_core::security::protect(&Input::new(&src), "pw", None, &dir.path().join("p.pdf")).unwrap();
    let locked = Input { path: prot, password: Some("pw".into()) };
    let no = AtomicBool::new(false);
    let mark = Mark::Text { text: "DRAFT".into(), font_size: 30.0, color: [0.0; 3] };
    let jobs = [
        Job::PageNumbers { items: vec![(Input::new(&src), dir.path().join("n.pdf")), (locked.clone(), dir.path().join("n2.pdf"))], options: NumberOptions::default() },
        Job::Watermark { items: vec![(locked.clone(), dir.path().join("w.pdf"))], options: WatermarkOptions { mark, ..Default::default() } },
        Job::Crop { input: locked.clone(), pages: None, spec: CropSpec::Margins { top: 10.0, bottom: 10.0, left: 10.0, right: 10.0 }, out: dir.path().join("c.pdf") },
    ];
    for job in &jobs {
        let o = run(job, &no, &|_| {}).unwrap();
        assert!(o.was_encrypted, "{job:?}");
        assert!(o.outputs.iter().all(|p| p.is_file()));
    }
    let plain = Job::Crop { input: Input::new(&src), pages: Some("1".into()), spec: CropSpec::Remove, out: dir.path().join("c2.pdf") };
    assert!(!run(&plain, &no, &|_| {}).unwrap().was_encrypted);
}

#[test]
fn pdfa_job_reports_conformance_or_the_rules_still_broken() {
    use sheaf_core::pdfa::PdfALevel;
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let no = AtomicBool::new(false);
    let o = run(&Job::PdfA { items: vec![(Input::new(&src), dir.path().join("a-pdfa.pdf"))], level: PdfALevel::A2b, allow_incomplete: true }, &no, &|_| {}).unwrap();
    assert_eq!(o.summary, "Conforms to PDF/A-2b");
    assert!(o.notes.is_empty() && !o.was_encrypted);
    // Text in a font that is not embedded cannot be fixed: the rule is listed.
    let text = helvetica_pdf(dir.path());
    let prot = sheaf_core::security::protect(&Input::new(&text), "pw", None, &dir.path().join("p.pdf")).unwrap();
    let locked = Input { path: prot, password: Some("pw".into()) };
    let o = run(&Job::PdfA { items: vec![(locked, dir.path().join("p-pdfa.pdf"))], level: PdfALevel::A3b, allow_incomplete: true }, &no, &|_| {}).unwrap();
    assert!(o.was_encrypted);
    assert_eq!(o.summary, "1 file does not fully conform to PDF/A-3b");
    assert!(o.notes.iter().any(|n| n.starts_with("6.2.11.4: ")), "{:?}", o.notes);
}

/// One A4 page of text in Helvetica, not embedded.
fn helvetica_pdf(dir: &Path) -> PathBuf {
    use pdfium_render::prelude::*;
    let _g = pdfium_lock();
    let mut doc = sheaf_core::engine::new_document();
    let font = doc.fonts_mut().helvetica();
    let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
    page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(720.0), "Hello", font, PdfPoints::new(24.0)).unwrap();
    let path = dir.join("text.pdf");
    doc.save_to_file(&path).unwrap();
    path
}

#[test]
fn ocr_job_reports_progress_and_a_cancel_inside_a_file_writes_nothing() {
    use sheaf_core::ocr::OcrOptions;
    use std::sync::atomic::Ordering;
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let text = helvetica_pdf(dir.path());
    let no = AtomicBool::new(false);
    let seen = std::cell::RefCell::new(Vec::new());
    let items = vec![(Input::new(&src), dir.path().join("a-ocr.pdf")), (Input::new(&text), dir.path().join("t-ocr.pdf"))];
    let o = run(&Job::Ocr { items: items.clone(), options: OcrOptions::default() }, &no, &|p| seen.borrow_mut().push(p)).unwrap();
    assert_eq!(o.outputs.len(), 2);
    assert_eq!(o.summary, "2 pages read, 0 words added, 1 page already had text");
    let seen = seen.into_inner();
    assert!(seen.windows(2).all(|w| w[0] <= w[1]) && seen.last() == Some(&1.0), "{seen:?}");
    assert!(seen.iter().any(|p| *p > 0.0 && *p < 0.5), "progress inside the first file: {seen:?}");
    // Cancel pressed while the first file is being read.
    let cancel = AtomicBool::new(false);
    let r = run(&Job::Ocr { items, options: OcrOptions::default() }, &cancel, &|p| if p > 0.0 { cancel.store(true, Ordering::Relaxed) });
    assert!(matches!(r, Err(OpdfError::Cancelled)), "{r:?}");
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    assert!(!names.iter().any(|n| n.contains("ocr (1)") || n.ends_with(".part")), "{names:?}");
}

#[test]
fn export_jobs_write_office_files_and_never_warn() {
    use sheaf_app::jobs::ExportKind;
    use sheaf_core::{excel::SheetLayout, export::ExportOptions};
    let dir = tempfile::tempdir().unwrap();
    let text = helvetica_pdf(dir.path());
    let prot = sheaf_core::security::protect(&Input::new(&text), "pw", None, &dir.path().join("p.pdf")).unwrap();
    let locked = Input { path: prot, password: Some("pw".into()) };
    let no = AtomicBool::new(false);
    for (kind, ext) in [(ExportKind::Word, "docx"), (ExportKind::PowerPoint, "pptx"), (ExportKind::Excel(SheetLayout::OneSheet), "xlsx")] {
        let out = dir.path().join(format!("p.{ext}"));
        let o = run(&Job::Export { items: vec![(locked.clone(), out.clone())], kind, options: ExportOptions::default() }, &no, &|_| {}).unwrap();
        assert_eq!(o.outputs, vec![out.clone()]);
        assert!(!o.was_encrypted, "an Office file is not a PDF: no protection warning");
        let z = std::process::Command::new("unzip").arg("-l").arg(&out).output().unwrap();
        assert!(z.status.success(), "{ext} is a zip");
    }
}

#[test]
fn compare_job_shows_the_counts() {
    use sheaf_core::compare::CompareMode;
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let text = helvetica_pdf(dir.path());
    let no = AtomicBool::new(false);
    let o = run(&Job::Compare { old: Input::new(&text), new: Input::new(&text), mode: CompareMode::Text, out: dir.path().join("c.pdf"), pages: [None, None] }, &no, &|_| {}).unwrap();
    assert_eq!(o.summary, "No differences: the documents' text is identical.");
    assert_eq!(o.outputs.len(), 1);
    let o = run(&Job::Compare { old: Input::new(&src), new: Input::new(&text), mode: CompareMode::Overlay, out: dir.path().join("o.pdf"), pages: [None, None] }, &no, &|_| {}).unwrap();
    // The page-number-free image PDF has no words: every word of the new one is added. Its red
    // picture is gone too, a change no word accounts for: outlined. Its second page has no
    // counterpart: said.
    assert_eq!(o.summary, "1 added, 0 removed, 0 changed; 1 other changed area outlined; 1 page removed at the end");
    // Its first page only against the other: no page left over.
    let o = run(&Job::Compare { old: Input::new(&src), new: Input::new(&text), mode: CompareMode::Overlay, out: dir.path().join("p.pdf"), pages: [Some("1".into()), None] }, &no, &|_| {}).unwrap();
    assert_eq!(o.summary, "1 added, 0 removed, 0 changed; 1 other changed area outlined");
}

#[test]
fn redact_job_counts_what_it_redacts_and_warns_on_protected_input() {
    use sheaf_core::redact::{self, Query};
    let dir = tempfile::tempdir().unwrap();
    let text = helvetica_pdf(dir.path());
    // qpdf, not `security::protect`: about 1 in 30 of its outputs is refused by PdfCraft's
    // parser (InvalidEncryption), which would make this test flaky.
    let prot = dir.path().join("p.pdf");
    protect_with_qpdf(&text, &prot, "pw", "owner");
    let locked = Input { path: prot, password: Some("pw".into()) };
    let hits = redact::find(&locked, &Query::Literal { text: "hello".into(), case_insensitive: true, whole_word: false }).unwrap();
    let mut marks = redact::mark_text(&hits);
    marks.extend(redact::mark_rects(&[(0, [300.0, 300.0, 400.0, 400.0])]));
    let no = AtomicBool::new(false);
    let o = run(&Job::Redact { input: locked, marks, look: Default::default(), out: dir.path().join("r.pdf") }, &no, &|_| {}).unwrap();
    assert_eq!(o.summary, "1 text passage and 1 area redacted");
    assert!(o.was_encrypted);
    let d = lopdf::Document::load(&o.outputs[0]).unwrap();
    assert!(!d.extract_text(&[1]).unwrap().contains("Hello"));
}

#[test]
fn sign_job_places_the_signature_in_the_box_clicked_on_a_turned_page() {
    let dir = tempfile::tempdir().unwrap();
    // Page 2 of the fixture is landscape; turned a quarter it shows portrait.
    let src = two_page_pdf(dir.path());
    let turned = dir.path().join("turned.pdf");
    assert!(std::process::Command::new("qpdf").arg(&src).arg("--rotate=+90:2").arg(&turned).status().unwrap().success());
    let info = sheaf_core::sign::page_info(&Input::new(&turned), 1).unwrap();
    assert_eq!(info.rotation, 90);
    let (w, h) = (info.width as f64, info.height as f64);
    assert!(h > w, "shown portrait: {w} x {h}");
    // A box as displayed (points, y down): the middle 60 % across, around 40 % down.
    let view_box = [0.2 * w, 0.35 * h, 0.8 * w, 0.45 * h];
    let no_cancel = AtomicBool::new(false);
    let job = Job::Sign {
        input: Input::new(&turned),
        page: 1,
        view_box,
        graphic: sheaf_core::sign::Graphic::Drawn(vec![vec![[0.0, 0.0], [1.0, 0.0]]]),
        digital: None,
        out: dir.path().join("signed.pdf"),
    };
    let o = run(&job, &no_cancel, &|_| {}).unwrap();
    assert_eq!(o.summary, "Signed");
    assert!(!o.was_encrypted);
    // A horizontal line as displayed: rendered upright (2 px per point), the ink runs across
    // the box at its middle height.
    let doc = sheaf_core::pdfcraft::Doc::open(&Input::new(&o.outputs[0])).unwrap();
    let r = doc.render(1, 2.0).unwrap();
    let k = r.width as f64 / w;
    assert!(r.height > r.width);
    let dark: Vec<(u32, u32)> = (0..r.height)
        .flat_map(|y| (0..r.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let p = &r.rgba[((y * r.width + x) * 4) as usize..];
            p[3] > 128 && p[0] < 60 && p[1] < 60 && p[2] < 60
        })
        .collect();
    assert!(!dark.is_empty(), "no ink");
    let (xs, ys): (Vec<u32>, Vec<u32>) = dark.into_iter().unzip();
    let px = |v: &[u32], max: bool| *if max { v.iter().max() } else { v.iter().min() }.unwrap() as f64 / k;
    let (x0, x1, y0, y1) = (px(&xs, false), px(&xs, true), px(&ys, false), px(&ys, true));
    let near = |a: f64, b: f64| (a - b).abs() < 0.04 * w + 3.0;
    assert!(near(x0, view_box[0]) && near(x1, view_box[2]), "ink x {x0:.1}..{x1:.1}, box {view_box:?}");
    assert!(near((y0 + y1) / 2.0, 0.4 * h) && y1 - y0 < 0.05 * h, "ink y {y0:.1}..{y1:.1}");
}

#[test]
fn edit_job_puts_text_and_image_where_placed_in_the_viewer_on_a_turned_page() {
    use sheaf_app::jobs::EditPlaced;
    let dir = tempfile::tempdir().unwrap();
    // A blank A4 page shown turned a quarter (landscape as displayed).
    let blank = dir.path().join("blank.png");
    image::RgbImage::from_pixel(595, 842, image::Rgb([255, 255, 255])).save(&blank).unwrap();
    let src = images_to_pdf(&[blank], PageLayout::FitImage, &dir.path().join("blank.pdf")).unwrap();
    let turned = dir.path().join("turned.pdf");
    assert!(std::process::Command::new("qpdf").arg(&src).arg("--rotate=+90:1").arg(&turned).status().unwrap().success());
    let green = dir.path().join("g.png");
    image::RgbImage::from_pixel(20, 10, image::Rgb([20, 170, 40])).save(&green).unwrap();
    let job = Job::Edit {
        input: Input::new(&turned),
        items: vec![
            EditPlaced::Text { page: 0, at: [100.0, 50.0], text: "Placed".into(), size: 24.0, color: [26, 77, 217] },
            EditPlaced::Image { page: 0, rect: [500.0, 300.0, 600.0, 350.0], path: green },
        ],
        out: dir.path().join("edited.pdf"),
    };
    let o = run(&job, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!((o.summary.as_str(), o.was_encrypted), ("2 items added", false));
    let doc = sheaf_core::pdfcraft::Doc::open(&Input::new(&o.outputs[0])).unwrap();
    let r = doc.render(0, 1.0).unwrap();
    assert!(r.width > r.height, "shown landscape: {}x{}", r.width, r.height);
    let boxed = |f: &dyn Fn(&[u8]) -> bool| {
        let mut b: Option<[u32; 4]> = None;
        for y in 0..r.height {
            for x in 0..r.width {
                if f(&r.rgba[((y * r.width + x) * 4) as usize..][..4]) {
                    b = Some(b.map_or([x, y, x, y], |b| [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]));
                }
            }
        }
        b
    };
    let blue = boxed(&|p| p[2] > 150 && p[0] < 100 && p[1] < 130).expect("no text");
    assert!(blue[0] >= 100 && blue[0] <= 104 && blue[1] >= 50 && blue[3] < 80 && blue[2] - blue[0] > 50, "text ink {blue:?}");
    let g = boxed(&|p| p[1] > 140 && p[0] < 60 && p[2] < 80).expect("no image");
    assert!(g.iter().zip([500, 300, 599, 349]).all(|(a, e)| a.abs_diff(e) <= 1), "image at {g:?}");
}

#[test]
fn protect_job_applies_permission_restrictions() {
    let dir = tempfile::tempdir().unwrap();
    let src = two_page_pdf(dir.path());
    let out = dir.path().join("restricted.pdf");
    run(&Job::Protect {
        items: vec![(Input::new(&src), out.clone())],
        password: "user".into(),
        owner: Some("owner".into()),
        permissions: sheaf_core::security::ProtectPermissions { printing: false, ..Default::default() },
        sanitize: false,
    }, &AtomicBool::new(false), &|_| {}).unwrap();
    let result = std::process::Command::new("qpdf")
        .args(["--show-encryption", "--password=user"]).arg(&out).output().unwrap();
    assert!(result.status.success(), "{result:?}");
    let info = String::from_utf8(result.stdout).unwrap();
    assert!(info.contains("print low resolution: not allowed"), "{info}");
    assert!(info.contains("print high resolution: not allowed"), "{info}");
}
