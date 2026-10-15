mod common;
use common::{dark_in, fixture_pdf, page_texts, qpdf_clean, render};
use sheaf_core::pages::Input;
use sheaf_core::watermark::{watermark, Mark, Placement, WatermarkOptions};

fn text(s: &str) -> Mark {
    Mark::Text { text: s.into(), font_size: 40.0, color: [0.0, 0.0, 0.0] }
}

/// Every dictionary in the file (direct objects and stream dictionaries).
fn dicts(d: &lopdf::Document) -> Vec<lopdf::Dictionary> {
    d.objects
        .values()
        .filter_map(|o| match o {
            lopdf::Object::Dictionary(d) => Some(d.clone()),
            lopdf::Object::Stream(s) => Some(s.dict.clone()),
            _ => None,
        })
        .collect()
}

fn page_contents(d: &lopdf::Document, page: u32) -> Vec<lopdf::ObjectId> {
    let id = d.get_pages()[&page];
    d.get_page_contents(id)
}

#[test]
fn text_lands_on_the_selected_pages_only() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 4);
    let o = WatermarkOptions { mark: text("CONFIDENTIAL"), from: Some(2), to: Some(3), ..Default::default() };
    let (out, warned) = watermark(&Input::new(&a), &o, &t.path().join("w.pdf")).unwrap();
    assert!(!warned);
    qpdf_clean(&out);
    let texts = page_texts(&out);
    assert!(!texts[0].contains("CONFIDENTIAL") && !texts[3].contains("CONFIDENTIAL"), "{texts:?}");
    assert!(texts[1].contains("CONFIDENTIAL") && texts[2].contains("CONFIDENTIAL"), "{texts:?}");
}

#[test]
fn transparency_adds_an_extgstate_with_ca() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let o = WatermarkOptions { mark: text("DRAFT"), opacity: 0.5, ..Default::default() };
    let (out, _) = watermark(&Input::new(&a), &o, &t.path().join("w.pdf")).unwrap();
    qpdf_clean(&out);
    let d = lopdf::Document::load(&out).unwrap();
    let page = d.get_pages()[&1];
    let res = d.get_dictionary(page).unwrap().get(b"Resources").unwrap();
    let res = d.dereference(res).unwrap().1.as_dict().unwrap();
    let gs = d.dereference(res.get(b"ExtGState").unwrap()).unwrap().1.as_dict().unwrap();
    let ca: Vec<f32> = gs.iter().filter_map(|(_, g)| d.dereference(g).ok()?.1.as_dict().ok()?.get(b"ca").ok()?.as_float().ok()).collect();
    assert!(ca.contains(&0.5), "{ca:?}");
}

#[test]
fn image_mode_adds_one_image_xobject_even_as_a_mosaic() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let png = t.path().join("logo.png");
    image::RgbImage::from_pixel(40, 20, image::Rgb([200, 30, 30])).save(&png).unwrap();
    let o = WatermarkOptions { mark: Mark::Image { path: png, scale: 0.2 }, placement: Placement::Mosaic, ..Default::default() };
    let (out, _) = watermark(&Input::new(&a), &o, &t.path().join("w.pdf")).unwrap();
    qpdf_clean(&out);
    let d = lopdf::Document::load(&out).unwrap();
    let images = dicts(&d).iter().filter(|x| x.get(b"Subtype").ok().and_then(|v| v.as_name().ok()) == Some(&b"Image"[..])).count();
    assert_eq!(images, 1, "the picture is embedded once and reused");
    let marks = |p| page_contents(&d, p).iter().filter(|id| tag(&d, **id) == Some("Watermark".into())).count();
    assert_eq!((marks(1), marks(2)), (9, 9), "mosaic: 3×3 tiles per page");
}

fn tag(d: &lopdf::Document, id: lopdf::ObjectId) -> Option<String> {
    let s = d.get_object(id).ok()?.as_stream().ok()?;
    Some(String::from_utf8_lossy(s.dict.get(b"PCMark").ok()?.as_name().ok()?).into_owned())
}

#[test]
fn below_the_content_draws_first() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    for (behind, first) in [(true, true), (false, false)] {
        let o = WatermarkOptions { mark: text("UNDER"), behind, ..Default::default() };
        let (out, _) = watermark(&Input::new(&a), &o, &t.path().join("w.pdf")).unwrap();
        qpdf_clean(&out);
        let d = lopdf::Document::load(&out).unwrap();
        let c = page_contents(&d, 1);
        assert_eq!(tag(&d, c[0]) == Some("Watermark".into()), first, "behind={behind}");
    }
}

#[test]
fn grid_position_places_the_mark_in_its_corner() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let o = WatermarkOptions { mark: text("XYZ"), placement: Placement::Grid(8), rotation: 0.0, opacity: 1.0, ..Default::default() };
    let (out, _) = watermark(&Input::new(&a), &o, &t.path().join("w.pdf")).unwrap();
    let img = render(&out, 0);
    assert!(dark_in(&img, 0.6, 1.0, 0.85, 1.0) > 50, "bottom-right corner");
    assert_eq!(dark_in(&img, 0.0, 1.0, 0.3, 0.85), 0, "nothing in the middle band");
}

#[test]
fn out_of_range_scale_and_font_size_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let png = t.path().join("logo.png");
    image::RgbImage::from_pixel(4, 4, image::Rgb([0, 0, 0])).save(&png).unwrap();
    let err = |mark: Mark| match watermark(&Input::new(&a), &WatermarkOptions { mark, ..Default::default() }, &t.path().join("w.pdf")) {
        Err(sheaf_core::OpdfError::Unsupported(m)) => m,
        other => panic!("{:?}", other.map(|_| ())),
    };
    for scale in [0.0, 0.01, 1.5, f64::NAN] {
        assert!(err(Mark::Image { path: png.clone(), scale }).contains("0.05 to 1"), "{scale}");
    }
    for font_size in [0.5, 501.0, -3.0] {
        assert!(err(Mark::Text { text: "X".into(), font_size, color: [0.0; 3] }).contains("1 to 500"), "{font_size}");
    }
    // 0 = automatic size.
    let o = WatermarkOptions { mark: Mark::Text { text: "AUTO".into(), font_size: 0.0, color: [0.0; 3] }, ..Default::default() };
    assert!(watermark(&Input::new(&a), &o, &t.path().join("w.pdf")).is_ok());
}
