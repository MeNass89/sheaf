//! Copy and paste: comments, stamps, pictures, areas and objects of a page.
use super::*;

#[test]
fn a_large_area_is_copied_whole() {
    assert_eq!(area_scale(100.0, 100.0), AREA_SCALE);
    // A 2000 pt square page, a black square in its bottom-right corner: 2000 × 3 would pass the
    // 4096 px tile and be cut off there (Sol, 26/10: the old test's A3 page stayed under it).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let mut doc = sheaf_core::engine::new_document();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::Custom(PdfPoints::new(2000.0), PdfPoints::new(2000.0))).unwrap();
        let corner = PdfRect::new_from_values(0.0, 1900.0, 100.0, 2000.0);
        page.objects_mut().create_path_object_rect(corner, None, None, Some(PdfColor::new(0, 0, 0, 255))).unwrap();
        doc.save_to_file(&path).unwrap();
    }
    let (ops, rx, _) = started(&path);
    ops.send(Op::CopyArea { page: 0, rect: [0.0, 0.0, 2000.0, 2000.0] }).unwrap();
    match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::ImageCopied(Ok((w, h, rgba))) => {
            assert!(w <= 4096 && h <= 4096 && w >= 4000 && w.abs_diff(h) <= 2, "{w}×{h}");
            // The corner's black is in the picture's bottom-right corner: nothing was cut off.
            let at = |x: u32, y: u32| rgba[((y * w + x) * 4) as usize];
            assert!(at(w - 10, h - 10) < 40 && at(10, 10) > 200, "corner {} / {}", at(w - 10, h - 10), at(10, 10));
        }
        Reply::ImageCopied(Err(e)) => panic!("{e}"),
        _ => panic!("expected ImageCopied"),
    }
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
fn a_stamp_of_a_picture_and_text_is_copied_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture(dir.path(), 1);
    // The stamp's artwork: a red picture on its left half, a big black "OK" on its right.
    let art = dir.path().join("art.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let mut doc = sheaf_core::engine::new_document();
        let font = doc.fonts_mut().helvetica();
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::Custom(PdfPoints::new(200.0), PdfPoints::new(100.0))).unwrap();
        let red = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(30, 20, image::Rgb([200, 30, 30])));
        page.objects_mut().create_image_object(PdfPoints::new(0.0), PdfPoints::new(0.0), &red, Some(PdfPoints::new(100.0)), Some(PdfPoints::new(100.0))).unwrap();
        page.objects_mut().create_text_object(PdfPoints::new(110.0), PdfPoints::new(30.0), "OK", font, PdfPoints::new(60.0)).unwrap();
        doc.save_to_file(&art).unwrap();
    }
    let file = sheaf_core::pdfcraft::engine::MarkFile { name: "art.pdf".into(), bytes: Arc::new(std::fs::read(&art).unwrap()), page: 0 };
    let (ops, rx, _) = started(&path);
    ops.send(Op::Apply { edit: Box::new(Edit::AddCustomStamp { page: 0, rect: [100.0, 100.0, 300.0, 200.0], name: "art".into(), file, author: "Ada".into() }), pages: vec![0] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    ops.send(Op::CopyComment { nm: snap.annots[0].nm.clone(), page: 0, rect: snap.annots[0].rect }).unwrap();
    let (name, bytes) = match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::Copied(_, Some(p)) => p,
        _ => panic!("expected the stamp's picture"),
    };
    // The whole stamp, 3× its size: the picture on the left and the text's ink on the right.
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!((name.as_str(), img.width(), img.height()), ("Stamp.png", 600, 300));
    assert_eq!(&img.get_pixel(150, 150).0[..3], &[200, 30, 30]);
    let ink = (300..600).flat_map(|x| (0..300).map(move |y| (x, y))).filter(|&(x, y)| img.get_pixel(x, y).0[3] > 200 && img.get_pixel(x, y).0[0] < 60).count();
    assert!(ink > 2000, "{ink} dark pixels where the text is");
}

#[test]
fn objects_copied_are_pasted_as_objects_without_what_lay_between_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("objects.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let mut doc = sheaf_core::engine::new_document();
        let font = doc.fonts_mut().helvetica();
        for k in 0..2 {
            let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
            if k == 0 {
                for (y, t) in [(700.0, "Picked first"), (600.0, "Lying between"), (500.0, "Picked last")] {
                    page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(y), t, font, PdfPoints::new(14.0)).unwrap();
                }
            }
        }
        doc.save_to_file(&path).unwrap();
    }
    // As poppler reads it (the pasted text is in a Form, as drawn).
    let shown = |bytes: &[u8], page: i32| {
        let f = dir.path().join(format!("read-{}.pdf", bytes.len()));
        std::fs::write(&f, bytes).unwrap();
        let n = (page + 1).to_string();
        let o = std::process::Command::new("pdftotext").args(["-f", &n, "-l", &n]).arg(&f).arg("-").output().unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let (ops, rx, _) = started(&path);
    ops.send(Op::Blocks(0)).unwrap();
    let objects = loop {
        if let Reply::Blocks(0, _, _, objects) = rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            break objects;
        }
    };
    // The first and the last line picked; the one between is inside their box but not picked.
    let picked: Vec<_> = objects.iter().filter(|o| o.1[1] > 690.0 || o.1[3] < 520.0).cloned().collect();
    assert_eq!(picked.len(), 2, "{objects:?}");
    let rect = picked.iter().map(|o| o.1).reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]).unwrap();
    ops.send(Op::CopyObjects { page: 0, objects: picked.iter().map(|o| o.0).collect(), rect, shown: rect }).unwrap();
    let (pdf, at) = match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
        Reply::ObjectsCopied(Ok((_, pdf, at))) => (pdf, at),
        Reply::ObjectsCopied(Err(e)) => panic!("{e}"),
        _ => panic!("expected ObjectsCopied"),
    };
    let copied = shown(&pdf, 0);
    assert!(copied.contains("Picked first") && copied.contains("Picked last") && !copied.contains("Lying between"), "copied {copied:?}; picked {picked:?}");
    // Pasted on page 2: still text there, one added item at their place.
    ops.send(Op::Apply { edit: Box::new(Edit::AddArtwork { page: 1, rect: at, pdf }), pages: vec![1] }).unwrap();
    let (snap, err) = next(&rx);
    assert!(err.is_none(), "{err:?}");
    let out = dir.path().join("pasted.pdf");
    ops.send(Op::Save { to: Some(out.clone()) }).unwrap();
    next(&rx);
    let page2 = shown(&std::fs::read(&out).unwrap(), 1);
    assert!(page2.contains("Picked first") && page2.contains("Picked last") && !page2.contains("Lying between"), "{page2}");
    let _ = snap;
    // One item the Edit tool picks, moves and deletes whole.
    ops.send(Op::Blocks(1)).unwrap();
    let on2 = loop {
        if let Reply::Blocks(1, _, _, objects) = rx.recv_timeout(Duration::from_secs(30)).unwrap() {
            break objects;
        }
    };
    assert_eq!(on2.iter().map(|o| o.0.kind).collect::<Vec<_>>(), [sheaf_core::pdfcraft::engine::ObjectKind::Added], "{on2:?}");
    qpdf_clean(&out);
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
