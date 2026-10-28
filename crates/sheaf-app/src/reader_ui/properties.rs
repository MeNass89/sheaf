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
    // Acrobat's Audit Space Usage: what takes the file's bytes.
    let total: u64 = f.space.iter().map(|s| s.1).sum();
    if total > 0 {
        let size = |n: u64| if n >= 1 << 20 { format!("{:.1} MB", n as f64 / 1_048_576.0) } else { format!("{} KB", n.div_ceil(1024)) };
        let parts: Vec<String> = f.space.iter().map(|(what, n)| format!("{what} {} ({}%)", size(*n), n * 100 / total)).collect();
        rows.push(("Space used".into(), parts.join(", ")));
    }
    rows.extend(signatures);
    rows
}

/// Copy report (Acrobat's PDF info, as Preflight lists it): what the sheet shows and the fonts,
/// each said embedded or not, as text on the clipboard.
pub fn props_copy(ui: &AppWindow, tab_id: i32) {
    let Some(text) = update(tab_id, |r, _| report(&r.name, &r.snap.props, r.facts.as_ref())) else { return };
    let copied = arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).is_ok();
    crate::flow::toast(ui, &if copied { sheaf_app::i18n::tr("Report copied") } else { sheaf_app::i18n::tr("The clipboard could not be reached") });
}

/// Document properties ▸ Save report… (Acrobat's PDF info report, saved): the same report as a
/// JSON file (description, facts, fonts) for a script or an audit, or as text when named `.txt`.
pub fn props_report(ui: &AppWindow, tab_id: i32) {
    let Some((dir, stem)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())) else { return };
    let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(format!("{stem} - report.json")).add_filter("JSON", &["json"]).add_filter("Text", &["txt"]);
    crate::dialog::save_file(ui, d, move |ui, to| {
        let text = to.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt"));
        let Some(body) = update(tab_id, |r, _| if text { report(&r.name, &r.snap.props, r.facts.as_ref()) } else { report_json(&r.name, &r.snap.props, r.facts.as_ref()) }) else { return };
        match std::fs::write(&to, body) {
            Ok(()) => crate::flow::toast(ui, &sheaf_app::i18n::trf("Report saved as {}", &[&file_name(&to)])),
            Err(e) => crate::flow::toast(ui, &sheaf_app::i18n::trf("The report was not saved: {}", &[&e])),
        }
    });
}

/// [`report`] as JSON: `file`, `description` (title, author, subject, keywords, custom
/// properties), `facts` (each row of the sheet) and `fonts` (name, embedded).
pub(super) fn report_json(name: &str, props: &reader_doc::Props, facts: Option<&sheaf_core::facts::Facts>) -> String {
    use serde_json::{json, Map, Value};
    let mut description = Map::new();
    for (n, v) in [("title", &props.title), ("author", &props.author), ("subject", &props.subject), ("keywords", &props.keywords)] {
        description.insert(n.into(), v.as_str().into());
    }
    if let Some(f) = facts {
        description.insert("custom".into(), f.custom.iter().map(|(n, v)| (n.clone(), Value::from(v.as_str()))).collect::<Map<_, _>>().into());
    }
    let rows: Map<String, Value> = about_rows(&props.about, facts).into_iter().map(|(n, v)| (n, v.into())).collect();
    let fonts: Vec<Value> = facts.map(|f| f.fonts.iter().map(|(n, e)| json!({ "name": n, "embedded": e })).collect()).unwrap_or_default();
    let v = json!({ "file": name, "description": description, "facts": rows, "fonts": fonts });
    serde_json::to_string_pretty(&v).unwrap_or_default() + "\n"
}

/// The report [`props_copy`] copies: the file's name, its description, then a line per fact.
pub(super) fn report(name: &str, props: &reader_doc::Props, facts: Option<&sheaf_core::facts::Facts>) -> String {
    let or_none = |v: &str| if v.trim().is_empty() { "—".to_string() } else { v.to_string() };
    let mut lines = vec![name.to_string(), String::new()];
    for (n, v) in [("Title", &props.title), ("Author", &props.author), ("Subject", &props.subject), ("Keywords", &props.keywords)] {
        lines.push(format!("{n}: {}", or_none(v)));
    }
    if let Some(f) = facts {
        lines.extend(f.custom.iter().map(|(n, v)| format!("{n}: {v}")));
    }
    lines.push(String::new());
    lines.extend(about_rows(&props.about, facts).into_iter().map(|(n, v)| format!("{n}: {v}")));
    if let Some(f) = facts.filter(|f| !f.fonts.is_empty()) {
        lines.push(String::new());
        lines.push("Fonts".into());
        lines.extend(f.fonts.iter().map(|(n, embedded)| format!("{n}{}", if *embedded { " (embedded)" } else { " (not embedded)" })));
    }
    lines.join("\n") + "\n"
}

pub fn props_cancel(tab_id: i32) {
    update(tab_id, |r, _| r.props_open = false);
}

/// The entries changed become one undo step (PdfCraft's SetInfo per entry).
/// `opening`: When opened's page typed and its layout, zoom, panel and window ([`opening_edit`]).
#[allow(clippy::too_many_arguments)]
pub fn props_save(ui: &AppWindow, tab_id: i32, title: &str, author: &str, subject: &str, keywords: &str, custom: &str, opening: (&str, i32, i32, i32, i32)) {
    let edit = update(tab_id, |r, _| {
        let view = match opening_edit(r, opening.0, opening.1, opening.2, opening.3, opening.4) {
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
        Err(e) => crate::flow::toast(ui, &sheaf_app::i18n::trf("The signed version could not be opened: {}", &[&e])),
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

/// Document properties ▸ Clear signature (Acrobat's, needed before signing the field again): the
/// field of signature `k` left unsigned and empty, one undo step; the sheet closes on the change.
pub fn clear_signature(ui: &AppWindow, tab_id: i32, k: i32) {
    let Some((name, page)) = update(tab_id, |r, _| {
        let k = usize::try_from(k).ok()?;
        let name = r.snap.props.signed_fields.get(k)?.clone();
        let page = r.snap.fields.iter().find(|f| f.name == name).and_then(|f| f.widgets.first()).map_or(0, |w| w.0);
        r.props_open = false;
        Some((name, page))
    })
    .flatten() else { return };
    edits::apply(ui, tab_id, Edit::ClearSignature { name }, &[page], None);
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
    // The document's zoom over Preferences' (Sol, 26/10: a fit to the page asked by the
    // preferences took over a document asking for its width or actual size).
    match o.magnification {
        M::FitPage | M::FitVisible | M::FitHeight => {
            (v.pending_fit, v.pending_actual) = (true, None);
            r.zoom_mode = 1;
        }
        M::FitWidth => {
            (v.pending_fit, v.pending_actual) = (false, None);
            v.viewer.set_zoom(100);
            r.zoom_mode = 0;
        }
        M::ActualSize => {
            (v.pending_fit, v.pending_actual) = (false, Some(100.0));
            r.zoom_mode = 2;
        }
        M::Percent(p) if p > 0.0 => {
            (v.pending_fit, v.pending_actual) = (false, Some(p));
            r.zoom_mode = 2;
        }
        _ => {}
    }
    if o.page > 0 {
        // Again once the view has its size: scrolled now, the pages may not be laid out yet.
        v.show_page(o.page, None);
        v.pending_page = Some(o.page);
    }
    match o.navigation {
        N::Bookmarks => (r.panel, r.panel_open) = (1, true),
        N::Pages => (r.panel, r.panel_open) = (0, true),
        N::Attachments => (r.panel, r.panel_open) = (4, true),
        N::Layers if !r.snap.layers.is_empty() => (r.panel, r.panel_open) = (5, true),
        _ => {}
    }
    // The window options (Sol, 26/10: saved but not followed here): full screen and no toolbar,
    // asked of the window once (`opened`); the title shown is the model's; fitting and centring the
    // window, once the document is shown ([`window_as_asked`]).
    r.opened = (o.full_screen, o.hide_toolbar, r.opened.2 + 1);
    r.window_ask = (o.fit_window || o.center_window).then_some((o.fit_window, o.center_window));
}

/// When opened ▸ Resize window to the page and Centre window (Acrobat's): the window as wide and
/// tall as the first page shown at actual size with the toolbar and tabs (the screen's 90 % at
/// most), then centred on its screen.
pub(super) fn window_as_asked(ui: &AppWindow, fit: bool, centre: bool, page: (f64, f64)) {
    use slint::winit_030::{winit, WinitWindowAccessor};
    ui.window().with_winit_window(|w: &winit::window::Window| {
        let Some(m) = w.current_monitor() else { return };
        let scale = m.scale_factor();
        let screen = m.size().to_logical::<f64>(scale);
        if fit {
            // Points at actual size (96 px an inch), with the bars above the pages (84 px) and margins.
            let (pw, ph) = (page.0 * 96.0 / 72.0 + 48.0, page.1 * 96.0 / 72.0 + 84.0 + 32.0);
            let size = winit::dpi::LogicalSize::new(pw.min(screen.width * 0.9), ph.min(screen.height * 0.9));
            let _ = w.request_inner_size(size);
        }
        if centre {
            let outer = w.outer_size().to_logical::<f64>(scale);
            let origin = m.position().to_logical::<f64>(scale);
            let at = winit::dpi::LogicalPosition::new(origin.x + (screen.width - outer.width).max(0.0) / 2.0, origin.y + (screen.height - outer.height).max(0.0) / 2.0);
            w.set_outer_position(at);
        }
    });
}

/// Document Properties ▸ When opened, as the sheet's controls give it: the page (1-based, typed),
/// layout (0 as the reader likes, 1 one page, 2 two pages, 3 two with the cover alone), zoom (0 as
/// the reader likes, 1 fit page, 2 fit width, 3 actual size), panel (0 none, 1 bookmarks, 2 pages).
/// None: nothing changed, or a page that is not in the document.
pub(super) fn opening_edit(r: &ReaderUi, page: &str, layout: i32, zoom: i32, panel: i32, window: i32) -> Option<Result<Edit, String>> {
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
    // Acrobat's Window and User interface options (Sol, 26/10), bits as [`opening_controls`] gives them.
    (v.full_screen, v.display_title, v.fit_window, v.center_window, v.hide_toolbar) =
        (window & 1 != 0, window & 2 != 0, window & 4 != 0, window & 8 != 0, window & 16 != 0);
    (v != r.snap.opening).then(|| Ok(Edit::SetInitialView(Box::new(v))))
}

/// The sheet's controls from the document's Initial View (see [`opening_edit`]); the window bits:
/// 1 full screen, 2 the title in the bar, 4 the window fitted to the page, 8 centred, 16 no toolbar.
pub(super) fn opening_controls(o: &sheaf_core::pdfcraft::engine::InitialView) -> (i32, i32, i32, i32, i32) {
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
    let window = [o.full_screen, o.display_title, o.fit_window, o.center_window, o.hide_toolbar].iter().enumerate().map(|(k, on)| (*on as i32) << k).sum();
    (o.page as i32 + 1, layout, zoom, panel, window)
}

#[cfg(test)]
mod tests {
    use super::{about_rows, custom_pairs, custom_text, report, report_json};

    #[test]
    fn the_report_lists_the_description_the_facts_and_the_fonts() {
        let props = crate::reader_doc::Props { title: "Lease".into(), about: vec![("Pages".into(), "4".into())], ..Default::default() };
        let facts = sheaf_core::facts::Facts { fonts: vec![("Helvetica".into(), false), ("Inter".into(), true)], ..Default::default() };
        let r = report("lease.pdf", &props, Some(&facts));
        assert!(r.starts_with("lease.pdf\n\nTitle: Lease\nAuthor: —\n"), "{r}");
        assert!(r.contains("\nPages: 4\n") && r.contains("\nPDF/A: No\n"), "{r}");
        assert!(r.ends_with("Fonts\nHelvetica (not embedded)\nInter (embedded)\n"), "{r}");
    }

    #[test]
    fn the_report_is_saved_as_json_too() {
        let props = crate::reader_doc::Props { title: "Lease".into(), about: vec![("Pages".into(), "4".into())], ..Default::default() };
        let facts = sheaf_core::facts::Facts { fonts: vec![("Helvetica".into(), false)], custom: vec![("Client".into(), "Acme".into())], ..Default::default() };
        let v: serde_json::Value = serde_json::from_str(&report_json("lease.pdf", &props, Some(&facts))).unwrap();
        assert_eq!(v["file"], "lease.pdf");
        assert_eq!((v["description"]["title"].as_str(), v["description"]["custom"]["Client"].as_str()), (Some("Lease"), Some("Acme")));
        assert_eq!((v["facts"]["Pages"].as_str(), v["facts"]["PDF/A"].as_str()), (Some("4"), Some("No")));
        assert_eq!(v["fonts"][0], serde_json::json!({ "name": "Helvetica", "embedded": false }));
    }

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
