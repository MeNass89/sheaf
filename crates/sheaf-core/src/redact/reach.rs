//! What the pages paint and draw with ([`Reach`]): content streams, the forms, patterns and
//! glyph procedures they paint, and the fonts, images and colour profiles they use. The
//! clean-up removes hidden content outside it; the audit exempts drawn content inside it.

use super::pdf::{decoded, page_resources, resolve, sub_dict};
use lopdf::{Dictionary, Object, ObjectId};
use std::collections::HashSet;

/// What the audit may skip because it is drawn (and so checked on the page by [`verify`]) or
/// is a drawing resource.
#[derive(Default)]
pub(super) struct Reach {
    /// Page content streams, Form XObjects painted by `Do` from them (recursively), tiling
    /// patterns and Type3 glyph procedures they use.
    pub(super) painted: HashSet<ObjectId>,
    /// Font dictionaries (with their descriptors, CMaps and programs), images (with their
    /// masks) and ICC profiles used by the painted streams or by annotation appearances.
    pub(super) assets: HashSet<ObjectId>,
    /// Font programs (/FontFile*) among the assets: their bytes are only noted, never refused.
    pub(super) programs: HashSet<ObjectId>,
    /// ToUnicode CMaps among the assets: their mapped text is only noted.
    pub(super) cmaps: HashSet<ObjectId>,
    /// Annotation appearance streams already walked (they are not exempt, so not in `painted`).
    appearances: HashSet<ObjectId>,
    /// Some content could not be parsed: what it paints is unknown.
    pub(super) broken: bool,
}

impl Reach {
    pub(super) fn of(d: &lopdf::Document) -> Reach {
        let mut r = Reach::default();
        for (_, page) in d.get_pages() {
            let res = page_resources(d, page);
            for c in d.get_page_contents(page) {
                if r.painted.insert(c) {
                    r.content(d, c, &res, true, 0);
                }
            }
            // Annotation appearances: their resources count as reached; the appearance
            // streams themselves are not exempt (verify does not read annotations).
            let annots = d.get_dictionary(page).ok().and_then(|p| p.get(b"Annots").ok()).and_then(|a| resolve(d, a)).and_then(|a| a.as_array().ok()).cloned().unwrap_or_default();
            for a in annots {
                let Some(ap) = resolve(d, &a).and_then(|a| a.as_dict().ok()).and_then(|a| sub_dict(d, a, b"AP")) else { continue };
                for (_, v) in ap.iter() {
                    let mut forms: Vec<ObjectId> = v.as_reference().into_iter().collect();
                    if let Some(Object::Dictionary(states)) = resolve(d, v) {
                        forms.extend(states.iter().filter_map(|(_, x)| x.as_reference().ok()));
                    }
                    for f in forms {
                        if r.appearances.insert(f) {
                            r.content(d, f, &res, false, 0);
                        }
                    }
                }
            }
        }
        // Output intents' ICC profiles describe the whole document's colour.
        let intents = d.catalog().ok().and_then(|c| c.get(b"OutputIntents").ok()).and_then(|o| resolve(d, o)).and_then(|o| o.as_array().ok()).cloned().unwrap_or_default();
        for i in intents {
            if let Some(p) = resolve(d, &i).and_then(|i| i.as_dict().ok()).and_then(|i| i.get(b"DestOutputProfile").and_then(Object::as_reference).ok()) {
                r.assets.insert(p);
            }
        }
        // The form's default resources: the fonts its fields are drawn with.
        let dr = d.catalog().ok().and_then(|c| sub_dict(d, c, b"AcroForm")).and_then(|a| sub_dict(d, a, b"DR")).cloned();
        if let Some(fonts) = dr.as_ref().and_then(|x| sub_dict(d, x, b"Font")).cloned() {
            for (_, f) in fonts.iter() {
                if let Ok(f) = f.as_reference() {
                    r.font(d, f, dr.as_ref().unwrap_or(&Dictionary::new()), 0);
                }
            }
        }
        r
    }

    /// Walks the stream `id` as content drawn with `parent` resources (or its own).
    fn content(&mut self, d: &lopdf::Document, id: ObjectId, parent: &Dictionary, painted: bool, depth: usize) {
        if depth > 32 {
            return;
        }
        let Ok(Object::Stream(s)) = d.get_object(id) else { return };
        let res = sub_dict(d, &s.dict, b"Resources").cloned().unwrap_or_else(|| parent.clone());
        let Ok(ops) = lopdf::content::Content::decode(&decoded(s)) else {
            self.broken = true;
            return;
        };
        let named = |cat: &[u8], op: &lopdf::content::Operation| -> Option<ObjectId> {
            let name = op.operands.iter().rev().find_map(|o| o.as_name().ok())?;
            sub_dict(d, &res, cat)?.get(name).ok()?.as_reference().ok()
        };
        for op in &ops.operations {
            match op.operator.as_str() {
                "Do" => {
                    let Some(x) = named(b"XObject", op) else { continue };
                    let Ok(Object::Stream(xs)) = d.get_object(x) else { continue };
                    match xs.dict.get(b"Subtype").and_then(Object::as_name).ok() {
                        Some(b"Form") => {
                            let first = if painted { self.painted.insert(x) } else { self.appearances.insert(x) };
                            if first {
                                self.content(d, x, &res, painted, depth + 1);
                            }
                        }
                        Some(b"Image") => self.image(d, x),
                        _ => {}
                    }
                }
                "Tf" => {
                    if let Some(f) = named(b"Font", op) {
                        self.font(d, f, &res, depth);
                    }
                }
                "cs" | "CS" => {
                    if let Some(cs) = op.operands.first().and_then(|o| o.as_name().ok()).and_then(|n| sub_dict(d, &res, b"ColorSpace")?.get(n).ok().cloned()) {
                        self.color_space(d, &cs, 0);
                    }
                }
                "scn" | "SCN" => {
                    if let Some(p) = named(b"Pattern", op)
                        && d.get_object(p).is_ok_and(|o| o.as_stream().is_ok())
                        && (if painted { self.painted.insert(p) } else { self.appearances.insert(p) })
                    {
                        self.content(d, p, &res, painted, depth + 1);
                    }
                }
                _ => {}
            }
        }
    }

    fn image(&mut self, d: &lopdf::Document, id: ObjectId) {
        if !self.assets.insert(id) {
            return;
        }
        let Ok(Object::Stream(s)) = d.get_object(id) else { return };
        for k in [&b"SMask"[..], b"Mask"] {
            if let Ok(m) = s.dict.get(k).and_then(Object::as_reference) {
                self.image(d, m);
            }
        }
        if let Ok(cs) = s.dict.get(b"ColorSpace") {
            self.color_space(d, &cs.clone(), 0);
        }
    }

    fn color_space(&mut self, d: &lopdf::Document, cs: &Object, depth: usize) {
        if depth > 8 {
            return;
        }
        if let Ok(r) = cs.as_reference() {
            if d.get_object(r).is_ok_and(|o| o.as_stream().is_ok()) {
                self.assets.insert(r);
            }
            if let Ok(o) = d.get_object(r) {
                let o = o.clone();
                self.color_space(d, &o, depth + 1);
            }
            return;
        }
        if let Object::Array(a) = cs {
            for x in a {
                if let Ok(r) = x.as_reference()
                    && d.get_object(r).is_ok_and(|o| o.as_stream().is_ok())
                {
                    // An ICC profile, an indexed lookup table or a tint function.
                    self.assets.insert(r);
                } else if matches!(x, Object::Array(_) | Object::Reference(_)) {
                    self.color_space(d, x, depth + 1);
                }
            }
        }
    }

    /// A font and everything it is made of: descriptor, programs, CMaps, descendants, Type3
    /// glyph procedures.
    fn font(&mut self, d: &lopdf::Document, id: ObjectId, parent: &Dictionary, depth: usize) {
        if !self.assets.insert(id) {
            return;
        }
        let Ok(f) = d.get_dictionary(id) else { return };
        let f = f.clone();
        for (k, v) in f.iter() {
            match k.as_slice() {
                b"FontDescriptor" | b"ToUnicode" | b"Encoding" | b"CIDSystemInfo" | b"CIDToGIDMap" => self.font_part(d, k, v),
                b"DescendantFonts" => {
                    let kids = resolve(d, v).and_then(|a| a.as_array().ok()).cloned().unwrap_or_default();
                    for kid in kids {
                        if let Ok(r) = kid.as_reference() {
                            self.font(d, r, parent, depth);
                        }
                    }
                }
                b"CharProcs" => {
                    let res = sub_dict(d, &f, b"Resources").cloned().unwrap_or_else(|| parent.clone());
                    let procs: Vec<ObjectId> = resolve(d, v).and_then(|p| p.as_dict().ok()).map(|p| p.iter().filter_map(|(_, x)| x.as_reference().ok()).collect()).unwrap_or_default();
                    for p in procs {
                        if self.painted.insert(p) {
                            self.content(d, p, &res, true, depth + 1);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// A descriptor, CMap or encoding of a font, and the streams it points to (FontFile*,
    /// CIDSet, an embedded CMap's /UseCMap, …).
    fn font_part(&mut self, d: &lopdf::Document, key: &[u8], v: &Object) {
        let Ok(r) = v.as_reference() else { return };
        if !self.assets.insert(r) {
            return;
        }
        if key == b"ToUnicode" {
            self.cmaps.insert(r);
        }
        let dict = match d.get_object(r) {
            Ok(Object::Dictionary(x)) => x.clone(),
            Ok(Object::Stream(s)) => s.dict.clone(),
            _ => return,
        };
        for (k, x) in dict.iter() {
            if let Ok(xr) = x.as_reference()
                && d.get_object(xr).is_ok_and(|o| o.as_stream().is_ok())
            {
                self.assets.insert(xr);
                if k.starts_with(b"FontFile") {
                    self.programs.insert(xr);
                }
            }
        }
    }
}
