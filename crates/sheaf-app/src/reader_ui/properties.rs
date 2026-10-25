//! Document Properties (⌘D, Acrobat's sheet): title, author, subject and keywords to edit, and
//! what the file is (pages, size, version, producer, security) to read.

use super::*;

/// Opens the sheet; what the engine's summary lacks (restrictions, PDF/A, dates, page boxes) is
/// read from the file on the side and shows when ready, so a large file opens the sheet at once.
pub fn props_open(tab_id: i32) {
    let Some((bytes, password)) = update(tab_id, |r, _| {
        r.props_open = true;
        (r.snap.bytes.clone(), r.password())
    }) else {
        return;
    };
    std::thread::spawn(move || {
        let f = sheaf_core::facts::read(&bytes, password.as_deref());
        let _ = slint::invoke_from_event_loop(move || {
            update(tab_id, |r, _| r.facts = Some(f));
        });
    });
}

/// The sheet's read-only rows: the engine's, the security line made precise, then the facts read
/// from the file (none until they are).
pub(super) fn about_rows(about: &[(String, String)], facts: Option<&sheaf_core::facts::Facts>) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = about.to_vec();
    let Some(f) = facts else { return rows };
    if !f.restricted.is_empty()
        && let Some(sec) = rows.iter_mut().find(|(n, _)| n == "Security")
    {
        sec.1 = format!("Restricted: no {}", f.restricted.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>().join(", no "));
    }
    rows.push(("PDF/A".into(), f.pdfa.clone().unwrap_or_else(|| "No".into())));
    for (name, v) in [("Created", &f.created), ("Modified", &f.modified)] {
        if let Some(v) = v {
            rows.push((name.into(), v.clone()));
        }
    }
    rows.extend(f.boxes.iter().map(|(n, v)| (n.to_string(), v.clone())));
    rows
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
