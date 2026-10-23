//! Editing an existing paragraph in place (PdfCraft's EditTextBlock), as the reader's Edit tool does.

use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::engine::{BlockStyle, Edit};
use sheaf_core::pdfcraft::Doc;
use std::path::{Path, PathBuf};

/// One page holding a Word-like paragraph: rows of runs in Helvetica and Helvetica-Bold, shown
/// with TJ kerning; a row whose run ends mid-word at the end of the first content stream and goes
/// on in a second run placed with Td (as Word writes them; Courier, so the second starts where the
/// first ends); then another paragraph.
fn mixed_paragraph(dir: &Path) -> PathBuf {
    let content = b"BT /F2 1 Tf 10 0 0 10 72 700 Tm [(Sympto)6.6 (ms)] TJ /F1 1 Tf [( : pain, diar)4.2 (rhoea, weight loss)] TJ ET
BT /F1 1 Tf 10 0 0 10 72 688 Tm [(- General )] TJ /F2 1 Tf [(sig)5 (ns)] TJ /F1 1 Tf [(: fever and tired)3.9 (ness in many patients)] TJ ET
BT /F3 1 Tf 10 0 0 10 72 676 Tm [(- Bowel: il)]"
        .to_vec();
    // The page's second content stream goes on with the operator of the first's last array.
    let rest = b" TJ 0.001 Tc 6.6 0 Td [(iac fossa)] TJ ET
BT /F1 1 Tf 10 0 0 10 72 600 Tm (Another paragraph further down) Tj ET"
        .to_vec();
    page_pdf(dir, "mixed.pdf", vec![content, rest])
}

/// A Letter page drawing `streams` (its content, in pieces), with Helvetica as F1, Helvetica-Bold
/// as F2 and Courier as F3.
fn page_pdf(dir: &Path, name: &str, streams: Vec<Vec<u8>>) -> PathBuf {
    use lopdf::{Dictionary, Object, Stream};
    let font = |base: &str| {
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Font".to_vec()));
        d.set("Subtype", Object::Name(b"Type1".to_vec()));
        d.set("BaseFont", Object::Name(base.as_bytes().to_vec()));
        d.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        Object::Dictionary(d)
    };
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let streams: Vec<Object> = streams.into_iter().map(|c| Object::Reference(doc.add_object(Stream::new(Dictionary::new(), c)))).collect();
    let mut fonts = Dictionary::new();
    fonts.set("F1", font("Helvetica"));
    fonts.set("F2", font("Helvetica-Bold"));
    fonts.set("F3", font("Courier"));
    let mut res = Dictionary::new();
    res.set("Font", Object::Dictionary(fonts));
    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages));
    page.set("Contents", Object::Array(streams));
    page.set("Resources", Object::Dictionary(res));
    page.set("MediaBox", Object::Array([0, 0, 612, 792].iter().map(|n| Object::Integer(*n)).collect()));
    let page = doc.add_object(page);
    let mut kids = Dictionary::new();
    kids.set("Type", Object::Name(b"Pages".to_vec()));
    kids.set("Kids", Object::Array(vec![Object::Reference(page)]));
    kids.set("Count", Object::Integer(1));
    doc.objects.insert(pages, Object::Dictionary(kids));
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages));
    let catalog = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog));
    let path = dir.join(name);
    doc.save(&path).unwrap();
    path
}

/// The paragraph holding `word`: its index, editor text and plain text.
fn block_with(doc: &Doc, word: &str) -> (usize, String, String) {
    let blocks = doc.session().get(doc.id()).unwrap().text_blocks(0);
    let (i, b) = blocks.iter().enumerate().find(|(_, b)| b.text.contains(word)).unwrap_or_else(|| panic!("no paragraph holds {word:?}"));
    (i, b.editor_text.clone(), b.text.clone())
}

#[test]
fn a_mixed_style_paragraph_edited_twice_stays_one_paragraph() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Doc::open(&Input::new(mixed_paragraph(dir.path()))).unwrap();
    let n0 = doc.session().get(doc.id()).unwrap().text_blocks(0).len();
    for insert in ["Été ", "ñü "] {
        let (i, editor, _) = block_with(&doc, "Sympto");
        // Typed into the middle of a word of the second row, as a reader would.
        let at = editor.find("tired").unwrap_or_else(|| panic!("{editor:?}")) + "tired".len();
        let text = format!("{}{insert}{}", &editor[..at], &editor[at..]);
        doc.apply(Edit::EditTextBlock { page: 0, block: i, text, style: BlockStyle::default() }).unwrap();
        let (_, _, plain) = block_with(&doc, "Sympto");
        for word in ["Symptoms", "weight", "General", "signs", insert.trim(), "patients", "Bowel", "iliac", "fossa"] {
            assert!(plain.contains(word), "after inserting {insert:?}, the paragraph lost {word:?}: {plain:?}");
        }
        assert_eq!(doc.session().get(doc.id()).unwrap().text_blocks(0).len(), n0, "the paragraph split: {plain:?}");
    }
}


/// One line in a subset font that holds only ASCII (its widths stop at `~`).
fn ascii_subset(dir: &Path) -> PathBuf {
    use lopdf::{Dictionary, Object, Stream};
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let stream = doc.add_object(Stream::new(Dictionary::new(), b"BT /F1 12 Tf 72 700 Td (Plain cafe menu) Tj ET".to_vec()));
    let mut font = Dictionary::new();
    font.set("Type", Object::Name(b"Font".to_vec()));
    font.set("Subtype", Object::Name(b"TrueType".to_vec()));
    font.set("BaseFont", Object::Name(b"ABCDEF+Arial".to_vec()));
    font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
    font.set("FirstChar", Object::Integer(32));
    font.set("LastChar", Object::Integer(126));
    font.set("Widths", Object::Array(vec![Object::Integer(500); 95]));
    let mut fonts = Dictionary::new();
    fonts.set("F1", Object::Dictionary(font));
    let mut res = Dictionary::new();
    res.set("Font", Object::Dictionary(fonts));
    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages));
    page.set("Contents", Object::Reference(stream));
    page.set("Resources", Object::Dictionary(res));
    page.set("MediaBox", Object::Array([0, 0, 612, 792].iter().map(|n| Object::Integer(*n)).collect()));
    let page = doc.add_object(page);
    let mut kids = Dictionary::new();
    kids.set("Type", Object::Name(b"Pages".to_vec()));
    kids.set("Kids", Object::Array(vec![Object::Reference(page)]));
    kids.set("Count", Object::Integer(1));
    doc.objects.insert(pages, Object::Dictionary(kids));
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages));
    let catalog = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog));
    let path = dir.join("subset.pdf");
    doc.save(&path).unwrap();
    path
}

#[test]
fn a_character_the_subset_lacks_is_drawn_in_a_standard_font_and_said() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Doc::open(&Input::new(ascii_subset(dir.path()))).unwrap();
    let (i, _, _) = block_with(&doc, "cafe");
    doc.apply(Edit::EditTextBlock { page: 0, block: i, text: "Plain cafe menu".into(), style: BlockStyle::default() }).unwrap();
    assert_eq!(doc.document().font_substituted, None, "every character is in the subset");
    doc.apply(Edit::EditTextBlock { page: 0, block: i, text: "Plain café menu".into(), style: BlockStyle::default() }).unwrap();
    assert_eq!(doc.document().font_substituted.as_deref(), Some("Helvetica"));
    assert!(block_with(&doc, "café").2.contains("Plain café menu"));
}

/// A Courier paragraph (6 pt a character) wrapped at 40 characters, then a two-item list.
fn wrapped_paragraph(dir: &Path) -> PathBuf {
    use lopdf::{Dictionary, Object, Stream};
    let rows = ["The quick brown fox jumps over the lazy", "dog and then runs far away into the deep", "forest."];
    let mut content = String::new();
    for (i, row) in rows.iter().enumerate() {
        content += &format!("BT /F1 10 Tf 72 {} Td ({row}) Tj ET\n", 700 - 12 * i);
    }
    content += "BT /F1 10 Tf 72 600 Td (- first item) Tj ET\nBT /F1 10 Tf 72 588 Td (- second item) Tj ET\n";
    let mut doc = lopdf::Document::with_version("1.7");
    let pages = doc.new_object_id();
    let stream = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
    let mut font = Dictionary::new();
    font.set("Type", Object::Name(b"Font".to_vec()));
    font.set("Subtype", Object::Name(b"Type1".to_vec()));
    font.set("BaseFont", Object::Name(b"Courier".to_vec()));
    font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
    let mut fonts = Dictionary::new();
    fonts.set("F1", Object::Dictionary(font));
    let mut res = Dictionary::new();
    res.set("Font", Object::Dictionary(fonts));
    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages));
    page.set("Contents", Object::Reference(stream));
    page.set("Resources", Object::Dictionary(res));
    page.set("MediaBox", Object::Array([0, 0, 612, 792].iter().map(|n| Object::Integer(*n)).collect()));
    let page = doc.add_object(page);
    let mut kids = Dictionary::new();
    kids.set("Type", Object::Name(b"Pages".to_vec()));
    kids.set("Kids", Object::Array(vec![Object::Reference(page)]));
    kids.set("Count", Object::Integer(1));
    doc.objects.insert(pages, Object::Dictionary(kids));
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages));
    let catalog = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog));
    let path = dir.join("wrapped.pdf");
    doc.save(&path).unwrap();
    path
}

#[test]
fn wraps_are_told_from_typed_breaks_and_an_edit_rewraps_the_whole_paragraph() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Doc::open(&Input::new(wrapped_paragraph(dir.path()))).unwrap();
    let blocks = doc.session().get(doc.id()).unwrap().text_blocks(0);
    let para = blocks.iter().position(|b| b.text.contains("quick")).unwrap();
    assert_eq!(blocks[para].soft_breaks, [true, true], "the paragraph's rows were wrapped");
    let list = blocks.iter().find(|b| b.text.contains("first item")).unwrap();
    assert!(list.soft_breaks.iter().all(|s| !s), "list items start their own lines: {:?}", list.soft_breaks);
    // "very " typed into the first row: the words flow on through the rows below, as the editor
    // sends it (wraps as spaces).
    let text = "The very quick brown fox jumps over the lazy dog and then runs far away into the deep forest.";
    doc.apply(Edit::EditTextBlock { page: 0, block: para, text: text.into(), style: BlockStyle::default() }).unwrap();
    let (_, editor, _) = block_with(&doc, "quick");
    let rows: Vec<&str> = editor.lines().map(str::trim).collect();
    assert_eq!(rows, ["The very quick brown fox jumps over the", "lazy dog and then runs far away into the", "deep forest."], "{editor:?}");
}

/// The font resource each word of page 1 is shown in (from the saved file's content).
fn word_fonts(bytes: &[u8]) -> Vec<(String, String)> {
    let doc = lopdf::Document::load_mem(bytes).unwrap();
    let page = *doc.get_pages().get(&1).unwrap();
    let content = lopdf::content::Content::decode(&doc.get_page_content(page)).unwrap();
    let mut font = String::new();
    let mut out = Vec::new();
    for op in content.operations {
        match op.operator.as_str() {
            "Tf" => font = op.operands[0].as_name().map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default(),
            "Tj" | "TJ" => {
                let strings: Vec<&lopdf::Object> = match &op.operands[0] {
                    lopdf::Object::Array(a) => a.iter().collect(),
                    o => vec![o],
                };
                for s in strings {
                    if let lopdf::Object::String(b, _) = s {
                        for w in String::from_utf8_lossy(b).split_whitespace() {
                            out.push((w.to_string(), font.clone()));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn two_edits_apart_keep_the_styles_between_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Doc::open(&Input::new(mixed_paragraph(dir.path()))).unwrap();
    let (i, editor, _) = block_with(&doc, "Sympto");
    // One word changed in the first row, one in the last: "signs" (bold) lies between them.
    let text = editor.replace("pain, diar", "ache, diar").replace("fossa", "fossae");
    doc.apply(Edit::EditTextBlock { page: 0, block: i, text, style: BlockStyle::default() }).unwrap();
    let words = word_fonts(&doc.session().save_full_bytes(doc.id()).unwrap());
    let font_of = |w: &str| words.iter().find(|(x, _)| x.starts_with(w)).map(|(_, f)| f.clone()).unwrap_or_else(|| panic!("{w} not drawn: {words:?}"));
    assert_eq!(font_of("signs"), "F2", "{words:?}");
    assert_eq!(font_of("General"), "F1");
    assert_eq!(font_of("ache"), "F1");
}


#[test]
fn text_cut_between_two_content_streams_is_exported_where_it_is_drawn() {
    // The fixture's last row ends its first content stream with a TJ's operands; the operator
    // starts the second stream, followed by the rest of the row ("iac fossa").
    let dir = tempfile::tempdir().unwrap();
    let doc = Doc::open(&Input::new(mixed_paragraph(dir.path()))).unwrap();
    let pages = doc.document().export_pages();
    let b = pages[0].blocks.iter().find(|b| b.text.contains("fossa")).expect("drawn text is exported");
    assert!((b.size - 10.0).abs() < 0.01, "size {} (was 1: text space)", b.size);
    assert!(b.rect[0] >= 71.0 && b.rect[1] > 600.0, "where it is drawn: {:?}", b.rect);
}

#[test]
fn a_space_before_a_word_in_another_style_is_kept_in_the_export() {
    // "- General " (regular, its space drawn last) then "signs" (bold).
    let dir = tempfile::tempdir().unwrap();
    let doc = Doc::open(&Input::new(mixed_paragraph(dir.path()))).unwrap();
    let pages = doc.document().export_pages();
    let b = pages[0].blocks.iter().find(|b| b.text.contains("General")).unwrap();
    assert!(b.text.ends_with(' '), "{:?}", b.text);
    let out = dir.path().join("mixed.docx");
    sheaf_core::export::to_office(&Input::new(dir.path().join("mixed.pdf")), sheaf_core::export::Office::Word, &Default::default(), &out, &mut |_| true).unwrap();
    let xml = String::from_utf8(std::process::Command::new("unzip").arg("-p").arg(&out).arg("word/document.xml").output().unwrap().stdout).unwrap();
    let text: String = xml.split("<w:t").skip(1).filter_map(|t| t.split_once('>').and_then(|(_, r)| r.split_once("</w:t>")).map(|(t, _)| t.to_string())).collect();
    assert!(text.contains("General signs"), "{text}");
    // One paragraph holds the regular and the bold run (the word in bold stays in its sentence).
    assert!(xml.split("</w:p>").any(|p| p.contains(">- General ") && p.contains("<w:b/>") && p.contains(">signs")), "{xml}");
}

#[test]
fn a_row_drawn_in_one_string_across_cells_goes_into_each_cell() {
    // Word draws a table row's cells, side by side and in one style, as one string whose spaces are
    // stretched by word spacing (Tw) to reach the next cell, the rest of a cell placed with Td:
    // "Tabagisme | Facteur de risque".
    let dir = tempfile::tempdir().unwrap();
    let mut grid = String::new();
    for x in [72, 200, 400] {
        grid.push_str(&format!("{x} 680 0.5 40 re f "));
    }
    for y in [680, 700, 720] {
        grid.push_str(&format!("72 {y} 328 0.5 re f "));
    }
    let content = format!("{grid}BT /F2 1 Tf 0 Tc 11.2 Tw 10 0 0 10 80 704 Tm (Tabagisme Facteur )Tj 20.6 0 Td 0 Tw (de risque)Tj ET BT /F1 1 Tf 0 Tw 10 0 0 10 80 684 Tm (Genetique)Tj ET");
    let pdf = page_pdf(dir.path(), "row.pdf", vec![content.into_bytes()]);
    let out = dir.path().join("row.docx");
    sheaf_core::export::to_office(&Input::new(pdf), sheaf_core::export::Office::Word, &Default::default(), &out, &mut |_| true).unwrap();
    let xml = String::from_utf8(std::process::Command::new("unzip").arg("-p").arg(&out).arg("word/document.xml").output().unwrap().stdout).unwrap();
    let cells: Vec<String> = xml
        .split("<w:tc>")
        .skip(1)
        .map(|c| c.split("<w:t").skip(1).filter_map(|t| t.split_once('>').and_then(|(_, r)| r.split_once("</w:t>")).map(|(t, _)| t.to_string())).collect::<String>())
        .collect();
    let cells: Vec<&str> = cells.iter().map(|c| c.trim()).collect();
    assert!(cells.windows(2).any(|w| w == ["Tabagisme", "Facteur de risque"]), "{cells:?}");
}

#[test]
fn a_box_filled_as_a_path_is_a_fill_of_the_export() {
    // Word shades a paragraph with `m l l l h f`, not `re f`.
    let dir = tempfile::tempdir().unwrap();
    let pdf = page_pdf(dir.path(), "path.pdf", vec![b"0.8 0.9 0.7 rg 100 600 m 300 600 l 300 620 l 100 620 l h f 0 g 10 10 m 20 40 l 30 10 l h f".to_vec()]);
    let doc = Doc::open(&Input::new(pdf)).unwrap();
    let fills: Vec<[f64; 4]> = doc.document().export_pages()[0].fills.iter().map(|(r, _)| *r).collect();
    assert!(fills.iter().any(|r| (r[0] - 100.0).abs() < 0.5 && (r[2] - 300.0).abs() < 0.5 && (r[3] - r[1] - 20.0).abs() < 0.5), "{fills:?}");
    assert_eq!(fills.len(), 1, "a triangle is no box: {fills:?}");
}
