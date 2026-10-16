mod common;
use common::{qpdf_clean, render, unprotected_and_clean};
use pdfium_render::prelude::*;
use sheaf_core::engine::{new_document, pdfium_lock};
use sheaf_core::pages::Input;
use sheaf_core::redact::{self, Pattern, Query};
use std::path::{Path, PathBuf};

const PAGES: [&[&str]; 3] = [
    &["Contact alice@example.com or call (555) 123-4567", "The secret word is Zebra"],
    &["Mail bob@test.org today", "Phone 555-987-6543 and the Secret plan"],
    &["Rotated page for carol@example.net", "secret notes"],
];

/// Three A4 pages of Helvetica text (lines of `PAGES`); page 3 has /Rotate 90.
fn fixture(dir: &Path) -> PathBuf {
    let _g = pdfium_lock();
    let mut doc = new_document();
    let font = doc.fonts_mut().helvetica();
    for (k, lines) in PAGES.iter().enumerate() {
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        for (i, l) in lines.iter().enumerate() {
            page.objects_mut().create_text_object(PdfPoints::new(72.0), PdfPoints::new(760.0 - 30.0 * i as f32), *l, font, PdfPoints::new(14.0)).unwrap();
        }
        if k == 2 {
            page.set_rotation(PdfPageRenderRotation::Degrees90);
        }
    }
    let path = dir.join("private.pdf");
    doc.save_to_file(&path).unwrap();
    path
}

/// Text of every page via lopdf, and every word PdfCraft's text layer still finds.
fn all_text(path: &Path) -> String {
    let lo = common::page_texts(path).join("\n");
    let doc = sheaf_core::pdfcraft::Doc::open(&Input::new(path)).unwrap();
    let words: Vec<String> = doc.document().words().into_iter().map(|w| w.text).collect();
    format!("{lo}\n{}", words.join(" "))
}

#[test]
fn search_finds_patterns_and_words_on_every_page_including_the_rotated_one() {
    let t = tempfile::tempdir().unwrap();
    let pdf = Input::new(fixture(t.path()));
    let emails = redact::find(&pdf, &Query::Pattern(Pattern::Email)).unwrap();
    let texts: Vec<(usize, &str)> = emails.iter().map(|m| (m.page, m.text.as_str())).collect();
    assert_eq!(texts, [(0, "alice@example.com"), (1, "bob@test.org"), (2, "carol@example.net")]);
    let phones = redact::find(&pdf, &Query::Pattern(Pattern::Phone)).unwrap();
    assert_eq!(phones.iter().map(|m| m.page).collect::<Vec<_>>(), [0, 1], "{phones:?}");
    let secret = redact::find(&pdf, &Query::Literal { text: "secret".into(), case_insensitive: true }).unwrap();
    assert_eq!(secret.iter().map(|m| m.page).collect::<Vec<_>>(), [0, 1, 2]);
    let exact = redact::find(&pdf, &Query::Literal { text: "secret".into(), case_insensitive: false }).unwrap();
    assert_eq!(exact.iter().map(|m| m.page).collect::<Vec<_>>(), [0, 2]);
    // On the rotated page the match box is in user space, where the line was drawn (wide, near
    // the top of the unrotated page), not where it shows on screen (running down the left).
    let r = redact::bounds(&emails[2].quads);
    assert!(r[2] - r[0] > r[3] - r[1] && r[1] > 740.0 && r[3] < 790.0 && r[0] > 72.0, "rotated match box {r:?}");
}

#[test]
fn applied_redactions_remove_the_words_draw_black_boxes_and_pass_the_audit() {
    let t = tempfile::tempdir().unwrap();
    let pdf = Input::new(fixture(t.path()));
    let mut hits = redact::find(&pdf, &Query::Pattern(Pattern::Email)).unwrap();
    hits.extend(redact::find(&pdf, &Query::Pattern(Pattern::Phone)).unwrap());
    hits.extend(redact::find(&pdf, &Query::Literal { text: "secret".into(), case_insensitive: true }).unwrap());
    let marks = redact::mark_text(&hits);
    assert_eq!(marks.len(), 8);
    let before = render(&pdf.path, 0);
    let r = redact::apply(&pdf, &marks, &t.path().join("out.pdf")).unwrap();
    assert_eq!((r.areas, r.was_encrypted), (8, false));
    qpdf_clean(&r.path);
    let text = all_text(&r.path);
    for gone in ["alice", "example.com", "bob@", "carol", "123-4567", "987-6543", "secret", "Secret"] {
        assert!(!text.contains(gone), "{gone:?} still extractable: {text}");
    }
    for kept in ["Contact", "Zebra", "Mail", "notes", "Rotated"] {
        assert!(text.contains(kept), "{kept:?} lost: {text}");
    }
    // The e-mail on page 1 is now a black box: many more dark pixels on its line.
    let after = render(&r.path, 0);
    let line = |img| common::dark_in(img, 0.2, 0.55, 0.08, 0.12);
    assert!(line(&after) > line(&before) * 3, "{} vs {}", line(&after), line(&before));
    // Independent check: re-opened, no mark is left and the audit passes. PdfCraft's raw audit
    // works on whole lines, so it lists lines with words kept on both sides of a box ("Mail ▮
    // today"); `verify` confirms that no glyph of those lines lies under any box.
    let doc = sheaf_core::pdfcraft::Doc::open(&Input::new(&r.path)).unwrap();
    assert_eq!(doc.document().redaction_marks(), 0);
    let raw = doc.document().audit_redactions().unwrap();
    assert!(raw.iter().all(|f| f.kind.clone().id() == "covered-text"), "{raw:?}");
    redact::verify(&r.path).unwrap();
}

#[test]
fn whole_pages_and_rectangles_can_be_marked() {
    let t = tempfile::tempdir().unwrap();
    let pdf = Input::new(fixture(t.path()));
    let mut marks = redact::mark_pages(&pdf, &[1]).unwrap();
    // A rectangle around "Zebra" on page 1 (Helvetica 14 at x 72, baseline 730).
    let zebra = redact::find(&pdf, &Query::Literal { text: "Zebra".into(), case_insensitive: false }).unwrap();
    let zr = redact::bounds(&zebra[0].quads);
    marks.extend(redact::mark_rects(&[(0, [zr[0] - 2.0, zr[1] - 2.0, zr[2] + 2.0, zr[3] + 2.0])]));
    let r = redact::apply(&pdf, &marks, &t.path().join("out.pdf")).unwrap();
    assert_eq!(r.areas, 2);
    let pages = common::page_texts(&r.path);
    assert!(pages[0].contains("Contact") && !pages[0].contains("Zebra"), "{pages:?}");
    assert!(!pages[1].contains("Mail") && !pages[1].contains("Phone"), "{pages:?}");
    // The whole of page 2 is black.
    let img = render(&r.path, 1);
    let total = (img.0 * img.1) as usize;
    assert!(common::dark_in(&img, 0.0, 1.0, 0.0, 1.0) > total * 9 / 10);
    qpdf_clean(&r.path);
}

#[test]
fn nothing_marked_is_refused_and_writes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let pdf = Input::new(fixture(t.path()));
    let out = t.path().join("out.pdf");
    assert!(redact::apply(&pdf, &[], &out).is_err());
    assert!(!out.exists());
}

#[test]
fn a_protected_input_gives_an_unprotected_redacted_output() {
    let t = tempfile::tempdir().unwrap();
    let plain = fixture(t.path());
    // qpdf, not `security::protect`: about 1 in 30 of its outputs is refused by PdfCraft's
    // parser (InvalidEncryption), which would make this test flaky.
    let locked = t.path().join("locked.pdf");
    let o = std::process::Command::new("qpdf").args(["--encrypt", "pw", "owner", "256", "--"]).arg(&plain).arg(&locked).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let pdf = Input { path: locked, password: Some("pw".into()) };
    let hits = redact::find(&pdf, &Query::Pattern(Pattern::Email)).unwrap();
    assert_eq!(hits.len(), 3);
    let r = redact::apply(&pdf, &redact::mark_text(&hits), &t.path().join("out.pdf")).unwrap();
    assert!(r.was_encrypted);
    unprotected_and_clean(&r.path);
    assert!(!all_text(&r.path).contains("alice"));
}

/// A page with a 40×40 grey image drawn at (100, 600)–(300, 800), uncompressed DeviceRGB.
fn image_pdf(dir: &Path) -> PathBuf {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let img = doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 40, "Height" => 40, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8 },
        vec![60u8; 40 * 40 * 3],
    ));
    let content = doc.add_object(Stream::new(dictionary! {}, b"q 200 0 0 200 100 600 cm /Im1 Do Q".to_vec()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => content, "Resources" => dictionary! { "XObject" => dictionary! { "Im1" => img } },
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let path = dir.join("image.pdf");
    doc.save(&path).unwrap();
    path
}

/// The pixels of every image XObject in the file, decoded.
fn image_pixels(path: &Path) -> Vec<(i64, i64, Vec<u8>)> {
    let d = lopdf::Document::load(path).unwrap();
    d.objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| s.dict.get(b"Subtype").and_then(|x| x.as_name()).ok() == Some(b"Image".as_slice()))
        .map(|s| {
            let w = s.dict.get(b"Width").unwrap().as_i64().unwrap();
            let h = s.dict.get(b"Height").unwrap().as_i64().unwrap();
            (w, h, s.decompressed_content().unwrap_or_else(|_| s.content.clone()))
        })
        .collect()
}

#[test]
fn a_rectangle_over_an_image_clears_the_covered_pixels_in_the_file() {
    let t = tempfile::tempdir().unwrap();
    let pdf = Input::new(image_pdf(t.path()));
    // The left half of the image (user x 100–200, all of its height).
    let marks = redact::mark_rects(&[(0, [100.0, 600.0, 200.0, 800.0])]);
    let r = redact::apply(&pdf, &marks, &t.path().join("out.pdf")).unwrap();
    qpdf_clean(&r.path);
    let images = image_pixels(&r.path);
    assert_eq!(images.len(), 1, "the original image must be gone, a cleared copy kept");
    let (w, h, px) = &images[0];
    assert_eq!((*w, *h), (40, 40));
    let at = |x: usize, y: usize| px[(y * 40 + x) * 3];
    // PdfCraft zeroes every pixel whose centre is under the mark; the rest keep their value.
    assert!((0..40).all(|y| (0..20).all(|x| at(x, y) == 0)), "covered pixels kept");
    assert!((0..40).all(|y| (20..40).all(|x| at(x, y) == 60)), "uncovered pixels changed");
}

/// One page with "Account 4242 secret" drawn, then a black rectangle painted over the line:
/// a redaction that only looks done.
fn faux_pdf(dir: &Path) -> PathBuf {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let ops = b"BT /F1 14 Tf 72 700 Td (Account 4242 secret) Tj ET 0 0 0 rg 68 695 150 20 re f".to_vec();
    let content = doc.add_object(Stream::new(dictionary! {}, ops));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => content, "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let path = dir.join("faux.pdf");
    doc.save(&path).unwrap();
    path
}

#[test]
fn a_box_drawn_over_text_fails_verification() {
    let t = tempfile::tempdir().unwrap();
    let faux = faux_pdf(t.path());
    assert!(common::page_texts(&faux)[0].contains("secret"), "fixture: the text is still there");
    let e = redact::verify(&faux).unwrap_err().to_string();
    assert!(e.starts_with("conversion failed: redaction audit failed") && e.contains("covered-text"), "{e}");
}
