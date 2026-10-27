//! Sheaf: Acrobat's Add Printer Marks, with the existing trim and bleed kept intact.

use pdfcraft_cos::{Dict, Document, Object, Stream};

use crate::EditError;

/// Add black crop marks outside each selected page's bleed. The sheet grows by 18 points past
/// the bleed for the marks; the original artwork stays clipped to its previous crop. No bleed
/// artwork is invented, and the effective trim, bleed and art boxes are kept.
pub fn add_crop_marks(doc: &mut Document, pages: &[usize]) -> Result<(), EditError> {
    let all = pdfcraft_model::pages(doc);
    let boxes = pdfcraft_organize::page_boxes(doc)?;
    for &page in pages {
        let p = all.get(page).ok_or_else(|| EditError::Invalid(format!("page {} does not exist", page + 1)))?;
        if matches!(p.dict.get(b"SheafCropMarks"), Some(Object::Bool(true))) {
            continue;
        }
        let [media, crop, bleed, trim, art] = boxes[page];
        let unit = p.dict.get(b"UserUnit").map(|u| doc.resolve(u)).and_then(|u| u.as_f64()).filter(|u| u.is_finite() && *u > 0.0).unwrap_or(1.0).min(pdfcraft_model::MAX_USER_UNIT);
        let margin = 18.0 / unit;
        let gap = 3.0 / unit;
        let length = 9.0 / unit;
        let paper = [media[0].min(bleed[0] - margin), media[1].min(bleed[1] - margin), media[2].max(bleed[2] + margin), media[3].max(bleed[3] + margin)];
        let [x0, y0, x1, y1] = trim;
        let mut marks = format!("Q\nq /Artifact BMC 0 G {} w\n", 0.25 / unit);
        for y in [y0, y1] {
            for (a, b) in [(bleed[0] - gap - length, bleed[0] - gap), (bleed[2] + gap, bleed[2] + gap + length)] {
                marks.push_str(&format!("{a} {y} m {b} {y} l S\n"));
            }
        }
        for x in [x0, x1] {
            for (a, b) in [(bleed[1] - gap - length, bleed[1] - gap), (bleed[3] + gap, bleed[3] + gap + length)] {
                marks.push_str(&format!("{x} {a} m {x} {b} l S\n"));
            }
        }
        marks.push_str("EMC Q\n");
        let first = doc.add(Stream::from_raw(Dict::new(), format!("q {} {} {} {} re W n\n", crop[0], crop[1], crop[2] - crop[0], crop[3] - crop[1]).into_bytes()));
        let last = doc.add(Stream::from_raw(Dict::new(), marks.into_bytes()));
        let mut contents = vec![Object::Ref(first)];
        if let Some(c) = p.dict.get(b"Contents") {
            match &*doc.resolve(c) {
                Object::Array(a) => contents.extend(a.iter().cloned()),
                _ => contents.push(c.clone()),
            }
        }
        contents.push(Object::Ref(last));
        let rect = |r: [f64; 4]| Object::Array(r.into_iter().map(Object::Real).collect());
        doc.update_dict(p.obj, |d| {
            d.set(b"MediaBox".to_vec(), rect(paper));
            d.set(b"CropBox".to_vec(), rect(paper));
            d.set(b"TrimBox".to_vec(), rect(trim));
            d.set(b"BleedBox".to_vec(), rect(bleed));
            d.set(b"ArtBox".to_vec(), rect(art));
            d.set(b"Contents".to_vec(), Object::Array(contents));
            d.set(b"SheafCropMarks".to_vec(), Object::Bool(true));
        }).map_err(|e| EditError::Write(e.to_string()))?;
    }
    Ok(())
}
