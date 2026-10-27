//! Edit ▸ Check Spelling… (Acrobat's Check Spelling in comments and form fields): the texts of a
//! document that the pass goes through, and a corrected text put back. The pass itself (the
//! dictionary, the word shown, Change / Ignore / Learn / Next) is in `platform_ui`.

use super::*;

/// What a text checked belongs to: a comment (its page and index there) or a form text field.
#[derive(Debug, Clone, PartialEq)]
pub enum SpellTarget {
    Comment { page: usize, index: usize },
    Field { name: String },
}

/// One text to check: whose it is, where it is said to be ("Comment on page 3"), the text, and the
/// page to show while it is checked.
pub type SpellText = (SpellTarget, String, String, Option<usize>);

/// The comments with text (notes, text boxes, markups' notes; not links, form widgets, popups or
/// Fill & Sign marks) in page order, then the form text fields the user can type in.
pub fn spelling_texts(tab_id: i32) -> Vec<SpellText> {
    update(tab_id, |r, _| {
        let mut out: Vec<SpellText> = Vec::new();
        let mut annots: Vec<&AnnotEntry> = r.snap.annots.iter().filter(|a| !a.fill_sign && !a.contents.trim().is_empty() && !matches!(a.subtype.as_str(), "Link" | "Widget" | "Popup")).collect();
        annots.sort_by_key(|a| (a.page, a.index));
        for a in annots {
            out.push((SpellTarget::Comment { page: a.page, index: a.index }, format!("{} on page {}", if a.subtype == "FreeText" { "Text box" } else { "Comment" }, a.page + 1), a.contents.clone(), Some(a.page)));
        }
        for f in r.snap.fields.iter().filter(|f| f.kind == 0 && !f.read_only) {
            let value = f.value.join("");
            if !value.trim().is_empty() {
                out.push((SpellTarget::Field { name: f.name.clone() }, format!("Form field “{}”", f.name), value, None));
            }
        }
        out
    })
    .unwrap_or_default()
}

/// A text corrected by the pass, put back where it came from (one undo step each).
pub fn spelling_set(ui: &AppWindow, tab_id: i32, target: &SpellTarget, text: &str) {
    let (edit, pages) = match target {
        SpellTarget::Comment { page, index } => (Edit::SetAnnotationContents { page: *page, index: *index, text: text.to_string() }, vec![*page]),
        // A form value can change other pages (widgets, scripts): all render again.
        SpellTarget::Field { name } => (Edit::SetFieldValue { name: name.clone(), value: FieldValue::Text(text.to_string()) }, vec![]),
    };
    edits::apply(ui, tab_id, edit, &pages, None);
}
