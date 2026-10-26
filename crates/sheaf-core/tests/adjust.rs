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
