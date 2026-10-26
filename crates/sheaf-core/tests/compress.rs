mod common;
use lopdf::{dictionary, Document, Object, Stream};
use sheaf_core::{compress::*, images::{images_to_pdf, PageLayout}, pages::Input};
use std::path::{Path, PathBuf};

fn photo_pdf(dir: &Path) -> PathBuf {
    // a noisy 3000×2000 image: large, compresses well when downscaled
    let img = image::RgbImage::from_fn(3000, 2000, |x, y| image::Rgb([(x * 7 % 251) as u8, (y * 13 % 241) as u8, ((x ^ y) % 239) as u8]));
    let p = dir.join("photo.png");
    img.save(&p).unwrap();
    images_to_pdf(&[p], PageLayout::FitImage, &dir.join("photo.pdf")).unwrap()
}

/// One page drawing each given image XObject; returns the PDF path.
fn lopdf_with_images(path: &Path, images: Vec<Stream>) -> PathBuf {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut xobjects = lopdf::Dictionary::new();
    let mut ops = String::new();
    for (i, s) in images.into_iter().enumerate() {
        let id = doc.add_object(s);
        xobjects.set(format!("Im{i}"), id);
        ops.push_str(&format!("q 300 0 0 200 0 {} cm /Im{i} Do Q\n", i * 200));
    }
    let content = doc.add_object(Stream::new(dictionary! {}, ops.into_bytes()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 600.into(), 800.into()],
        "Contents" => content, "Resources" => dictionary! { "XObject" => xobjects },
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    doc.save(path).unwrap();
    path.to_path_buf()
}

fn raw_image(w: u32, h: u32, cs: &str, data: Vec<u8>) -> Stream {
    let mut s = Stream::new(dictionary! {
        "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
        "ColorSpace" => cs, "BitsPerComponent" => 8,
    }, data);
    s.compress().unwrap(); // Flate
    s
}

/// Photo-like bytes: a gradient plus xorshift grain, which Flate cannot shrink much.
fn noise(n: usize) -> Vec<u8> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    (0..n).map(|i| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; ((i / 4096) % 200) as u8 + (x % 48) as u8 }).collect()
}

/// The image streams of a saved PDF, in object-id order.
fn image_dicts(path: &Path) -> Vec<lopdf::Dictionary> {
    let d = Document::load(path).unwrap();
    d.objects.values().filter_map(|o| o.as_stream().ok())
        .filter(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"))
        .map(|s| s.dict.clone()).collect()
}

#[test]
fn levels_are_monotonic_and_smaller() {
    let t = tempfile::tempdir().unwrap();
    let src = photo_pdf(t.path());
    let low = compress(&Input::new(&src), Level::Low, &t.path().join("l.pdf")).unwrap();
    let rec = compress(&Input::new(&src), Level::Recommended, &t.path().join("r.pdf")).unwrap();
    let ext = compress(&Input::new(&src), Level::Extreme, &t.path().join("e.pdf")).unwrap();
    assert!(low.after < low.before, "{low:?}");
    assert!(ext.after <= rec.after && rec.after <= low.after, "{low:?} {rec:?} {ext:?}");
    let _g = sheaf_core::engine::pdfium_lock();
    assert_eq!(sheaf_core::engine::open_pdf(&ext.path, None).unwrap().pages().len(), 1);
}
#[test]
fn compress_never_grows() {
    let t = tempfile::tempdir().unwrap();
    let src = common::fixture_pdf(t.path(), "text.pdf", 2);
    let r = compress(&Input::new(&src), Level::Extreme, &t.path().join("c.pdf")).unwrap();
    assert!(r.after <= r.before);
    if r.already_optimal { assert_eq!(std::fs::read(&r.path).unwrap(), std::fs::read(&src).unwrap()); }
    assert_eq!(common::page_texts(&r.path), ["Page 1", "Page 2"]);
}
#[test]
fn compress_does_not_overwrite_existing_output() {
    let t = tempfile::tempdir().unwrap();
    let src = common::fixture_pdf(t.path(), "text.pdf", 1);
    let out = t.path().join("c.pdf");
    std::fs::write(&out, b"keep").unwrap();
    let r = compress(&Input::new(&src), Level::Low, &out).unwrap();
    assert!(r.path.ends_with("c (1).pdf"));
    assert_eq!(std::fs::read(&out).unwrap(), b"keep");
}
#[test]
fn grayscale_stays_grayscale_and_is_downscaled() {
    let t = tempfile::tempdir().unwrap();
    let src = lopdf_with_images(&t.path().join("g.pdf"), vec![raw_image(2000, 1500, "DeviceGray", noise(2000 * 1500))]);
    let r = compress(&Input::new(&src), Level::Extreme, &t.path().join("o.pdf")).unwrap();
    assert!(r.after < r.before / 2, "{r:?}");
    let d = &image_dicts(&r.path)[0];
    assert_eq!(d.get(b"Filter").unwrap().as_name().unwrap(), b"DCTDecode");
    assert_eq!(d.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceGray");
    assert_eq!((d.get(b"Width").unwrap().as_i64().unwrap(), d.get(b"Height").unwrap().as_i64().unwrap()), (1000, 750));
    assert!(!d.has(b"DecodeParms"));
}
#[test]
fn small_images_are_not_upscaled() {
    let t = tempfile::tempdir().unwrap();
    let src = lopdf_with_images(&t.path().join("s.pdf"), vec![raw_image(600, 400, "DeviceRGB", noise(600 * 400 * 3))]);
    let r = compress(&Input::new(&src), Level::Extreme, &t.path().join("o.pdf")).unwrap();
    let d = &image_dicts(&r.path)[0];
    assert_eq!(d.get(b"Filter").unwrap().as_name().unwrap(), b"DCTDecode");
    assert_eq!(d.get(b"Width").unwrap().as_i64().unwrap(), 600);
}
/// One-page PDF drawing the given image XObjects; `build` receives the document to add helper objects.
fn pdf_with(path: &Path, build: impl FnOnce(&mut Document) -> Vec<Stream>) -> PathBuf {
    let mut doc = Document::with_version("1.7");
    let imgs = build(&mut doc);
    let pages_id = doc.new_object_id();
    let mut xo = lopdf::Dictionary::new();
    let mut ops = String::new();
    for (i, s) in imgs.into_iter().enumerate() { xo.set(format!("Im{i}"), doc.add_object(s)); ops.push_str(&format!("q 300 0 0 200 0 0 cm /Im{i} Do Q\n")); }
    let content = doc.add_object(Stream::new(dictionary! {}, ops.into_bytes()));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 600.into(), 800.into()], "Contents" => content, "Resources" => dictionary! { "XObject" => xo } });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let cat = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", cat);
    doc.save(path).unwrap();
    path.to_path_buf()
}

/// Loaded output plus its single image whose /Width is `w` (the base image, not its mask).
fn image_by_width(path: &Path, w: i64) -> (Document, lopdf::Stream) {
    let d = Document::load(path).unwrap();
    let s = d.objects.values().filter_map(|o| o.as_stream().ok())
        .find(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") && s.dict.get(b"Width").unwrap().as_i64().unwrap() == w && !s.dict.has(b"XKind"))
        .unwrap().clone();
    (d, s)
}

#[test]
fn calrgb_keeps_its_colour_space_and_shrinks() {
    let t = tempfile::tempdir().unwrap();
    let cal = || Object::Array(vec![Object::Name(b"CalRGB".to_vec()), Object::Dictionary(dictionary! {
        "Gamma" => vec![Object::Real(2.2), Object::Real(2.2), Object::Real(2.2)],
        "Matrix" => vec![0.4124, 0.2126, 0.0193, 0.3576, 0.7152, 0.1192, 0.1805, 0.0722, 0.9505].into_iter().map(Object::Real).collect::<Vec<_>>(),
        "WhitePoint" => vec![Object::Real(0.9505), Object::Integer(1), Object::Real(1.089)],
    })]);
    let src = pdf_with(&t.path().join("c.pdf"), |doc| {
        let cs = doc.add_object(cal());
        let mut s = raw_image(1200, 800, "DeviceRGB", noise(1200 * 800 * 3));
        s.dict.set("ColorSpace", cs);
        vec![s]
    });
    let r = compress(&Input::new(&src), Level::Recommended, &t.path().join("o.pdf")).unwrap();
    assert!(r.after < r.before / 2, "{r:?}");
    let (d, s) = image_by_width(&r.path, 1200);
    assert_eq!(s.dict.get(b"Filter").unwrap().as_name().unwrap(), b"DCTDecode");
    let cs = d.dereference(s.dict.get(b"ColorSpace").unwrap()).unwrap().1;
    assert_eq!(cs, &cal());
}
#[test]
fn soft_masked_image_keeps_size_and_mask() {
    let t = tempfile::tempdir().unwrap();
    let mask_bytes = std::cell::RefCell::new(Vec::new());
    let src = pdf_with(&t.path().join("s.pdf"), |doc| {
        let mut m = raw_image(2000, 1500, "DeviceGray", noise(2000 * 1500));
        m.dict.set("XKind", "mask"); // test marker only
        *mask_bytes.borrow_mut() = m.content.clone();
        let mid = doc.add_object(m);
        let mut s = raw_image(2000, 1500, "DeviceRGB", noise(2000 * 1500 * 3));
        s.dict.set("SMask", mid);
        vec![s]
    });
    let r = compress(&Input::new(&src), Level::Extreme, &t.path().join("o.pdf")).unwrap();
    assert!(r.after < r.before, "{r:?}");
    let (d, s) = image_by_width(&r.path, 2000); // base keeps its 2000 px width despite Extreme's 1000 px cap
    assert_eq!(s.dict.get(b"Height").unwrap().as_i64().unwrap(), 1500);
    assert_eq!(s.dict.get(b"Filter").unwrap().as_name().unwrap(), b"DCTDecode");
    let mref = s.dict.get(b"SMask").unwrap().as_reference().unwrap();
    let m = d.get_object(mref).unwrap().as_stream().unwrap();
    assert_eq!(m.content, *mask_bytes.borrow());
    assert_eq!(m.dict.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode");
}
#[test]
fn image_with_mask_is_untouched() {
    let t = tempfile::tempdir().unwrap();
    let src = pdf_with(&t.path().join("m.pdf"), |_| {
        let mut s = raw_image(1500, 1000, "DeviceRGB", noise(1500 * 1000 * 3));
        s.dict.set("Mask", vec![0.into(), 10.into(), 0.into(), 10.into(), 0.into(), 10.into()]); // colour-key mask
        vec![s]
    });
    let orig = image_dicts(&src)[0].clone();
    let r = compress(&Input::new(&src), Level::Extreme, &t.path().join("o.pdf")).unwrap();
    let d = &image_dicts(&r.path)[0];
    assert_eq!(d.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode");
    assert_eq!(d.get(b"Width").unwrap().as_i64().unwrap(), 1500);
    assert_eq!(d.get(b"Length").ok(), orig.get(b"Length").ok());
}
#[test]
fn exotic_images_are_left_untouched() {
    let t = tempfile::tempdir().unwrap();
    let src = pdf_with(&t.path().join("x.pdf"), |doc| {
        let mut inverted = raw_image(1500, 1000, "DeviceGray", noise(1500 * 1000));
        inverted.dict.set("Decode", vec![1.into(), 0.into()]);
        let cmyk = raw_image(1500, 1000, "DeviceCMYK", noise(1500 * 1000 * 4));
        let pal = doc.add_object(Stream::new(dictionary! {}, noise(256 * 3)));
        let mut indexed = raw_image(1500, 1000, "DeviceRGB", noise(1500 * 1000));
        indexed.dict.set("ColorSpace", vec![Object::Name(b"Indexed".to_vec()), Object::Name(b"DeviceRGB".to_vec()), 255.into(), pal.into()]);
        let mut icc = raw_image(1500, 1000, "DeviceRGB", noise(1500 * 1000 * 3));
        let prof = doc.add_object(Stream::new(dictionary! { "N" => 3 }, vec![0; 64]));
        icc.dict.set("ColorSpace", vec![Object::Name(b"ICCBased".to_vec()), prof.into()]);
        vec![inverted, cmyk, indexed, icc]
    });
    let r = compress(&Input::new(&src), Level::Extreme, &t.path().join("o.pdf")).unwrap();
    let dicts = image_dicts(&r.path);
    assert_eq!(dicts.len(), 4);
    for d in dicts {
        assert_eq!(d.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode", "{d:?}");
        assert_eq!(d.get(b"Width").unwrap().as_i64().unwrap(), 1500);
    }
}
#[test]
fn compress_real_world_guide() {
    // a real-world PDF (the 3.3 MB, 7-page welcome guide of the app we replace); skip when SHEAF_GUIDE_PDF is unset
    let Some(g) = std::env::var_os("SHEAF_GUIDE_PDF") else { return };
    let g = Path::new(&g);
    let t = tempfile::tempdir().unwrap();
    let r = compress(&Input::new(g), Level::Recommended, &t.path().join("g.pdf")).unwrap();
    eprintln!("guide: {} -> {} bytes", r.before, r.after);
    assert!(std::process::Command::new("qpdf").arg("--check").arg(&r.path).status().unwrap().code() != Some(2));
}
#[test]
fn encrypted_input_gives_unencrypted_output() {
    let t = tempfile::tempdir().unwrap();
    let a = common::fixture_pdf(t.path(), "a.pdf", 2);
    let p = sheaf_core::security::protect(&Input::new(&a), "pw", None, &t.path().join("p.pdf")).unwrap();
    assert!(matches!(compress(&Input::new(&p), Level::Extreme, &t.path().join("x.pdf")), Err(sheaf_core::OpdfError::PasswordRequired(_))));
    let input = Input { path: p.clone(), password: Some("pw".into()) };
    let r = compress(&input, Level::Extreme, &t.path().join("c.pdf")).unwrap();
    assert!(r.was_encrypted, "{r:?}");
    assert_ne!(std::fs::read(&r.path).unwrap(), std::fs::read(&p).unwrap());
    let enc = std::process::Command::new("qpdf").arg("--show-encryption").arg(&r.path).output().unwrap();
    assert!(String::from_utf8_lossy(&enc.stdout).contains("File is not encrypted"), "{enc:?}");
    assert_eq!(common::page_texts(&r.path), ["Page 1", "Page 2"]);
    let plain = compress(&Input::new(&a), Level::Extreme, &t.path().join("d.pdf")).unwrap();
    assert!(!plain.was_encrypted);
}
#[test]
fn cmyk_jpeg_labelled_rgb_is_left_untouched() {
    // a 4-channel (CMYK) JPEG mislabelled /DeviceRGB: decoding would silently convert it, so we must skip it
    let t = tempfile::tempdir().unwrap();
    let jpeg = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cmyk.jpg")).unwrap();
    let src = pdf_with(&t.path().join("k.pdf"), |_| vec![Stream::new(dictionary! {
        "Type" => "XObject", "Subtype" => "Image", "Width" => 64, "Height" => 64,
        "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode",
    }, jpeg.clone())]);
    let r = compress(&Input::new(&src), Level::Extreme, &t.path().join("o.pdf")).unwrap();
    let d = Document::load(&r.path).unwrap();
    let s = d.objects.values().filter_map(|o| o.as_stream().ok()).find(|s| s.dict.has(b"Width")).unwrap();
    assert_eq!(s.content, jpeg);
}
#[test]
fn array_decode_parms_with_predictor_are_left_untouched() {
    // lopdf only reads /DecodeParms as a dictionary: the TIFF predictor in the array form would
    // be ignored and the still-differenced pixels re-encoded as JPEG. Such images must be skipped.
    let t = tempfile::tempdir().unwrap();
    let (w, h) = (1200u32, 800u32);
    let px = noise((w * h * 3) as usize);
    // TIFF predictor 2: each sample minus the same component of the pixel to its left.
    let mut diff = px.clone();
    for row in 0..h as usize {
        for i in (3..w as usize * 3).rev() { let o = row * w as usize * 3; diff[o + i] = px[o + i].wrapping_sub(px[o + i - 3]); }
    }
    let mut enc = flate2_like(&diff);
    let src = pdf_with(&t.path().join("p.pdf"), |_| {
        let s = Stream::new(dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
            "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
            "Filter" => vec![Object::Name(b"FlateDecode".to_vec())],
            "DecodeParms" => vec![Object::Dictionary(dictionary! { "Predictor" => 2, "Colors" => 3, "Columns" => w as i64, "BitsPerComponent" => 8 })],
        }, std::mem::take(&mut enc));
        vec![s]
    });
    let before = image_by_width(&src, w as i64).1;
    let r = compress(&Input::new(&src), Level::Extreme, &t.path().join("o.pdf")).unwrap();
    let after = image_by_width(&r.path, w as i64).1;
    assert_eq!(after.content, before.content);
    assert_eq!(after.dict.get(b"DecodeParms").unwrap(), before.dict.get(b"DecodeParms").unwrap());
}

/// zlib-compresses `data` the way lopdf's `Stream::compress` does.
fn flate2_like(data: &[u8]) -> Vec<u8> {
    let mut s = Stream::new(dictionary! {}, data.to_vec());
    s.compress().unwrap();
    s.content
}
#[test]
fn absurd_image_dimensions_do_not_panic() {
    let t = tempfile::tempdir().unwrap();
    let src = pdf_with(&t.path().join("h.pdf"), |_| {
        let mut s = raw_image(8, 8, "DeviceRGB", noise(8 * 8 * 3));
        s.dict.set("Width", u32::MAX as i64);
        s.dict.set("Height", u32::MAX as i64);
        vec![s]
    });
    compress(&Input::new(&src), Level::Extreme, &t.path().join("o.pdf")).unwrap();
}
#[test]
fn encrypted_object_stream_input_picks_the_smaller_candidate() {
    // qpdf packs objects in object streams and encrypts; lopdf's own recompressed save can come out
    // larger. Compress must keep the smaller of that and the plain decrypted save, and flag growth.
    let t = tempfile::tempdir().unwrap();
    let pngs: Vec<PathBuf> = (0..30).map(|i| {
        let p = t.path().join(format!("i{i}.png"));
        image::RgbImage::from_pixel(8, 8, image::Rgb([i as u8 * 8, 40, 90])).save(&p).unwrap();
        p
    }).collect();
    let a = images_to_pdf(&pngs, PageLayout::FitImage, &t.path().join("a.pdf")).unwrap();
    let e = t.path().join("e.pdf");
    assert!(std::process::Command::new("qpdf").args(["--object-streams=generate", "--encrypt", "u", "u", "256", "--"]).arg(&a).arg(&e).status().unwrap().success());
    let mut plain = Document::load_with_password(&e, "u").unwrap();
    let mut plain_bytes = Vec::new();
    plain.save_to(&mut plain_bytes).unwrap();
    let r = compress(&Input { path: e.clone(), password: Some("u".into()) }, Level::Recommended, &t.path().join("c.pdf")).unwrap();
    assert!(r.was_encrypted);
    // Here the recompressed save (object + xref streams) beats the plain save, so it must be the one kept.
    assert!(r.after < plain_bytes.len() as u64, "{r:?} plain {}", plain_bytes.len());
    assert_eq!(r.grew, r.after > r.before, "{r:?}");
    assert_eq!(std::fs::metadata(&r.path).unwrap().len(), r.after);
    assert_eq!(Document::load(&r.path).unwrap().get_pages().len(), 30);
}

#[test]
fn the_same_picture_embedded_twice_is_kept_once() {
    let t = tempfile::tempdir().unwrap();
    // A small CMYK picture (left as it is by the recompression) written twice as its own object.
    let img = || raw_image(40, 30, "DeviceCMYK", noise(40 * 30 * 4));
    let src = lopdf_with_images(&t.path().join("twice.pdf"), vec![img(), img()]);
    assert_eq!(image_dicts(&src).len(), 2);
    let r = compress(&Input::new(&src), Level::Low, &t.path().join("once.pdf")).unwrap();
    assert_eq!(image_dicts(&r.path).len(), 1, "one copy left");
    assert!(r.after < r.before);
    // Both places still draw it.
    let d = Document::load(&r.path).unwrap();
    let page = d.get_pages()[&1];
    let res = d.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
    let x = res.get(b"XObject").unwrap().as_dict().unwrap();
    assert_eq!(x.get(b"Im0").unwrap().as_reference().unwrap(), x.get(b"Im1").unwrap().as_reference().unwrap());
}

#[test]
fn compress_says_how_far_it_is_and_stops_when_cancelled() {
    let t = tempfile::tempdir().unwrap();
    let a = photo_pdf(t.path());
    let mut seen = Vec::new();
    compress_with(&Input::new(&a), Level::Recommended, &t.path().join("c.pdf"), &mut |p| {
        seen.push(p);
        true
    })
    .unwrap();
    assert!(seen.len() >= 2 && seen.windows(2).all(|w| w[0] <= w[1]) && seen.last() == Some(&1.0), "{seen:?}");
    let out = t.path().join("cancelled.pdf");
    let r = compress_with(&Input::new(&a), Level::Recommended, &out, &mut |_| false);
    assert!(matches!(r, Err(sheaf_core::OpdfError::Cancelled)) && !out.exists());
    // A file without pictures: progress before the end, and a Cancel heard before it is written.
    let plain = common::fixture_pdf(t.path(), "plain.pdf", 3);
    let mut seen = Vec::new();
    compress_with(&Input::new(&plain), Level::Recommended, &t.path().join("p.pdf"), &mut |p| {
        seen.push(p);
        true
    })
    .unwrap();
    assert!(seen.iter().any(|p| *p > 0.0 && *p < 0.95), "{seen:?}");
    let out = t.path().join("plain-cancelled.pdf");
    assert!(matches!(compress_with(&Input::new(&plain), Level::Recommended, &out, &mut |_| false), Err(sheaf_core::OpdfError::Cancelled)) && !out.exists());
}

#[test]
fn a_target_size_tries_stronger_settings_until_the_file_fits() {
    let t = tempfile::tempdir().unwrap();
    let a = photo_pdf(t.path());
    let low = compress(&Input::new(&a), Level::Low, &t.path().join("low.pdf")).unwrap();
    let extreme = compress(&Input::new(&a), Level::Extreme, &t.path().join("ext.pdf")).unwrap();
    // A target the gentlest settings meet: they are the ones used.
    let roomy = compress(&Input::new(&a), Level::Target(low.after), &t.path().join("roomy.pdf")).unwrap();
    assert!(roomy.after == low.after && !roomy.missed_target, "{roomy:?} {low:?}");
    // One between Low and Extreme: a stronger step, under it.
    let mid = compress(&Input::new(&a), Level::Target((low.after + extreme.after) / 2), &t.path().join("mid.pdf")).unwrap();
    assert!(mid.after <= (low.after + extreme.after) / 2 && !mid.missed_target, "{mid:?}");
    // Out of reach: the smallest made, said to miss.
    let tiny = compress(&Input::new(&a), Level::Target(10), &t.path().join("tiny.pdf")).unwrap();
    assert!(tiny.missed_target && tiny.after <= extreme.after, "{tiny:?} {extreme:?}");
}

#[test]
fn advanced_settings_take_out_what_was_asked() {
    let t = tempfile::tempdir().unwrap();
    let a = common::fixture_pdf(t.path(), "a.pdf", 2);
    // The document's title and a structure tree, to be taken out.
    let mut d = lopdf::Document::load(&a).unwrap();
    let info = d.add_object(lopdf::dictionary! { "Title" => lopdf::Object::string_literal("Secret title") });
    d.trailer.set("Info", info);
    let st = d.add_object(lopdf::dictionary! { "Type" => "StructTreeRoot" });
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_object_mut(root).unwrap().as_dict_mut().unwrap().set("StructTreeRoot", st);
    d.save(&a).unwrap();
    let adv = Advanced { discard_tags: true, discard_metadata: true, ..Default::default() };
    let out = t.path().join("c.pdf");
    compress_advanced(&Input::new(&a), Level::Recommended, &adv, &out, &mut |_| true).unwrap();
    let q = std::process::Command::new("qpdf").args(["--qdf", "--object-streams=disable"]).arg(&out).arg("-").output().unwrap();
    let s = String::from_utf8_lossy(&q.stdout);
    assert!(!s.contains("Secret title") && !s.contains("/StructTreeRoot"), "taken out");
    assert_eq!(common::page_texts(&out).len(), 2);
}

/// Compress ▸ Convert to grayscale (Stirling's): a red square drawn in the page, a photo and text
/// all come out grey, the text still text.
#[test]
fn grayscale_makes_drawings_and_pictures_grey_and_keeps_the_text() {
    let t = tempfile::tempdir().unwrap();
    let photo = photo_pdf(t.path());
    let mut d = Document::load(&photo).unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let font = d.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let drawn = d.add_object(Stream::new(dictionary! {}, b"q 1 0 0 rg 0 0 400 400 re f Q BT /FG 48 Tf 0 0 1 rg 20 500 Td (Grey words) Tj ET".to_vec()));
    let mut contents = d.get_page_contents(page).into_iter().map(Object::Reference).collect::<Vec<_>>();
    contents.push(Object::Reference(drawn));
    let pd = d.get_object_mut(page).unwrap().as_dict_mut().unwrap();
    pd.set("Contents", contents);
    let res = pd.get(b"Resources").unwrap().clone();
    let res_id = res.as_reference().ok();
    let fonts = dictionary! { "FG" => font };
    match res_id {
        Some(id) => d.get_object_mut(id).unwrap().as_dict_mut().unwrap().set("Font", fonts),
        None => d.get_object_mut(page).unwrap().as_dict_mut().unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("Font", fonts),
    }
    let colour = t.path().join("colour.pdf");
    d.save(&colour).unwrap();
    let out = t.path().join("grey.pdf");
    let adv = Advanced { grayscale: true, ..Default::default() };
    compress_advanced(&Input::new(&colour), Level::Recommended, &adv, &out, &mut |_| true).unwrap();
    let (w, h, px) = common::render(&out, 0);
    let coloured = px.as_chunks::<4>().0.iter().filter(|p| (p[0] as i32 - p[1] as i32).abs() > 3 || (p[1] as i32 - p[2] as i32).abs() > 3).count();
    assert_eq!(coloured, 0, "every pixel grey ({w}×{h})");
    let (_, _, before) = common::render(&colour, 0);
    assert!(before.as_chunks::<4>().0.iter().filter(|p| (p[0] as i32 - p[1] as i32).abs() > 3).count() > 1000, "the original has colour");
    assert!(common::page_texts(&out)[0].contains("Grey words"), "the text is still text");
}
