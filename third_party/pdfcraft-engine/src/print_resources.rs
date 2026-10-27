//! Acrobat's Preflight resource inventory. These are declarations, including unused resources;
//! this does not claim to simulate painted separations, overprint or total area coverage.

use pdfcraft_cos::{Dict, Document, ObjRef, Object};
use std::collections::{BTreeSet, HashSet};
use super::Finding;

#[derive(Default)]
struct Inventory {
    spots: BTreeSet<String>,
    overprint: bool,
    transparency: bool,
    incomplete: bool,
    seen: HashSet<ObjRef>,
    remaining: usize,
}

pub(super) fn findings(doc: &Document, pages: &[pdfcraft_model::Page]) -> Vec<Finding> {
    let mut result = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let mut inventory = Inventory { remaining: 100_000, ..Default::default() };
        if let Some(resources) = page.dict.get(b"Resources") { inventory.visit(doc, resources, 0); }
        if let Some(group) = page.dict.get(b"Group") { inventory.visit(doc, group, 0); }
        let page = (i + 1) as u32;
        let mut push = |id, message| result.push(Finding { id, severity: "warn", message, page: Some(page) });
        if !inventory.spots.is_empty() {
            push("spot-colour", format!("Page {page} declares spot colour resources: {}. Confirm these plates with the printer; declarations may be unused.", inventory.spots.into_iter().collect::<Vec<_>>().join(", ")));
        }
        if inventory.overprint {
            push("overprint", format!("Page {page} declares overprint enabled in its resources. Ink interaction is not simulated; a declared state may be unused."));
        }
        if inventory.transparency {
            push("transparency", format!("Page {page} declares transparency, a soft mask or a blend mode. No flattening was performed; declarations may be unused."));
        }
        if inventory.incomplete {
            push("resource-inventory-incomplete", format!("Page {page} resource inventory reached its depth or object limit. Spot colours, overprint and transparency have not been fully inventoried."));
        }
    }
    result
}

impl Inventory {
    fn visit(&mut self, doc: &Document, object: &Object, depth: usize) {
        if depth > 64 || self.remaining == 0 { self.incomplete = true; return; }
        if let Object::Ref(id) = object {
            if !self.seen.insert(*id) { return; }
        }
        self.remaining -= 1;
        let value = doc.resolve(object);
        match &*value {
            Object::Array(values) => {
                match values.first().and_then(Object::as_name) {
                    Some(b"Separation") => {
                        if let Some(name) = values.get(1).map(|v| doc.resolve(v)).and_then(|v| v.as_name().map(|n| String::from_utf8_lossy(n).into_owned())) {
                            if name != "None" { self.spots.insert(name); }
                        }
                    }
                    Some(b"DeviceN") => {
                        if let Some(names) = values.get(1).map(|v| doc.resolve(v)).and_then(|v| v.as_array().cloned()) {
                            for name in names.iter().filter_map(Object::as_name) {
                                if !matches!(name, b"None" | b"Cyan" | b"Magenta" | b"Yellow" | b"Black") {
                                    self.spots.insert(String::from_utf8_lossy(name).into_owned());
                                }
                            }
                        }
                    }
                    _ => {}
                }
                for value in values { self.visit(doc, value, depth + 1); }
            }
            Object::Dict(dict) => self.dictionary(doc, dict, depth),
            Object::Stream(stream) => self.dictionary(doc, &stream.dict, depth),
            _ => {}
        }
    }

    fn dictionary(&mut self, doc: &Document, dict: &Dict, depth: usize) {
        self.overprint |= [b"OP".as_slice(), b"op".as_slice()].iter()
            .any(|key| dict.get(key).is_some_and(|v| matches!(&*doc.resolve(v), Object::Bool(true))));
        self.transparency |= [b"ca".as_slice(), b"CA".as_slice()].iter()
            .any(|key| dict.get(key).map(|v| doc.resolve(v)).and_then(|v| v.as_f64()).is_some_and(|v| v < 1.0));
        self.transparency |= dict.name(b"S") == Some(b"Transparency");
        self.transparency |= dict.get(b"SMask").is_some_and(|v| !matches!(&*doc.resolve(v), Object::Name(n) if n == b"None") && !matches!(&*doc.resolve(v), Object::Null));
        if let Some(blend) = dict.get(b"BM").map(|v| doc.resolve(v)) {
            let unusual = |v: &Object| v.as_name().is_some_and(|n| !matches!(n, b"Normal" | b"Compatible"));
            self.transparency |= unusual(&blend) || blend.as_array().is_some_and(|a| a.iter().any(unusual));
        }
        for (_, value) in dict.iter() { self.visit(doc, value, depth + 1); }
    }
}
