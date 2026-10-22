mod common;
use common::fixture_pdf;
use sheaf_core::{images::*, pages::Input};

#[test]
fn pdf_to_jpg_one_file_per_page_at_dpi() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 12);
    let outs = pdf_to_jpg(&Input::new(&a), 72, Some(85), t.path()).unwrap();
    assert_eq!(outs.len(), 12);
    assert!(outs[0].ends_with("a-01.jpg"));
    let img = image::open(&outs[0]).unwrap();
    assert_eq!(img.width(), 595); // A4 = 595 pt wide; 72 dpi → 1 px per pt
    // PNG instead, at 600 dpi (one page: a 600 dpi render takes a second).
    let b = fixture_pdf(t.path(), "b.pdf", 1);
    std::fs::create_dir(t.path().join("png")).unwrap();
    let png = pdf_to_jpg(&Input::new(&b), 600, None, &t.path().join("png")).unwrap();
    assert!(png[0].ends_with("b-1.png"));
    assert_eq!(image::ImageReader::open(&png[0]).unwrap().with_guessed_format().unwrap().format(), Some(image::ImageFormat::Png));
    let w = image::image_dimensions(&png[0]).unwrap().0;
    assert!((4955..=4965).contains(&w), "{w} px for 595 pt at 600 dpi");
}
#[test]
fn images_to_pdf_fit_and_a4() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("x.png");
    image::RgbImage::from_pixel(400, 200, image::Rgb([200, 30, 30])).save(&p).unwrap();
    let fit = images_to_pdf(&[p.clone(), p.clone()], PageLayout::FitImage, &t.path().join("f.pdf")).unwrap();
    let a4 = images_to_pdf(&[p], PageLayout::A4 { landscape: false, margin_pt: 36.0 }, &t.path().join("a.pdf")).unwrap();
    let _g = sheaf_core::engine::pdfium_lock();
    let d = sheaf_core::engine::open_pdf(&fit, None).unwrap();
    assert_eq!(d.pages().len(), 2);
    assert_eq!(d.pages().get(0).unwrap().width().value.round(), 300.0); // 400 px at 96 dpi = 300 pt
    assert_eq!(sheaf_core::engine::open_pdf(&a4, None).unwrap().pages().get(0).unwrap().width().value.round(), 595.0);
}
#[test]
fn images_to_pdf_rejects_non_image() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("x.png");
    std::fs::write(&p, b"not an image").unwrap();
    assert!(images_to_pdf(&[p], PageLayout::FitImage, &t.path().join("f.pdf")).is_err());
}
#[test]
fn images_to_pdf_jpeg_passthrough_keeps_size() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("photo.jpg");
    let img = image::RgbImage::from_fn(1200, 800, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]));
    let f = std::fs::File::create(&p).unwrap();
    img.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(f, 80)).unwrap();
    let out = images_to_pdf(std::slice::from_ref(&p), PageLayout::FitImage, &t.path().join("j.pdf")).unwrap();
    let (jpg, pdf) = (std::fs::metadata(&p).unwrap().len(), std::fs::metadata(&out).unwrap().len());
    assert!((pdf as f64) < 1.3 * jpg as f64, "pdf {pdf} vs jpg {jpg}");
    let _g = sheaf_core::engine::pdfium_lock();
    let d = sheaf_core::engine::open_pdf(&out, None).unwrap();
    let page = d.pages().get(0).unwrap();
    assert_eq!(page.width().value.round(), 900.0); // 1200 px at 96 dpi
    // rendered at source resolution, a pixel near the top-left corner keeps its colour (not blank, not flipped)
    let r = page.render_with_config(&pdfium_render::prelude::PdfRenderConfig::new().set_target_width(1200)).unwrap().as_image().unwrap().to_rgb8();
    let (a, b) = (r.get_pixel(100, 50).0, img.get_pixel(100, 50).0);
    assert!(a.iter().zip(b).all(|(x, y)| x.abs_diff(y) < 24), "{a:?} vs {b:?}");
}
#[test]
fn images_to_pdf_rejects_empty_list() {
    let t = tempfile::tempdir().unwrap();
    assert!(matches!(images_to_pdf(&[], PageLayout::FitImage, &t.path().join("f.pdf")), Err(sheaf_core::OpdfError::Unsupported(_))));
}
#[test]
fn images_to_pdf_rejects_oversized_margin() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("x.png");
    image::RgbImage::from_pixel(40, 20, image::Rgb([1, 2, 3])).save(&p).unwrap();
    let layout = PageLayout::A4 { landscape: true, margin_pt: 297.5 }; // half of 595
    assert_eq!(layout, PageLayout::A4 { landscape: true, margin_pt: 297.5 });
    assert!(matches!(images_to_pdf(&[p], layout, &t.path().join("f.pdf")), Err(sheaf_core::OpdfError::Unsupported(_))));
}
#[test]
fn images_to_pdf_rejects_negative_or_nan_margin() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("x.png");
    image::RgbImage::from_pixel(40, 20, image::Rgb([1, 2, 3])).save(&p).unwrap();
    for m in [-1.0, f32::NAN, f32::INFINITY] {
        let r = images_to_pdf(std::slice::from_ref(&p), PageLayout::A4 { landscape: false, margin_pt: m }, &t.path().join("f.pdf"));
        assert!(matches!(r, Err(sheaf_core::OpdfError::Unsupported(_))), "{m}");
    }
}
