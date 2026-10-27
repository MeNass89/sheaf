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
/// Fill & Sign marks) in page order, each followed by its replies, then the form text fields the
/// user can type in.
pub fn spelling_texts(tab_id: i32) -> Vec<SpellText> {
    update(tab_id, |r, _| {
        let mut annots: Vec<&AnnotEntry> = r.snap.annots.iter().filter(|a| !a.fill_sign && !matches!(a.subtype.as_str(), "Link" | "Widget" | "Popup")).collect();
        annots.sort_by_key(|a| (a.page, a.index));
        let comments: Vec<Comment> = annots.iter().map(|a| (a.page, a.index, a.subtype.as_str(), a.contents.as_str(), a.replies.as_slice())).collect();
        let mut out = comment_texts(&comments);
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

/// A comment as the pass reads it: page, index there, subtype, its text, its replies ("author:
/// text", page, index).
type Comment<'a> = (usize, usize, &'a str, &'a str, &'a [(String, usize, usize)]);

/// The comments' texts to check, each followed by its replies. A comment whose own text is empty
/// (a highlight without a note) is skipped, its replies are not.
fn comment_texts(comments: &[Comment]) -> Vec<SpellText> {
    let mut out = Vec::new();
    for &(page, index, subtype, contents, replies) in comments {
        if !contents.trim().is_empty() {
            out.push((SpellTarget::Comment { page, index }, format!("{} on page {}", if subtype == "FreeText" { "Text box" } else { "Comment" }, page + 1), contents.to_string(), Some(page)));
        }
        for (shown, page, index) in replies {
            let text = reply_text(shown);
            if !text.trim().is_empty() {
                out.push((SpellTarget::Comment { page: *page, index: *index }, format!("Reply on page {}", page + 1), text.to_string(), Some(*page)));
            }
        }
    }
    out
}

/// A reply's own text from its line in the panel, "author: text" (the author is "—" when the file
/// names none).
fn reply_text(shown: &str) -> &str {
    shown.split_once(": ").map_or(shown, |(_, t)| t)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_are_checked_even_when_their_comment_has_no_text() {
        let replies = vec![("Ada: teh reply".to_string(), 0, 3)];
        let texts = comment_texts(&[(0, 2, "Highlight", "", replies.as_slice()), (1, 0, "Text", "  ", &[]), (1, 1, "FreeText", "a box", &[])]);
        let got: Vec<(&str, &str)> = texts.iter().map(|t| (t.1.as_str(), t.2.as_str())).collect();
        assert_eq!(got, [("Reply on page 1", "teh reply"), ("Text box on page 2", "a box")]);
        assert_eq!(texts[0].0, SpellTarget::Comment { page: 0, index: 3 });
    }

    #[test]
    fn a_replys_text_is_checked_without_its_author() {
        assert_eq!(reply_text("Ada: I agre: twice"), "I agre: twice");
        assert_eq!(reply_text("—: teh reply"), "teh reply");
        assert_eq!(reply_text("no author"), "no author");
    }
}
