//! Document Properties (⌘D, Acrobat's sheet): title, author, subject and keywords to edit, and
//! what the file is (pages, size, version, producer, security) to read.

use super::*;

pub fn props_open(tab_id: i32) {
    update(tab_id, |r, _| r.props_open = true);
}

pub fn props_cancel(tab_id: i32) {
    update(tab_id, |r, _| r.props_open = false);
}

/// The entries changed become one undo step (PdfCraft's SetInfo per entry).
pub fn props_save(ui: &AppWindow, tab_id: i32, title: &str, author: &str, subject: &str, keywords: &str) {
    let edit = update(tab_id, |r, _| {
        r.props_open = false;
        let p = r.snap.props.clone();
        let edits: Vec<Edit> = [("Title", &p.title, title), ("Author", &p.author, author), ("Subject", &p.subject, subject), ("Keywords", &p.keywords, keywords)]
            .into_iter()
            .filter(|(_, old, new)| old.trim() != new.trim())
            .map(|(key, _, new)| Edit::SetInfo { key: key.into(), value: new.trim().into() })
            .collect();
        (!edits.is_empty()).then(|| Edit::Batch { label: "Document properties".into(), edits })
    })
    .flatten();
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[], None);
    }
}
