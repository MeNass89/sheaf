//! iLovePDF's long-page destination, applied to locally printed paper pages. Stacking preserves
//! paper breaks; it does not emulate a browser viewport, JavaScript or screen-media CSS.

use crate::{paths::atomic_write, OpdfError, Result};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use std::{collections::BTreeMap, path::{Path, PathBuf}};

#[derive(Clone, Copy)]
struct Placement {
    dx: f64,
    dy: f64,
    top: f64,
}

fn unsupported() -> OpdfError {
    OpdfError::Unsupported("Cannot stack this printed layout safely. Turn off Stack pages into one long page".into())
}

fn inherited(pdf: &Document, mut id: ObjectId, key: &[u8]) -> Result<Option<Object>> {
    for _ in 0..64 {
        let dict = pdf.get_dictionary(id)?;
        if let Ok(value) = dict.get(key) { return Ok(Some(pdf.dereference(value)?.1.clone())); }
        let Ok(parent) = dict.get(b"Parent").and_then(Object::as_reference) else { return Ok(None); };
        id = parent;
    }
    Err(unsupported())
}

fn number(value: &Object) -> Result<f64> {
    let n = f64::from(value.as_float()?);
    if !n.is_finite() { return Err(unsupported()); }
    Ok(n)
}

fn rect(value: &Object) -> Result<[f64; 4]> {
    let v = value.as_array()?;
    if v.len() != 4 { return Err(unsupported()); }
    let r = [number(&v[0])?, number(&v[1])?, number(&v[2])?, number(&v[3])?];
    if r[2] <= r[0] || r[3] <= r[1] { return Err(unsupported()); }
    Ok(r)
}

/// Stack vectors, searchable text and local pictures as Forms; link rectangles and internal
/// destinations move with their source page. Existing output/source files are never replaced.
pub(super) fn stack(input: &Path, out: &Path) -> Result<PathBuf> {
    let mut pdf = Document::load(input)?;
    let pages = pdf.get_pages();
    if pages.is_empty() || pages.len() > 1000 || pdf.is_encrypted() || pdf.catalog()?.has(b"StructTreeRoot") || pdf.catalog()?.has(b"AcroForm") { return Err(unsupported()); }
    let mut boxes = Vec::new();
    for &id in pages.values() {
        if inherited(&pdf, id, b"Rotate")?.is_some_and(|v| v.as_i64().map_or(true, |n| n.rem_euclid(360) != 0)) { return Err(unsupported()); }
        if inherited(&pdf, id, b"UserUnit")?.is_some_and(|v| v.as_float().map_or(true, |n| (n - 1.0).abs() > 1e-6)) { return Err(unsupported()); }
        let media = inherited(&pdf, id, b"MediaBox")?.ok_or_else(unsupported)?;
        let crop = inherited(&pdf, id, b"CropBox")?.unwrap_or(media);
        boxes.push(rect(&crop)?);
    }
    let width = boxes.iter().map(|r| r[2] - r[0]).fold(0.0, f64::max);
    let height: f64 = boxes.iter().map(|r| r[3] - r[1]).sum();
    if !height.is_finite() || height > 1_000_000.0 { return Err(unsupported()); }
    // ISO 32000's 14,400 default-unit limit: UserUnit keeps the physical size without clipping.
    let unit = (width.max(height) / 14_400.0).max(1.0);
    let root_pages = pdf.new_object_id();
    let long_page = pdf.new_object_id();
    let mut xobjects = Dictionary::new();
    let mut annotations = Vec::new();
    let mut drawing = String::new();
    let mut placements = BTreeMap::new();
    let mut top = height;
    for ((index, &id), crop) in pages.iter().zip(boxes) {
        let h = crop[3] - crop[1];
        let p = Placement { dx: -crop[0], dy: top - h - crop[1], top };
        placements.insert(id, p);
        let resources = inherited(&pdf, id, b"Resources")?.unwrap_or_else(|| dictionary! {}.into());
        let data = pdf.get_page_content_with_limit(id, 32 * 1024 * 1024)?;
        let mut form_dict = dictionary! {
            "Type" => "XObject", "Subtype" => "Form", "FormType" => 1,
            "BBox" => crop.into_iter().map(|v| Object::Real(v as f32)).collect::<Vec<_>>(), "Resources" => resources
        };
        if let Ok(group) = pdf.get_dictionary(id)?.get(b"Group") { form_dict.set("Group", group.clone()); }
        let form = pdf.add_object(Stream::new(form_dict, data));
        let name = format!("Paper{index}");
        xobjects.set(name.as_bytes(), form);
        drawing.push_str(&format!("q {} 0 0 {} {} {} cm /{name} Do Q\n", 1.0 / unit, 1.0 / unit, p.dx / unit, p.dy / unit));
        let page = pdf.get_dictionary(id)?.clone();
        if let Ok(annots) = page.get(b"Annots") {
            let annots = pdf.dereference(annots)?.1.as_array()?.clone();
            for annotation in annots {
                let mut annotation = pdf.dereference(&annotation)?.1.as_dict()?.clone();
                if annotation.get(b"Subtype").and_then(Object::as_name)? != b"Link" { return Err(unsupported()); }
                let r = rect(pdf.dereference(annotation.get(b"Rect")?)?.1)?;
                annotation.set("Rect", vec![Object::Real(((r[0] + p.dx) / unit) as f32), Object::Real(((r[1] + p.dy) / unit) as f32), Object::Real(((r[2] + p.dx) / unit) as f32), Object::Real(((r[3] + p.dy) / unit) as f32)]);
                if let Ok(quads) = annotation.get(b"QuadPoints") {
                    let quads = pdf.dereference(quads)?.1.as_array()?;
                    if quads.len() % 8 != 0 { return Err(unsupported()); }
                    let moved = quads.iter().enumerate().map(|(i, v)| Ok(Object::Real(((number(v)? + if i % 2 == 0 { p.dx } else { p.dy }) / unit) as f32))).collect::<Result<Vec<_>>>()?;
                    annotation.set("QuadPoints", moved);
                }
                annotation.set("P", long_page);
                annotations.push(pdf.add_object(annotation).into());
            }
        }
        top -= h;
    }
    let stream = pdf.add_object(Stream::new(dictionary! {}, drawing.into_bytes()));
    pdf.objects.insert(root_pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![long_page.into()] }.into());
    pdf.objects.insert(long_page, dictionary! {
        "Type" => "Page", "Parent" => root_pages, "MediaBox" => vec![0.into(), 0.into(), Object::Real((width / unit) as f32), Object::Real((height / unit) as f32)],
        "UserUnit" => unit, "Resources" => dictionary! { "XObject" => xobjects }, "Contents" => stream, "Annots" => annotations
    }.into());
    pdf.catalog_mut()?.set("Pages", root_pages);
    pdf.catalog_mut()?.remove(b"PageLabels");
    // Destinations can live in outline dictionaries, name trees, actions or separate objects.
    for object in pdf.objects.values_mut() { move_destinations(object, &placements, long_page, unit, 0)?; }
    pdf.version = "1.7".into();
    pdf.prune_objects();
    atomic_write(out, |part| { pdf.save(part)?; Ok(()) })
}

fn move_destinations(object: &mut Object, pages: &BTreeMap<ObjectId, Placement>, target: ObjectId, unit: f64, depth: usize) -> Result<()> {
    if depth > 64 { return Err(unsupported()); }
    match object {
        Object::Array(values) => {
            let placement = values.first().and_then(|v| v.as_reference().ok()).and_then(|id| pages.get(&id));
            if let Some(p) = placement.filter(|_| values.get(1).is_some_and(|v| v.as_name().is_ok())) {
                let mode = values[1].as_name()?;
                let at = |index, offset| -> Result<Object> {
                    Ok(match values.get(index) {
                        Some(Object::Null) | None => Object::Real((offset / unit) as f32),
                        Some(v) => Object::Real(((number(v)? + offset) / unit) as f32),
                    })
                };
                let xyz = |left, top| vec![target.into(), Object::Name(b"XYZ".to_vec()), left, top, Object::Null];
                let moved = match mode {
                    b"XYZ" => {
                        let left = at(2, p.dx)?;
                        let top = if matches!(values.get(3), Some(Object::Null) | None) { Object::Real((p.top / unit) as f32) } else { at(3, p.dy)? };
                        vec![target.into(), Object::Name(b"XYZ".to_vec()), left, top, values.get(4).cloned().unwrap_or(Object::Null)]
                    }
                    b"Fit" | b"FitB" => xyz(0.into(), Object::Real((p.top / unit) as f32)),
                    b"FitH" | b"FitBH" => xyz(0.into(), if matches!(values.get(2), Some(Object::Null) | None) { Object::Real((p.top / unit) as f32) } else { at(2, p.dy)? }),
                    b"FitV" | b"FitBV" => xyz(at(2, p.dx)?, Object::Real((p.top / unit) as f32)),
                    b"FitR" => vec![target.into(), Object::Name(b"FitR".to_vec()), at(2, p.dx)?, at(3, p.dy)?, at(4, p.dx)?, at(5, p.dy)?],
                    _ => return Err(unsupported()),
                };
                *values = moved;
            } else {
                for value in values { move_destinations(value, pages, target, unit, depth + 1)?; }
            }
        }
        Object::Dictionary(dict) => { for (_, value) in dict.iter_mut() { move_destinations(value, pages, target, unit, depth + 1)?; } }
        Object::Stream(stream) => { for (_, value) in stream.dict.iter_mut() { move_destinations(value, pages, target, unit, depth + 1)?; } }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn printed(path: &Path, height: f32) {
        let mut pdf = Document::with_version("1.4");
        let pages = pdf.new_object_id();
        let second = pdf.new_object_id();
        let font = pdf.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
        let resources = dictionary! { "Font" => dictionary! { "F" => font } };
        let link = pdf.add_object(dictionary! { "Type" => "Annot", "Subtype" => "Link", "Rect" => vec![10.into(), 10.into(), 80.into(), 30.into()], "Dest" => vec![second.into(), Object::Name(b"XYZ".to_vec()), 10.into(), 30.into(), Object::Null] });
        let uri = pdf.add_object(dictionary! { "Type" => "Annot", "Subtype" => "Link", "Rect" => vec![10.into(), 10.into(), 80.into(), 30.into()], "A" => dictionary! { "S" => "URI", "URI" => Object::string_literal("https://example.invalid/") } });
        let first_stream = pdf.add_object(Stream::new(dictionary! {}, b"BT /F 12 Tf 10 40 Td (First page) Tj ET".to_vec()));
        let second_stream = pdf.add_object(Stream::new(dictionary! {}, b"BT /F 12 Tf 10 40 Td (Second page) Tj ET".to_vec()));
        let first = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Resources" => resources.clone(), "Contents" => first_stream, "Annots" => vec![link.into()] });
        pdf.objects.insert(second, dictionary! { "Type" => "Page", "Parent" => pages, "Resources" => resources, "Contents" => second_stream, "Annots" => vec![uri.into()] }.into());
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 2, "Kids" => vec![first.into(), second.into()], "MediaBox" => vec![0.into(), 0.into(), 100.into(), height.into()] }.into());
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages, "Dests" => dictionary! { "chapter" => vec![second.into(), Object::Name(b"Fit".to_vec())] } });
        pdf.trailer.set("Root", root);
        pdf.save(path).unwrap();
    }

    #[test]
    fn long_local_output_keeps_vector_content_link_rectangles_and_internal_destinations() {
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("printed.pdf");
        printed(&input, 200.0);
        let original = std::fs::read(&input).unwrap();
        let out = stack(&input, &t.path().join("long.pdf")).unwrap();
        let pdf = Document::load(out).unwrap();
        assert_eq!(pdf.get_pages().len(), 1);
        let id = pdf.get_pages()[&1];
        let page = pdf.get_dictionary(id).unwrap();
        assert_eq!(rect(page.get(b"MediaBox").unwrap()).unwrap(), [0.0, 0.0, 100.0, 400.0]);
        let resources = page.get(b"Resources").unwrap().as_dict().unwrap();
        let forms = resources.get(b"XObject").unwrap().as_dict().unwrap();
        for (name, text) in [(b"Paper1".as_slice(), "First page"), (b"Paper2".as_slice(), "Second page")] {
            let form = pdf.get_object(forms.get(name).unwrap().as_reference().unwrap()).unwrap().as_stream().unwrap();
            assert!(String::from_utf8_lossy(&form.get_plain_content().unwrap()).contains(text));
            assert!(form.dict.get(b"Resources").unwrap().as_dict().unwrap().has(b"Font"));
        }
        let annotations = page.get(b"Annots").unwrap().as_array().unwrap();
        let first = pdf.get_dictionary(annotations[0].as_reference().unwrap()).unwrap();
        assert_eq!(rect(first.get(b"Rect").unwrap()).unwrap(), [10.0, 210.0, 80.0, 230.0]);
        let dest = first.get(b"Dest").unwrap().as_array().unwrap();
        assert_eq!(dest[0].as_reference().unwrap(), id);
        assert_eq!(dest[3].as_float().unwrap(), 30.0);
        let second = pdf.get_dictionary(annotations[1].as_reference().unwrap()).unwrap();
        assert_eq!(second.get(b"A").unwrap().as_dict().unwrap().get(b"URI").unwrap().as_str().unwrap(), b"https://example.invalid/");
        let dest = pdf.catalog().unwrap().get(b"Dests").unwrap().as_dict().unwrap().get(b"chapter").unwrap().as_array().unwrap();
        assert_eq!(dest[0].as_reference().unwrap(), id);
        assert_eq!(dest[1].as_name().unwrap(), b"XYZ");
        assert_eq!(dest[3].as_float().unwrap(), 200.0);
        assert_eq!(std::fs::read(input).unwrap(), original);
    }

    #[test]
    fn very_long_local_output_scales_page_units_and_rejects_unsupported_rotation_atomically() {
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("printed.pdf");
        printed(&input, 8000.0);
        let out = stack(&input, &t.path().join("long.pdf")).unwrap();
        let pdf = Document::load(out).unwrap();
        let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
        let unit = f64::from(page.get(b"UserUnit").unwrap().as_float().unwrap());
        let media = rect(page.get(b"MediaBox").unwrap()).unwrap();
        assert!((media[3] * unit - 16_000.0).abs() < 0.1);
        assert!(media[3] <= 14_400.0 && unit > 1.0);
        let mut pdf = Document::load(&input).unwrap();
        let page = pdf.get_pages()[&1];
        pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap().set("Rotate", 90);
        pdf.save(&input).unwrap();
        let out = t.path().join("unsupported.pdf");
        assert!(stack(&input, &out).unwrap_err().to_string().contains("Turn off Stack"));
        assert!(!out.exists());
    }
}
