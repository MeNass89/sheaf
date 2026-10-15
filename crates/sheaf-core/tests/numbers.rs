mod common;
use common::{dark_in, fixture_pdf, page_texts, qpdf_clean, render};
use sheaf_core::numbers::{number_pages, NumberOptions, NumberText};
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
