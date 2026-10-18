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
    Some(DigitalSpec { p12: p12.into(), password: password.into(), reason: Some("Approved".into()), location: None })
}

fn spec(page: usize, rect: [f64; 4], graphic: Graphic, digital: Option<DigitalSpec>) -> SignSpec {
    SignSpec { page, rect, graphic, digital }
}

/// The signatures of `path` as PdfCraft reads them: (signed, intact: not Invalid).
fn signatures(path: &Path) -> Vec<(bool, bool)> {
    let doc = Doc::open(&Input::new(path)).unwrap();
    assert_eq!(doc.document().is_signed(), doc.document().signatures.iter().any(|s| s.signed));
    doc.document().signatures.iter().map(|s| (s.signed, s.status != sheaf_core::pdfcraft::engine::SignatureStatus::Invalid)).collect()
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
    let r = sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada Lovelace".into()), digital(&p12, "secret")), &out).unwrap();
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
    let once = sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("First".into()), digital(&p12, "secret")), &t.path().join("once.pdf")).unwrap().path;
    let strokes = vec![vec![[0.0, 0.0], [0.3, 0.2], [0.6, 0.0], [1.0, 0.25]]];
    let twice = sign::sign(&Input::new(&once), &spec(0, [320.0, 72.0, 520.0, 132.0], Graphic::Drawn(strokes), digital(&p12, "secret")), &t.path().join("twice.pdf")).unwrap().path;
    assert!(std::fs::read(&twice).unwrap().starts_with(&std::fs::read(&once).unwrap()));
    assert_eq!(signatures(&twice), [(true, true), (true, true)]);
    qpdf_ok(&twice);
    if let Some(s) = pdfsig(&twice) {
        assert_eq!(s.matches("Signature is Valid").count(), 2, "pdfsig: {s}");
    }
    // A visual-only signature on a signed file is an incremental update too.
    let third = sign::sign(&Input::new(&twice), &spec(0, [72.0, 200.0, 222.0, 240.0], Graphic::Typed("Witness".into()), None), &t.path().join("third.pdf")).unwrap().path;
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
    let graphic = Graphic::Typed("Ada Lovelace".into());
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
    let out = sign::sign(&src, &spec(0, rect, Graphic::Typed("Ada".into()), digital(&p12, "secret")), &t.path().join("out.pdf")).unwrap().path;
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

#[test]
fn encrypted_input_is_refused_and_nothing_is_written() {
    let t = tempfile::tempdir().unwrap();
    let locked = t.path().join("locked.pdf");
    let o = Command::new("qpdf").args(["--encrypt", "user", "owner", "256", "--"]).arg(invoice()).arg(&locked).output().unwrap();
    assert!(o.status.success());
    let owner_only = t.path().join("owner-only.pdf");
    let o = Command::new("qpdf").args(["--encrypt", "", "owner", "256", "--"]).arg(invoice()).arg(&owner_only).output().unwrap();
    assert!(o.status.success());
    let before = files_in(t.path());
    for input in [Input { path: locked.clone(), password: Some("user".into()) }, Input::new(&locked), Input::new(&owner_only)] {
        let e = sign::sign(&input, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada".into()), None), &t.path().join("out.pdf")).unwrap_err();
        assert!(e.to_string().contains("unlock the PDF first"), "{e}");
    }
    assert_eq!(files_in(t.path()), before);
}

#[test]
fn wrong_certificate_password_is_its_own_error_and_nothing_is_written() {
    let t = tempfile::tempdir().unwrap();
    let p12 = test_id(t.path());
    let before = files_in(t.path());
    let e = sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada".into()), digital(&p12, "nope")), &t.path().join("out.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::CertificatePassword(_)), "{e:?}");
    assert!(e.to_string().contains("wrong certificate password"), "{e}");
    let not_id = t.path().join("k.pem");
    let e = sign::sign(&Input::new(invoice()), &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("Ada".into()), digital(&not_id, "secret")), &t.path().join("out.pdf")).unwrap_err();
    assert!(e.to_string().contains("not a usable digital ID"), "{e}");
    assert_eq!(files_in(t.path()), before);
}

#[test]
fn bad_specs_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("out.pdf");
    let i = Input::new(invoice());
    let typed = || Graphic::Typed("Ada".into());
    assert!(sign::sign(&i, &spec(3, [72.0, 72.0, 272.0, 132.0], typed(), None), &out).unwrap_err().to_string().contains("page 4 does not exist"));
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 72.0, 132.0], typed(), None), &out).is_err());
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Typed("  ".into()), None), &out).is_err());
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Drawn(vec![vec![]]), None), &out).is_err());
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Drawn(vec![vec![[f64::NAN, 0.0]]]), None), &out).is_err());
    assert!(sign::sign(&i, &spec(0, [72.0, 72.0, 272.0, 132.0], Graphic::Image(invoice()), None), &out).is_err());
    assert!(!out.exists());
}
