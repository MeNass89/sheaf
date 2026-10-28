mod common;
use sheaf_core::adjust::{adjust_colours, Adjust};
use sheaf_core::pages::Input;

/// Adjust colours ▸ Invert: a page of black text on white comes out light text on a dark page,
/// the text still text.
#[test]
fn an_inverted_page_is_light_on_dark_and_keeps_its_text() {
    let t = tempfile::tempdir().unwrap();
    let src = common::fixture_pdf(t.path(), "a.pdf", 1);
    let out = t.path().join("night.pdf");
    let (out, _) = adjust_colours(&Input::new(&src), &Adjust { invert: true, ..Default::default() }, &out).unwrap();
    let (_, _, before) = common::render(&src, 0);
    let (_, _, after) = common::render(&out, 0);
    let mean = |px: &[u8]| px.as_chunks::<4>().0.iter().map(|p| p[0] as f64).sum::<f64>() / (px.len() / 4) as f64;
    assert!(mean(&before) > 200.0 && mean(&after) < 55.0, "white paper turned dark ({:.0} → {:.0})", mean(&before), mean(&after));
    assert_eq!(common::page_texts(&out), common::page_texts(&src));
}

/// A page whose picture is a CMYK JPEG, as Acrobat and Photoshop write them (Adobe's inverted
/// components with `/Decode [1 0 1 0 1 0 1 0]`), or the same JPEG with no /Decode.
fn cmyk_jpeg_page(dir: &std::path::Path, decode: bool) -> std::path::PathBuf {
    use lopdf::{dictionary, Object, Stream};
    let jpg = dir.join("red.jpg");
    let made = std::process::Command::new("convert").args(["-size", "16x16", "xc:red", "-colorspace", "CMYK"]).arg(&jpg).status();
    assert!(made.is_ok_and(|s| s.success()), "ImageMagick's convert makes the CMYK JPEG");
    let mut doc = lopdf::Document::with_version("1.7");
    let mut im = dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 16, "Height" => 16, "ColorSpace" => "DeviceCMYK", "BitsPerComponent" => 8, "Filter" => "DCTDecode" };
    if decode {
        im.set("Decode", [1, 0, 1, 0, 1, 0, 1, 0].map(Object::Integer).to_vec());
    }
    let img = doc.add_object(Stream::new(im, std::fs::read(&jpg).unwrap()));
    let content = doc.add_object(Stream::new(dictionary! {}, b"q 200 0 0 200 0 0 cm /Im0 Do Q".to_vec()));
    let pages = doc.new_object_id();
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()], "Contents" => content,
        "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => img } } });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    let path = dir.join(if decode { "adobe.pdf" } else { "raw.pdf" });
    doc.save(&path).unwrap();
    path
}

/// Invert on a CMYK JPEG, with Adobe's inverted /Decode or none: the page shows each colour's
/// opposite of what PDFium showed before, as Stirling's Invert of the rendered page does (Sol, 27/10:
/// the inverted /Decode was left out of Invert, untested).
#[test]
fn a_cmyk_jpeg_is_inverted_as_shown_with_or_without_its_decode() {
    let t = tempfile::tempdir().unwrap();
    for decode in [true, false] {
        let src = cmyk_jpeg_page(t.path(), decode);
        let out = t.path().join(format!("inv-{decode}.pdf"));
        let (out, _) = adjust_colours(&Input::new(&src), &Adjust { invert: true, ..Default::default() }, &out).unwrap();
        let (w, _, before) = common::render(&src, 0);
        let (_, _, after) = common::render(&out, 0);
        let mid = (100 * w as usize + 100) * 4;
        let (b, a) = (&before[mid..mid + 3], &after[mid..mid + 3]);
        if decode {
            assert!(b[0] > 200 && b[1] < 60, "Adobe's JPEG shows red: {b:?}");
        }
        // Each channel the opposite of the one shown. Up to 40 apart: PDFium turns CMYK into RGB with
        // Adobe's table (red shows 237, 29, 36), the JPEG decoder with the plain formula (255, 0, 0).
        for c in 0..3 {
            assert!((255 - b[c] as i32 - a[c] as i32).abs() <= 40, "decode {decode}: {b:?} inverted is {a:?}");
        }
    }
}

/// Invert on a monochrome scan, a CCITT Group 4 picture (ImageMagick's, as scanners write them): the
/// black half turns white and the white half black, as Stirling's Invert of the rendered page
/// (Sol, 28/10: 1-bit CCITT and JBIG2 pictures were left as they were).
#[test]
fn a_ccitt_scan_is_inverted() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("scan.pdf");
    let made = std::process::Command::new("convert")
        .args(["-size", "40x40", "xc:white", "-fill", "black", "-draw", "rectangle 0,0 19,39", "-monochrome", "-compress", "Group4"])
        .arg(&src)
        .status();
    assert!(made.is_ok_and(|s| s.success()), "ImageMagick's convert makes the CCITT PDF");
    let doc = lopdf::Document::load(&src).unwrap();
    assert!(doc.objects.values().any(|o| o.as_stream().is_ok_and(|s| s.dict.get(b"Filter").ok().and_then(|f| f.as_array().ok().and_then(|a| a.first()).or(Some(f))).and_then(|f| f.as_name().ok()) == Some(b"CCITTFaxDecode"))), "a CCITT picture");
    let out = t.path().join("inv.pdf");
    let (out, kept) = adjust_colours(&Input::new(&src), &Adjust { invert: true, ..Default::default() }, &out).unwrap();
    assert_eq!(kept, 0, "nothing left as it was");
    let (w, h, before) = common::render(&src, 0);
    let (_, _, after) = common::render(&out, 0);
    let at = |px: &[u8], fx: usize| px[((h as usize / 2) * w as usize + fx * w as usize / 4) * 4];
    assert!(at(&before, 1) < 50 && at(&before, 3) > 200, "black left, white right: {} {}", at(&before, 1), at(&before, 3));
    assert!(at(&after, 1) > 200 && at(&after, 3) < 50, "white left, black right: {} {}", at(&after, 1), at(&after, 3));
}

/// Invert on palette pictures (/Indexed): a black-and-white scan with a two-colour palette (1 bit
/// per pixel, black and white) and an RGB palette of red and white, its table in a stream: each
/// shows the opposite colours, as Stirling's Invert of the rendered page (Sol, 28/10: palette
/// pictures were left as they were).
#[test]
fn indexed_pictures_are_inverted() {
    use lopdf::{dictionary, Object, Stream, StringFormat};
    let t = tempfile::tempdir().unwrap();
    let mut doc = lopdf::Document::with_version("1.7");
    // 16 × 16, 1 bit: the left half index 0 (black), the right half index 1 (white).
    let bw = Object::Array(vec!["Indexed".into(), "DeviceGray".into(), 1.into(), Object::String(vec![0, 255], StringFormat::Hexadecimal)]);
    let scan = doc.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 16, "Height" => 16, "ColorSpace" => bw, "BitsPerComponent" => 1 }, [0x00_u8, 0xFF].repeat(16)));
    // 16 × 16, 8 bits: left index 0 (red), right index 1 (white); the palette a stream.
    let table = doc.add_object(Stream::new(dictionary! {}, vec![255, 0, 0, 255, 255, 255]));
    let rgb = Object::Array(vec!["Indexed".into(), "DeviceRGB".into(), 1.into(), Object::Reference(table)]);
    let px: Vec<u8> = (0..16).flat_map(|_| (0..16).map(|x| u8::from(x >= 8))).collect();
    let colour = doc.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 16, "Height" => 16, "ColorSpace" => rgb, "BitsPerComponent" => 8 }, px));
    let content = doc.add_object(Stream::new(dictionary! {}, b"q 100 0 0 100 0 100 cm /A Do Q q 100 0 0 100 0 0 cm /B Do Q".to_vec()));
    let pages = doc.new_object_id();
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 100.into(), 200.into()], "Contents" => content,
        "Resources" => dictionary! { "XObject" => dictionary! { "A" => scan, "B" => colour } } });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    let src = t.path().join("indexed.pdf");
    doc.save(&src).unwrap();
    let out = t.path().join("inv.pdf");
    let (out, kept) = adjust_colours(&Input::new(&src), &Adjust { invert: true, ..Default::default() }, &out).unwrap();
    assert_eq!(kept, 0, "nothing left as it was");
    let (w, _, before) = common::render(&src, 0);
    let (_, _, after) = common::render(&out, 0);
    // The scan above (y 0–100 of the rendering), the red picture below; left and right quarters.
    let at = |px: &[u8], x: usize, y: usize| { let i = (y * w as usize + x) * 4; [px[i], px[i + 1], px[i + 2]] };
    let (q1, q3) = (w as usize / 4, w as usize * 3 / 4);
    let (top, low) = (w as usize / 2, w as usize * 3 / 2);
    assert_eq!((at(&before, q1, top), at(&before, q3, top)), ([0, 0, 0], [255, 255, 255]), "the scan: black then white");
    assert_eq!((at(&after, q1, top), at(&after, q3, top)), ([255, 255, 255], [0, 0, 0]), "the scan inverted");
    assert_eq!((at(&before, q1, low), at(&before, q3, low)), ([255, 0, 0], [255, 255, 255]), "red then white");
    assert_eq!((at(&after, q1, low), at(&after, q3, low)), ([0, 255, 255], [0, 0, 0]), "cyan then black");
}
