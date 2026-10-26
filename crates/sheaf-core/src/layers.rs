//! Layers shown or hidden on screen (Acrobat's Layers panel): a copy of the document whose default
//! layer configuration hides the layers asked for, for pdfium to draw. The file is not changed.
//!
//! The copy is the original bytes with one incremental update that rewrites the object holding
//! `/OCProperties`' `/D`: `/BaseState /ON`, `/OFF` the hidden layers, the automatic states (`/AS`)
//! dropped so they cannot turn a hidden layer back on. An encrypted document is written whole,
//! decrypted, since the copy only ever lives in memory.

use lopdf::{Dictionary, Document, IncrementalDocument, Object, ObjectId};

/// `bytes` (opened with `password`) with the layers in `off` hidden and every other one shown.
pub fn shown(bytes: &[u8], password: Option<&str>, off: &[(u32, u16)]) -> Result<Vec<u8>, String> {
    let mut doc = match password {
        Some(p) if !p.is_empty() => Document::load_mem_with_options(bytes, lopdf::LoadOptions::with_password(p)),
        _ => Document::load_mem(bytes),
    }
    .map_err(|e| e.to_string())?;
    let config = |d: &mut Dictionary| {
        d.set("BaseState", Object::Name(b"ON".to_vec()));
        d.set("OFF", Object::Array(off.iter().map(|&(n, g)| Object::Reference((n, g))).collect()));
        d.remove(b"ON");
        d.remove(b"AS");
    };
    let holder = holder_of_default(&doc)?;
    let mut out = Vec::new();
    if doc.is_encrypted() || doc.encryption_state.is_some() {
        doc.trailer.remove(b"Encrypt");
        doc.encryption_state = None;
        edit_default(&mut doc, holder, config)?;
        doc.save_to(&mut out).map_err(|e| e.to_string())?;
        return Ok(out);
    }
    let mut inc = IncrementalDocument::create_from(bytes.to_vec(), doc);
    inc.opt_clone_object_to_new_document(holder).map_err(|e| e.to_string())?;
    // `/D` kept as its own object is read from the previous revision.
    let default = default_of(inc.get_prev_documents(), holder);
    edit_with(&mut inc.new_document, holder, default, config)?;
    inc.save_to(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The object whose dictionary holds `/D` directly: `/OCProperties` itself when it is an indirect
/// object, else the catalog.
fn holder_of_default(doc: &Document) -> Result<ObjectId, String> {
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|_| "the document has no catalog")?;
    let catalog = doc.get_dictionary(root).map_err(|e| e.to_string())?;
    match catalog.get(b"OCProperties") {
        Ok(Object::Reference(id)) => Ok(*id),
        Ok(Object::Dictionary(_)) => Ok(root),
        _ => Err("the document has no layers".into()),
    }
}

/// `/OCProperties` in `holder`'s dictionary: the dictionary itself, or its inline entry.
fn props(d: &mut Dictionary) -> Option<&mut Dictionary> {
    if d.has(b"OCGs") { Some(d) } else { d.get_mut(b"OCProperties").and_then(Object::as_dict_mut).ok() }
}

/// The default configuration `/D` as a dictionary (resolved when it is its own object).
fn default_of(doc: &Document, holder: ObjectId) -> Dictionary {
    let Some(d) = doc.get_dictionary(holder).ok().cloned().as_mut().and_then(|d| props(d).cloned()) else { return Dictionary::new() };
    match d.get(b"D") {
        Ok(Object::Reference(id)) => doc.get_dictionary(*id).cloned().unwrap_or_default(),
        Ok(Object::Dictionary(d)) => d.clone(),
        _ => Dictionary::new(),
    }
}

fn edit_default(doc: &mut Document, holder: ObjectId, f: impl FnOnce(&mut Dictionary)) -> Result<(), String> {
    let default = default_of(doc, holder);
    edit_with(doc, holder, default, f)
}

/// `f` applied to `default`, written inline as `holder`'s `/D` (the original `/D` object, if it
/// was one, is left to the revision before).
fn edit_with(doc: &mut Document, holder: ObjectId, mut default: Dictionary, f: impl FnOnce(&mut Dictionary)) -> Result<(), String> {
    f(&mut default);
    let d = doc.get_object_mut(holder).and_then(Object::as_dict_mut).map_err(|e| e.to_string())?;
    props(d).ok_or("the layers' properties are not where expected")?.set("D", Object::Dictionary(default));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Stream, dictionary};

    /// A page drawing two squares, each in a layer (`inline` keeps /OCProperties in the catalog).
    fn two_layers(inline: bool) -> (Vec<u8>, ObjectId, ObjectId) {
        let mut d = Document::with_version("1.7");
        let red = d.add_object(dictionary! { "Type" => "OCG", "Name" => Object::string_literal("Red") });
        let blue = d.add_object(dictionary! { "Type" => "OCG", "Name" => Object::string_literal("Blue") });
        let content = b"/OC /R BDC 1 0 0 rg 10 10 40 40 re f EMC /OC /B BDC 0 0 1 rg 60 10 40 40 re f EMC".to_vec();
        let c = d.add_object(Stream::new(dictionary! {}, content));
        let pages = d.new_object_id();
        let page = d.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 110.into(), 60.into()], "Contents" => c,
            "Resources" => dictionary! { "Properties" => dictionary! { "R" => red, "B" => blue } },
        });
        d.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
        // Blue starts hidden, and an automatic state that would show it for viewing.
        let props = dictionary! {
            "OCGs" => vec![red.into(), blue.into()],
            "D" => dictionary! { "OFF" => vec![blue.into()], "AS" => vec![Object::Dictionary(dictionary! { "Event" => "View", "OCGs" => vec![blue.into()] })] },
        };
        let props = if inline { Object::Dictionary(props) } else { Object::Reference(d.add_object(props)) };
        let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages, "OCProperties" => props });
        d.trailer.set("Root", cat);
        let mut out = Vec::new();
        d.save_to(&mut out).unwrap();
        (out, red, blue)
    }

    fn colours(bytes: &[u8]) -> (bool, bool) {
        let doc = crate::engine::open_pdf_bytes(bytes, None).unwrap();
        let page = doc.pages().get(0).unwrap();
        let bmp = page.render_with_config(&pdfium_render::prelude::PdfRenderConfig::new().set_target_width(110)).unwrap();
        let img = bmp.as_image().unwrap().to_rgba8();
        let red = img.get_pixel(30, img.height() - 30);
        let blue = img.get_pixel(80, img.height() - 30);
        (red[0] > 200 && red[2] < 80, blue[2] > 200 && blue[0] < 80)
    }

    #[test]
    fn hidden_layers_are_left_out_of_the_pages_drawn_and_the_file_is_kept() {
        for inline in [true, false] {
            let (bytes, red, blue) = two_layers(inline);
            let _g = crate::engine::pdfium_lock();
            assert_eq!(colours(&bytes), (true, false), "as the file asks (inline {inline})");
            let both = shown(&bytes, None, &[]).unwrap();
            assert_eq!(&both[..bytes.len()], &bytes[..], "the original revision is kept whole");
            assert_eq!(colours(&both), (true, true), "every layer shown (inline {inline})");
            assert_eq!(colours(&shown(&bytes, None, &[red]).unwrap()), (false, true), "red hidden (inline {inline})");
            assert_eq!(colours(&shown(&bytes, None, &[red, blue]).unwrap()), (false, false));
        }
    }
}
