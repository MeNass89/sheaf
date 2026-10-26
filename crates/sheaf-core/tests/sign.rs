mod common;
use common::{qpdf_clean, render};
use pdfium_render::prelude::*;
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::Doc;
use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
use sheaf_core::OpdfError;
use std::path::{Path, PathBuf};
use std::process::Command;

fn invoice() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf")
}

/// A throwaway self-signed digital ID made with `openssl` in `dir` (password "secret").
fn test_id(dir: &Path) -> PathBuf {
    let (key, cert, p12) = (dir.join("k.pem"), dir.join("c.pem"), dir.join("id.p12"));
    let o = Command::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-subj", "/CN=Sheaf Test Signer", "-keyout"])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .output()
        .unwrap();
    assert!(o.status.success(), "openssl req: {}", String::from_utf8_lossy(&o.stderr));
    let o = Command::new("openssl").args(["pkcs12", "-export", "-passout", "pass:secret", "-inkey"]).arg(&key).arg("-in").arg(&cert).arg("-out").arg(&p12).output().unwrap();
    assert!(o.status.success(), "openssl pkcs12: {}", String::from_utf8_lossy(&o.stderr));
    p12
}

fn digital(p12: &Path, password: &str) -> Option<DigitalSpec> {
    Some(DigitalSpec { p12: p12.into(), password: password.into(), reason: Some("Approved".into()), location: None, certify: None })
}

fn spec(page: usize, rect: [f64; 4], graphic: Graphic, digital: Option<DigitalSpec>) -> SignSpec {
    SignSpec { page, rect, graphic, digital }
}

/// The signatures of `path` as PdfCraft reads them: (signed, intact). A self-signed test ID is
/// not in any trust store, so an intact signature reads `Unknown` (signer not trusted), never
/// `Valid` or `Invalid`; anything else is not intact.
fn signatures(path: &Path) -> Vec<(bool, bool)> {
    let doc = Doc::open(&Input::new(path)).unwrap();
    assert_eq!(doc.document().is_signed(), doc.document().signatures.iter().any(|s| s.signed));
    doc.document().signatures.iter().map(|s| (s.signed, s.status == sheaf_core::pdfcraft::engine::SignatureStatus::Unknown)).collect()
}

/// `pdfsig` output, or `None` when poppler isn't installed.
fn pdfsig(path: &Path) -> Option<String> {
    let o = Command::new("pdfsig").arg("-nocert").arg(path).output().ok()?;
    Some(format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

/// `qpdf --check` passes; an incremental update may draw a warning about its xref, nothing else.
fn qpdf_ok(path: &Path) {
    let o = Command::new("qpdf").arg("--check").arg(path).output().unwrap();
    let s = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(o.status.success() && s.contains("No syntax or stream encoding errors"), "qpdf --check: {s}");
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn digital_signature_is_signed_valid_and_written_verbatim() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    let out = t.path().join("signed.pdf");
    let r = sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada Lovelace".into(), Default::default()), digital(&p12, "secret")), &out).unwrap();
    assert_eq!(r.path, out);
    assert_eq!(r.signer.as_deref(), Some("Sheaf Test Signer"));
    // Incremental: the original file is a prefix of the signed one.
    let (orig, signed) = (std::fs::read(invoice()).unwrap(), std::fs::read(&out).unwrap());
    assert!(signed.starts_with(&orig), "the signature is not an incremental update");
    assert_eq!(signatures(&out), [(true, true)]);
    let doc = Doc::open(&Input::new(&out)).unwrap();
    assert_eq!(doc.document().signatures[0].reason.as_deref(), Some("Approved"));
    qpdf_ok(&out);
    match pdfsig(&out) {
        Some(s) => {
            assert!(s.contains("Signature is Valid"), "pdfsig: {s}");
            assert!(s.contains("Sheaf Test Signer"), "pdfsig: {s}");
        }
        None => eprintln!("pdfsig not installed: poppler check skipped"),
    }
}

#[test]
fn signing_twice_keeps_both_signatures_valid() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    let once = sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("First".into(), Default::default()), digital(&p12, "secret")), &t.path().join("once.pdf")).unwrap().path;
    let strokes = vec![vec![[0.0, 0.0], [0.3, 0.2], [0.6, 0.0], [1.0, 0.25]]];
    let twice = sign::sign(&Input::new(&once), &spec(0, [320.0, 72.0, 520.0, 132.0], Graphic::Drawn(strokes), digital(&p12, "secret")), &t.path().join("twice.pdf")).unwrap().path;
    assert!(std::fs::read(&twice).unwrap().starts_with(&std::fs::read(&once).unwrap()));
    assert_eq!(signatures(&twice), [(true, true), (true, true)]);
    qpdf_ok(&twice);
    if let Some(s) = pdfsig(&twice) {
        assert_eq!(s.matches("Signature is Valid").count(), 2, "pdfsig: {s}");
    }
    // A visual-only signature on a signed file is an incremental update too.
    let third = sign::sign(&Input::new(&twice), &spec(0, [72.0, 200.0, 222.0, 240.0], Graphic::Typed("Witness".into(), Default::default()), None), &t.path().join("third.pdf")).unwrap().path;
    assert!(std::fs::read(&third).unwrap().starts_with(&std::fs::read(&twice).unwrap()));
    assert_eq!(signatures(&third), [(true, true), (true, true)]);
    if let Some(s) = pdfsig(&third) {
        assert_eq!(s.matches("Signature is Valid").count(), 2, "pdfsig: {s}");
    }
}

/// An A4 portrait page shown turned a quarter (`/Rotate 90`), with no content.
fn rotated(dir: &Path) -> PathBuf {
    let _g = pdfium_lock();
    let mut doc = new_document();
    let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
    page.set_rotation(PdfPageRenderRotation::Degrees90);
    let path = dir.join("turned.pdf");
    doc.save_to_file(&path).unwrap();
    path
}

/// The box of dark pixels (alpha-composited ink) in a rendered page: [x0, y0, x1, y1] in pixels.
fn ink(img: &(u32, u32, Vec<u8>)) -> Option<[u32; 4]> {
    let (w, h, px) = img;
    let mut b: Option<[u32; 4]> = None;
    for y in 0..*h {
        for x in 0..*w {
            let p = &px[((y * w + x) * 4) as usize..];
            if p[3] > 128 && p[0] < 128 {
                b = Some(match b {
                    None => [x, y, x, y],
                    Some(b) => [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)],
                });
            }
        }
    }
    b
}

#[test]
fn visual_signature_on_a_turned_page_lands_upright_where_placed() {
    let t = tempfile::tempdir().unwrap();
    let src = Input::new(rotated(t.path()));
    let info = sign::page_info(&src, 0).unwrap();
    assert_eq!((info.rotation, info.width.round(), info.height.round()), (90, 842.0, 595.0));
    // Clicked at (200, 150) on the landscape page as displayed, a 200 pt wide signature.
    let graphic = Graphic::Typed("Ada Lovelace".into(), Default::default());
    let aspect = sign::aspect(&graphic).unwrap();
    assert!(aspect < 0.6, "a name is wider than tall: {aspect}");
    let rect = sign::box_at(&info, [200.0, 150.0], 200.0, aspect);
    // In user space the box is turned: taller than wide.
    assert!(rect[3] - rect[1] > rect[2] - rect[0], "{rect:?}");
    let out = sign::sign(&src, &spec(0, rect, graphic, None), &t.path().join("out.pdf")).unwrap().path;
    qpdf_clean(&out);
    // Flattened: page content, not an annotation left to edit.
    let lo = lopdf::Document::load(&out).unwrap();
    let page = lo.get_pages()[&1];
    let annots = lo.get_dictionary(page).unwrap().get(b"Annots").ok().and_then(|a| lo.dereference(a).ok()).and_then(|(_, a)| a.as_array().ok().cloned()).unwrap_or_default();
    assert!(annots.is_empty(), "the signature stayed an annotation: {annots:?}");
    // Rendered as displayed (842 x 595), the ink sits in the box around (200, 150), wider than tall.
    let img = render(&out, 0);
    assert_eq!(img.0, 842);
    let [x0, y0, x1, y1] = ink(&img).expect("no ink on the page");
    let (cx, cy) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
    assert!((cx - 200.0).abs() < 15.0 && (cy - 150.0).abs() < 15.0, "ink centred at ({cx}, {cy})");
    assert!(x1 - x0 > 2 * (y1 - y0), "the name does not read across: {:?}", [x0, y0, x1, y1]);
    assert!(x1 - x0 <= 202 && x1 - x0 > 150, "ink width {}", x1 - x0);
}

#[test]
fn a_digital_signature_on_a_turned_page_draws_its_graphic_upright() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    let src = Input::new(rotated(t.path()));
    let info = sign::page_info(&src, 0).unwrap();
    let rect = sign::box_at(&info, [600.0, 400.0], 180.0, 0.3);
    let out = sign::sign(&src, &spec(0, rect, Graphic::Typed("Ada".into(), Default::default()), digital(&p12, "secret")), &t.path().join("out.pdf")).unwrap().path;
    assert_eq!(signatures(&out), [(true, true)]);
    qpdf_ok(&out);
    let [x0, y0, x1, y1] = ink(&render(&out, 0)).expect("no ink");
    let (cx, cy) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
    assert!((cx - 600.0).abs() < 15.0 && (cy - 400.0).abs() < 15.0 && x1 - x0 > y1 - y0, "ink {:?}", [x0, y0, x1, y1]);
}

#[test]
fn visual_signatures_drawn_and_image_land_in_their_box() {
    let t = tempfile::tempdir().unwrap();
    let png = t.path().join("sig.png");
    let mut im = image::RgbaImage::from_pixel(300, 100, image::Rgba([255, 255, 255, 0]));
    for x in 20..280 {
        for y in 45..55 {
            im.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
        }
    }
    im.save(&png).unwrap();
    let rect = [100.0, 400.0, 400.0, 500.0]; // 300 x 100 on an unturned page
    let strokes = vec![vec![[0.0, 0.5], [1.0, 0.5]], vec![[0.5, 0.4], [0.5, 0.6]]];
    for (name, g) in [("drawn", Graphic::Drawn(strokes)), ("image", Graphic::Image(png.clone()))] {
        let out = sign::sign(&Input::new(invoice()), &spec(0, rect, g, None), &t.path().join(format!("{name}.pdf"))).unwrap().path;
        qpdf_clean(&out);
        let img = render(&out, 0);
        // The page is 612 x 792: the box is x 100..400, y (from the top) 292..392.
        let n = common::dark_in(&img, 100.0 / img.0 as f32, 400.0 / img.0 as f32, 292.0 / img.1 as f32, 392.0 / img.1 as f32);
        assert!(n > 200, "{name}: {n} dark pixels in the box");
    }
}

/// `qpdf --requires-password`: 0 when the file needs a password to open, 2 when it is not
/// encrypted, 3 when it is encrypted but opens without one.
fn requires_password(path: &Path) -> i32 {
    Command::new("qpdf").arg("--requires-password").arg(path).status().unwrap().code().unwrap_or(-1)
}

#[test]
fn a_protected_input_is_signed_and_stays_protected() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    let locked = t.path().join("locked.pdf");
    let o = Command::new("qpdf").args(["--encrypt", "user", "owner", "256", "--"]).arg(invoice()).arg(&locked).output().unwrap();
    assert!(o.status.success());
    let owner_only = t.path().join("owner-only.pdf");
    let o = Command::new("qpdf").args(["--encrypt", "", "owner", "256", "--print=none", "--"]).arg(invoice()).arg(&owner_only).output().unwrap();
    assert!(o.status.success());
    let open = Input { path: locked.clone(), password: Some("user".into()) };
    // Without its password: asked for, as any tool asks.
    let e = sign::sign(&Input::new(&locked), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada".into(), Default::default()), None), &t.path().join("x.pdf")).unwrap_err();
    assert!(matches!(e, sheaf_core::OpdfError::PasswordRequired(_)), "{e}");
    for (name, input, d) in [("visual.pdf", open.clone(), None), ("digital.pdf", open.clone(), digital(&p12, "secret")), ("owner-only-signed.pdf", Input::new(&owner_only), None)] {
        let out = t.path().join(name);
        sign::sign(&input, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada".into(), Default::default()), d.clone()), &out).unwrap();
        let want = if input.password.is_some() { 0 } else { 3 };
        assert_eq!(requires_password(&out), want, "{name} keeps its protection");
        let doc = Doc::open(&Input { path: out.clone(), password: input.password.clone() }).unwrap();
        if d.is_some() {
            assert!(doc.document().is_signed(), "{name} is signed");
            // Valid for a validator too, opened with the user password.
            if let Ok(o) = Command::new("pdfsig").args(["-nocert", "-upw", "user"]).arg(&out).output() {
                let s = String::from_utf8_lossy(&o.stdout);
                assert!(s.contains("Signature is Valid"), "pdfsig: {s}{}", String::from_utf8_lossy(&o.stderr));
            }
        }
    }
}

#[test]
fn signing_is_refused_where_the_owner_password_forbids_it() {
    let t = tempfile::tempdir().unwrap();
    let no_forms = t.path().join("no-forms.pdf");
    let o = Command::new("qpdf").args(["--encrypt", "", "owner", "256", "--form=n", "--annotate=n", "--"]).arg(invoice()).arg(&no_forms).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let before = files_in(t.path());
    let e = sign::sign(&Input::new(&no_forms), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada".into(), Default::default()), None), &t.path().join("out.pdf")).unwrap_err();
    assert!(e.to_string().contains("does not allow signing"), "{e}");
    assert_eq!(files_in(t.path()), before);
}

#[test]
fn wrong_certificate_password_is_its_own_error_and_nothing_is_written() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    let before = files_in(t.path());
    let e = sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada".into(), Default::default()), digital(&p12, "nope")), &t.path().join("out.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::CertificatePassword(_)), "{e:?}");
    assert!(e.to_string().contains("wrong certificate password"), "{e}");
    let not_id = t.path().join("k.pem");
    let e = sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada".into(), Default::default()), digital(&not_id, "secret")), &t.path().join("out.pdf")).unwrap_err();
    assert!(e.to_string().contains("not a usable digital ID"), "{e}");
    assert_eq!(files_in(t.path()), before);
}

#[test]
fn bad_specs_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("out.pdf");
    let i = Input::new(invoice());
    let typed = || Graphic::Typed("Ada".into(), Default::default());
    assert!(sign::sign(&i, &spec(3, [72.0, 72.0, 272.0, 132.0], typed(), None), &out).unwrap_err().to_string().contains("page 4 does not exist"));
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 72.0, 132.0], typed(), None), &out).is_err());
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("  ".into(), Default::default()), None), &out).is_err());
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Drawn(vec![vec![]]), None), &out).is_err());
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Drawn(vec![vec![[f64::NAN, 0.0]]]), None), &out).is_err());
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Image(invoice()), None), &out).is_err());
    assert!(!out.exists());
}

#[test]
fn a_digital_signature_draws_the_same_ink_as_a_visual_one() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    let rect = [72.0, 72.0, 272.0, 132.0];
    let strokes = vec![vec![[0.0, 0.0], [0.4, 0.3], [0.7, 0.05], [1.0, 0.3]]];
    // Every handwriting font (their descents differ: the box must hold the same ink either way).
    let typed = sheaf_core::script::ScriptFont::ALL.map(|f| (f.name(), Graphic::Typed("Ada Lovelace".into(), f)));
    for (name, g) in typed.into_iter().chain([("drawn", Graphic::Drawn(strokes))]) {
        let visual = sign::sign(&Input::new(invoice()), &spec(0, rect, g.clone(), None), &t.path().join(format!("{name}-v.pdf").replace(' ', "-"))).unwrap().path;
        let digital = sign::sign(&Input::new(invoice()), &spec(0, rect, g, digital(&p12, "secret")), &t.path().join(format!("{name}-d.pdf").replace(' ', "-"))).unwrap().path;
        // Only the signature box: the invoice's own text is outside it.
        let ink_in_box = |path: &Path| {
            let img = render(path, 0);
            let h = img.1 as f64;
            let mut b: Option<[u32; 4]> = None;
            for y in (h - rect[3]) as u32..(h - rect[1]) as u32 {
                for x in rect[0] as u32..rect[2] as u32 {
                    let p = &img.2[((y * img.0 + x) * 4) as usize..];
                    if p[3] > 128 && p[0] < 128 {
                        b = Some(b.map_or([x, y, x, y], |b| [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]));
                    }
                }
            }
            b.expect("no ink in the box")
        };
        let (v, d) = (ink_in_box(&visual), ink_in_box(&digital));
        assert!(v.iter().zip(d).all(|(a, b)| a.abs_diff(b) <= 2), "{name}: visual ink {v:?}, digital ink {d:?}");
        assert_eq!(signatures(&digital), [(true, true)]);
    }
}

/// One page, MediaBox [0 0 600 800] with CropBox [100 200 500 700], turned `rotation`.
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

#[test]
fn visual_and_digital_signatures_land_where_placed_on_every_rotation_with_a_crop_offset() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    for rotation in [0, 90, 180, 270] {
        let src = Input::new(cropped_turned(t.path(), rotation));
        let info = sign::page_info(&src, 0).unwrap();
        let (w, h) = (info.width as f64, info.height as f64);
        assert_eq!((w, h), if rotation % 180 == 0 { (400.0, 500.0) } else { (500.0, 400.0) }, "rot {rotation}");
        // Centred a third across and a quarter down the page as displayed, 160 pt wide.
        let center = [w / 3.0, h / 4.0];
        let g = Graphic::Typed("Ada Lovelace".into(), Default::default());
        let rect = sign::box_at(&info, center, 160.0, sign::aspect(&g).unwrap());
        for (kind, d) in [("visual", None), ("digital", digital(&p12, "secret"))] {
            let out = sign::sign(&src, &spec(0, rect, g.clone(), d.clone()), &t.path().join(format!("out-{rotation}-{kind}.pdf"))).unwrap().path;
            if d.is_some() {
                assert_eq!(signatures(&out), [(true, true)], "rot {rotation}");
                qpdf_ok(&out);
            } else {
                qpdf_clean(&out);
            }
            let img = render(&out, 0);
            assert!((img.0 as f64 - w).abs() <= 1.0 && (img.1 as f64 - h).abs() <= 1.0, "rot {rotation}: render {}x{}", img.0, img.1);
            let [x0, y0, x1, y1] = ink(&img).unwrap_or_else(|| panic!("rot {rotation} {kind}: no ink"));
            let (cx, cy) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
            assert!((cx - center[0]).abs() < 8.0 && (cy - center[1]).abs() < 8.0, "rot {rotation} {kind}: ink centred at ({cx}, {cy}), placed at {center:?}");
            assert!(x1 - x0 > 2 * (y1 - y0) && (x1 - x0) as f64 > 140.0 && (x1 - x0) as f64 <= 162.0, "rot {rotation} {kind}: ink {:?} not upright", [x0, y0, x1, y1]);
        }
    }
}

#[test]
fn a_typed_name_takes_its_handwriting_font() {
    use sheaf_core::script::ScriptFont;
    let aspects: Vec<f64> = ScriptFont::ALL.iter().map(|f| sign::aspect(&Graphic::Typed("Ada Lovelace".into(), *f)).unwrap()).collect();
    assert!(aspects.windows(2).all(|w| (w[0] - w[1]).abs() > 1e-6), "each font its own shape: {aspects:?}");
}

#[test]
fn a_signature_on_paper_loses_its_background_and_keeps_its_ink() {
    let t = tempfile::tempdir().unwrap();
    // Off-white paper with a dark blue stroke across the middle.
    let src = t.path().join("photo.png");
    image::RgbImage::from_fn(120, 40, |x, y| if (18..22).contains(&y) && (10..110).contains(&x) { image::Rgb([20, 30, 120]) } else { image::Rgb([236, 232, 225]) }).save(&src).unwrap();
    let out = sign::clear_background(&src).unwrap();
    let img = image::open(&out).unwrap().to_rgba8();
    assert_eq!(img.get_pixel(2, 2)[3], 0, "the paper is transparent");
    assert_eq!(*img.get_pixel(60, 20), image::Rgba([20, 30, 120, 255]), "the ink keeps its colour, opaque");
    // A picture already transparent is kept as it is.
    assert_eq!(sign::clear_background(&out).unwrap(), out);
}

#[test]
fn a_certifying_signature_says_which_changes_it_allows() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    let out = t.path().join("certified.pdf");
    let d = DigitalSpec { certify: Some(2), ..digital(&p12, "secret").unwrap() };
    sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada Lovelace".into(), Default::default()), Some(d)), &out).unwrap();
    assert_eq!(signatures(&out), [(true, true)]);
    let doc = Doc::open(&Input::new(&out)).unwrap();
    assert_eq!(doc.document().signatures[0].certify, Some(2), "a certification allowing forms and signing");
}
