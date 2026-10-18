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
    let r = redact::apply(&pdf, &marks, &[], &t.path().join("out.pdf")).unwrap();
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
    redact::verify(&r.path, &marks).unwrap();
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
    let r = redact::apply(&pdf, &marks, &[], &t.path().join("out.pdf")).unwrap();
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
    assert!(redact::apply(&pdf, &[], &[], &out).is_err());
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
    let r = redact::apply(&pdf, &redact::mark_text(&hits), &[], &t.path().join("out.pdf")).unwrap();
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
    let r = redact::apply(&pdf, &marks, &[], &t.path().join("out.pdf")).unwrap();
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
    // A mark over the box: the audit's own failure.
    let e = redact::verify(&faux, &redact::mark_rects(&[(0, [68.0, 695.0, 218.0, 715.0])])).unwrap_err().to_string();
    assert!(e.starts_with("conversion failed: redaction audit failed") && e.contains("covered-text"), "{e}");
    // No mark near it: the box was in the input already, and the message says where.
    let e = redact::verify(&faux, &redact::mark_rects(&[(0, [300.0, 100.0, 400.0, 120.0])])).unwrap_err().to_string();
    assert_eq!(e, "This PDF already hides text under a box on page 1. Mark that area too to remove it.");
}

// ---- Fix round 1 -------------------------------------------------------------------------------

/// A PDF assembled from raw objects: `objs[0]` is object 1 (the catalog), and so on; a
/// `(dict, stream)` entry is a stream. `info` is the object number of the /Info dictionary.
fn raw_pdf(path: &Path, objs: &[(&str, Option<&[u8]>)], info: Option<usize>) -> PathBuf {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, (d, s)) in objs.iter().enumerate() {
        offs.push(out.len());
        match s {
            Some(s) => {
                out.extend(format!("{} 0 obj {} /Length {} >>\nstream\n", i + 1, d.trim_end_matches(">>").trim_end(), s.len()).as_bytes());
                out.extend(*s);
                out.extend(b"\nendstream endobj\n");
            }
            None => out.extend(format!("{} 0 obj {d} endobj\n", i + 1).as_bytes()),
        }
    }
    let x = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    let info = info.map(|i| format!(" /Info {i} 0 R")).unwrap_or_default();
    out.extend(format!("trailer << /Size {} /Root 1 0 R{info} >>\nstartxref\n{x}\n%%EOF\n", objs.len() + 1).as_bytes());
    std::fs::write(path, out).unwrap();
    path.to_path_buf()
}

const HELV: &str = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>";

/// pdfium's text of every page.
fn pdfium_text(path: &Path) -> String {
    let _g = pdfium_lock();
    let doc = sheaf_core::engine::open_pdf(path, None).unwrap();
    doc.pages().iter().map(|p| p.text().unwrap().all()).collect::<Vec<_>>().join("\n")
}

/// Every decoded string of the file (document information, bookmarks, annotations, fields)
/// and every XMP stream, as one text.
fn strings_and_xmp(path: &Path) -> String {
    fn walk(o: &lopdf::Object, out: &mut String) {
        match o {
            lopdf::Object::String(..) => {
                out.push_str(&lopdf::decode_text_string(o).unwrap_or_default());
                out.push('\n');
            }
            lopdf::Object::Array(a) => a.iter().for_each(|x| walk(x, out)),
            lopdf::Object::Dictionary(d) => d.iter().for_each(|(_, v)| walk(v, out)),
            lopdf::Object::Stream(s) => {
                s.dict.iter().for_each(|(_, v)| walk(v, out));
                if s.dict.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"Metadata".as_slice()) {
                    out.push_str(&String::from_utf8_lossy(&s.decompressed_content().unwrap_or_else(|_| s.content.clone())));
                }
            }
            _ => {}
        }
    }
    let d = lopdf::Document::load(path).unwrap();
    let mut out = String::new();
    d.objects.values().for_each(|o| walk(o, &mut out));
    out
}

/// "secretword" on the page, in /Info (Title, Subject, Author as UTF-16BE, Keywords), in XMP,
/// in a bookmark title, in a comment's /Contents and /T away from the text, and in a text
/// field's value.
fn metadata_pdf(dir: &Path) -> PathBuf {
    let author = {
        let mut b = vec![0xFE, 0xFF];
        b.extend("Ré secretword".encode_utf16().flat_map(|u| u.to_be_bytes()));
        format!("<{}>", b.iter().map(|x| format!("{x:02X}")).collect::<String>())
    };
    let info = format!("<< /Title (Doc about secretword) /Subject (secretword) /Keywords (a, SecretWord, b) /Author {author} >>");
    let xmp: &[u8] = b"<x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><rdf:Description xmlns:dc='http://purl.org/dc/elements/1.1/'><dc:title>About secretword</dc:title></rdf:Description></rdf:RDF></x:xmpmeta>";
    raw_pdf(
        &dir.join("meta.pdf"),
        &[
            ("<< /Type /Catalog /Pages 2 0 R /Outlines 7 0 R /Metadata 10 0 R /AcroForm << /Fields [11 0 R] >> >>", None),
            ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>", None),
            ("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> /Annots [9 0 R 11 0 R] >>", None),
            ("<< >>", Some(b"BT /F1 14 Tf 72 760 Td (Name secretword here) Tj ET BT /F1 14 Tf 72 600 Td (Other text) Tj ET")),
            (HELV, None),
            (&info, None),
            ("<< /Type /Outlines /First 8 0 R /Last 8 0 R /Count 1 >>", None),
            ("<< /Title (Chapter secretword) /Parent 7 0 R /Dest [3 0 R /Fit] >>", None),
            ("<< /Type /Annot /Subtype /Text /Rect [400 400 420 420] /Contents (note secretword) /T (by secretword) >>", None),
            ("<< /Type /Metadata /Subtype /XML >>", Some(xmp)),
            ("<< /Type /Annot /Subtype /Widget /FT /Tx /T (name) /V (secretword) /Rect [300 300 500 320] /P 3 0 R >>", None),
        ],
        Some(6),
    )
}

#[test]
fn a_searched_text_is_removed_from_metadata_bookmarks_comments_and_fields_too() {
    let t = tempfile::tempdir().unwrap();
    let pdf = Input::new(metadata_pdf(t.path()));
    assert!(strings_and_xmp(&pdf.path).matches("secretword").count() >= 7, "fixture");
    let hits = redact::find(&pdf, &Query::Literal { text: "secretword".into(), case_insensitive: true }).unwrap();
    assert_eq!(hits.len(), 1, "only the page text is a match");
    let r = redact::apply(&pdf, &redact::mark_text(&hits), &["secretword".into()], &t.path().join("out.pdf")).unwrap();
    qpdf_clean(&r.path);
    let rest = strings_and_xmp(&r.path).to_lowercase();
    assert!(!rest.contains("secretword"), "{rest}");
    assert!(rest.contains("doc about") && rest.contains("chapter") && rest.contains("note"), "only the match goes: {rest}");
    assert!(!pdfium_text(&r.path).to_lowercase().contains("secretword"));
    // The match's own text is enough (the app passes no typed text).
    let r2 = redact::apply(&pdf, &redact::mark_text(&hits), &[], &t.path().join("out2.pdf")).unwrap();
    assert!(!strings_and_xmp(&r2.path).to_lowercase().contains("secretword"));
}

#[test]
fn a_whole_page_mark_leaves_nothing_of_the_page_not_even_outside_the_media_box() {
    let t = tempfile::tempdir().unwrap();
    let pdf = Input::new(raw_pdf(
        &t.path().join("off.pdf"),
        &[
            ("<< /Type /Catalog /Pages 2 0 R >>", None),
            ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>", None),
            ("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> /Annots [6 0 R] >>", None),
            ("<< >>", Some(b"BT /F1 14 Tf 72 760 Td (On page text) Tj ET BT /F1 14 Tf 700 400 Td (offpage secretword) Tj ET")),
            (HELV, None),
            ("<< /Type /Annot /Subtype /Text /Rect [400 400 420 420] /Contents (a note) >>", None),
        ],
        None,
    ));
    let r = redact::apply(&pdf, &redact::mark_pages(&pdf, &[0]).unwrap(), &[], &t.path().join("out.pdf")).unwrap();
    qpdf_clean(&r.path);
    let d = lopdf::Document::load(&r.path).unwrap();
    let raw: Vec<u8> = d.objects.values().filter_map(|o| o.as_stream().ok()).flat_map(|s| s.decompressed_content().unwrap_or_else(|_| s.content.clone())).collect();
    assert!(!String::from_utf8_lossy(&raw).contains("secretword") && !String::from_utf8_lossy(&raw).contains("On page"), "content kept");
    assert!(pdfium_text(&r.path).trim().is_empty());
    let page = d.get_dictionary(d.get_pages()[&1]).unwrap();
    assert!(!page.has(b"Annots") && !strings_and_xmp(&r.path).contains("a note"));
    let img = render(&r.path, 0);
    assert!(common::dark_in(&img, 0.0, 1.0, 0.0, 1.0) > (img.0 * img.1) as usize * 9 / 10);
}

#[test]
fn text_left_under_a_mark_in_a_form_xobject_fails_the_independent_check() {
    let t = tempfile::tempdir().unwrap();
    // The text is in a Form XObject, the black box is drawn on the page: a redaction that only
    // looks done, which PdfCraft's audit (page content only) does not see.
    let faux = raw_pdf(
        &t.path().join("xobj.pdf"),
        &[
            ("<< /Type /Catalog /Pages 2 0 R >>", None),
            ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>", None),
            ("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /XObject << /X1 6 0 R >> >> >>", None),
            ("<< >>", Some(b"q /X1 Do Q 0 0 0 rg 68 695 250 20 re f")),
            (HELV, None),
            ("<< /Type /XObject /Subtype /Form /BBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> >>", Some(b"BT /F1 14 Tf 72 700 Td (Account 4242 secret) Tj ET")),
        ],
        None,
    );
    assert!(pdfium_text(&faux).contains("secret"), "fixture");
    let e = redact::verify(&faux, &redact::mark_rects(&[(0, [68.0, 695.0, 318.0, 715.0])])).unwrap_err().to_string();
    assert!(e.contains("text is still under a black box on page 1") && e.contains("secret"), "{e}");
    // A real redaction of the same area passes.
    let r = redact::apply(&Input::new(&faux), &redact::mark_rects(&[(0, [68.0, 695.0, 318.0, 715.0])]), &[], &t.path().join("out.pdf")).unwrap();
    assert!(!pdfium_text(&r.path).contains("secret"));
}

#[test]
fn a_failed_audit_writes_nothing_not_even_for_a_moment_under_the_final_name() {
    let t = tempfile::tempdir().unwrap();
    let faux = faux_pdf(t.path());
    let out = t.path().join("out.pdf");
    // The input's own box lies away from the mark, so the audit fails after the redaction.
    let e = redact::apply(&Input::new(&faux), &redact::mark_rects(&[(0, [300.0, 100.0, 400.0, 120.0])]), &[], &out).unwrap_err().to_string();
    assert_eq!(e, "This PDF already hides text under a box on page 1. Mark that area too to remove it.");
    let left: Vec<_> = std::fs::read_dir(t.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left, ["faux.pdf"], "nothing written, no .part left");
    // A search mark over part of the box ("secret") does not make it ours: "Account 4242"
    // is still hidden under the input's box only.
    let secret = redact::find(&Input::new(&faux), &Query::Literal { text: "secret".into(), case_insensitive: false }).unwrap();
    let e = redact::apply(&Input::new(&faux), &redact::mark_text(&secret), &[], &out).unwrap_err().to_string();
    assert_eq!(e, "This PDF already hides text under a box on page 1. Mark that area too to remove it.");
    assert!(!out.exists());
    // Marking that area too removes it.
    let r = redact::apply(&Input::new(&faux), &redact::mark_rects(&[(0, [68.0, 695.0, 218.0, 715.0])]), &[], &out).unwrap();
    assert!(!pdfium_text(&r.path).contains("secret"));
}


// ---- Fix round 2 -------------------------------------------------------------------------------

/// A page "Name secretword here about" / "Other text" (object 4, font object 5); `extra` goes
/// into the page dictionary (object 3). Objects from 6 on are `more`.
fn probe(dir: &Path, name: &str, catalog: &str, page_extra: &str, more: &[(&str, Option<&[u8]>)], info: Option<usize>) -> Input {
    let cat = format!("<< /Type /Catalog /Pages 2 0 R {catalog} >>");
    let page = format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> {page_extra} >>");
    let mut objs: Vec<(&str, Option<&[u8]>)> = vec![
        (&cat, None),
        ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>", None),
        (&page, None),
        ("<< >>", Some(b"BT /F1 14 Tf 72 760 Td (Name secretword here about) Tj ET BT /F1 14 Tf 72 600 Td (Other text) Tj ET")),
        (HELV, None),
    ];
    objs.extend_from_slice(more);
    Input::new(raw_pdf(&dir.join(name), &objs, info))
}

fn xmp_packet(inner: &str) -> Vec<u8> {
    format!("<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">{inner}</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>").into_bytes()
}

fn secret_marks(pdf: &Input, word: &str) -> Vec<redact::Mark> {
    redact::mark_text(&redact::find(pdf, &Query::Literal { text: word.into(), case_insensitive: true }).unwrap())
}

#[test]
fn xmp_holding_the_text_in_any_spelling_is_removed_whole_with_a_warning() {
    let t = tempfile::tempdir().unwrap();
    let cases: [(&str, &str); 4] = [
        ("ncr", "<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">&#115;ecret&#x77;ord</rdf:li></rdf:Alt></dc:title>"),
        ("comment", "<dc:description>secret<!-- x -->word</dc:description>"),
        ("cdata", "<dc:description>secret<![CDATA[word]]></dc:description>"),
        ("plain", "<dc:title>About secretword</dc:title>"),
    ];
    for (name, inner) in cases {
        let xmp = xmp_packet(inner);
        let pdf = probe(t.path(), &format!("{name}.pdf"), "/Metadata 6 0 R", "", &[("<< /Type /Metadata /Subtype /XML >>", Some(&xmp))], None);
        let r = redact::apply(&pdf, &secret_marks(&pdf, "secretword"), &[], &t.path().join(format!("{name}-out.pdf"))).unwrap();
        assert_eq!(r.warnings, ["XMP metadata removed: it contained the redacted text"], "{name}");
        let d = lopdf::Document::load(&r.path).unwrap();
        assert!(!d.catalog().unwrap().has(b"Metadata"), "{name}");
        assert!(!d.objects.values().any(|o| o.as_stream().is_ok_and(|s| s.dict.has_type(b"Metadata"))), "{name}");
        qpdf_clean(&r.path);
    }
    // XMP without the text stays as it is.
    let xmp = xmp_packet("<dc:title>Report</dc:title>");
    let pdf = probe(t.path(), "keep.pdf", "/Metadata 6 0 R", "", &[("<< /Type /Metadata /Subtype /XML >>", Some(&xmp))], None);
    let r = redact::apply(&pdf, &secret_marks(&pdf, "secretword"), &[], &t.path().join("keep-out.pdf")).unwrap();
    assert!(r.warnings.is_empty());
    assert!(strings_and_xmp(&r.path).contains("<dc:title>Report</dc:title>"));
}

#[test]
fn only_displayed_text_values_are_scrubbed_names_links_and_field_names_survive() {
    let t = tempfile::tempdir().unwrap();
    let xmp = xmp_packet("<dc:title>Report</dc:title>");
    let pdf = probe(
        t.path(),
        "corrupt.pdf",
        "/Metadata 6 0 R /Names << /Dests 7 0 R >> /AcroForm << /Fields [9 0 R] /DA (/Helv 0 Tf 0 g) >>",
        "/Annots [8 0 R 9 0 R 10 0 R]",
        &[
            ("<< /Type /Metadata /Subtype /XML >>", Some(&xmp)),
            ("<< /Names [(aboutpage) [3 0 R /Fit]] >>", None),
            ("<< /Type /Annot /Subtype /Link /Rect [72 500 200 520] /A << /S /URI /URI (https://example.org/about) >> >>", None),
            ("<< /Type /Annot /Subtype /Widget /FT /Tx /T (about_field) /Rect [72 400 200 420] /V (keep about) /TU (Tell about) /DA (/Helv 0 Tf 0 g) /P 3 0 R >>", None),
            ("<< /Type /Annot /Subtype /Link /Rect [72 300 200 320] /Dest (aboutpage) >>", None),
        ],
        None,
    );
    let r = redact::apply(&pdf, &secret_marks(&pdf, "about"), &[], &t.path().join("out.pdf")).unwrap();
    qpdf_clean(&r.path);
    // Written with object streams too (as PdfCraft or another tool may), the check reads
    // their objects one by one, not the packed bytes where names and URIs sit side by side.
    let packed = t.path().join("packed.pdf");
    let o = std::process::Command::new("qpdf").args(["--object-streams=generate"]).arg(&r.path).arg(&packed).output().unwrap();
    assert!(o.status.success());
    let again = redact::apply(&Input::new(&packed), &redact::mark_rects(&[(0, [300.0, 100.0, 310.0, 110.0])]), &["about".into()], &t.path().join("again.pdf"));
    assert!(again.is_ok(), "{again:?}");
    let d = lopdf::Document::load(&r.path).unwrap();
    let all = strings_and_xmp(&r.path);
    for kept in ["rdf:about=\"\"", "aboutpage", "https://example.org/about", "about_field", "/Helv 0 Tf 0 g"] {
        assert!(all.contains(kept), "{kept:?} lost: {all}");
    }
    assert!(!all.contains("keep about") && all.contains("keep") && !all.contains("Tell about"), "field value and tooltip scrubbed: {all}");
    // The links still resolve: pdfium follows the named destination and the URI.
    let _g = pdfium_lock();
    let doc = sheaf_core::engine::open_pdf(&r.path, None).unwrap();
    let page = doc.pages().get(0).unwrap();
    let mut uri = None;
    let mut dest_page = None;
    for l in page.links().iter() {
        if let Some(a) = l.action()
            && let Some(u) = a.as_uri_action()
        {
            uri = u.uri().ok();
        }
        if let Some(dst) = l.destination() {
            dest_page = dst.page_index().ok();
        }
    }
    assert_eq!(uri.as_deref(), Some("https://example.org/about"));
    assert_eq!(dest_page, Some(0), "the named destination aboutpage resolves to page 1");
    assert!(d.catalog().unwrap().has(b"Names"));
}

#[test]
fn attachments_and_scripts_with_the_text_are_removed_and_named() {
    let t = tempfile::tempdir().unwrap();
    let pdf = probe(
        t.path(),
        "embed.pdf",
        "/Names << /EmbeddedFiles << /Names [(a.txt) 6 0 R (b.txt) 8 0 R] >> /JavaScript << /Names [(init) 10 0 R] >> >>",
        "",
        &[
            ("<< /Type /Filespec /F (a.txt) /UF (a.txt) /EF << /F 7 0 R >> >>", None),
            ("<< /Type /EmbeddedFile >>", Some(b"the secretword is here")),
            ("<< /Type /Filespec /F (b.txt) /EF << /F 9 0 R >> >>", None),
            ("<< /Type /EmbeddedFile >>", Some(b"nothing to see")),
            ("<< /S /JavaScript /JS 11 0 R >>", None),
            ("<< >>", Some(b"app.alert('secretword');")),
        ],
        None,
    );
    let r = redact::apply(&pdf, &secret_marks(&pdf, "secretword"), &[], &t.path().join("out.pdf")).unwrap();
    assert_eq!(
        r.warnings,
        ["Attachment a.txt removed: it contained the redacted text", "JavaScript in an action removed: it contained the redacted text"],
    );
    qpdf_clean(&r.path);
    let d = lopdf::Document::load(&r.path).unwrap();
    let streams: Vec<String> = d.objects.values().filter_map(|o| o.as_stream().ok()).map(|s| String::from_utf8_lossy(&s.decompressed_content().unwrap_or_else(|_| s.content.clone())).into_owned()).collect();
    assert!(!streams.iter().any(|s| s.to_lowercase().contains("secretword")), "{streams:?}");
    assert!(streams.iter().any(|s| s == "nothing to see"), "the other attachment stays");
}

#[test]
fn an_emptied_value_loses_its_key_and_every_encoding_is_read() {
    let t = tempfile::tempdir().unwrap();
    let u16: String = "Doc SecretWord".encode_utf16().map(|u| format!("{u:04X}")).collect();
    let u8b: String = "secretword".bytes().map(|b| format!("{b:02X}")).collect();
    let info = format!("<< /Title <FEFF{u16}> /Subject <EFBBBF{u8b}> /Keywords (secr\\145tword) /Author (secret\nword) /Creator (Writer) >>");
    let pdf = probe(t.path(), "info.pdf", "", "", &[(&info, None)], Some(6));
    let r = redact::apply(&pdf, &secret_marks(&pdf, "secretword"), &[], &t.path().join("out.pdf")).unwrap();
    qpdf_clean(&r.path);
    let d = lopdf::Document::load(&r.path).unwrap();
    let info = d.get_dictionary(d.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();
    let keys: Vec<String> = info.iter().map(|(k, _)| String::from_utf8_lossy(k).into_owned()).collect();
    for gone in ["Subject", "Keywords", "Author"] {
        assert!(!keys.iter().any(|k| k == gone), "emptied {gone} is removed, not left as () or a bare BOM: {keys:?}");
    }
    assert!(keys.iter().any(|k| k == "Creator"), "{keys:?}");
    assert_eq!(lopdf::decode_text_string(info.get(b"Title").unwrap()).unwrap(), "Doc ");
}

#[test]
fn decomposed_and_composed_accents_match_each_other() {
    let t = tempfile::tempdir().unwrap();
    // The page draws "Élodie" in WinAnsi (one character); the title holds "E" + U+0301.
    let title: String = "E\u{301}lodie Secretword".encode_utf16().map(|u| format!("{u:04X}")).collect();
    let info = format!("<< /Title <FEFF{title}> >>");
    let objs: Vec<(&str, Option<&[u8]>)> = vec![
        ("<< /Type /Catalog /Pages 2 0 R >>", None),
        ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>", None),
        ("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>", None),
        ("<< >>", Some(b"BT /F1 14 Tf 72 760 Td (Name \xc9lodie here) Tj ET")),
        ("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>", None),
        (&info, None),
    ];
    let pdf = Input::new(raw_pdf(&t.path().join("nfd.pdf"), &objs, Some(6)));
    // Typed decomposed, as some keyboards and pastes give it.
    let typed = "E\u{301}lodie".to_string();
    let hits = redact::find(&pdf, &Query::Literal { text: typed.clone(), case_insensitive: true }).unwrap();
    assert_eq!(hits.len(), 1, "the decomposed search finds the composed page text");
    let r = redact::apply(&pdf, &redact::mark_text(&hits), &[typed], &t.path().join("out.pdf")).unwrap();
    let all = strings_and_xmp(&r.path);
    assert!(!all.contains("lodie"), "{all}");
    assert!(all.contains("Secretword"), "only the match goes: {all}");
    assert!(!pdfium_text(&r.path).contains("lodie"));
}

#[test]
fn outline_children_are_scrubbed_too() {
    let t = tempfile::tempdir().unwrap();
    let pdf = probe(
        t.path(),
        "outline.pdf",
        "/Outlines 6 0 R",
        "",
        &[
            ("<< /Type /Outlines /First 7 0 R /Last 7 0 R /Count 3 >>", None),
            ("<< /Title (Parent secretword) /Parent 6 0 R /First 8 0 R /Last 9 0 R /Count 2 /Dest [3 0 R /Fit] >>", None),
            ("<< /Title (Child one secretword) /Parent 7 0 R /Next 9 0 R /Dest [3 0 R /Fit] >>", None),
            ("<< /Title <FEFF00630068006900200053004500430052004500540057004F00520044> /Parent 7 0 R /Prev 8 0 R /Dest [3 0 R /Fit] >>", None),
        ],
        None,
    );
    let r = redact::apply(&pdf, &secret_marks(&pdf, "secretword"), &[], &t.path().join("out.pdf")).unwrap();
    let all = strings_and_xmp(&r.path);
    assert!(!all.to_lowercase().contains("secretword") && all.contains("Parent") && all.contains("Child one") && all.contains("chi"), "{all}");
}

// ---- Fix round 3: deny-by-default audit ---------------------------------------------------------

/// Redacts "secretword" on the probe page; Ok with the warnings, or the error text.
fn redact_secret(t: &Path, pdf: &Input, name: &str) -> std::result::Result<(PathBuf, Vec<String>), String> {
    let out = t.join(format!("{name}-out.pdf"));
    match redact::apply(pdf, &secret_marks(pdf, "secretword"), &[], &out) {
        Ok(r) => Ok((r.path, r.warnings)),
        Err(e) => {
            assert!(!out.exists(), "{name}: a refused run wrote its output");
            Err(e.to_string())
        }
    }
}

/// Every string, name and decoded stream of the file, object streams opened, as one text.
fn everything(path: &Path) -> String {
    fn walk(o: &lopdf::Object, out: &mut String) {
        match o {
            lopdf::Object::String(..) => out.push_str(&lopdf::decode_text_string(o).unwrap_or_else(|_| String::from_utf8_lossy(o.as_str().unwrap()).into_owned())),
            lopdf::Object::Name(n) => out.push_str(&String::from_utf8_lossy(n)),
            lopdf::Object::Array(a) => a.iter().for_each(|x| walk(x, out)),
            lopdf::Object::Dictionary(d) => d.iter().for_each(|(_, v)| walk(v, out)),
            lopdf::Object::Stream(s) => {
                s.dict.iter().for_each(|(_, v)| walk(v, out));
                out.push_str(&String::from_utf8_lossy(&s.decompressed_content().unwrap_or_else(|_| s.content.clone())));
            }
            _ => {}
        }
        out.push('\n');
    }
    let d = lopdf::Document::load(path).unwrap();
    let mut out = String::new();
    d.objects.values().for_each(|o| walk(o, &mut out));
    out
}

/// A probe either removes the text (with `warning` when given) or is refused with nothing
/// written; it never ships the text.
fn removed_or_refused(t: &Path, pdf: &Input, name: &str, warning: Option<&str>) -> Option<Vec<String>> {
    match redact_secret(t, pdf, name) {
        Ok((p, w)) => {
            let all = everything(&p).to_lowercase();
            assert!(!all.contains("secretword"), "{name}: text shipped: {all}");
            if let Some(x) = warning {
                assert!(w.iter().any(|y| y == x), "{name}: {w:?}");
            }
            qpdf_clean(&p);
            Some(w)
        }
        Err(e) => {
            assert!(e.contains("redaction audit failed"), "{name}: {e}");
            None
        }
    }
}

#[test]
fn a_form_hung_off_catalog_piece_info_is_removed() {
    let t = tempfile::tempdir().unwrap();
    let pdf = probe(
        t.path(),
        "piece.pdf",
        "/PieceInfo << /App << /Private 6 0 R /LastModified (D:2026) >> >>",
        "",
        &[("<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] /Resources << /Font << /F1 5 0 R >> >> >>", Some(b"BT /F1 9 Tf (old secretword) Tj ET"))],
        None,
    );
    let w = removed_or_refused(t.path(), &pdf, "piece", None).expect("removed, not refused");
    assert_eq!(w, ["Private application data removed: it contained the redacted text"]);
}

#[test]
fn streams_shaped_like_fonts_or_icc_are_checked_unless_a_page_uses_them() {
    let t = tempfile::tempdir().unwrap();
    // Unreached streams that only look like a font program (/Length1) or an ICC profile (/N),
    // hung off the catalog.
    for (name, dict) in [("length1", "<< /Length1 22 >>"), ("icc", "<< /N 3 >>")] {
        let pdf = probe(t.path(), &format!("{name}.pdf"), "/Extra 6 0 R", "", &[(dict, Some(b"stash: secretword here."))], None);
        let e = redact_secret(t.path(), &pdf, name).unwrap_err();
        assert!(e.contains("redaction audit failed: a redacted text is still in the document (stream content"), "{name}: {e}");
    }
    // A real ICC stream a page draws with is exempt (its bytes are a profile).
    let pdf = probe(
        t.path(),
        "used-icc.pdf",
        "",
        "",
        &[("<< /N 1 /Alternate /DeviceGray >>", Some(b"profile desc secretword")), ("<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace [/ICCBased 6 0 R] >>", Some(b"\x00"))],
        None,
    );
    // The page paints the image.
    let mut d = lopdf::Document::load(&pdf.path).unwrap();
    let page = d.get_pages()[&1];
    d.get_object_mut(page).unwrap().as_dict_mut().unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("XObject", lopdf::dictionary! { "Im" => (7, 0) });
    let c = d.get_dictionary(page).unwrap().get(b"Contents").unwrap().as_reference().unwrap();
    let s = d.get_object_mut(c).unwrap().as_stream_mut().unwrap();
    let mut body = s.content.clone();
    body.extend(b" q 10 0 0 10 300 300 cm /Im Do Q");
    s.set_plain_content(body);
    d.save(&pdf.path).unwrap();
    redact_secret(t.path(), &pdf, "used-icc").unwrap();
}

#[test]
fn an_unpainted_form_in_the_resources_is_removed() {
    let t = tempfile::tempdir().unwrap();
    let objs_page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /XObject << /Unused 6 0 R >> >> >>";
    let pdf = Input::new(raw_pdf(
        &t.path().join("unpainted.pdf"),
        &[
            ("<< /Type /Catalog /Pages 2 0 R >>", None),
            ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>", None),
            (objs_page, None),
            ("<< >>", Some(b"BT /F1 14 Tf 72 760 Td (Name secretword here about) Tj ET")),
            (HELV, None),
            ("<< /Type /XObject /Subtype /Form /BBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> >>", Some(b"BT /F1 14 Tf 72 300 Td (never shown secretword) Tj ET")),
        ],
        None,
    ));
    let w = removed_or_refused(t.path(), &pdf, "unpainted", None).expect("removed, not refused");
    assert_eq!(w, ["Hidden content removed: it contained the redacted text"]);
    // A form the page does paint is left to the redaction itself.
    let painted = Input::new(raw_pdf(
        &t.path().join("painted.pdf"),
        &[
            ("<< /Type /Catalog /Pages 2 0 R >>", None),
            ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>", None),
            ("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /XObject << /X 6 0 R >> >> >>", None),
            ("<< >>", Some(b"q /X Do Q")),
            (HELV, None),
            ("<< /Type /XObject /Subtype /Form /BBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> >>", Some(b"BT /F1 14 Tf 72 300 Td (shown secretword) Tj ET")),
        ],
        None,
    ));
    let (p, w) = redact_secret(t.path(), &painted, "painted").unwrap();
    assert!(w.is_empty() && pdfium_text(&p).contains("shown") && !everything(&p).contains("secretword"), "{w:?}");
}

#[test]
fn metadata_typed_only_as_xml_is_xmp() {
    let t = tempfile::tempdir().unwrap();
    let xmp = xmp_packet("<dc:title>secret<!-- -->word</dc:title>");
    let pdf = probe(t.path(), "xml.pdf", "/Metadata 6 0 R", "", &[("<< /Subtype /XML >>", Some(&xmp))], None);
    let w = removed_or_refused(t.path(), &pdf, "xml", None).expect("removed, not refused");
    assert_eq!(w, ["XMP metadata removed: it contained the redacted text"]);
    // A stream that is only the target of a /Metadata key (no /Type, no /Subtype).
    let pdf = probe(t.path(), "bare.pdf", "/Metadata 6 0 R", "", &[("<< >>", Some(&xmp))], None);
    let w = removed_or_refused(t.path(), &pdf, "bare", None).expect("removed, not refused");
    assert_eq!(w, ["XMP metadata removed: it contained the redacted text"]);
}

#[test]
fn info_values_nested_in_arrays_and_names_are_scrubbed() {
    let t = tempfile::tempdir().unwrap();
    let pdf = probe(t.path(), "info.pdf", "", "", &[("<< /Title (Kept) /Arr [(secretword) (other)] /Tag /secretword /Deep << /X (a secretword b) >> >>", None)], Some(6));
    let (p, _) = redact_secret(t.path(), &pdf, "info").unwrap();
    let d = lopdf::Document::load(&p).unwrap();
    let info = d.get_dictionary(d.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();
    assert!(!info.has(b"Tag"), "a name holding the text loses its key");
    assert_eq!(info.get(b"Arr").unwrap().as_array().unwrap().len(), 2, "array positions kept");
    assert!(!everything(&p).to_lowercase().contains("secretword"));
    assert!(everything(&p).contains("other") && everything(&p).contains("Kept"));
}

#[test]
fn choice_options_and_widget_captions_are_scrubbed() {
    let t = tempfile::tempdir().unwrap();
    let pdf = probe(
        t.path(),
        "opt.pdf",
        "/AcroForm << /Fields [6 0 R 7 0 R] >>",
        "/Annots [6 0 R 7 0 R]",
        &[
            ("<< /Type /Annot /Subtype /Widget /FT /Ch /T (pick) /Rect [72 400 200 420] /Opt [[(e1) (secretword)] (plain) (x secretword)] /V (plain) /P 3 0 R >>", None),
            ("<< /Type /Annot /Subtype /Widget /FT /Btn /T (go) /Rect [72 300 200 320] /MK << /CA (Send secretword) /RC (secretword) /AC (Up) >> /P 3 0 R >>", None),
        ],
        None,
    );
    let (p, _) = redact_secret(t.path(), &pdf, "opt").unwrap();
    let all = everything(&p);
    assert!(!all.to_lowercase().contains("secretword"), "{all}");
    for kept in ["e1", "plain", "Send", "Up", "pick", "go"] {
        assert!(all.contains(kept), "{kept} lost");
    }
    let d = lopdf::Document::load(&p).unwrap();
    let opt = d.objects.values().filter_map(|o| o.as_dict().ok()).find_map(|x| x.get(b"Opt").ok()).unwrap().as_array().unwrap().clone();
    assert_eq!(opt.len(), 3, "option positions kept");
}

#[test]
fn page_piece_info_holding_the_text_is_removed() {
    let t = tempfile::tempdir().unwrap();
    let pdf = probe(t.path(), "pagepiece.pdf", "", "/PieceInfo << /App << /Private (draft secretword) /LastModified (D:2026) >> >>", &[], None);
    let w = removed_or_refused(t.path(), &pdf, "pagepiece", None).expect("removed, not refused");
    assert_eq!(w, ["Private application data removed: it contained the redacted text"]);
    // Private data without the text stays.
    let pdf = probe(t.path(), "pagepiece2.pdf", "", "/PieceInfo << /App << /Private (draft) >> >>", &[], None);
    let (p, w) = redact_secret(t.path(), &pdf, "pagepiece2").unwrap();
    assert!(w.is_empty() && everything(&p).contains("draft"));
}

#[test]
fn the_audit_refuses_text_anywhere_it_does_not_exempt() {
    // Deny by default: a stash under an unknown key of the catalog, as a string, a name, or
    // inside a packed object stream, fails with nothing written.
    let t = tempfile::tempdir().unwrap();
    for (name, cat) in [("string", "/Extra (stash secretword)"), ("name", "/Extra /secretword"), ("nested", "/Extra [<< /Y [(x) (secretword)] >>]")] {
        let pdf = probe(t.path(), &format!("deny-{name}.pdf"), cat, "", &[], None);
        let e = redact_secret(t.path(), &pdf, name).unwrap_err();
        assert!(e.contains("redaction audit failed: a redacted text is still in the document"), "{name}: {e}");
    }
    let pdf = probe(t.path(), "deny-packed.pdf", "/Extra 6 0 R", "", &[("<< /Note (stash secretword) >>", None)], None);
    let packed = t.path().join("packed-in.pdf");
    let o = std::process::Command::new("qpdf").args(["--object-streams=generate"]).arg(&pdf.path).arg(&packed).output().unwrap();
    assert!(o.status.success());
    let e = redact_secret(t.path(), &Input::new(&packed), "packed").unwrap_err();
    assert!(e.contains("redaction audit failed"), "{e}");
}
