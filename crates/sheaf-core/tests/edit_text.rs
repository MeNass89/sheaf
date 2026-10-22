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
    use lopdf::{Dictionary, Object, Stream};
    let content = b"BT /F2 1 Tf 10 0 0 10 72 700 Tm [(Sympto)6.6 (ms)] TJ /F1 1 Tf [( : pain, diar)4.2 (rhoea, weight loss)] TJ ET
BT /F1 1 Tf 10 0 0 10 72 688 Tm [(- General )] TJ /F2 1 Tf [(sig)5 (ns)] TJ /F1 1 Tf [(: fever and tired)3.9 (ness in many patients)] TJ ET
BT /F3 1 Tf 10 0 0 10 72 676 Tm [(- Bowel: il)]"
        .to_vec();
    // The page's second content stream goes on with the operator of the first's last array.
    let rest = b" TJ 0.001 Tc 6.6 0 Td [(iac fossa)] TJ ET
BT /F1 1 Tf 10 0 0 10 72 600 Tm (Another paragraph further down) Tj ET"
        .to_vec();
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
    let stream = doc.add_object(Stream::new(Dictionary::new(), content));
    let rest = doc.add_object(Stream::new(Dictionary::new(), rest));
    let mut fonts = Dictionary::new();
    fonts.set("F1", font("Helvetica"));
    fonts.set("F2", font("Helvetica-Bold"));
    fonts.set("F3", font("Courier"));
    let mut res = Dictionary::new();
    res.set("Font", Object::Dictionary(fonts));
    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages));
    page.set("Contents", Object::Array(vec![Object::Reference(stream), Object::Reference(rest)]));
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
    let path = dir.join("mixed.pdf");
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
