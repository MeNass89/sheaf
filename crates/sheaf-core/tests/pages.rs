mod common;
use common::{fixture_pdf, page_texts, shown_texts};
use sheaf_core::{pages::*, OpdfError};

#[test]
fn merge_concatenates_in_order() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let b = fixture_pdf(t.path(), "b.pdf", 1);
    let out = merge(&[Input::new(&b), Input::new(&a)], &t.path().join("m.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 1", "Page 1", "Page 2"]);
}
#[test]
fn merge_takes_the_pages_chosen_for_each_file() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 4);
    let b = fixture_pdf(t.path(), "b.pdf", 3);
    // a: pages 4, then 1-2 (in the order written); b: every page (an empty choice).
    let out = merge_pages(&[Input::new(&a), Input::new(&b)], &["4, 1-2".into(), String::new()], &t.path().join("m.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 4", "Page 1", "Page 2", "Page 1", "Page 2", "Page 3"]);
    let e = merge_pages(&[Input::new(&a)], &["5".into()], &t.path().join("x.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::InvalidRange { pages: 4, .. }), "{e:?}");
    assert!(!t.path().join("x.pdf").exists());
}
#[test]
fn assemble_mixes_the_pages_of_several_files_in_the_order_given() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 4);
    let b = fixture_pdf(t.path(), "b.pdf", 2);
    let inputs = [Input::new(&a), Input::new(&b)];
    // b's page 2, a's page 3, b's page 1, a's page 4: a's pages 1 and 2 left out.
    let out = assemble(&inputs, &[(1, 2), (0, 3), (1, 1), (0, 4)], &t.path().join("m.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 2", "Page 3", "Page 1", "Page 4"]);
    let e = assemble(&inputs, &[(1, 3)], &t.path().join("x.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::InvalidRange { pages: 2, .. }), "{e:?}");
    assert!(!t.path().join("x.pdf").exists());
}
#[test]
fn a_pdf_is_made_from_a_pasted_picture_or_text() {
    let t = tempfile::tempdir().unwrap();
    let rgba = [200u8, 30, 30, 255].repeat(30 * 20);
    let pic = from_pasted(&Pasted::Image { width: 30, height: 20, rgba }, &t.path().join("p.pdf")).unwrap();
    assert_eq!(page_texts(&pic).len(), 1);
    let text = from_pasted(&Pasted::Text("Hello from the clipboard\nSecond line".into()), &t.path().join("t.pdf")).unwrap();
    assert!(page_texts(&text)[0].contains("Hello from the clipboard"), "{:?}", page_texts(&text));
    assert!(from_pasted(&Pasted::Text("  ".into()), &t.path().join("e.pdf")).is_err());
}
#[test]
fn pages_change_size_with_their_content_scaled_to_fit() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    // Page 2 only, to A5: half the area of A4.
    let out = resize(&Input::new(&a), Some("2"), (420.0, 595.0), &t.path().join("r.pdf")).unwrap();
    let doc = lopdf::Document::load(&out).unwrap();
    let media = |n: u32| {
        let id = doc.get_pages()[&n];
        doc.get_dictionary(id).unwrap().get(b"MediaBox").unwrap().as_array().unwrap().iter().map(|o| o.as_float().unwrap_or(0.0)).collect::<Vec<f32>>()
    };
    assert_eq!(media(2), [0.0, 0.0, 420.0, 595.0]);
    assert_ne!(media(1), media(2), "page 1 kept its size");
    // The text is still there, smaller.
    assert_eq!(page_texts(&out), ["Page 1", "Page 2", "Page 3"]);
    assert!(resize(&Input::new(&a), None, (0.0, 10.0), &t.path().join("x.pdf")).is_err());
}
#[test]
fn a_flipped_page_is_its_mirror_image() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    // "Page 1" starts 72 pt from the left: mirrored, its text ends 72 pt from the right.
    let out = flip(&Input::new(&a), Some("1"), true, &t.path().join("f.pdf")).unwrap();
    let x = |path: &std::path::Path, page: i32| {
        let _g = sheaf_core::engine::pdfium_lock();
        let doc = sheaf_core::engine::open_pdf(path, None).unwrap();
        let p = doc.pages().get(page).unwrap();
        let t = p.text().unwrap();
        t.chars().iter().filter(|c| c.unicode_string().is_some_and(|s| !s.trim().is_empty())).map(|c| c.loose_bounds().unwrap()).fold((f32::MAX, f32::MIN), |(a, b), r| (a.min(r.left().value), b.max(r.right().value)))
    };
    let (l1, r1) = x(&out, 0);
    let (l2, _) = x(&out, 1);
    assert!((595.0 - r1 - 72.0).abs() < 3.0 && l1 > 300.0, "page 1 mirrored: {l1}..{r1}");
    assert!((l2 - 72.0).abs() < 3.0, "page 2 left alone: {l2}");
}
#[test]
fn a_cropped_page_is_mirrored_within_what_shows() {
    // Cropped to its left half: mirrored about the media box, "Page 1" would land outside it
    // (Sol, 26/10).
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let spec = sheaf_core::crop::CropSpec::Margins { top: 0.0, bottom: 0.0, left: 0.0, right: 300.0 };
    let (cropped, _) = sheaf_core::crop::crop(&Input::new(&a), None, &spec, &t.path().join("c.pdf")).unwrap();
    let out = flip(&Input::new(&cropped), None, true, &t.path().join("f.pdf")).unwrap();
    let o = std::process::Command::new("pdftotext").arg("-cropbox").arg(&out).arg("-").output().unwrap();
    // Its letters, mirrored, are still within the crop box.
    let shown = String::from_utf8_lossy(&o.stdout).chars().filter(|c| c.is_alphanumeric()).count();
    assert!(shown >= 5, "{:?}", String::from_utf8_lossy(&o.stdout));
}

#[test]
fn a_cropped_page_changes_size_from_what_shows_and_keeps_its_margin_hidden() {
    // Cropped to its right half ("Page 1", at the left, hidden), then put on landscape A4: what
    // shows is scaled into the middle, and the hidden text must not come back in the side margin
    // (Sol, 26/10: resizing started from the media box).
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let spec = sheaf_core::crop::CropSpec::Margins { top: 0.0, bottom: 0.0, left: 300.0, right: 0.0 };
    let (cropped, _) = sheaf_core::crop::crop(&Input::new(&a), None, &spec, &t.path().join("c.pdf")).unwrap();
    let out = resize(&Input::new(&cropped), None, (842.0, 595.0), &t.path().join("r.pdf")).unwrap();
    let img = common::render(&out, 0);
    assert_eq!(common::dark_in(&img, 0.0, 1.0, 0.0, 1.0), 0, "the cropped-away text came back");
}

#[test]
fn a_flipped_page_takes_its_comments_with_it() {
    use pdfium_render::prelude::*;
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let marked = t.path().join("m.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let doc = sheaf_core::engine::open_pdf(&a, None).unwrap();
        let mut page = doc.pages().get(0).unwrap();
        let mut square = page.annotations_mut().create_square_annotation().unwrap();
        square.set_bounds(PdfRect::new_from_values(700.0, 60.0, 740.0, 160.0)).unwrap();
        doc.save_to_file(&marked).unwrap();
    }
    // Drawn into the page, it is mirrored with it: no comment is left at its old place.
    let out = flip(&Input::new(&marked), None, true, &t.path().join("f.pdf")).unwrap();
    let doc = lopdf::Document::load(&out).unwrap();
    let page = doc.get_dictionary(doc.get_pages()[&1]).unwrap();
    let annots = page.get(b"Annots").ok().and_then(|a| doc.dereference(a).ok()).and_then(|(_, a)| a.as_array().ok().cloned()).unwrap_or_default();
    assert!(annots.is_empty(), "{annots:?}");
}

#[test]
fn a_page_changed_in_size_takes_its_comments_with_it() {
    use pdfium_render::prelude::*;
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let marked = t.path().join("m.pdf");
    {
        let _g = sheaf_core::engine::pdfium_lock();
        let doc = sheaf_core::engine::open_pdf(&a, None).unwrap();
        let mut page = doc.pages().get(0).unwrap();
        let mut square = page.annotations_mut().create_square_annotation().unwrap();
        square.set_bounds(PdfRect::new_from_values(700.0, 60.0, 740.0, 160.0)).unwrap();
        doc.save_to_file(&marked).unwrap();
    }
    // Drawn into the page, it is scaled with it: no comment is left at its old place.
    let out = resize(&Input::new(&marked), None, (420.0, 595.0), &t.path().join("r.pdf")).unwrap();
    let doc = lopdf::Document::load(&out).unwrap();
    let page = doc.get_dictionary(doc.get_pages()[&1]).unwrap();
    let annots = page.get(b"Annots").ok().and_then(|a| doc.dereference(a).ok()).and_then(|(_, a)| a.as_array().ok().cloned()).unwrap_or_default();
    assert!(annots.is_empty(), "{annots:?}");
}
#[test]
fn a_booklet_prints_its_pages_so_that_folded_they_read_in_order() {
    assert_eq!(booklet_order(8), [8, 1, 2, 7, 6, 3, 4, 5]);
    // Five pages: padded to eight with blanks.
    assert_eq!(booklet_order(5), [0, 1, 2, 0, 0, 3, 4, 5]);
    assert_eq!(booklet_order(0).len(), 4);
}
#[test]
fn several_pages_go_on_each_sheet() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 5);
    let four = impose(&Input::new(&a), 4, false, &t.path().join("4.pdf")).unwrap();
    assert_eq!(shown_texts(&four).len(), 2);
    assert!(shown_texts(&four)[0].contains("Page 1") && shown_texts(&four)[0].contains("Page 4"), "{:?}", shown_texts(&four));
    let book = impose(&Input::new(&a), 2, true, &t.path().join("b.pdf")).unwrap();
    // Eight pages, two to a side: four sides; the first holds pages 8 (blank) and 1.
    assert_eq!(shown_texts(&book).len(), 4);
    assert!(shown_texts(&book)[0].contains("Page 1") && !shown_texts(&book)[0].contains("Page 2"), "{:?}", shown_texts(&book));
    assert!(impose(&Input::new(&a), 3, false, &t.path().join("x.pdf")).is_err());
    // Sixteen (CUPS number-up=16, Preview's 16 per sheet): the five pages on one sheet.
    let sixteen = impose(&Input::new(&a), 16, false, &t.path().join("16.pdf")).unwrap();
    assert_eq!(shown_texts(&sixteen).len(), 1);
    assert!(shown_texts(&sixteen)[0].contains("Page 1") && shown_texts(&sixteen)[0].contains("Page 5"), "{:?}", shown_texts(&sixteen));
}
#[test]
fn each_page_is_cut_into_its_two_halves() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let out = halves(&Input::new(&a), true, &t.path().join("h.pdf")).unwrap();
    // Four pages: page 1's left half holds its words (at x 72), its right half none.
    assert_eq!(shown_texts(&out), ["Page 1", "", "Page 2", ""]);
    let down = halves(&Input::new(&a), false, &t.path().join("v.pdf")).unwrap();
    // Top halves first: the words sit near the top (y 720 of 842).
    assert_eq!(shown_texts(&down), ["Page 1", "", "Page 2", ""]);
}
#[test]
fn every_page_goes_on_one_long_page() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let out = long_page(&Input::new(&a), &t.path().join("l.pdf")).unwrap();
    assert_eq!(shown_texts(&out).len(), 1);
    assert!(shown_texts(&out)[0].contains("Page 1") && shown_texts(&out)[0].contains("Page 3"), "{:?}", shown_texts(&out));
    // 20 A4 pages would pass the 200-inch limit.
    let many = fixture_pdf(t.path(), "m.pdf", 20);
    assert!(long_page(&Input::new(&many), &t.path().join("m-l.pdf")).is_err());
}

#[test]
fn each_page_is_cut_into_a_grid() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    // 2 × 3: six parts, the words ("Page 1" at 72, 720: top left) in the first only.
    let out = cut(&Input::new(&a), 2, 3, &t.path().join("g.pdf")).unwrap();
    assert_eq!(shown_texts(&out), ["Page 1", "", "", "", "", ""]);
    assert!(cut(&Input::new(&a), 1, 1, &t.path().join("x.pdf")).is_err());
}

#[test]
fn split_ranges_writes_one_file_per_range() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 5);
    let outs = split_ranges(&Input::new(&a), "1-2,5", t.path()).unwrap();
    assert_eq!(outs.len(), 2);
    assert!(outs[0].ends_with("a-1-2.pdf"));
    assert_eq!(page_texts(&outs[1]), ["Page 5"]);
}
#[test]
fn split_every_chunks() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 5);
    let outs = split_every(&Input::new(&a), 2, t.path()).unwrap();
    assert_eq!(outs.iter().map(|p| page_texts(p).len()).collect::<Vec<_>>(), [2, 2, 1]);
}
#[test]
fn remove_drops_pages() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 4);
    let out = remove(&Input::new(&a), "2-3", &t.path().join("r.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 1", "Page 4"]);
}
#[test]
fn remove_all_pages_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    let e = remove(&Input::new(&a), "1-2", &t.path().join("r.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::WouldBeEmpty { pages: 2 }), "{e:?}");
    assert_eq!(e.to_string(), "cannot remove every page (the document has 2 pages)");
}
#[test]
fn organize_reorders_duplicates_and_inserts_blank() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let out = organize(&Input::new(&a), &[3, 0, 1, 1], &t.path().join("o.pdf")).unwrap();
    assert_eq!(page_texts(&out), ["Page 3", "", "Page 1", "Page 1"]);
}
#[test]
fn load_encrypted_without_password_is_password_required() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    let enc = t.path().join("e.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "u", "u", "256", "--"]).arg(&a).arg(&enc).status().unwrap().success());
    let e = merge(&[Input::new(&enc)], &t.path().join("m.pdf")).unwrap_err();
    assert!(matches!(e, OpdfError::PasswordRequired(_)));
    let ok = merge(&[Input { path: enc, password: Some("u".into()) }], &t.path().join("m.pdf")).unwrap();
    assert_eq!(page_texts(&ok), ["Page 1"]);
}

#[test]
fn concurrent_merges_do_not_crash() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 2);
    std::thread::scope(|s| {
        for th in 0..8 {
            let (a, dir) = (&a, t.path());
            s.spawn(move || {
                for i in 0..20 {
                    let out = merge(&[Input::new(a), Input::new(a)], &dir.join(format!("m-{th}-{i}.pdf"))).unwrap();
                    assert_eq!(lopdf::Document::load(&out).unwrap().get_pages().len(), 4);
                }
            });
        }
    });
}

#[test]
fn split_every_huge_n_gives_one_file() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 3);
    let outs = split_every(&Input::new(&a), u32::MAX, t.path()).unwrap();
    assert_eq!(outs.len(), 1);
    assert_eq!(page_texts(&outs[0]).len(), 3);
}
#[test]
fn encryption_is_detected_including_owner_only() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 1);
    assert!(!Input::new(&a).is_encrypted());
    let owner_only = t.path().join("o.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "", "owner", "256", "--"]).arg(&a).arg(&owner_only).status().unwrap().success());
    assert!(Input::new(&owner_only).is_encrypted());
    let user = t.path().join("u.pdf");
    assert!(std::process::Command::new("qpdf").args(["--encrypt", "u", "u", "128", "--use-aes=y", "--"]).arg(&a).arg(&user).status().unwrap().success());
    assert!(Input::new(&user).is_encrypted());
    assert!(Input { path: user, password: Some("u".into()) }.is_encrypted());
}

#[test]
fn split_by_top_level_bookmarks_names_each_file_after_its_bookmark() {
    use lopdf::{dictionary, Object};
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 5);
    // Bookmarks "Intro" on page 2 and "Methods" on page 4 (page 1 goes with Intro).
    let mut d = lopdf::Document::load(&a).unwrap();
    let pages = d.get_pages();
    let (p2, p4) = (pages[&2], pages[&4]);
    let outlines = d.new_object_id();
    let (i1, i2) = (d.new_object_id(), d.new_object_id());
    d.objects.insert(i1, Object::Dictionary(dictionary! { "Title" => Object::string_literal("Intro"), "Parent" => outlines, "Next" => i2, "Dest" => vec![p2.into(), "Fit".into()] }));
    d.objects.insert(i2, Object::Dictionary(dictionary! { "Title" => Object::string_literal("Methods/Results"), "Parent" => outlines, "Prev" => i1, "Dest" => vec![p4.into(), "Fit".into()] }));
    d.objects.insert(outlines, Object::Dictionary(dictionary! { "Type" => "Outlines", "First" => i1, "Last" => i2, "Count" => 2 }));
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_object_mut(root).unwrap().as_dict_mut().unwrap().set("Outlines", outlines);
    let marked = t.path().join("marked.pdf");
    d.save(&marked).unwrap();
    let out = t.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let files = split_bookmarks(&Input::new(&marked), &out).unwrap();
    let names: Vec<String> = files.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["marked-1 Intro.pdf", "marked-2 Methods Results.pdf"]);
    assert_eq!((page_texts(&files[0]).len(), page_texts(&files[1]).len()), (3, 2));
    // Fewer than two bookmarks: said, nothing written.
    assert!(split_bookmarks(&Input::new(&a), &out).unwrap_err().to_string().contains("fewer than two"));
}

#[test]
fn split_by_size_groups_pages_under_the_limit() {
    let t = tempfile::tempdir().unwrap();
    let a = fixture_pdf(t.path(), "a.pdf", 6);
    let (one, each) = (t.path().join("one"), t.path().join("each"));
    std::fs::create_dir(&one).unwrap();
    std::fs::create_dir(&each).unwrap();
    assert_eq!(split_size(&Input::new(&a), 100 << 20, &one).unwrap().len(), 1, "all under 100 MB: one file");
    assert_eq!(split_size(&Input::new(&a), 1, &each).unwrap().len(), 6, "each page over 1 byte: a file each");
    // Pages over the limit alone are said, not hidden.
    let again = t.path().join("again");
    std::fs::create_dir(&again).unwrap();
    let (files, over) = split_size_report(&Input::new(&a), 1, &again).unwrap();
    assert_eq!((files.len(), &over[..]), (6, &[1, 2, 3, 4, 5, 6][..]));
    assert_eq!(oversize_note(&over[..1]), "Page 1 alone is larger than the size asked: its file is too");
    assert!(oversize_note(&over).starts_with("6 pages alone") && oversize_note(&[]).is_empty());
}

#[test]
fn blank_pages_are_found_and_text_or_drawn_ones_are_not() {
    use sheaf_core::images::{images_to_pdf, PageLayout};
    let t = tempfile::tempdir().unwrap();
    let text = fixture_pdf(t.path(), "text.pdf", 2);
    let (white, drawn, grey) = (t.path().join("white.png"), t.path().join("drawn.png"), t.path().join("grey.png"));
    image::RgbImage::from_pixel(60, 80, image::Rgb([255, 255, 255])).save(&white).unwrap();
    image::RgbImage::from_fn(60, 80, |x, _| if x < 30 { image::Rgb([20, 20, 20]) } else { image::Rgb([255, 255, 255]) }).save(&drawn).unwrap();
    // An empty page scanned on grey paper.
    image::RgbImage::from_pixel(60, 80, image::Rgb([215, 215, 215])).save(&grey).unwrap();
    let pics = images_to_pdf(&[white, drawn, grey], PageLayout::FitImage, &t.path().join("pics.pdf")).unwrap();
    let all = merge(&[Input::new(&text), Input::new(&pics)], &t.path().join("all.pdf")).unwrap();
    assert_eq!(blank_pages(&Input::new(&all), 1).unwrap(), [3], "the white picture only");
    // Scans: the grey page is blank too; strict or not, the drawn one never is.
    assert_eq!(blank_pages(&Input::new(&all), 2).unwrap(), [3, 5]);
    assert_eq!(blank_pages(&Input::new(&all), 0).unwrap(), [3]);
}

#[test]
fn a_blank_document_is_one_empty_page_of_the_size_chosen() {
    let t = tempfile::tempdir().unwrap();
    for (letter, size) in [(false, [595, 842]), (true, [612, 792])] {
        let out = blank(letter, &t.path().join("Untitled.pdf")).unwrap();
        common::qpdf_clean(&out);
        let d = lopdf::Document::load(&out).unwrap();
        let page = *d.get_pages().values().next().unwrap();
        let mb = d.get_dictionary(page).unwrap().get(b"MediaBox").unwrap().as_array().unwrap().iter().map(|v| v.as_float().unwrap().round() as i64).collect::<Vec<_>>();
        assert_eq!(mb[2..], size, "letter {letter}");
        assert_eq!(page_texts(&out), [""]);
    }
}

#[test]
fn letter_paper_is_for_the_regions_that_use_it() {
    for l in ["en-US", "fr_CA.UTF-8", "es_MX", "en_PH"] {
        assert!(letter_region(Some(l)), "{l}");
    }
    for l in ["en-GB", "fr_FR.UTF-8", "de", "C", ""] {
        assert!(!letter_region(Some(l)), "{l}");
    }
    assert!(!letter_region(None));
}
