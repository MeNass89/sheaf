mod common;
use common::{fixture_pdf, page_texts};
use pdfium_render::prelude::*;
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::enhance::{enhance, rotate_white, skew_angle};
use sheaf_core::images::{images_to_pdf, PageLayout};
use sheaf_core::pages::{merge, Input};
use std::path::{Path, PathBuf};

/// A page of text lines drawn by pdfium, rendered at 150 dpi on greyish paper, turned `tilt`
/// degrees clockwise as a sheet fed askew, and that picture alone on a page.
fn crooked_scan(dir: &Path, tilt: f32) -> PathBuf {
    let png = dir.join("crooked.png");
    {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        for i in 0..20 {
            let y = 760.0 - i as f32 * 34.0;
            page.objects_mut().create_text_object(PdfPoints::new(60.0), PdfPoints::new(y), "Sheaf straightens a page fed in crooked", font, PdfPoints::new(18.0)).unwrap();
        }
        let cfg = PdfRenderConfig::new().set_target_width(1240).set_clear_color(PdfColor::new(222, 216, 204, 255));
        let rgb = page.render_with_config(&cfg).unwrap().as_image().unwrap().to_rgb8();
        rotate_white(&rgb, -tilt).save(&png).unwrap();
    }
    images_to_pdf(&[png], PageLayout::FitImage, &dir.join("crooked.pdf")).unwrap()
}

fn page_picture(path: &Path, page: i32) -> image::RgbImage {
    let _g = pdfium_lock();
    let doc = sheaf_core::engine::open_pdf(path, None).unwrap();
    let p = doc.pages().get(page).unwrap();
    p.render_with_config(&PdfRenderConfig::new().set_target_width(1000)).unwrap().as_image().unwrap().to_rgb8()
}

/// A scan of a page of text printed upside down (a picture, no text) is found by reading it, and
/// its turn is half a turn.
#[test]
fn a_scan_upside_down_is_found_by_reading_it() {
    if !sheaf_core::ocr::available() {
        eprintln!("skip: no OCR models");
        return;
    }
    let t = tempfile::tempdir().unwrap();
    // Text drawn as a picture: a PDF page of words rendered, turned upside down, made a scan.
    let png = t.path().join("page.png");
    {
        let _g = pdfium_lock();
        let mut doc = new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        for k in 0..8 {
            page.objects_mut().create_text_object(PdfPoints::new(60.0), PdfPoints::new(760.0 - 40.0 * k as f32), "The quick brown fox jumps over the lazy dog", font, PdfPoints::new(18.0)).unwrap();
        }
        let img = page.render_with_config(&PdfRenderConfig::new().set_target_width(1200)).unwrap().as_image().unwrap();
        img.rotate180().save(&png).unwrap();
    }
    let scan = sheaf_core::images::images_to_pdf(&[png], sheaf_core::images::PageLayout::FitImage, &t.path().join("scan.pdf")).unwrap();
    let turns = sheaf_core::enhance::scan_upright_turns(&std::fs::read(&scan).unwrap(), None, &[0]);
    assert_eq!(turns, [(0, 180)]);
}

#[test]
fn a_crooked_scan_comes_out_level_on_white_and_text_pages_are_kept() {
    let t = tempfile::tempdir().unwrap();
    let scan = crooked_scan(t.path(), 2.5);
    let before = image::imageops::grayscale(&page_picture(&scan, 0));
    assert!((skew_angle(&before) - 2.5).abs() < 0.2, "{}", skew_angle(&before));
    let text = fixture_pdf(t.path(), "text.pdf", 1);
    let both = merge(&[Input::new(&scan), Input::new(&text)], &t.path().join("both.pdf")).unwrap();
    let mut seen = Vec::new();
    let r = enhance(&Input::new(&both), &t.path().join("clean.pdf"), &mut |f| {
        seen.push(f);
        true
    })
    .unwrap();
    assert_eq!((r.enhanced, r.kept), (1, 1));
    assert_eq!(seen.last(), Some(&1.0));
    let after = page_picture(&r.path, 0);
    let skew = skew_angle(&image::imageops::grayscale(&after));
    assert!(skew.abs() < 0.2, "{skew}");
    // The paper is white now; the page keeps its size; the text page is untouched.
    assert_eq!(after.get_pixel(500, 20).0, [255, 255, 255]);
    let size = |p: &Path| {
        let _g = pdfium_lock();
        let d = sheaf_core::engine::open_pdf(p, None).unwrap();
        let pg = d.pages().get(0).unwrap();
        (pg.width().value.round(), pg.height().value.round())
    };
    assert_eq!(size(&r.path), size(&scan));
    assert_eq!(page_texts(&r.path)[1], "Page 1");
}

#[test]
fn enhancing_stops_when_asked() {
    let t = tempfile::tempdir().unwrap();
    let scan = crooked_scan(t.path(), 1.0);
    let out = t.path().join("x.pdf");
    assert!(matches!(enhance(&Input::new(&scan), &out, &mut |_| false), Err(sheaf_core::OpdfError::Cancelled)));
    assert!(!out.exists());
}

#[test]
fn a_scan_with_an_ocr_layer_is_still_cleaned() {
    let t = tempfile::tempdir().unwrap();
    let scan = crooked_scan(t.path(), 2.0);
    // The text a reader found, invisible over the picture (`3 Tr`, as OCR writes it).
    let ocred = t.path().join("ocred.pdf");
    common::add_invisible_text(&scan, "Sheaf straightens a page", &ocred);
    let r = enhance(&Input::new(&ocred), &t.path().join("clean.pdf"), &mut |_| true).unwrap();
    assert_eq!((r.enhanced, r.kept), (1, 0), "an OCR'd scan is a scan");
}
