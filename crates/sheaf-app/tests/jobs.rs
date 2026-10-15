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
        &Job::Merge { inputs: vec![Input::new(&src), Input::new(&src)], out: dir.path().join("m.pdf") },
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
        &Job::Merge { inputs: vec![Input::new(&src)], out: dir.path().join("m.pdf") },
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
    let err = run(&Job::Compress { items, level: Level::Recommended }, &cancel, &|_| {
        // After file 1: it is on disk, then the user cancels.
        if !first_done.get() {
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
    let out = run(&Job::Compress { items, level: Level::Recommended }, &AtomicBool::new(false), &|_| {}).unwrap();
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

    assert!(go(Job::Merge { inputs: vec![Input::new(&src), enc.clone()], out: o("m.pdf") }).was_encrypted);
    assert!(!go(Job::Merge { inputs: vec![Input::new(&src)], out: o("m2.pdf") }).was_encrypted);
    assert!(go(Job::Merge { inputs: vec![Input::new(&owner_only)], out: o("m3.pdf") }).was_encrypted, "owner-password-only counts");
    assert!(go(Job::Rotate { items: vec![(enc.clone(), o("r.pdf"))], degrees: 90, pages: None }).was_encrypted);
    assert!(go(Job::Repair { items: vec![(enc.clone(), o("f.pdf"))] }).was_encrypted);
    assert!(go(Job::ExtractPages { input: enc.clone(), pages: "1".into(), out: o("e.pdf") }).was_encrypted);
    assert!(!go(Job::Unlock { items: vec![(enc.clone(), o("u.pdf"))] }).was_encrypted);
    assert!(!go(Job::Protect { items: vec![(enc, o("pp.pdf"))], password: "x".into(), owner: None }).was_encrypted);
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
    let out = run(&Job::Compress { items, level: Level::Recommended }, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(out.outputs, vec![dir.path().join("c1.pdf")]);
    assert!(out.outputs[0].is_file());
    assert_eq!(out.failures.len(), 1, "{:?}", out.failures);
    assert_eq!(out.failures[0].0, garbage);
    assert!(!out.failures[0].1.is_empty());
    // Every file failing is still an error, so a single-file run keeps its toast.
    let only = vec![(Input::new(&garbage), dir.path().join("c3.pdf"))];
    assert!(run(&Job::Compress { items: only, level: Level::Recommended }, &AtomicBool::new(false), &|_| {}).is_err());
}

#[test]
fn office_batch_keeps_the_first_pdf_when_the_second_fails() {
    if sheaf_core::office::find_soffice().is_none() { return; }
    let dir = tempfile::tempdir().unwrap();
    let html = dir.path().join("a.html");
    std::fs::write(&html, "<html><body><p>Hello</p></body></html>").unwrap();
    let missing = dir.path().join("missing.docx");
    let items = vec![(html, dir.path().join("a.pdf")), (missing.clone(), dir.path().join("b.pdf"))];
    let out = run(&Job::Office { items }, &AtomicBool::new(false), &|_| {}).unwrap();
    assert_eq!(out.outputs, vec![dir.path().join("a.pdf")]);
    assert_eq!(out.failures.iter().map(|f| f.0.clone()).collect::<Vec<_>>(), vec![missing]);
}
