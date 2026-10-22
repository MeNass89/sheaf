//! The clean-up outside the page content, run on PdfCraft's output ([`post_pass`]): whole-page
//! marks emptied, the searched text scrubbed from the document information, bookmarks,
//! comments, fields and structure, and XMP, attachments, scripts, private data, hidden forms
//! and unused fonts holding it removed whole.

use super::content::{cmap_text, scrub_painted, shown_strings};
use super::matching::{contains_any, scrub};
use super::names::PDF_NAMES;
use super::pdf::{decode, decoded, resolve, stream_text, sub_dict, xmp_streams, xmp_text};
use super::reach::Reach;
use super::{bounds, Mark, Needle};
use crate::{OpdfError, Result};
use lopdf::{Dictionary, Object, ObjectId};
use std::collections::{BTreeSet, HashSet};

/// What a dictionary is, for the scrub's allow-list.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Info,
    Outline,
    Annot,
    Field,
    /// A widget's appearance characteristics (`/MK`): its captions.
    Captions,
    /// A structure element: its alternate and replacement texts.
    Struct,
}

/// The keys whose text values are scrubbed, by kind: what a reader shows as text. Names-tree
/// keys, destinations, URIs, default appearances, field names and anything else are left alone.
fn scrub_keys(k: Kind) -> &'static [&'static [u8]] {
    match k {
        Kind::Info => &[],
        Kind::Outline => &[b"Title"],
        Kind::Annot => &[b"Contents", b"T", b"Subj", b"RC"],
        Kind::Field => &[b"V", b"DV", b"TU", b"Opt"],
        Kind::Captions => &[b"CA", b"RC", b"AC"],
        Kind::Struct => &[b"Alt", b"ActualText", b"E"],
    }
}

/// The kinds of `d` (an annotation widget can be its own field, so possibly two).
/// `captions` are the `/MK` dictionaries written as their own objects.
fn kinds(d: &Dictionary, outline_items: &HashSet<ObjectId>, captions: &HashSet<ObjectId>, id: ObjectId) -> Vec<Kind> {
    let name = |k: &[u8]| d.get(k).and_then(Object::as_name).ok();
    let mut out = Vec::new();
    if outline_items.contains(&id) {
        out.push(Kind::Outline);
    }
    if captions.contains(&id) {
        out.push(Kind::Captions);
    }
    if name(b"Type") == Some(b"StructElem".as_slice()) || (d.has(b"S") && d.has(b"P") && d.has(b"K")) {
        out.push(Kind::Struct);
    }
    let annot = name(b"Type") == Some(b"Annot".as_slice()) || (d.has(b"Subtype") && d.has(b"Rect") && name(b"Type").is_none_or(|t| t == b"Annot"));
    // A widget's /T is its field's name: a widget is only scrubbed as a field.
    if annot && name(b"Subtype") != Some(b"Widget".as_slice()) {
        out.push(Kind::Annot);
    }
    if d.has(b"FT") || (d.has(b"T") && (d.has(b"V") || d.has(b"Kids")) && name(b"Subtype") != Some(b"Text".as_slice())) {
        out.push(Kind::Field);
    }
    out
}

/// The outline items reachable from the catalog's /Outlines (by /First and /Next).
fn outline_items(d: &lopdf::Document) -> HashSet<ObjectId> {
    let mut out = HashSet::new();
    let first = |id: ObjectId| d.get_dictionary(id).ok().and_then(|x| x.get(b"First").and_then(Object::as_reference).ok());
    let Some(root) = d.catalog().ok().and_then(|c| c.get(b"Outlines").and_then(Object::as_reference).ok()) else { return out };
    let mut todo: Vec<ObjectId> = first(root).into_iter().collect();
    while let Some(id) = todo.pop() {
        if !out.insert(id) || out.len() > 100_000 {
            continue;
        }
        if let Some(c) = first(id) {
            todo.push(c);
        }
        if let Some(n) = d.get_dictionary(id).ok().and_then(|x| x.get(b"Next").and_then(Object::as_reference).ok()) {
            todo.push(n);
        }
    }
    out
}

/// Scrubs the allow-listed keys of `d` (through arrays and nested dictionaries: a choice
/// field's /Opt); the document information is scrubbed whole with [`scrub_dict_all`].
fn scrub_dict(d: &mut Dictionary, kinds: &[Kind], needles: &[Needle]) -> bool {
    if kinds.contains(&Kind::Info) {
        return scrub_dict_all(d, needles);
    }
    let keys: Vec<Vec<u8>> = d.iter().filter(|(k, _)| kinds.iter().any(|kd| scrub_keys(*kd).contains(&k.as_slice()))).map(|(k, _)| k.clone()).collect();
    let mut changed = false;
    for k in keys {
        let Ok(v) = d.get_mut(&k) else { continue };
        match scrub_any(v, needles) {
            Scrubbed::Kept => {}
            Scrubbed::Changed => changed = true,
            Scrubbed::Drop => {
                d.remove(&k);
                changed = true;
            }
        }
    }
    // A widget's captions written inline.
    if let Ok(Object::Dictionary(mk)) = d.get_mut(b"MK") {
        changed |= scrub_dict(mk, &[Kind::Captions], needles);
    }
    changed
}

/// What the post-pass did that the user must be told.
type Warnings = Vec<String>;

/// The written bytes, after PdfCraft: whole-page marks emptied; `needles` removed from the
/// document information (through arrays and dictionaries; a name holding one loses its key),
/// bookmark titles, comments, form field values and options, widget captions and structure
/// alternate texts; XMP metadata, attachments, JavaScript, private application data
/// (/PieceInfo) and Form XObjects no page paints that contain one removed whole (each kind
/// named in the warnings). Unchanged bytes are returned as they are.
pub(super) fn post_pass(bytes: &[u8], marks: &[Mark], needles: &[Needle]) -> Result<(Vec<u8>, Warnings)> {
    let whole: BTreeSet<usize> = marks.iter().filter(|m| m.whole_page).map(|m| m.page).collect();
    let mut warnings = Vec::new();
    if whole.is_empty() && needles.is_empty() {
        return Ok((bytes.to_vec(), warnings));
    }
    let mut d = lopdf::Document::load_mem(bytes)?;
    let mut changed = false;
    // Whole pages: one black fill of the mark's box, nothing else.
    let page_ids = d.get_pages();
    let mut dropped: HashSet<ObjectId> = HashSet::new();
    for &p in &whole {
        let id = *page_ids.get(&(p as u32 + 1)).ok_or_else(|| OpdfError::Engine(format!("page {} is missing", p + 1)))?;
        let quads: Vec<[f64; 8]> = marks.iter().filter(|m| m.whole_page && m.page == p).flat_map(|m| m.quads.clone()).collect();
        let [x0, y0, x1, y1] = bounds(&quads);
        let fill = format!("q 0 0 0 rg {x0} {y0} {} {} re f Q", x1 - x0, y1 - y0).into_bytes();
        let content = d.add_object(lopdf::Stream::new(Dictionary::new(), fill));
        let annots: Vec<ObjectId> = match d.get_dictionary(id)?.get(b"Annots") {
            Ok(a) => d.dereference(a)?.1.as_array().map(|a| a.iter().filter_map(|x| x.as_reference().ok()).collect()).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        dropped.extend(annots);
        let page = d.get_object_mut(id)?.as_dict_mut()?;
        page.set("Contents", content);
        page.set("Resources", Dictionary::new());
        for k in [&b"Annots"[..], b"Thumb", b"PieceInfo", b"Metadata", b"B"] {
            page.remove(k);
        }
        changed = true;
    }
    if !dropped.is_empty() {
        prune_fields(&mut d, &dropped)?;
    }
    if needles.is_empty() {
        d.prune_objects();
        let mut out = Vec::new();
        d.save_to(&mut out)?;
        return Ok((out, warnings));
    }
    // Text values under the allow-list.
    let outlines = outline_items(&d);
    let captions: HashSet<ObjectId> = d.objects.values().filter_map(|o| o.as_dict().ok()).filter_map(|x| x.get(b"MK").and_then(Object::as_reference).ok()).collect();
    let ids: Vec<ObjectId> = d.objects.keys().copied().collect();
    let mut fields_changed = false;
    for id in ids {
        let Some(Object::Dictionary(dict)) = d.objects.get_mut(&id) else { continue };
        let mut ks = kinds(dict, &outlines, &captions, id);
        if dict.has(b"MK") {
            ks.push(Kind::Annot);
            ks.dedup();
        }
        let widget = dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Widget".as_slice());
        if widget && !ks.contains(&Kind::Field) {
            // A widget is scrubbed as a field (never its /T), plus its captions.
            ks.retain(|k| *k != Kind::Annot);
        }
        if ks.is_empty() || !scrub_dict(dict, &ks, needles) {
            continue;
        }
        changed = true;
        // An annotation or a field whose text changed loses its appearance (it still shows
        // the old text); viewers draw it again from the new value.
        dict.remove(b"AP");
        if ks.contains(&Kind::Field) {
            fields_changed = true;
            let kids: Vec<ObjectId> = dict.get(b"Kids").and_then(Object::as_array).map(|a| a.iter().filter_map(|x| x.as_reference().ok()).collect()).unwrap_or_default();
            for k in kids {
                if let Ok(kd) = d.get_object_mut(k).and_then(Object::as_dict_mut) {
                    kd.remove(b"AP");
                }
            }
        }
    }
    // The document information, direct or not.
    match d.trailer.get(b"Info").ok().cloned() {
        Some(Object::Reference(r)) => {
            if let Ok(info) = d.get_object_mut(r).and_then(Object::as_dict_mut) {
                changed |= scrub_dict(info, &[Kind::Info], needles);
            }
        }
        Some(Object::Dictionary(mut info)) => {
            if scrub_dict(&mut info, &[Kind::Info], needles) {
                d.trailer.set("Info", info);
                changed = true;
            }
        }
        _ => {}
    }
    if fields_changed
        && let Ok(af) = d.catalog().and_then(|c| c.get(b"AcroForm")).cloned()
    {
        match af {
            Object::Reference(r) => {
                if let Ok(a) = d.get_object_mut(r).and_then(Object::as_dict_mut) {
                    a.set("NeedAppearances", true);
                }
            }
            Object::Dictionary(mut a) => {
                a.set("NeedAppearances", true);
                let root = d.trailer.get(b"Root")?.as_reference()?;
                d.get_object_mut(root)?.as_dict_mut()?.set("AcroForm", a);
            }
            _ => {}
        }
    }
    // Streams with a match outside page content: XMP, attachments and JavaScript go whole.
    let mut gone: HashSet<ObjectId> = HashSet::new();
    let mut xmp_gone = false;
    for id in xmp_streams(&d) {
        if let Ok(Object::Stream(s)) = d.get_object(id)
            && contains_any(&xmp_text(&decoded(s)), needles)
        {
            gone.insert(id);
            xmp_gone = true;
        }
    }
    if xmp_gone {
        warnings.push("XMP metadata removed: it contained the redacted text".into());
    }
    let (attachments, scripts) = embedded_and_scripts(&d);
    for (name, file) in attachments {
        if d.get_object(file).ok().and_then(|o| o.as_stream().ok()).is_some_and(|s| contains_any(&stream_text(s), needles)) {
            gone.insert(file);
            warnings.push(format!("Attachment {name} removed: it contained the redacted text"));
        }
    }
    for (name, js) in scripts {
        let text = match d.get_object(js) {
            Ok(Object::Stream(s)) => stream_text(s),
            Ok(o @ Object::String(..)) => decode(o),
            _ => continue,
        };
        if contains_any(&text, needles) {
            gone.insert(js);
            warnings.push(format!("JavaScript {name} removed: it contained the redacted text"));
        }
    }
    // A JavaScript action written inline (`/JS (…)`) is a string: dropped with its key.
    for o in d.objects.values_mut() {
        if let Object::Dictionary(dict) = o
            && dict.get(b"S").and_then(Object::as_name).ok() == Some(b"JavaScript".as_slice())
            && dict.get(b"JS").ok().is_some_and(|j| matches!(j, Object::String(..)) && contains_any(&decode(j), needles))
        {
            dict.remove(b"JS");
            warnings.push("JavaScript removed: it contained the redacted text".into());
            changed = true;
        }
    }
    if !gone.is_empty() {
        changed = true;
        remove_objects(&mut d, &gone);
    }
    // Private application data (/PieceInfo, on the catalog, pages or forms) holding the text.
    let ids: Vec<ObjectId> = d.objects.keys().copied().collect();
    for id in ids {
        let piece = match d.objects.get(&id) {
            Some(Object::Dictionary(x)) => x.get(b"PieceInfo").ok().cloned(),
            Some(Object::Stream(s)) => s.dict.get(b"PieceInfo").ok().cloned(),
            _ => None,
        };
        if let Some(p) = piece
            && holds(&d, &p, needles, &mut HashSet::new())
        {
            match d.objects.get_mut(&id) {
                Some(Object::Dictionary(x)) => x.remove(b"PieceInfo"),
                Some(Object::Stream(s)) => s.dict.remove(b"PieceInfo"),
                _ => None,
            };
            warn(&mut warnings, "Private application data removed: it contained the redacted text");
            changed = true;
        }
    }
    // Form XObjects no page paints (an unused resource, a form hung off private data, an
    // annotation appearance) that hold the text: removed, with every reference to them. When a
    // page's content can't be read, what it paints is unknown: nothing is removed, and the
    // audit refuses if the text is left.
    // What nothing points to any more (a removed /PieceInfo's forms) goes first.
    d.prune_objects();
    let reach = Reach::of(&d);
    if !reach.broken {
        let hidden: HashSet<ObjectId> = d
            .objects
            .iter()
            .filter(|(id, o)| {
                !reach.painted.contains(id)
                    && o.as_stream().is_ok_and(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form".as_slice()))
            })
            .filter(|(_, o)| holds(&d, o, needles, &mut HashSet::new()))
            .map(|(id, _)| *id)
            .collect();
        if !hidden.is_empty() {
            remove_objects(&mut d, &hidden);
            warn(&mut warnings, "Hidden content removed: it contained the redacted text");
            changed = true;
        }
        // Painted content keeps its drawing; what it says outside the drawing (comments,
        // marked-content properties, inline image dictionaries) is scrubbed.
        changed |= scrub_painted_streams(&mut d, &reach, needles);
        let reach = Reach::of(&d);
        changed |= remove_unused_assets(&mut d, &reach, needles, &mut warnings);
    }
    if !changed {
        return Ok((bytes.to_vec(), warnings));
    }
    d.prune_objects();
    let mut out = Vec::new();
    d.save_to(&mut out)?;
    Ok((out, warnings))
}

/// Streams named for the user: ("a.txt", its object).
type Named = Vec<(String, ObjectId)>;

/// The embedded files (`/EmbeddedFile` streams, with the file name of their file
/// specification) and the JavaScript held in streams (`/JS` of every JavaScript action).
fn embedded_and_scripts(d: &lopdf::Document) -> (Named, Named) {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    let mut scripts = Vec::new();
    for o in d.objects.values() {
        let dict = match o {
            Object::Dictionary(x) => x,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        if let Ok(Object::Dictionary(ef)) = dict.get(b"EF") {
            let name = [&b"UF"[..], b"F"].iter().find_map(|k| dict.get(k).ok().map(decode)).unwrap_or_else(|| "(unnamed)".into());
            for (_, v) in ef.iter() {
                if let Ok(r) = v.as_reference()
                    && seen.insert(r)
                {
                    files.push((name.clone(), r));
                }
            }
        }
        if dict.get(b"S").and_then(Object::as_name).ok() == Some(b"JavaScript".as_slice())
            && let Ok(r) = dict.get(b"JS").and_then(Object::as_reference)
        {
            scripts.push(("in an action".to_string(), r));
        }
    }
    // Streams typed /EmbeddedFile that no file specification names.
    for (id, o) in &d.objects {
        if let Object::Stream(s) = o
            && s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"EmbeddedFile".as_slice())
            && seen.insert(*id)
        {
            files.push(("(unnamed)".into(), *id));
        }
    }
    (files, scripts)
}

/// Removes from the form every field whose widget was dropped (with a parent left without
/// kids), so a blacked-out page keeps no field values. A field met twice (a malformed form
/// whose /Kids loop back) is kept as it is the second time.
fn prune_fields(d: &mut lopdf::Document, dropped: &HashSet<ObjectId>) -> Result<()> {
    fn keep(d: &mut lopdf::Document, list: Vec<Object>, dropped: &HashSet<ObjectId>, seen: &mut HashSet<ObjectId>) -> Vec<Object> {
        let mut out = Vec::new();
        for o in list {
            let Ok(id) = o.as_reference() else {
                out.push(o);
                continue;
            };
            if dropped.contains(&id) {
                continue;
            }
            if !seen.insert(id) {
                out.push(o);
                continue;
            }
            let kids = d.get_dictionary(id).ok().and_then(|f| f.get(b"Kids").ok()).and_then(|k| k.as_array().ok()).cloned();
            if let Some(kids) = kids {
                let had = !kids.is_empty();
                let left = keep(d, kids, dropped, seen);
                if had && left.is_empty() {
                    continue;
                }
                if let Ok(f) = d.get_object_mut(id).and_then(Object::as_dict_mut) {
                    f.set("Kids", left);
                }
            }
            out.push(o);
        }
        out
    }
    let Ok(af) = d.catalog().and_then(|c| c.get(b"AcroForm")).cloned() else { return Ok(()) };
    let af_id = af.as_reference().ok();
    let af_dict = match af_id {
        Some(r) => d.get_dictionary(r)?.clone(),
        None => af.as_dict()?.clone(),
    };
    let Ok(fields) = af_dict.get(b"Fields").and_then(|f| d.dereference(f)).map(|(_, f)| f.as_array().cloned().unwrap_or_default()) else { return Ok(()) };
    let fields = keep(d, fields, dropped, &mut HashSet::new());
    let mut af_dict = af_dict;
    af_dict.set("Fields", fields);
    match af_id {
        Some(r) => d.objects.insert(r, Object::Dictionary(af_dict)).map(|_| ()).unwrap_or(()),
        None => {
            let root = d.trailer.get(b"Root")?.as_reference()?;
            d.get_object_mut(root)?.as_dict_mut()?.set("AcroForm", af_dict);
        }
    }
    Ok(())
}

/// Scrubs every painted stream of `d` (see [`scrub_painted`]); returns whether one changed.
fn scrub_painted_streams(d: &mut lopdf::Document, reach: &Reach, needles: &[Needle]) -> bool {
    let mut changed = false;
    for id in &reach.painted {
        let Some(Object::Stream(s)) = d.objects.get_mut(id) else { continue };
        if let Some(b) = scrub_painted(&decoded(s), needles) {
            s.set_plain_content(b);
            changed = true;
        }
    }
    changed
}

/// Fonts and colour spaces listed in a `/Resources` that no painted content or appearance uses,
/// whose dictionary, program or character map holds one of `needles`: removed from every
/// resource dictionary, each named in a warning.
fn remove_unused_assets(d: &mut lopdf::Document, reach: &Reach, needles: &[Needle], warnings: &mut Warnings) -> bool {
    let mut gone: Vec<(Vec<u8>, ObjectId, &'static str)> = Vec::new();
    for o in d.objects.values() {
        let dict = match o {
            Object::Dictionary(x) => x,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        let Some(res) = sub_dict(d, dict, b"Resources") else { continue };
        for (cat, label) in [(&b"Font"[..], "font"), (b"ColorSpace", "colour profile")] {
            let Some(list) = sub_dict(d, res, cat) else { continue };
            for (name, v) in list.iter() {
                let refs: Vec<ObjectId> = match resolve(d, v) {
                    _ if v.as_reference().is_ok() => vec![v.as_reference().unwrap_or((0, 0))],
                    Some(Object::Array(a)) => a.iter().filter_map(|x| x.as_reference().ok()).collect(),
                    _ => Vec::new(),
                };
                for r in refs {
                    if reach.assets.contains(&r) || gone.iter().any(|g| g.1 == r) {
                        continue;
                    }
                    let mut seen = HashSet::new();
                    let hit = d.get_object(r).is_ok_and(|o| holds(d, o, needles, &mut seen)) || seen.iter().any(|x| d.get_object(*x).is_ok_and(|o| o.as_stream().is_ok_and(|s| contains_any(&cmap_text(&decoded(s)), needles))));
                    if hit {
                        gone.push((name.clone(), r, label));
                    }
                }
            }
        }
    }
    if gone.is_empty() {
        return false;
    }
    for (name, _, label) in &gone {
        let n = String::from_utf8_lossy(name);
        if *label == "font" {
            warn(warnings, &format!("Unused font {n} removed: it contained the redacted text"));
        } else {
            warn(warnings, &format!("Unused colour profile {n} removed: it contained the redacted text"));
        }
    }
    let ids: HashSet<ObjectId> = gone.iter().map(|g| g.1).collect();
    remove_objects(d, &ids);
    true
}

/// What [`scrub_any`] did to a value.
#[derive(PartialEq)]
enum Scrubbed {
    Kept,
    Changed,
    /// Nothing of the value is left (an emptied string, a name holding the text): its key goes.
    Drop,
}

/// Removes `needles` from `o` and everything nested in it: strings lose the matched text, names
/// holding it (other than [`PDF_NAMES`]) go, arrays and dictionaries are read through.
/// References are not followed. In an array an emptied string stays as `()`, so the positions
/// of the others (a choice field's /Opt pairs) do not move.
fn scrub_any(o: &mut Object, needles: &[Needle]) -> Scrubbed {
    match o {
        Object::String(..) => match scrub(&decode(o), needles) {
            None => Scrubbed::Kept,
            Some(t) if t.trim().is_empty() => Scrubbed::Drop,
            Some(t) => {
                *o = lopdf::text_string(&t);
                Scrubbed::Changed
            }
        },
        Object::Name(n) => {
            let n = String::from_utf8_lossy(n);
            if !PDF_NAMES.contains(&n.as_ref()) && contains_any(&n, needles) { Scrubbed::Drop } else { Scrubbed::Kept }
        }
        Object::Array(a) => {
            let mut changed = false;
            let mut i = 0;
            while i < a.len() {
                match scrub_any(&mut a[i], needles) {
                    Scrubbed::Kept => {}
                    Scrubbed::Changed => changed = true,
                    Scrubbed::Drop if matches!(a[i], Object::String(..)) => {
                        a[i] = Object::String(Vec::new(), lopdf::StringFormat::Literal);
                        changed = true;
                    }
                    Scrubbed::Drop => {
                        a.remove(i);
                        changed = true;
                        continue;
                    }
                }
                i += 1;
            }
            match (changed, a.is_empty()) {
                (true, true) => Scrubbed::Drop,
                (true, false) => Scrubbed::Changed,
                _ => Scrubbed::Kept,
            }
        }
        Object::Dictionary(d) => {
            if scrub_dict_all(d, needles) { Scrubbed::Changed } else { Scrubbed::Kept }
        }
        _ => Scrubbed::Kept,
    }
}

/// [`scrub_any`] on every value of `d`; a dropped value loses its key.
pub(super) fn scrub_dict_all(d: &mut Dictionary, needles: &[Needle]) -> bool {
    let keys: Vec<Vec<u8>> = d.iter().map(|(k, _)| k.clone()).collect();
    let mut changed = false;
    for k in keys {
        let Ok(v) = d.get_mut(&k) else { continue };
        match scrub_any(v, needles) {
            Scrubbed::Kept => {}
            Scrubbed::Changed => changed = true,
            Scrubbed::Drop => {
                d.remove(&k);
                changed = true;
            }
        }
    }
    changed
}

/// Whether `o`, or anything it holds or points to (references followed once), contains one of
/// `needles`: strings, names, and streams read as text, as XMP and as content.
fn holds(d: &lopdf::Document, o: &Object, needles: &[Needle], seen: &mut HashSet<ObjectId>) -> bool {
    match o {
        Object::Reference(r) => seen.insert(*r) && d.get_object(*r).is_ok_and(|x| holds(d, x, needles, seen)),
        Object::String(..) => contains_any(&decode(o), needles),
        Object::Name(n) => {
            let n = String::from_utf8_lossy(n);
            !PDF_NAMES.contains(&n.as_ref()) && contains_any(&n, needles)
        }
        Object::Array(a) => a.iter().any(|x| holds(d, x, needles, seen)),
        Object::Dictionary(x) => x.iter().any(|(_, v)| holds(d, v, needles, seen)),
        Object::Stream(s) => {
            let raw = decoded(s);
            contains_any(&String::from_utf8_lossy(&raw), needles)
                || contains_any(&xmp_text(&raw), needles)
                || contains_any(&shown_strings(&raw), needles)
                || s.dict.iter().any(|(_, v)| holds(d, v, needles, seen))
        }
        _ => false,
    }
}

/// Removes the objects `gone` and every reference to them: dictionary keys pointing to one
/// (/Metadata, an /EF entry, an /XObject resource, an /AP state) and array entries.
fn remove_objects(d: &mut lopdf::Document, gone: &HashSet<ObjectId>) {
    fn unref(o: &mut Object, gone: &HashSet<ObjectId>) {
        match o {
            Object::Array(a) => {
                a.retain(|x| !x.as_reference().is_ok_and(|r| gone.contains(&r)));
                a.iter_mut().for_each(|x| unref(x, gone));
            }
            Object::Dictionary(x) => unref_dict(x, gone),
            Object::Stream(s) => unref_dict(&mut s.dict, gone),
            _ => {}
        }
    }
    fn unref_dict(x: &mut Dictionary, gone: &HashSet<ObjectId>) {
        let keys: Vec<Vec<u8>> = x.iter().filter(|(_, v)| v.as_reference().is_ok_and(|r| gone.contains(&r))).map(|(k, _)| k.clone()).collect();
        for k in keys {
            x.remove(&k);
        }
        x.iter_mut().for_each(|(_, v)| unref(v, gone));
    }
    for id in gone {
        d.objects.remove(id);
    }
    for o in d.objects.values_mut() {
        unref(o, gone);
    }
    unref_dict(&mut d.trailer, gone);
}

fn warn(warnings: &mut Warnings, w: &str) {
    if !warnings.iter().any(|x| x == w) {
        warnings.push(w.to_string());
    }
}
