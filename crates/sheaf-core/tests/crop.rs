mod common;
use common::{fixture_pdf, page_texts, qpdf_clean, render};
use sheaf_core::crop::{crop, CropSpec};
use sheaf_core::pages::Input;

fn crop_box(path: &std::path::Path, page: u32) -> Option<Vec<f32>> {
    let d = lopdf::Document::load(path).unwrap();
    let id = d.get_pages()[&page];
    let dict = d.get_dictionary(id).unwrap();
    dict.get(b"CropBox").ok().map(|a| a.as_array().unwrap().iter().map(|v| v.as_float().unwrap()).collect())
}

const MM10: f32 = 10.0 * 72.0 / 25.4;
/// The fixture's A4 media box, in points.
const W: f32 = 595.2756;
const H: f32 = 841.88983;

fn close(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01)
}

#[test]
fn margins_set_a_crop_box_on_all_pages() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let spec = CropSpec::Margins { top: MM10 as f64, bottom: MM10 as f64, left: MM10 as f64, right: MM10 as f64 };
    let (out, warned) = crop(&Input::new(&a), None, &spec, &t.path().join("c.pdf")).unwrap();
    assert!(!warned);
    qpdf_clean(&out);
    for p in [1, 2] {
        let b = crop_box(&out, p).expect("CropBox");
        assert!((b[2] - b[0] - (W - 2.0 * MM10)).abs() < 1.0 && (b[3] - b[1] - (H - 2.0 * MM10)).abs() < 1.0, "{b:?}");
    }
    assert_eq!(page_texts(&out), ["Page 1", "Page 2"]);
}

#[test]
fn selected_pages_only_and_remove_restores() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let spec = CropSpec::Margins { top: 50.0, bottom: 0.0, left: 0.0, right: 0.0 };
    let (out, _) = crop(&Input::new(&a), Some("2"), &spec, &t.path().join("c.pdf")).unwrap();
    assert_eq!(crop_box(&out, 1), None);
    assert!((crop_box(&out, 2).unwrap()[3] - (H - 50.0)).abs() < 0.01);
    assert_eq!(crop_box(&out, 3), None);
    let (back, _) = crop(&Input::new(&out), None, &CropSpec::Remove, &t.path().join("r.pdf")).unwrap();
    qpdf_clean(&back);
    for p in 1..=3 {
        let b = crop_box(&back, p);
        assert!(b.as_deref().is_none_or(|b| close(b, &[0.0, 0.0, W, H])), "page {p}: {b:?}");
    }
    assert_eq!(render(&back, 1).1, H.round() as u32);
}

#[test]
fn margins_follow_what_the_reader_sees_on_a_rotated_page() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let r = sheaf_core::rotate::rotate(&Input::new(&a), None, 90, &t.path().join("r.pdf")).unwrap();
    // Shown landscape (842 × 595): trim 100 pt off the visible top and 40 off the visible left.
    let spec = CropSpec::Margins { top: 100.0, bottom: 0.0, left: 40.0, right: 0.0 };
    let (out, _) = crop(&Input::new(&r), None, &spec, &t.path().join("c.pdf")).unwrap();
    qpdf_clean(&out);
    let (w, h, _) = render(&out, 0);
    assert!((w as i32 - 802).abs() <= 1 && (h as i32 - 495).abs() <= 1, "{w}×{h}");
    // Rotated 90° clockwise, the visible top is user-space left and the visible left is user-space bottom.
    let b = crop_box(&out, 1).unwrap();
    assert!(close(&b, &[100.0, 40.0, W, H]), "{b:?}");
}

#[test]
fn white_margins_cut_each_page_to_what_is_drawn() {
    let t = tempfile::tempdir().unwrap();
    // "Page 1" in Helvetica 24 at (72, 720) on an A4 page: about 70 × 17 pt of ink.
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let (out, _) = crop(&Input::new(&a), None, &CropSpec::WhiteMargins { pad: 3.0 }, &t.path().join("w.pdf")).unwrap();
    for page in [1, 2] {
        let b = crop_box(&out, page).expect("a crop box");
        assert!(b[0] > 60.0 && b[0] < 72.0 && b[1] > 705.0 && b[1] < 722.0, "page {page}: {b:?}");
        assert!(b[2] - b[0] > 60.0 && b[2] - b[0] < 110.0 && b[3] - b[1] > 15.0 && b[3] - b[1] < 35.0, "page {page}: {b:?}");
    }
    assert_eq!(page_texts(&out), ["Page 1", "Page 2"]);
}
