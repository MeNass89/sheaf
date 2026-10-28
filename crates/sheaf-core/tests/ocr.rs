mod common;
use common::{fixture_pdf, page_texts, qpdf_clean};
use pdfium_render::prelude::*;
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::images::{images_to_pdf, PageLayout};
use sheaf_core::ocr::{self, OcrOptions};
use sheaf_core::pages::Input;
use sheaf_core::security::protect;
use sheaf_core::OpdfError;
use std::path::{Path, PathBuf};

const WORDS: &str = "Sheaf OCR test 2026";

/// A scan-like PDF: a page of text drawn by pdfium, rendered at 300 dpi to a PNG, and that
/// picture as the only content of a page (no text layer).
fn scanned_pdf(dir: &Path) -> PathBuf {
    scan_of(dir, WORDS)
}

/// [`scanned_pdf`] showing `words`.
fn scan_of(dir: &Path, words: &str) -> PathBuf {
    let png = dir.join("scan.png");
    {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let size = PdfPagePaperSize::Custom(PdfPoints::new(420.0), PdfPoints::new(120.0));
        let mut page = doc.pages_mut().create_page_at_end(size).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(30.0), PdfPoints::new(50.0), words, font, PdfPoints::new(32.0)).unwrap();
        let px = (420.0 * 300.0 / 72.0) as i32;
        let cfg = PdfRenderConfig::new().set_target_width(px).set_clear_color(PdfColor::WHITE);
        page.render_with_config(&cfg).unwrap().as_image().unwrap().to_rgb8().save(&png).unwrap();
    }
    images_to_pdf(&[png], PageLayout::FitImage, &dir.join("scan.pdf")).unwrap()
}

fn words_in(path: &Path) -> usize {
    page_texts(path).iter().map(|t| t.split_whitespace().count()).sum()
}

fn no_cancel() -> impl FnMut(f32) -> bool {
    |_| true
}

#[test]
fn reads_the_words_of_a_scanned_page() {
    assert!(ocr::available(), "run scripts/fetch-models.sh");
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    assert_eq!(words_in(&scan), 0, "the input has no text layer");
    let mut seen = Vec::new();
    let r = ocr::ocr(&Input::new(&scan), &OcrOptions::default(), &t.path().join("scan-ocr.pdf"), &mut |p| {
        seen.push(p);
        true
    })
    .unwrap();
    assert!(!r.was_encrypted);
    assert_eq!((r.pages_read, r.pages_skipped), (1, 0));
    assert!(r.words >= 3, "{r:?}");
    let text = page_texts(&r.path).join(" ");
    assert!(text.contains("Sheaf") && text.contains("OCR"), "{text:?}");
    assert_eq!(seen.last(), Some(&1.0));
    qpdf_clean(&r.path);
}

#[test]
fn pages_that_already_have_text_are_skipped_by_default() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let r = ocr::ocr(&Input::new(&a), &OcrOptions::default(), &t.path().join("a-ocr.pdf"), &mut no_cancel()).unwrap();
    assert_eq!((r.words, r.pages_read, r.pages_skipped), (0, 0, 2));
    assert_eq!(page_texts(&r.path), ["Page 1", "Page 2"]);
    assert_eq!(words_in(&r.path), words_in(&a));
    qpdf_clean(&r.path);
}

#[test]
fn reading_text_pages_never_duplicates_their_text() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let opts = OcrOptions { read_text_pages: true, ..Default::default() };
    let r = ocr::ocr(&Input::new(&a), &opts, &t.path().join("a-ocr.pdf"), &mut no_cancel()).unwrap();
    assert_eq!((r.pages_read, r.pages_skipped), (1, 0));
    assert_eq!(page_texts(&r.path), ["Page 1"]);
}

#[test]
fn a_protected_scan_gives_an_unprotected_searchable_pdf() {
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    let p = protect(&Input::new(&scan), "pw", None, &t.path().join("p.pdf")).unwrap();
    let no_pw = ocr::ocr(&Input::new(&p), &OcrOptions::default(), &t.path().join("x.pdf"), &mut no_cancel());
    assert!(matches!(no_pw, Err(OpdfError::PasswordRequired(_))), "{no_pw:?}");
    let input = Input { path: p, password: Some("pw".into()) };
    let r = ocr::ocr(&input, &OcrOptions::default(), &t.path().join("p-ocr.pdf"), &mut no_cancel()).unwrap();
    assert!(r.was_encrypted);
    let o = std::process::Command::new("qpdf").arg("--show-encryption").arg(&r.path).output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).contains("File is not encrypted"));
    assert!(page_texts(&r.path).join(" ").contains("Sheaf"));
    qpdf_clean(&r.path);
}

#[test]
fn cancelling_writes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    let out = t.path().join("c.pdf");
    let r = ocr::ocr(&Input::new(&scan), &OcrOptions::default(), &out, &mut |_| false);
    assert!(matches!(r, Err(OpdfError::Cancelled)), "{r:?}");
    assert!(!out.exists());
    let left: Vec<_> = std::fs::read_dir(t.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left.len(), 2, "only scan.png and scan.pdf, no .part: {left:?}");
}

#[test]
fn accented_letters_are_read() {
    // ocrs' model reads unaccented Latin only ("Pr?valence"); PaddleOCR's Latin model reads them.
    let t = tempfile::tempdir().unwrap();
    let scan = scan_of(t.path(), "Prévalence élevée à âge");
    let opts = OcrOptions { language: "fr".into(), ..Default::default() };
    let r = ocr::ocr(&Input::new(&scan), &opts, &t.path().join("fr.pdf"), &mut no_cancel()).unwrap();
    let text = page_texts(&r.path).join(" ");
    assert!(text.contains("Prévalence") && text.contains("élevée"), "{text:?}");
}

#[test]
fn a_label_turned_on_its_side_is_read() {
    // A table's turned label ("rotate text 270°" in Word: it reads bottom to top), beside upright text.
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("turned.png");
    {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let size = PdfPagePaperSize::Custom(PdfPoints::new(420.0), PdfPoints::new(220.0));
        let mut page = doc.pages_mut().create_page_at_end(size).unwrap();
        let mut label = page.objects_mut().create_text_object(PdfPoints::ZERO, PdfPoints::ZERO, "Diagnostic", font, PdfPoints::new(24.0)).unwrap();
        label.rotate_counter_clockwise_degrees(90.0).unwrap();
        label.translate(PdfPoints::new(50.0), PdfPoints::new(30.0)).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(100.0), PdfPoints::new(100.0), "Sheaf OCR test", font, PdfPoints::new(24.0)).unwrap();
        let px = (420.0 * 300.0 / 72.0) as i32;
        let cfg = PdfRenderConfig::new().set_target_width(px).set_clear_color(PdfColor::WHITE);
        page.render_with_config(&cfg).unwrap().as_image().unwrap().to_rgb8().save(&png).unwrap();
    }
    let scan = images_to_pdf(&[png], PageLayout::FitImage, &t.path().join("turned.pdf")).unwrap();
    let r = ocr::ocr(&Input::new(&scan), &OcrOptions { language: "fr".into(), ..Default::default() }, &t.path().join("o.pdf"), &mut no_cancel()).unwrap();
    let text = page_texts(&r.path).join(" ");
    assert!(text.contains("Diagnostic") && text.contains("Sheaf"), "{text:?}");
}

#[test]
fn a_scan_fed_in_askew_is_read() {
    // Every line slants 2.5°, as a page fed crooked into a scanner: across a full line the slant
    // drifts more than a line's pitch (8 % of a dense page's words were read).
    let lines = [
        "Sheaf reads scanned pages even when the paper went into the scanner a little crooked",
        "the page is put straight before reading and every word is placed back where it is drawn",
        "so selecting a word on the scan highlights that word and searching for it finds it there",
        "a fiche printed dense with small type has its lines close together across the whole page",
        "and a slant of two or three degrees is enough for one line to run into the next one down",
        "which is why the reader would otherwise mix the words of two lines into one jumbled line",
    ];
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("askew.png");
    {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let size = PdfPagePaperSize::Custom(PdfPoints::new(560.0), PdfPoints::new(180.0));
        let mut page = doc.pages_mut().create_page_at_end(size).unwrap();
        for (k, line) in lines.iter().enumerate() {
            let mut o = page.objects_mut().create_text_object(PdfPoints::ZERO, PdfPoints::ZERO, line, font, PdfPoints::new(11.0)).unwrap();
            o.rotate_counter_clockwise_degrees(2.5).unwrap();
            o.translate(PdfPoints::new(20.0), PdfPoints::new(130.0 - 15.0 * k as f32)).unwrap();
        }
        let px = (560.0 * 200.0 / 72.0) as i32;
        let cfg = PdfRenderConfig::new().set_target_width(px).set_clear_color(PdfColor::WHITE);
        page.render_with_config(&cfg).unwrap().as_image().unwrap().to_rgb8().save(&png).unwrap();
    }
    let scan = images_to_pdf(&[png], PageLayout::FitImage, &t.path().join("askew.pdf")).unwrap();
    let r = ocr::ocr(&Input::new(&scan), &OcrOptions::default(), &t.path().join("o.pdf"), &mut no_cancel()).unwrap();
    let text = page_texts(&r.path).join(" ").to_lowercase();
    let want: Vec<&str> = lines.iter().flat_map(|l| l.split_whitespace()).collect();
    let found = want.iter().filter(|w| text.contains(&w.to_lowercase())).count();
    assert!(found * 10 >= want.len() * 9, "{found} of {} words: {text:?}", want.len());
}

#[test]
fn words_beyond_winansi_are_kept_in_the_text_layer() {
    // Helvetica's WinAnsi has no Greek and no arrows: "TNFα" was written "TNF?", "→" "?".
    use sheaf_core::pdfcraft::engine::{ocr::PlacedWord, Edit};
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let mut doc = sheaf_core::pdfcraft::Doc::open(&Input::new(&a)).unwrap();
    let word = |text: &str, x: f64| PlacedWord { text: text.into(), confidence: None, origin: [x, 500.0], across: [40.0, 0.0], up: [0.0, 12.0] };
    doc.apply(Edit::AddOcrText { page: 0, words: vec![word("anti-TNFα", 100.0), word("→", 150.0), word("élevée", 170.0)] }).unwrap();
    let out = doc.save(&t.path().join("o.pdf")).unwrap();
    let _g = pdfium_lock();
    let d = sheaf_core::engine::open_pdf(&out, None).unwrap();
    let text = d.pages().get(0).unwrap().text().unwrap().all();
    assert!(text.contains("anti-TNFα") && text.contains('→') && text.contains("élevée"), "{text:?}");
    qpdf_clean(&out);
}

#[test]
fn the_system_language_is_offered_first() {
    assert_eq!(ocr::default_language(Some("fr-FR")), "fr");
    assert_eq!(ocr::default_language(Some("de_CH.UTF-8")), "de");
    let japanese = if ocr::languages().iter().any(|(c, _)| *c == "ja") { "ja" } else { "en" };
    assert_eq!(ocr::default_language(Some("ja-JP")), japanese, "offer Japanese only with its installed model");
    assert_eq!(ocr::default_language(None), "en");
}

#[test]
fn unknown_languages_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let opts = OcrOptions { language: "xx".into(), ..Default::default() };
    let r = ocr::ocr(&Input::new(&a), &opts, &t.path().join("o.pdf"), &mut no_cancel());
    assert!(matches!(r, Err(OpdfError::Unsupported(_))), "{r:?}");
    assert!(ocr::languages().iter().any(|(code, _)| *code == "en"));
}

/// Files encrypted by other producers (qpdf, MuPDF: owner ≠ user password, AES-256 R6).
#[test]
fn foreign_encrypted_scans_are_read() {
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    for (path, pw) in common::foreign_encrypted(&scan, t.path()) {
        let input = Input { path: path.clone(), password: Some(pw.into()) };
        let r = ocr::ocr(&input, &OcrOptions::default(), &t.path().join("f-ocr.pdf"), &mut no_cancel()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(r.was_encrypted);
        assert!(page_texts(&r.path).join(" ").contains("Sheaf"), "{}", path.display());
        common::unprotected_and_clean(&r.path);
    }
}



#[test]
fn prepacked_recognition_keeps_the_words_and_their_placement() {
    use sheaf_core::pdfcraft::engine::ocr::{Models, Ocr};
    let models = Models::find().or_else(|| Models::in_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/models"))).unwrap();
    assert!(models.reads_accents(), "the Latin model is required for this comparison");
    let t = tempfile::tempdir().unwrap();
    let scan = scan_of(t.path(), "Révision OCR 2026");
    let d = sheaf_core::pdfcraft::Doc::open(&Input::new(scan)).unwrap();
    let shot = d.render(0, 300.0 / 72.0).unwrap();
    let ordinary = Ocr::load_with_prepacked_weights(&models, false).unwrap().recognize(&shot.rgba, shot.width, shot.height).unwrap();
    let packed = Ocr::load(&models).unwrap().recognize(&shot.rgba, shot.width, shot.height).unwrap();
    assert!(!packed.is_empty());
    assert_eq!(packed, ordinary, "packing changes matrix layout, not recognition or placement");
    let mut without_unused_english = models.clone();
    without_unused_english.recognition = t.path().join("not-installed.rten");
    let paddle_only = Ocr::load(&without_unused_english).unwrap().recognize(&shot.rgba, shot.width, shot.height).unwrap();
    assert_eq!(paddle_only, packed, "Paddle recognition must not depend on the unused English model");
}

/// Opt-in release benchmark for the orchestrator: the worker may write it but must not run it.
/// Set SHEAF_OCR_BENCH_PDF to the original MICI scan, with the bundled Latin models installed.
/// SHEAF_OCR_SECONDS_PER_PAGE overrides the recorded 11.75 s/page competitor target.
#[test]
#[ignore = "release-only OCR timing requires the MICI scan and an otherwise idle benchmark host"]
fn release_ocr_mici_benchmark_reports_cold_load_and_warm_page_time() {
    use sheaf_core::pdfcraft::{Doc, engine::ocr::{Models, Ocr, OcrSettings}};
    use std::time::Instant;
    if cfg!(debug_assertions) {
        eprintln!("skip: measure release OCR, never debug timings");
        return;
    }
    let source = std::env::var_os("SHEAF_OCR_BENCH_PDF").map(PathBuf::from).expect("Set SHEAF_OCR_BENCH_PDF to the MICI benchmark scan");
    let models = Models::find().or_else(|| Models::in_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/models"))).unwrap();
    assert!(models.reads_accents());
    let doc = Doc::open(&Input::new(&source)).unwrap();
    let mut baseline = None;
    let mut packed_median = 0.0;
    for packed in [false, true] {
        let cold = Instant::now();
        let engine = Ocr::load_with_prepacked_weights(&models, packed).unwrap();
        let load_seconds = cold.elapsed().as_secs_f64();
        let mut times = Vec::new();
        for run in 0..4 {
            let job = doc.session().ocr_job(doc.id(), &[], OcrSettings::default()).unwrap();
            let start = Instant::now();
            let pages = job.run(&engine, |_, _| true);
            let seconds = start.elapsed().as_secs_f64();
            assert!(!pages.is_empty() && pages.iter().all(|p| p.skipped.is_none()), "use an image-only MICI scan with no preexisting OCR text");
            let words: Vec<_> = pages.iter().flat_map(|p| p.words.iter()).map(|w| (w.text.clone(), w.origin, w.across, w.up)).collect();
            assert!(!words.is_empty());
            if let Some(expected) = &baseline { assert_eq!(&words, expected, "prepacking must preserve words and placement"); }
            else { baseline = Some(words); }
            eprintln!("OCR benchmark: prepacked={packed} run={run} pages={} words={} load_s={load_seconds:.3} page_s={:.3} source={} bytes={}", pages.len(), pages.iter().map(|p| p.words.len()).sum::<usize>(), seconds / pages.len() as f64, source.display(), std::fs::metadata(&source).unwrap().len());
            if run > 0 { times.push(seconds / pages.len() as f64); }
            // SHEAF_OCR_BENCH_OUT: the searchable PDF, to compare its text with the source's.
            if let Some(out) = std::env::var_os("SHEAF_OCR_BENCH_OUT").filter(|_| packed && run == 3) {
                let mut searchable = Doc::open(&Input::new(&source)).unwrap();
                for p in &pages {
                    searchable.apply(sheaf_core::pdfcraft::engine::Edit::AddOcrText { page: p.page, words: p.words.clone() }).unwrap();
                }
                searchable.save(Path::new(&out)).unwrap();
            }
        }
        times.sort_by(f64::total_cmp);
        eprintln!("OCR benchmark: prepacked={packed} engine={} warm_median_page_s={:.3}", engine.engine(), times[1]);
        if packed { packed_median = times[1]; }
    }
    let target: f64 = std::env::var("SHEAF_OCR_SECONDS_PER_PAGE").ok().map(|s| s.parse().unwrap()).unwrap_or(11.75);
    assert!(packed_median <= target, "bundled OCR {packed_median:.3} s/page exceeds the competitor target {target:.3} s/page");
}


/// The fast installed path must produce a searchable PDF, rather than just return TSV.
#[test]
fn automatic_installed_ocr_matches_explicit_local_text_and_preserves_scan() {
    if !std::process::Command::new("tesseract").arg("--list-langs").output()
        .is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).lines().any(|l| l == "eng")) {
        eprintln!("not available: local Tesseract English pack");
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let scan = scanned_pdf(t.path());
    let before = std::fs::read(&scan).unwrap();
    let started = std::time::Instant::now();
    let automatic = ocr::ocr(&Input::new(&scan), &OcrOptions::default(), &t.path().join("auto.pdf"), &mut no_cancel()).unwrap();
    let auto_seconds = started.elapsed().as_secs_f64();
    let started = std::time::Instant::now();
    let explicit = ocr::ocr(&Input::new(&scan), &OcrOptions { use_local_engine: true, ..Default::default() }, &t.path().join("local.pdf"), &mut no_cancel()).unwrap();
    eprintln!("Installed OCR short-page check: automatic={auto_seconds:.3}s explicit={:.3}s; {} words (not the MICI timing corpus)", started.elapsed().as_secs_f64(), automatic.words);
    assert_eq!(page_texts(&automatic.path), page_texts(&explicit.path));
    assert!(page_texts(&automatic.path).join(" ").contains("Sheaf OCR"));
    let render = |path: &Path| {
        let doc = sheaf_core::pdfcraft::Doc::open(&Input::new(path)).unwrap();
        doc.render(0, 1.0).unwrap().rgba
    };
    assert!(render(&scan) == render(&automatic.path), "the automatic OCR text layer repainted the scan");
    assert_eq!(std::fs::read(scan).unwrap(), before);
    qpdf_clean(&automatic.path);
}
