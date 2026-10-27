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
