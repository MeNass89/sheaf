//! Acrobat's Edit All JavaScripts: page and link actions stay attached to their original target.
//! Replacing source preserves /Next and the other event actions; ambiguous chain deletion refuses.
use pdfcraft_cos::{Dict, Document, ObjRef, Object, PdfString};
use crate::js::{ActionScriptTarget, ConsoleScript};

fn source(doc: &Document, action: &Object) -> Option<String> {
    let action = doc.resolve(action);
    let dict = action.as_dict()?;
    if dict.name(b"S") != Some(b"JavaScript") { return None; }
    match &*doc.resolve(dict.get(b"JS")?) {
        Object::String(s) => (s.bytes.len() <= 1_000_000).then(|| s.to_text()),
        Object::Stream(s) => s.decoded_within(1_000_000).ok().map(|b| PdfString::literal(b).to_text()),
        _ => None,
    }
}

fn collect(doc: &Document, object: &Dict, page: usize, link: Option<usize>, out: &mut Vec<ConsoleScript>) {
    let events: &[&str] = if link.is_some() { &["A", "E", "X", "D", "U", "Fo", "Bl", "PO", "PC", "PV", "PI"] } else { &["O", "C"] };
    let aa = object.get(b"AA").map(|o| doc.resolve(o)).and_then(|o| o.as_dict().cloned()).unwrap_or_default();
    for event in events {
        let action = if *event == "A" { object.get(b"A") } else { aa.get(event.as_bytes()) };
        if let Some(source) = action.and_then(|a| source(doc, a)) {
            let name = link.map(|i| format!("Page {} · Link {} · {event}", page + 1, i + 1)).unwrap_or_else(|| format!("Page {} · {event}", page + 1));
            out.push(ConsoleScript { name, source, event: None, target: Some(ActionScriptTarget { page, link, event: (*event).into() }) });
        }
    }
}

pub(crate) fn inventory(doc: &Document) -> Vec<ConsoleScript> {
    let mut out = Vec::new();
    for (page, reference) in pdfcraft_annot::page_refs(doc).unwrap_or_default().into_iter().enumerate() {
        let object = doc.get(reference);
        let Some(dict) = object.as_dict() else { continue };
        collect(doc, dict, page, None, &mut out);
        let annots = dict.get(b"Annots").map(|a| doc.resolve(a)).and_then(|a| a.as_array().cloned()).unwrap_or_default();
        for (index, annot) in annots.iter().enumerate() {
            let object = doc.resolve(annot);
            if let Some(dict) = object.as_dict().filter(|d| d.name(b"Subtype") == Some(b"Link")) {
                collect(doc, dict, page, Some(index), &mut out);
            }
        }
    }
    out
}

fn target_object(doc: &mut Document, target: &ActionScriptTarget) -> Result<ObjRef, String> {
    let pages = pdfcraft_annot::page_refs(doc).map_err(|e| e.to_string())?;
    let page = *pages.get(target.page).ok_or("The script page no longer exists")?;
    let Some(index) = target.link else { return Ok(page) };
    let mut annots = doc.get(page).as_dict().and_then(|d| d.get(b"Annots")).map(|a| doc.resolve(a)).and_then(|a| a.as_array().cloned()).ok_or("The page has no link annotations")?;
    let annot = annots.get(index).ok_or("The script link no longer exists")?;
    if doc.resolve(annot).as_dict().and_then(|d| d.name(b"Subtype")) != Some(b"Link") { return Err("The script target is no longer a link".into()); }
    if let Some(r) = annot.as_ref() { return Ok(r); }
    let reference = doc.add((*doc.resolve(annot)).clone());
    annots[index] = Object::Ref(reference);
    doc.update_dict(page, |d| d.set(b"Annots".to_vec(), Object::Array(annots))).map_err(|e| e.to_string())?;
    Ok(reference)
}

pub(crate) fn set(doc: &mut Document, target: &ActionScriptTarget, script: Option<&str>) -> Result<(), String> {
    let allowed: &[&str] = if target.link.is_some() { &["A", "E", "X", "D", "U", "Fo", "Bl", "PO", "PC", "PV", "PI"] } else { &["O", "C"] };
    if !allowed.contains(&target.event.as_str()) { return Err("Unknown page or link script event".into()); }
    if script.is_some_and(|s| s.len() > 1_000_000) { return Err("The action script exceeds 1 MB".into()); }
    let reference = target_object(doc, target)?;
    let mut dict = doc.get(reference).as_dict().cloned().ok_or("The script target is not a dictionary")?;
    let mut aa = dict.get(b"AA").map(|a| doc.resolve(a)).and_then(|a| a.as_dict().cloned()).unwrap_or_default();
    let old = if target.event == "A" { dict.get(b"A") } else { aa.get(target.event.as_bytes()) };
    let mut action = old.map(|a| doc.resolve(a)).and_then(|a| a.as_dict().cloned()).unwrap_or_default();
    if old.is_some() && action.name(b"S") != Some(b"JavaScript") { return Err("This event has a non-JavaScript action; it was left unchanged".into()); }
    if script.is_none() && action.contains(b"Next") { return Err("This script has chained actions. Delete or reorganize the chain in a full action editor".into()); }
    if let Some(source) = script {
        action.set(b"S".to_vec(), Object::name("JavaScript"));
        action.set(b"JS".to_vec(), Object::String(PdfString::text(source)));
    }
    let container = if target.event == "A" { &mut dict } else { &mut aa };
    let key = target.event.as_bytes().to_vec();
    if script.is_some() { container.set(key, Object::Dict(action)); } else { container.remove(&key); }
    if target.event != "A" { if aa.is_empty() { dict.remove(b"AA"); } else { dict.set(b"AA".to_vec(), Object::Dict(aa)); } }
    if target.event == "A" && script.is_some() { dict.remove(b"Dest"); }
    doc.set(reference, Object::Dict(dict));
    Ok(())
}
