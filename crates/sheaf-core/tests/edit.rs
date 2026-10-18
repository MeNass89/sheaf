mod common;
use common::{foreign_encrypted, qpdf_clean, render, unprotected_and_clean};
use sheaf_core::edit::{self, EditSpec, Item};
use sheaf_core::pages::Input;
use sheaf_core::OpdfError;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One page, MediaBox [0 0 600 800] with CropBox [100 200 500 700], turned `rotation`, empty.
fn cropped_turned(dir: &Path, rotation: i64) -> PathBuf {
    use lopdf::{Dictionary, Object};
    let dict = |pairs: Vec<(&str, Object)>| {
        let mut d = Dictionary::new();
        for (k, v) in pairs {
            d.set(k, v);
        }
        d
    };
    let nums = |v: [i64; 4]| Object::Array(v.iter().map(|n| Object::Integer(*n)).collect());
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let content = doc.add_object(lopdf::Stream::new(Dictionary::new(), Vec::new()));
    let page = doc.add_object(dict(vec![
        ("Type", Object::Name(b"Page".to_vec())),
        ("Parent", Object::Reference(pages)),
        ("Contents", Object::Reference(content)),
        ("Resources", Object::Dictionary(Dictionary::new())),
        ("MediaBox", nums([0, 0, 600, 800])),
        ("CropBox", nums([100, 200, 500, 700])),
        ("Rotate", Object::Integer(rotation)),
    ]));
    doc.objects.insert(
        pages,
        Object::Dictionary(dict(vec![("Type", Object::Name(b"Pages".to_vec())), ("Kids", Object::Array(vec![Object::Reference(page)])), ("Count", Object::Integer(1))])),
    );
    let catalog = doc.add_object(dict(vec![("Type", Object::Name(b"Catalog".to_vec())), ("Pages", Object::Reference(pages))]));
    doc.trailer.set("Root", catalog);
    let path = dir.join(format!("crop-{rotation}.pdf"));
    doc.save(&path).unwrap();
    path
}

/// A 40 x 20 px solid image (PNG or JPEG by extension).
fn picture(dir: &Path, name: &str, rgb: [u8; 3]) -> PathBuf {
    let p = dir.join(name);
    image::RgbImage::from_pixel(40, 20, image::Rgb(rgb)).save(&p).unwrap();
    p
}

/// A 40 x 20 px PNG: left half red, right half blue, so its orientation shows.
fn two_colour(dir: &Path) -> PathBuf {
    let p = dir.join("halves.png");
    image::RgbImage::from_fn(40, 20, |x, _| if x < 20 { image::Rgb([220, 20, 20]) } else { image::Rgb([20, 40, 220]) }).save(&p).unwrap();
    p
}

/// Words and their boxes as `pdftotext -cropbox -bbox` reads them (y down, in the frame of the
/// page as displayed), with the page size it reports, which is the CropBox's, unturned:
/// (width, height, [(word, [x0, y0, x1, y1])]).
fn words(path: &Path) -> (f64, f64, Vec<(String, [f64; 4])>) {
    let o = Command::new("pdftotext").args(["-cropbox", "-bbox", "-enc", "UTF-8"]).arg(path).arg("-").output().expect("pdftotext (poppler) is needed");
    let s = String::from_utf8_lossy(&o.stdout).into_owned();
    let attr = |line: &str, k: &str| -> f64 {
        let i = line.find(&format!("{k}=\"")).unwrap() + k.len() + 2;
        line[i..].split('"').next().unwrap().parse().unwrap()
    };
    let page = s.lines().find(|l| l.contains("<page ")).unwrap();
    let (w, h) = (attr(page, "width"), attr(page, "height"));
    let words = s
        .lines()
        .filter(|l| l.contains("<word "))
        .map(|l| {
            let text = l.split('>').nth(1).unwrap().split('<').next().unwrap().to_string();
            (text, [attr(l, "xMin"), attr(l, "yMin"), attr(l, "xMax"), attr(l, "yMax")])
        })
        .collect();
    (w, h, words)
}

/// Pixels of `rgb` (within 40 per channel) and their box in a rendered page.
fn coloured(img: &(u32, u32, Vec<u8>), rgb: [u8; 3]) -> (usize, Option<[u32; 4]>) {
    let (w, h, px) = img;
    let (mut n, mut b): (usize, Option<[u32; 4]>) = (0, None);
    for y in 0..*h {
        for x in 0..*w {
            let p = &px[((y * w + x) * 4) as usize..];
            if p[3] > 200 && (0..3).all(|i| p[i].abs_diff(rgb[i]) < 40) {
                n += 1;
                b = Some(match b {
                    None => [x, y, x, y],
                    Some(b) => [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)],
                });
            }
        }
    }
    (n, b)
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn text_and_image_land_where_placed_on_every_rotation_with_a_crop_offset() {
    let t = tempfile::tempdir().unwrap();
    let png = two_colour(t.path());
    for rotation in [0, 90, 180, 270] {
        let src = Input::new(cropped_turned(t.path(), rotation));
        let info = edit::page_info(&src, 0).unwrap();
        let (w, h) = (info.width as f64, info.height as f64);
        assert_eq!((w, h), if rotation % 180 == 0 { (400.0, 500.0) } else { (500.0, 400.0) }, "rot {rotation}");
        // Text with its top-left at (60, 80) as displayed, 20 pt, green; an image 120 x 60 at
        // (150, 250), red on its left half and blue on its right half as displayed.
        let at = edit::view_to_user(&info, [60.0, 80.0]);
        let rect = edit::image_rect(&info, [150.0, 250.0], 120.0, 0.5);
        let spec = EditSpec {
            items: vec![
                Item::Text { page: 0, at, text: "Hello Sheaf".into(), font_size: 20.0, color: [20, 160, 40] },
                Item::Image { page: 0, rect, path: png.clone() },
            ],
        };
        let r = edit::edit(&src, &spec, &t.path().join(format!("out-{rotation}.pdf"))).unwrap();
        assert_eq!((r.added, r.was_encrypted), (2, false));
        qpdf_clean(&r.path);

        // Text: poppler finds the words, upright, where placed, at the size asked.
        let (pw, ph, ws) = words(&r.path);
        assert_eq!((pw, ph), (400.0, 500.0), "rot {rotation}: pdftotext page");
        let hello = ws.iter().find(|(s, _)| s == "Hello").unwrap_or_else(|| panic!("rot {rotation}: no text in {ws:?}"));
        assert!(ws.iter().any(|(s, _)| s == "Sheaf"), "rot {rotation}: {ws:?}");
        let [x0, y0, x1, y1] = hello.1;
        assert!((x0 - 60.0).abs() < 2.0, "rot {rotation}: text starts at x {x0}");
        // PdfCraft sets the first baseline 0.95 em below the box top: capitals start ~0.23 em down.
        assert!((80.0..90.0).contains(&y0), "rot {rotation}: text top at y {y0}");
        assert!(x1 - x0 > 40.0 && (y1 - y0 - 20.0).abs() < 4.0, "rot {rotation}: word box {:?} (not upright or wrong size)", hello.1);

        // Rendered: the green text in its box, and the image upright: red fills the left half of
        // its box (150..210), blue the right half (210..270), both the full height (250..310).
        let img = render(&r.path, 0);
        assert!((img.0 as f64 - w).abs() <= 1.0, "rot {rotation}: render {}x{}", img.0, img.1);
        let (n, b) = coloured(&img, [20, 160, 40]);
        let b = b.unwrap_or_else(|| panic!("rot {rotation}: no green text"));
        assert!(n > 100 && b[0] >= 59 && b[0] <= 64 && b[1] >= 80 && b[3] <= 105, "rot {rotation}: text ink {b:?}");
        for (rgb, want, side) in [([220, 20, 20], [150, 250, 209, 309], "red (left)"), ([20, 40, 220], [210, 250, 269, 309], "blue (right)")] {
            let (n, b) = coloured(&img, rgb);
            let b = b.unwrap_or_else(|| panic!("rot {rotation}: no {side} half"));
            assert!(b.iter().zip(want).all(|(a, e)| a.abs_diff(e) <= 1), "rot {rotation}: {side} half at {b:?}, want {want:?}");
            assert!(n > 58 * 58, "rot {rotation}: {side} half not filled ({n} px)");
        }
    }
}

#[test]
fn colour_and_size_are_written_into_the_page_content() {
    let t = tempfile::tempdir().unwrap();
    let src = Input::new(cropped_turned(t.path(), 0));
    let info = edit::page_info(&src, 0).unwrap();
    let spec = EditSpec {
        items: vec![Item::Text { page: 0, at: edit::view_to_user(&info, [10.0, 10.0]), text: "Sized".into(), font_size: 33.0, color: [255, 0, 0] }],
    };
    let out = edit::edit(&src, &spec, &t.path().join("o.pdf")).unwrap().path;
    let lo = lopdf::Document::load(&out).unwrap();
    let page = lo.get_pages()[&1];
    let content = String::from_utf8_lossy(&lo.get_page_content(page)).into_owned();
    assert!(content.contains(" 33 Tf 1 0 0 rg"), "{content}");
    // Page content, not an annotation.
    assert!(lo.get_dictionary(page).unwrap().get(b"Annots").is_err());
}

#[test]
fn a_jpeg_on_a_real_document_lands_in_its_box() {
    let t = tempfile::tempdir().unwrap();
    let src = Input::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf"));
    let jpg = picture(t.path(), "g.jpg", [20, 180, 40]);
    let info = edit::page_info(&src, 0).unwrap();
    let rect = edit::image_rect(&info, [300.0, 400.0], 100.0, 0.5);
    let spec = EditSpec { items: vec![Item::Image { page: 0, rect, path: jpg }] };
    let r = edit::edit(&src, &spec, &t.path().join("o.pdf")).unwrap();
    qpdf_clean(&r.path);
    let (_, b) = coloured(&render(&r.path, 0), [20, 180, 40]);
    let b = b.expect("no image");
    assert!(b.iter().zip([300, 400, 399, 449]).all(|(a, e)| a.abs_diff(e) <= 2), "image at {b:?}");
    // The document's own text is still there.
    let (_, _, ws) = words(&r.path);
    assert!(ws.len() > 5);
}

#[test]
fn an_invalid_image_is_refused_and_nothing_is_written() {
    let t = tempfile::tempdir().unwrap();
    let src = Input::new(cropped_turned(t.path(), 0));
    let fake = t.path().join("fake.png");
    std::fs::write(&fake, b"not an image at all").unwrap();
    let mut broken = std::fs::read(picture(t.path(), "ok.png", [1, 2, 3])).unwrap();
    broken.truncate(60);
    let broken_path = t.path().join("broken.png");
    std::fs::write(&broken_path, broken).unwrap();
    let gif = t.path().join("x.gif");
    image::RgbImage::from_pixel(4, 4, image::Rgb([0, 0, 0])).save(&gif).unwrap();
    let out_dir = t.path().join("out");
    std::fs::create_dir(&out_dir).unwrap();
    for (path, why) in [(&fake, "not a PNG or JPEG"), (&broken_path, "damaged"), (&gif, "not a PNG or JPEG")] {
        let spec = EditSpec {
            items: vec![
                Item::Text { page: 0, at: [150.0, 650.0], text: "Kept?".into(), font_size: 12.0, color: [0, 0, 0] },
                Item::Image { page: 0, rect: [150.0, 300.0, 250.0, 350.0], path: path.clone() },
            ],
        };
        match edit::edit(&src, &spec, &out_dir.join("o.pdf")) {
            Err(OpdfError::Unsupported(m)) => assert!(m.contains(why), "{m}"),
            r => panic!("{} accepted: {r:?}", path.display()),
        }
    }
    assert!(files_in(&out_dir).is_empty(), "{:?}", files_in(&out_dir));
}

#[test]
fn bad_items_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let src = Input::new(cropped_turned(t.path(), 0));
    let text = |text: &str, size: f64, page: usize| EditSpec { items: vec![Item::Text { page, at: [150.0, 650.0], text: text.into(), font_size: size, color: [0; 3] }] };
    for (spec, why) in [
        (EditSpec::default(), "nothing to add"),
        (text("  ", 12.0, 0), "empty"),
        (text("Hi", 0.5, 0), "font size"),
        (text("Hi", 12.0, 3), "page 4 does not exist"),
        (text("Snowman \u{2603}", 12.0, 0), "can't be drawn in Helvetica"),
        // Wholly outside the displayed page (CropBox [100 200 500 700]): above and to the left.
        (EditSpec { items: vec![Item::Text { page: 0, at: [10.0, 900.0], text: "Lost".into(), font_size: 12.0, color: [0; 3] }] }, "outside the page"),
        (EditSpec { items: vec![Item::Image { page: 0, rect: [0.0, 0.0, 90.0, 150.0], path: PathBuf::from("/none.png") }] }, "outside the page"),
    ] {
        match edit::edit(&src, &spec, &t.path().join("o.pdf")) {
            Err(OpdfError::Unsupported(m)) => assert!(m.contains(why), "expected {why:?}, got {m:?}"),
            r => panic!("accepted: {r:?}"),
        }
    }
    assert_eq!(files_in(t.path()), ["crop-0.pdf"]);
}

#[test]
fn an_encrypted_input_gives_an_unprotected_output_and_says_so() {
    let t = tempfile::tempdir().unwrap();
    let plain = cropped_turned(t.path(), 90);
    for (enc, pw) in foreign_encrypted(&plain, t.path()) {
        let src = Input { path: enc.clone(), password: Some(pw.into()) };
        let info = edit::page_info(&src, 0).unwrap();
        let spec = EditSpec {
            items: vec![Item::Text { page: 0, at: edit::view_to_user(&info, [60.0, 80.0]), text: "Unlocked".into(), font_size: 14.0, color: [0, 0, 0] }],
        };
        let r = edit::edit(&src, &spec, &t.path().join("enc-out.pdf")).unwrap();
        assert!(r.was_encrypted, "{}", enc.display());
        unprotected_and_clean(&r.path);
        let (_, _, ws) = words(&r.path);
        assert!(ws.iter().any(|(s, b)| s == "Unlocked" && (b[0] - 60.0).abs() < 2.0), "{}: {ws:?}", enc.display());
        std::fs::remove_file(&r.path).unwrap();
    }
}

#[test]
fn view_and_user_space_round_trip() {
    let t = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let info = edit::page_info(&Input::new(cropped_turned(t.path(), rotation)), 0).unwrap();
        let rect = edit::image_rect(&info, [30.0, 40.0], 100.0, 0.25);
        let item = Item::Image { page: 0, rect, path: PathBuf::new() };
        let v = edit::item_view_rect(&info, &item);
        assert!(v.iter().zip([30.0, 40.0, 130.0, 65.0]).all(|(a, e)| (a - e).abs() < 1e-3), "rot {rotation}: {v:?}");
        let p = edit::user_to_view(&info, edit::view_to_user(&info, [12.5, 99.0]));
        assert!((p[0] - 12.5).abs() < 1e-3 && (p[1] - 99.0).abs() < 1e-3);
    }
}
