//! Sheaf: Acrobat's Add Printer Marks, with the existing trim and bleed kept intact.

use pdfcraft_cos::{Dict, Document, Object, Stream};

use crate::EditError;

/// Add black crop marks outside each selected page's bleed. The sheet grows by 18 points past
/// the bleed for the marks; the original artwork stays clipped to its previous crop. No bleed
/// artwork is invented, and the effective trim, bleed and art boxes are kept.
pub fn add_crop_marks(doc: &mut Document, pages: &[usize]) -> Result<(), EditError> {
    add_printer_marks(doc, pages, false, false, false, 0.25)
}

/// Acrobat's printer marks: registration targets use the All separation (all process/spot plates),
/// colour bars use process CMYK, and page information embeds the bundled Inter font once per job.
pub fn add_printer_marks(doc: &mut Document, pages: &[usize], registration: bool, colour_bars: bool, page_information: bool, width: f64) -> Result<(), EditError> {
    if !width.is_finite() || !(0.1..=2.0).contains(&width) {
        return Err(EditError::Invalid("mark width must be 0.1 to 2 points".into()));
    }
    let requested = 1 | if registration { 2 } else { 0 } | if colour_bars { 4 } else { 0 } | if page_information { 8 } else { 0 };
    let mut info_fonts = Dict::new();
    let mut info_font = None;
    let all = pdfcraft_model::pages(doc);
    let boxes = pdfcraft_organize::page_boxes(doc)?;
    for &page in pages {
        let p = all.get(page).ok_or_else(|| EditError::Invalid(format!("page {} does not exist", page + 1)))?;
        let existing = p.dict.get(b"SheafPrinterMarks").and_then(Object::as_int)
            .unwrap_or(if matches!(p.dict.get(b"SheafCropMarks"), Some(Object::Bool(true))) { 1 } else { 0 });
        if existing & requested == requested { continue; }
        let add = requested & !existing;
        let mut resources = p.dict.get(b"Resources").and_then(|r| doc.dict(r)).unwrap_or_default();
        let [media, crop, bleed, trim, art] = boxes[page];
        let unit = p.dict.get(b"UserUnit").map(|u| doc.resolve(u)).and_then(|u| u.as_f64()).filter(|u| u.is_finite() && *u > 0.0).unwrap_or(1.0).min(pdfcraft_model::MAX_USER_UNIT);
        let margin = if requested == 1 { 18.0 / unit } else { 30.0 / unit };
        let gap = 3.0 / unit;
        let length = 9.0 / unit;
        let paper = [media[0].min(bleed[0] - margin), media[1].min(bleed[1] - margin), media[2].max(bleed[2] + margin), media[3].max(bleed[3] + margin)];
        let [x0, y0, x1, y1] = trim;
        let mut marks = format!("Q\nq /Artifact BMC 0 G {} w\n", width / unit);
        if add & 1 != 0 {
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
        }
        if add & 2 != 0 {
            let mut spaces = resources.get(b"ColorSpace").and_then(|v| doc.dict(v)).unwrap_or_default();
            let name = free_name(&spaces, "SheafRegistration");
            let mut tint = Dict::new();
            tint.set(b"FunctionType".to_vec(), 2);
            tint.set(b"Domain".to_vec(), numbers(&[0.0, 1.0]));
            tint.set(b"C0".to_vec(), numbers(&[0.0; 4]));
            tint.set(b"C1".to_vec(), numbers(&[1.0; 4]));
            tint.set(b"N".to_vec(), 1);
            spaces.set(name.as_bytes().to_vec(), Object::Array(vec![Object::name("Separation"), Object::name("All"), Object::name("DeviceCMYK"), Object::Dict(tint)]));
            resources.set(b"ColorSpace".to_vec(), Object::Dict(spaces));
            marks.push_str(&format!("/{name} CS 1 SCN\n"));
            let radius = 3.0 / unit;
            for (x, y) in [((bleed[0] + bleed[2]) / 2.0, bleed[1] - 10.0 / unit), ((bleed[0] + bleed[2]) / 2.0, bleed[3] + 10.0 / unit), (bleed[0] - 10.0 / unit, (bleed[1] + bleed[3]) / 2.0), (bleed[2] + 10.0 / unit, (bleed[1] + bleed[3]) / 2.0)] {
                let r = radius; let k = r * 0.5522847498;
                marks.push_str(&format!("{} {y} m\n", x+r));
                for [a,b,c,d,e,f] in [[x+r,y+k,x+k,y+r,x,y+r], [x-k,y+r,x-r,y+k,x-r,y], [x-r,y-k,x-k,y-r,x,y-r], [x+k,y-r,x+r,y-k,x+r,y]] {
                    marks.push_str(&format!("{a} {b} {c} {d} {e} {f} c\n"));
                }
                marks.push_str("S\n");
                marks.push_str(&format!("{} {y} m {} {y} l {x} {} m {x} {} l S\n", x-r*1.5, x+r*1.5, y-r*1.5, y+r*1.5));
            }
        }
        if add & 4 != 0 {
            let colours = [[1.0,0.0,0.0,0.0], [0.0,1.0,0.0,0.0], [0.0,0.0,1.0,0.0], [0.0,0.0,0.0,1.0], [1.0,1.0,0.0,0.0], [0.0,1.0,1.0,0.0], [1.0,0.0,1.0,0.0]];
            let size = (6.0 / unit).min((bleed[2] - bleed[0]) / 9.0);
            for (i, [c,m,y,k]) in colours.into_iter().enumerate() {
                marks.push_str(&format!("{c} {m} {y} {k} k {} {} {size} {size} re f\n", bleed[0] + (i as f64 + 1.0) * size, bleed[3] + 20.0 / unit));
            }
        }
        if add & 8 != 0 {
            if info_font.is_none() {
                info_font = Some(pdfcraft_fonts::embedding::embed(doc, &mut info_fonts, pdfcraft_fonts::INTER_REGULAR, "Inter-Regular").map_err(EditError::Invalid)?);
            }
            if let Some(font) = &info_font {
                let source = info_fonts.get(font.as_bytes()).cloned().ok_or_else(|| EditError::Invalid("the marks font is missing".into()))?;
                let mut fonts = resources.get(b"Font").and_then(|r| doc.dict(r)).unwrap_or_default();
                let name = free_name(&fonts, "SheafMarksFont");
                let dict = doc.dict(&source).ok_or_else(|| EditError::Invalid("the marks font is unreadable".into()))?;
                let metrics = pdfcraft_fonts::pdf::Metrics::from_dict(doc, &dict);
                let text = format!("Page {} / {} | {:.1} x {:.1} mm", page + 1, all.len(), (trim[2]-trim[0])*unit*25.4/72.0, (trim[3]-trim[1])*unit*25.4/72.0);
                let encoded = metrics.encode(&text).ok_or_else(|| EditError::Invalid("page information cannot be encoded".into()))?;
                let hex: String = encoded.iter().map(|b| format!("{b:02X}")).collect();
                // Fit information to the trim width even on small labels; bars sit on the other edge.
                let size = (7.0 / unit).min((trim[2]-trim[0]) / (text.len() as f64 * 0.65));
                fonts.set(name.as_bytes().to_vec(), source);
                resources.set(b"Font".to_vec(), Object::Dict(fonts));
                marks.push_str(&format!("0 g BT /{name} {size} Tf 1 0 0 1 {} {} Tm <{hex}> Tj ET\n", bleed[0], bleed[1] - 26.0 / unit));
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
            d.set(b"SheafPrinterMarks".to_vec(), Object::Int(existing | requested));
            d.set(b"Resources".to_vec(), Object::Dict(resources));
        }).map_err(|e| EditError::Write(e.to_string()))?;
    }
    Ok(())
}

fn numbers(values: &[f64]) -> Object { Object::Array(values.iter().copied().map(Object::Real).collect()) }

fn free_name(dict: &Dict, prefix: &str) -> String {
    let mut index = 0;
    loop {
        let name = format!("{prefix}{index}");
        if !dict.contains(name.as_bytes()) { return name; }
        index += 1;
    }
}
