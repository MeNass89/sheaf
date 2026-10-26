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
/// from the file (none until they are), the signatures last (long lines that would push the
/// facts below the sheet's fold).
pub(super) fn about_rows(about: &[(String, String)], facts: Option<&sheaf_core::facts::Facts>) -> Vec<(String, String)> {
    let (signatures, mut rows): (Vec<_>, Vec<_>) = about.iter().cloned().partition(|(n, _)| n.starts_with("Signature "));
    let Some(f) = facts else {
        rows.extend(signatures);
        return rows;
    };
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
    rows.extend(signatures);
    rows
}

pub fn props_cancel(tab_id: i32) {
    update(tab_id, |r, _| r.props_open = false);
}

/// The entries changed become one undo step (PdfCraft's SetInfo per entry).
/// `opening`: When opened's page typed and its layout, zoom and panel ([`opening_edit`]).
pub fn props_save(ui: &AppWindow, tab_id: i32, title: &str, author: &str, subject: &str, keywords: &str, custom: &str, opening: (&str, i32, i32, i32)) {
    let edit = update(tab_id, |r, _| {
        let view = match opening_edit(r, opening.0, opening.1, opening.2, opening.3) {
            Some(Err(why)) => return Some(Err(why)),
            Some(Ok(e)) => Some(e),
            None => None,
        };
        r.props_open = false;
        let p = r.snap.props.clone();
        let mut edits: Vec<Edit> = [("Title", &p.title, title), ("Author", &p.author, author), ("Subject", &p.subject, subject), ("Keywords", &p.keywords, keywords)]
            .into_iter()
            .filter(|(_, old, new)| old.trim() != new.trim())
            .map(|(key, _, new)| Edit::SetInfo { key: key.into(), value: new.trim().into() })
            .collect();
        // Custom properties: those changed or added are set, those taken out are emptied.
        let before = r.facts.as_ref().map(|f| f.custom.clone()).unwrap_or_default();
        let after = custom_pairs(custom);
        edits.extend(after.iter().filter(|kv| !before.contains(kv)).map(|(k, v)| Edit::SetInfo { key: k.clone(), value: v.clone() }));
        edits.extend(before.iter().filter(|(k, _)| !after.iter().any(|(a, _)| a == k)).map(|(k, _)| Edit::SetInfo { key: k.clone(), value: String::new() }));
        edits.extend(view);
        (!edits.is_empty()).then(|| Ok(Edit::Batch { label: "Document properties".into(), edits }))
    })
    .flatten();
    match edit {
        Some(Ok(e)) => apply(ui, tab_id, e, &[], None),
        // The sheet stays open on a page that is not in the document.
        Some(Err(why)) => crate::flow::toast(ui, &why),
        None => {}
    }
}

/// Document properties ▸ Open the version signed by … (Acrobat's View Signed Version): the file as
/// it was when signature `k` was made, opened in its own tab (a copy in the temporary folder).
pub fn open_signed(ui: &AppWindow, tab_id: i32, k: i32) {
    let Some((path, name, (who, len, _, _))) = update(tab_id, |r, _| Some((r.path.clone(), r.name.clone(), r.snap.props.signed.get(k.max(0) as usize)?.clone()))).flatten() else { return };
    let made = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
        let part = bytes.get(..len).ok_or("the file is shorter than its signature says")?;
        let dir = std::env::temp_dir().join(format!("sheaf-signed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stem = name.trim_end_matches(".pdf");
        let to = sheaf_core::paths::unique_path(&dir.join(format!("{stem} (signed by {who}).pdf")));
        std::fs::write(&to, part).map_err(|e| e.to_string())?;
        Ok(to)
    });
    match made {
        Ok(to) => super::open(ui, to, None),
        Err(e) => crate::flow::toast(ui, &format!("The signed version could not be opened: {e}")),
    }
}

/// Document properties ▸ Trust …'s certificate: kept as trusted, the signatures validated again.
pub fn trust_signature(tab_id: i32, k: i32) {
    update(tab_id, |r, _| {
        if let Some(&(_, _, index, _)) = r.snap.props.signed.get(k.max(0) as usize) {
            r.in_flight += 1;
            r.send(Op::TrustSignature { index });
        }
    });
}

/// The custom properties as typed ("Client: Acme; Project: Atlas"): name and value pairs, names
/// made PDF names (spaces become "_"), empty ones dropped.
pub(super) fn custom_pairs(typed: &str) -> Vec<(String, String)> {
    typed
        .split(';')
        .filter_map(|p| p.split_once(':'))
        .map(|(k, v)| (k.trim().replace(char::is_whitespace, "_"), v.trim().to_string()))
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .collect()
}

/// [`custom_pairs`] read back for the field.
pub(super) fn custom_text(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::{about_rows, custom_pairs, custom_text};

    #[test]
    fn custom_properties_are_typed_as_name_and_value_pairs() {
        let p = custom_pairs(" Client : Acme;Project code: A-1 ; nothing; empty: ");
        assert_eq!(p, [("Client".to_string(), "Acme".to_string()), ("Project_code".into(), "A-1".into())]);
        assert_eq!(custom_text(&p), "Client: Acme; Project_code: A-1");
    }

    #[test]
    fn signatures_come_after_the_facts_and_the_security_line_says_what_is_forbidden() {
        let row = |n: &str, v: &str| (n.to_string(), v.to_string());
        let about = [row("Pages", "4"), row("Security", "Encrypted"), row("Signature 1 · Ada", "Valid"), row("Fonts", "1")];
        let facts = sheaf_core::facts::Facts { restricted: vec!["Printing"], pdfa: Some("PDF/A-2b".into()), ..Default::default() };
        let names: Vec<String> = about_rows(&about, Some(&facts)).into_iter().map(|r| r.0).collect();
        assert_eq!(names, ["Pages", "Security", "Fonts", "PDF/A", "Signature 1 · Ada"]);
        assert_eq!(about_rows(&about, Some(&facts))[1].1, "Restricted: no printing");
        assert_eq!(about_rows(&about, None).last().unwrap().0, "Signature 1 · Ada");
    }
}

/// The document's Initial View applied to a first opening: page layout, magnification, the page
/// opened to and the side panel (bookmarks or pages); what it leaves at Default stays as set.
pub(super) fn open_as_asked(r: &mut ReaderUi, v: &mut ViewerUi) {
    use sheaf_core::pdfcraft::engine::{InitialLayout as L, Magnification as M, Navigation as N};
    let o = r.snap.opening.clone();
    match o.layout {
        L::SinglePage | L::SinglePageContinuous => v.viewer.set_spread(0),
        L::TwoUp | L::TwoUpContinuous => v.viewer.set_spread(1),
        L::TwoUpCoverPage | L::TwoUpContinuousCoverPage => v.viewer.set_spread(2),
        L::Default => false,
    };
    match o.magnification {
        M::FitPage | M::FitVisible | M::FitHeight => v.pending_fit = true,
        M::FitWidth => {
            v.viewer.set_zoom(100);
        }
        M::ActualSize => v.pending_actual = Some(100.0),
        M::Percent(p) if p > 0.0 => v.pending_actual = Some(p),
        _ => {}
    }
    if o.page > 0 {
        v.show_page(o.page, None);
    }
    match o.navigation {
        N::Bookmarks => (r.panel, r.panel_open) = (1, true),
        N::Pages => (r.panel, r.panel_open) = (0, true),
        _ => {}
    }
}

/// Document Properties ▸ When opened, as the sheet's controls give it: the page (1-based, typed),
/// layout (0 as the reader likes, 1 one page, 2 two pages, 3 two with the cover alone), zoom (0 as
/// the reader likes, 1 fit page, 2 fit width, 3 actual size), panel (0 none, 1 bookmarks, 2 pages).
/// None: nothing changed, or a page that is not in the document.
pub(super) fn opening_edit(r: &ReaderUi, page: &str, layout: i32, zoom: i32, panel: i32) -> Option<Result<Edit, String>> {
    use sheaf_core::pdfcraft::engine::{InitialLayout as L, Magnification as M, Navigation as N};
    let mut v = r.snap.opening.clone();
    let count = r.snap.boxes.len();
    let page = match page.trim() {
        "" => 0,
        t => match t.parse::<usize>() {
            Ok(p) if (1..=count).contains(&p) => p - 1,
            _ => return Some(Err(format!("Open at page: \"{t}\" is not a page of this document (1 to {count})"))),
        },
    };
    v.page = page;
    v.layout = [L::Default, L::SinglePageContinuous, L::TwoUpContinuous, L::TwoUpContinuousCoverPage][layout.clamp(0, 3) as usize];
    v.magnification = [M::Default, M::FitPage, M::FitWidth, M::ActualSize][zoom.clamp(0, 3) as usize];
    v.navigation = [N::PageOnly, N::Bookmarks, N::Pages][panel.clamp(0, 2) as usize];
    (v != r.snap.opening).then(|| Ok(Edit::SetInitialView(Box::new(v))))
}

/// The sheet's controls from the document's Initial View (see [`opening_edit`]).
pub(super) fn opening_controls(o: &sheaf_core::pdfcraft::engine::InitialView) -> (i32, i32, i32, i32) {
    use sheaf_core::pdfcraft::engine::{InitialLayout as L, Magnification as M, Navigation as N};
    let layout = match o.layout {
        L::Default => 0,
        L::SinglePage | L::SinglePageContinuous => 1,
        L::TwoUp | L::TwoUpContinuous => 2,
        L::TwoUpCoverPage | L::TwoUpContinuousCoverPage => 3,
    };
    let zoom = match o.magnification {
        M::FitPage | M::FitVisible | M::FitHeight => 1,
        M::FitWidth => 2,
        M::ActualSize => 3,
        _ => 0,
    };
    let panel = match o.navigation {
        N::Bookmarks => 1,
        N::Pages => 2,
        _ => 0,
    };
    (o.page as i32 + 1, layout, zoom, panel)
}
