mod common;
use common::{dark_in, fixture_pdf, page_texts, qpdf_clean, render};
use sheaf_core::numbers::{number_pages, standard_font, Bates, NumberOptions, NumberText};
use sheaf_core::pages::Input;

#[test]
fn numbers_every_page_in_order() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 4);
    let o = NumberOptions { start: 101, ..Default::default() };
    let (out, warned) = number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")).unwrap();
    assert!(!warned);
    qpdf_clean(&out);
    let texts = page_texts(&out);
    for (i, s) in texts.iter().enumerate() {
        assert!(s.contains(&format!("Page {}", i + 1)), "original text kept: {s}");
        assert!(s.contains(&(101 + i).to_string()), "page {} numbered: {s}", i + 1);
    }
}

#[test]
fn cover_page_is_skipped_and_numbering_starts_on_page_two() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let o = NumberOptions { cover: true, text: NumberText::Custom("Folio {n} of {p}".into()), ..Default::default() };
    let (out, _) = number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")).unwrap();
    qpdf_clean(&out);
    let texts = page_texts(&out);
    assert_eq!(texts[0], "Page 1");
    assert!(texts[1].contains("Folio 1 of 3"), "{}", texts[1]);
    assert!(texts[2].contains("Folio 2 of 3"), "{}", texts[2]);
}

#[test]
fn page_templates_and_a_page_range() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 4);
    let o = NumberOptions { text: NumberText::PageOfTotal, from: Some(2), to: Some(3), ..Default::default() };
    let (out, _) = number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")).unwrap();
    let texts = page_texts(&out);
    assert_eq!(texts[0], "Page 1");
    assert!(texts[1].contains("Page 1 of 4"), "{}", texts[1]);
    assert!(texts[2].contains("Page 2 of 4"), "{}", texts[2]);
    assert_eq!(texts[3], "Page 4");
}

#[test]
fn bad_range_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let o = NumberOptions { from: Some(3), ..Default::default() };
    assert!(number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")).is_err());
    let o = NumberOptions { position: 4, ..Default::default() };
    assert!(number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")).is_err(), "middle row is not a number position");
}

#[test]
fn double_sided_mirrors_left_and_right_on_even_pages() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    // Bottom-left on odd pages, so bottom-right on even ones.
    let o = NumberOptions { position: 6, double_sided: true, font_size: 30.0, ..Default::default() };
    let (out, _) = number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")).unwrap();
    qpdf_clean(&out);
    let (p1, p2) = (render(&out, 0), render(&out, 1));
    assert!(dark_in(&p1, 0.0, 0.5, 0.85, 1.0) > 20 && dark_in(&p1, 0.5, 1.0, 0.85, 1.0) == 0, "page 1: number bottom-left");
    assert!(dark_in(&p2, 0.5, 1.0, 0.85, 1.0) > 20 && dark_in(&p2, 0.0, 0.5, 0.85, 1.0) == 0, "page 2: number bottom-right");
    let texts = page_texts(&out);
    assert!(texts[0].ends_with('1') && texts[1].ends_with('2'), "{texts:?}");
}

#[test]
fn protected_input_gives_an_unprotected_output_and_warns() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let p = sheaf_core::security::protect(&Input::new(&a), "pw", None, &t.path().join("p.pdf")).unwrap();
    let input = Input { path: p, password: Some("pw".into()) };
    let (out, warned) = number_pages(&input, &NumberOptions::default(), &t.path().join("n.pdf")).unwrap();
    assert!(warned);
    qpdf_clean(&out);
    assert!(page_texts(&out)[0].contains('1'));
}

#[test]
fn custom_text_cannot_smuggle_pdfcraft_tokens() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    for bad in ["<<m/d/yyyy>>", "Page {n} <<", ">> {n}"] {
        let o = NumberOptions { text: NumberText::Custom(bad.into()), ..Default::default() };
        match number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")) {
            Err(sheaf_core::OpdfError::Unsupported(m)) => assert_eq!(m, "custom text cannot contain << or >>"),
            other => panic!("{bad}: {:?}", other.map(|_| ())),
        }
    }
    assert!(!t.path().join("n.pdf").exists());
}

#[test]
fn from_page_zero_is_an_invalid_range() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    for cover in [false, true] {
        let o = NumberOptions { from: Some(0), cover, ..Default::default() };
        assert!(matches!(number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")), Err(sheaf_core::OpdfError::InvalidRange { .. })));
    }
}

/// The fonts a PDF uses, as pdffonts lists them.
fn fonts(path: &std::path::Path) -> String {
    String::from_utf8_lossy(&std::process::Command::new("pdffonts").arg(path).output().unwrap().stdout).into_owned()
}

#[test]
fn numbers_take_the_chosen_font() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    assert_eq!(standard_font(0, false, false), None);
    let o = NumberOptions { font: standard_font(1, true, true), ..Default::default() };
    let (out, _) = number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")).unwrap();
    qpdf_clean(&out);
    assert!(fonts(&out).contains("Times-BoldItalic"), "{}", fonts(&out));
    assert!(page_texts(&out)[1].contains('2'));
    // A font's names are structure, not text: redacting the word "Bold" passes the audit of a
    // file whose numbers are set in Times-Bold.
    let o = NumberOptions { font: standard_font(1, true, false), text: NumberText::Custom("Bold {n}".into()), ..Default::default() };
    let (out, _) = number_pages(&Input::new(&a), &o, &t.path().join("b.pdf")).unwrap();
    let hits = sheaf_core::redact::find(&Input::new(&out), &sheaf_core::redact::Query::Literal { text: "Bold".into(), case_insensitive: false, whole_word: true }).unwrap();
    assert_eq!(hits.len(), 2);
    sheaf_core::redact::apply(&Input::new(&out), &sheaf_core::redact::mark_text(&hits), &[], &t.path().join("r.pdf")).unwrap();
}

#[test]
fn numbers_can_be_underlined_in_a_custom_colour() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    assert_eq!(sheaf_core::numbers::parse_hex_colour("#1F6FEB"), Some([31.0 / 255.0, 111.0 / 255.0, 235.0 / 255.0]));
    for bad in ["", "12345", "GGGGGG", "1234567"] {
        assert_eq!(sheaf_core::numbers::parse_hex_colour(bad), None, "{bad:?}");
    }
    // The underline is the only filled rectangle the numbering adds, and it is in the colour.
    let rects = |underline: bool| {
        let o = NumberOptions { underline, color: [1.0, 0.0, 0.0], ..Default::default() };
        let (out, _) = number_pages(&Input::new(&a), &o, &t.path().join(format!("u{underline}.pdf"))).unwrap();
        let qdf = std::process::Command::new("qpdf").args(["--qdf", "--object-streams=disable"]).arg(&out).arg("-").output().unwrap().stdout;
        let qdf = String::from_utf8_lossy(&qdf).into_owned();
        (qdf.matches(" re f").count(), qdf.contains("1 0 0 rg"))
    };
    let (plain, red) = rects(false);
    assert!(red);
    assert_eq!(rects(true).0, plain + 1, "one underline");
}

#[test]
fn a_range_past_the_end_stops_at_the_last_page_when_clamped() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let o = NumberOptions { from: Some(2), to: Some(9), ..Default::default() };
    assert!(number_pages(&Input::new(&a), &o, &t.path().join("e.pdf")).is_err());
    let o = NumberOptions { clamp_to_last: true, ..o };
    let (out, _) = number_pages(&Input::new(&a), &o, &t.path().join("c.pdf")).unwrap();
    let texts = page_texts(&out);
    assert_eq!(texts[0].matches('1').count(), 1, "page 1 not numbered: {}", texts[0]);
    assert!(texts[2].contains('2'), "page 3 numbered (2): {}", texts[2]);
}

#[test]
fn header_and_footer_text_and_bates_numbers_without_a_page_number() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let mut texts: [String; 6] = Default::default();
    texts[0] = "Case {bates}".into();
    texts[5] = "Sheet {n} of {p}".into();
    let o = NumberOptions { text: NumberText::Nothing, texts, bates: Bates { digits: 4, start: 7, prefix: "ACME-".into(), suffix: String::new() }, ..Default::default() };
    let (out, _) = number_pages(&Input::new(&a), &o, &t.path().join("n.pdf")).unwrap();
    qpdf_clean(&out);
    let pages = page_texts(&out);
    assert!(pages[0].contains("Case ACME-0007") && pages[0].contains("Sheet 1 of 2"), "{}", pages[0]);
    assert!(pages[1].contains("Case ACME-0008"), "{}", pages[1]);
    // Again with Replace: the first header and footer go, so each page holds one Bates number.
    let o2 = NumberOptions { replace: true, ..o.clone() };
    let (again, _) = number_pages(&Input::new(&out), &o2, &t.path().join("n2.pdf")).unwrap();
    assert_eq!(page_texts(&again)[0].matches("ACME-").count(), 1, "{}", page_texts(&again)[0]);
    // Double-sided (each page its own edit): the Bates numbers still run on (Sol, 26/10: all 0007).
    let o3 = NumberOptions { double_sided: true, ..o.clone() };
    let (two, _) = number_pages(&Input::new(&a), &o3, &t.path().join("n3.pdf")).unwrap();
    let pages = page_texts(&two);
    assert!(pages[0].contains("ACME-0007") && pages[1].contains("ACME-0008"), "{pages:?}");
    // Nothing at all to write is refused.
    let empty = NumberOptions { text: NumberText::Nothing, ..Default::default() };
    assert!(number_pages(&Input::new(&a), &empty, &t.path().join("e.pdf")).is_err());
}
