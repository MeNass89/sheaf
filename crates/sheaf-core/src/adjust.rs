//! Adjust colours (Stirling's Adjust Colours/Contrast, Preview's Adjust Color): brightness,
//! contrast and saturation changed, or the colours inverted (a dark page for reading at night),
//! text and drawings still vector.
//!
//! The colours set in the pages' and forms' content (`rg`, `g`, `k` and their stroking forms, and
//! `sc`/`scn` in grey, RGB or CMYK spaces; CMYK written as RGB) and the 8-bit grey and RGB pictures
//! are changed alike. The paper and the default black change too: each page is painted first in
//! what white becomes and starts drawing in what black becomes, so an inverted page is light on
//! dark. Left as they are, and counted: gradients, spot and indexed colours, CMYK JPEGs.

use crate::grayscale::{components, luma, num, page_resources};
use crate::pages::Input;
use crate::paths::atomic_write;
use crate::Result;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::path::{Path, PathBuf};

/// The change asked: each of brightness, contrast and saturation from −1 to 1 (0 leaves it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Adjust {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub invert: bool,
}

impl Adjust {
    fn tone(&self, c: f32) -> f32 {
        let c = (c - 0.5) * (1.0 + self.contrast.clamp(-1.0, 1.0)) + 0.5 + self.brightness.clamp(-1.0, 1.0);
        let c = c.clamp(0.0, 1.0);
        if self.invert { 1.0 - c } else { c }
    }

    /// An RGB colour (0–1) as it becomes.
    pub fn rgb(&self, [r, g, b]: [f32; 3]) -> [f32; 3] {
        let l = luma(r, g, b);
        let s = 1.0 + self.saturation.clamp(-1.0, 1.0);
        [r, g, b].map(|c| self.tone(l + (c - l) * s))
    }

    pub fn gray(&self, g: f32) -> f32 {
        self.tone(g)
    }

    fn is_identity(&self) -> bool {
        *self == Adjust::default()
    }
}

fn cmyk_rgb(c: f32, m: f32, y: f32, k: f32) -> [f32; 3] {
    [(1.0 - c) * (1.0 - k), (1.0 - m) * (1.0 - k), (1.0 - y) * (1.0 - k)]
}

fn reals(v: &[f32]) -> Vec<Object> {
    v.iter().map(|&x| Object::Real((x * 10000.0).round() / 10000.0)).collect()
}

/// `input` with its colours changed as `a` asks, written to `out`. Returns how many colour
/// settings and pictures were left as they were.
pub fn adjust_colours(input: &Input, a: &Adjust, out: &Path) -> Result<(PathBuf, usize)> {
    let mut doc = input.load_lopdf()?;
    if doc.is_encrypted() || doc.encryption_state.is_some() {
        doc.trailer.remove(b"Encrypt");
        doc.encryption_state = None;
    }
    let kept = adjust_doc(&mut doc, a);
    let path = atomic_write(out, |part| {
        doc.save(part)?;
        Ok(())
    })?;
    Ok((path, kept))
}

/// [`adjust_colours`] on a loaded document; returns what was left.
pub fn adjust_doc(doc: &mut Document, a: &Adjust) -> usize {
    if a.is_identity() {
        return 0;
    }
    let mut kept = 0;
    let mut jobs: Vec<(ObjectId, Option<Dictionary>)> = Vec::new();
    for (_, page) in doc.get_pages() {
        let res = page_resources(doc, page);
        for c in doc.get_page_contents(page) {
            jobs.push((c, res.clone()));
        }
    }
    for (id, o) in &doc.objects {
        if let Ok(s) = o.as_stream()
            && s.dict.get(b"Subtype").and_then(Object::as_name).is_ok_and(|n| n == b"Form")
        {
            let res = s.dict.get(b"Resources").ok().and_then(|r| doc.dereference(r).ok()).and_then(|(_, r)| r.as_dict().ok().cloned());
            jobs.push((*id, res));
        }
    }
    jobs.sort_by_key(|j| j.0);
    jobs.dedup_by_key(|j| j.0);
    for (id, res) in jobs {
        let Some(data) = doc.get_object(id).ok().and_then(|o| o.as_stream().ok()).and_then(|s| s.decompressed_content().ok()) else { continue };
        let Ok(content) = Content::decode(&data) else { continue };
        let (ops, changed, k) = adjust_ops(doc, res.as_ref(), content.operations, a);
        kept += k;
        if changed
            && let Ok(bytes) = (Content { operations: ops }).encode()
            && let Ok(Object::Stream(s)) = doc.get_object_mut(id)
        {
            s.set_plain_content(bytes);
            let _ = s.compress();
        }
    }
    // The paper in what white becomes, the default black in what black becomes.
    let (paper, ink) = (a.gray(1.0), a.gray(0.0));
    if (paper - 1.0).abs() > 1e-3 || ink.abs() > 1e-3 {
        for (_, page) in doc.get_pages() {
            let media = doc.get_dictionary(page).ok().and_then(|p| p.get(b"MediaBox").ok().and_then(|m| m.as_array().ok()).map(|m| m.iter().filter_map(num).collect::<Vec<_>>()));
            let Some(&[x0, y0, x1, y1]) = media.as_deref() else { continue };
            let first = format!("q {paper} g {x0} {y0} {} {} re f Q {ink} g {ink} G\n", x1 - x0, y1 - y0);
            let id = doc.add_object(lopdf::Stream::new(Dictionary::new(), first.into_bytes()));
            let mut contents: Vec<Object> = vec![Object::Reference(id)];
            contents.extend(doc.get_page_contents(page).into_iter().map(Object::Reference));
            if let Ok(p) = doc.get_object_mut(page).and_then(Object::as_dict_mut) {
                p.set("Contents", contents);
            }
        }
    }
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        let Some(Ok(n)) = doc.get_object(id).ok().and_then(|o| o.as_stream().ok()).filter(|s| s.dict.get(b"Subtype").and_then(Object::as_name).is_ok_and(|n| n == b"Image")).map(|s| {
            if s.dict.get(b"ImageMask").and_then(Object::as_bool).unwrap_or(false) { return Err(()) }
            s.dict.get(b"ColorSpace").ok().and_then(|c| components(doc, c)).ok_or(())
        }) else {
            continue;
        };
        let done = matches!(n, 1 | 3 | 4) && doc.get_object_mut(id).and_then(Object::as_stream_mut).is_ok_and(|s| adjust_image(s, n, a));
        if !done {
            kept += 1;
        }
    }
    kept
}

/// The content's colours changed; whether any were, and how many settings were left.
fn adjust_ops(doc: &Document, res: Option<&Dictionary>, ops: Vec<Operation>, a: &Adjust) -> (Vec<Operation>, bool, usize) {
    let spaces = res.and_then(|r| r.get(b"ColorSpace").ok()).and_then(|c| doc.dereference(c).ok()).and_then(|(_, c)| c.as_dict().ok().cloned());
    // The fill's and the stroke's space: its components (1, 3, 4) when changed here, else 0.
    let (mut fill, mut stroke) = (1u8, 1u8);
    let mut saved: Vec<(u8, u8)> = Vec::new();
    let (mut out, mut changed, mut kept) = (Vec::with_capacity(ops.len()), false, 0);
    let vals = |o: &Operation| o.operands.iter().map(num).collect::<Option<Vec<f32>>>();
    let colour = |v: &[f32], n: u8| -> Option<Vec<f32>> {
        match (n, v) {
            (1, [g]) => Some(vec![a.gray(*g)]),
            (3, [r, g, b]) => Some(a.rgb([*r, *g, *b]).to_vec()),
            (4, [c, m, y, k]) => Some(a.rgb(cmyk_rgb(*c, *m, *y, *k)).to_vec()),
            _ => None,
        }
    };
    for o in ops {
        let new = match o.operator.as_str() {
            "q" => {
                saved.push((fill, stroke));
                None
            }
            "Q" => {
                (fill, stroke) = saved.pop().unwrap_or((1, 1));
                None
            }
            op @ ("g" | "G" | "rg" | "RG" | "k" | "K") => {
                let n = match op { "g" | "G" => 1, "rg" | "RG" => 3, _ => 4 };
                if op.eq_ignore_ascii_case("g") { if op == "g" { fill = 1 } else { stroke = 1 } }
                if op.eq_ignore_ascii_case("rg") { if op == "rg" { fill = 3 } else { stroke = 3 } }
                // CMYK is written as RGB.
                let to = match op { "k" => { fill = 3; "rg" } "K" => { stroke = 3; "RG" } other => other };
                vals(&o).and_then(|v| colour(&v, n)).map(|c| Operation::new(to, reals(&c)))
            }
            cs @ ("cs" | "CS") => {
                let n = o.operands.first().and_then(|name| {
                    let by_name = name.as_name().ok().and_then(|k| spaces.as_ref()?.get(k).ok().cloned());
                    components(doc, by_name.as_ref().unwrap_or(name))
                });
                let (mode, new) = match n {
                    Some(4) => (4, Some(Operation::new(cs, vec![Object::Name(b"DeviceRGB".to_vec())]))),
                    Some(m @ (1 | 3)) => (m, None),
                    _ => {
                        kept += 1;
                        (0, None)
                    }
                };
                if cs == "cs" { fill = mode } else { stroke = mode }
                new
            }
            s @ ("sc" | "scn" | "SC" | "SCN") => {
                let n = if s.starts_with('s') { fill } else { stroke };
                (n != 0).then(|| vals(&o).and_then(|v| colour(&v, n))).flatten().map(|c| Operation::new(s, reals(&c)))
            }
            _ => None,
        };
        match new {
            Some(n) => {
                changed = true;
                out.push(n);
            }
            None => out.push(o),
        }
    }
    (out, changed, kept)
}

/// A grey or RGB picture's pixels changed (stored losslessly); false when it cannot be read.
fn adjust_image(s: &mut lopdf::Stream, n: u8, a: &Adjust) -> bool {
    // A 1-bit grey picture (a scan in CCITT or JBIG2, read by no decoder here): its two values are
    // changed through /Decode, which maps each sample's 0 and 1 to a grey, the data left as it is
    // (Sol, 28/10: Invert left monochrome scans; Stirling inverts the rendered page).
    if n == 1 && s.dict.get(b"BitsPerComponent").and_then(Object::as_i64).ok() == Some(1) {
        let decode = match s.dict.get(b"Decode").and_then(Object::as_array) {
            Ok(d) if d.len() == 2 => d.iter().map(num).collect::<Option<Vec<f32>>>(),
            Ok(_) => None,
            Err(_) => Some(vec![0.0, 1.0]),
        };
        let Some(d) = decode else { return false };
        s.dict.set("Decode", d.iter().map(|&v| Object::Real(a.gray(v))).collect::<Vec<_>>());
        return true;
    }
    // CMYK: read as its RGB, written back in RGB (Sol, 26/10: CMYK pictures were left, so an
    // inverted page mixed both; 27/10: CMYK JPEGs too).
    if n == 4 {
        let jpeg = crate::compress::single_filter(s) == Some(b"DCTDecode");
        if jpeg {
            let Some(img) = crate::compress::cmyk_jpeg_rgb(s) else { return false };
            let data: Vec<u8> = img.pixels().flat_map(|p| a.rgb([p[0], p[1], p[2]].map(|c| c as f32 / 255.0)).map(|c| (c * 255.0).round() as u8)).collect();
            s.set_plain_content(data);
            s.dict.remove(b"Filter");
            s.dict.remove(b"DecodeParms");
            // The inverted /Decode of Adobe's CMYK JPEGs does not apply to the RGB written here.
            s.dict.remove(b"Decode");
            s.dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
            s.dict.set("BitsPerComponent", 8);
            let _ = s.compress();
            return true;
        }
        let (w, h) = (s.dict.get(b"Width").and_then(Object::as_i64).unwrap_or(0), s.dict.get(b"Height").and_then(Object::as_i64).unwrap_or(0));
        if jpeg || s.dict.get(b"BitsPerComponent").and_then(Object::as_i64).ok() != Some(8) || s.dict.has(b"Decode") {
            return false;
        }
        let Ok(raw) = s.decompressed_content() else { return false };
        if w <= 0 || h <= 0 || raw.len() != (w * h * 4) as usize {
            return false;
        }
        let data: Vec<u8> = raw.as_chunks::<4>().0.iter().flat_map(|p| a.rgb(cmyk_rgb(p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0)).map(|c| (c * 255.0).round() as u8)).collect();
        s.set_plain_content(data);
        s.dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
        let _ = s.compress();
        return true;
    }
    let Some(img) = crate::compress::image_pixels(s, n == 1) else { return false };
    let data: Vec<u8> = if n == 1 {
        img.to_luma8().into_raw().into_iter().map(|g| (a.gray(g as f32 / 255.0) * 255.0).round() as u8).collect()
    } else {
        img.to_rgb8().pixels().flat_map(|p| a.rgb([p[0], p[1], p[2]].map(|c| c as f32 / 255.0)).map(|c| (c * 255.0).round() as u8)).collect()
    };
    s.set_plain_content(data);
    s.dict.remove(b"Filter");
    s.dict.remove(b"DecodeParms");
    let _ = s.compress();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_change_as_asked() {
        let inv = Adjust { invert: true, ..Default::default() };
        assert_eq!(inv.rgb([1.0, 0.0, 0.0]), [0.0, 1.0, 1.0]);
        let grey = Adjust { saturation: -1.0, ..Default::default() };
        let g = grey.rgb([1.0, 0.0, 0.0]);
        assert!(g.iter().all(|c| (c - 0.299).abs() < 1e-4), "{g:?}");
        let bright = Adjust { brightness: 0.2, ..Default::default() };
        assert!((bright.gray(0.5) - 0.7).abs() < 1e-5 && bright.gray(0.9) == 1.0);
        let flat = Adjust { contrast: -1.0, ..Default::default() };
        assert_eq!(flat.gray(0.0), 0.5);
    }

    #[test]
    fn content_colours_are_changed_and_cmyk_becomes_rgb() {
        let doc = Document::with_version("1.7");
        let a = Adjust { invert: true, ..Default::default() };
        let ops = Content::decode(b"1 0 0 rg 0 G 0 0 0 1 k /DeviceRGB cs 0 0 1 sc /Pattern cs /P0 scn").unwrap().operations;
        let (out, changed, kept) = adjust_ops(&doc, None, ops, &a);
        assert!(changed);
        assert_eq!(kept, 1, "the pattern is left");
        let text = String::from_utf8(Content { operations: out }.encode().unwrap()).unwrap();
        for want in ["0 1 1 rg", "1 G", "1 1 1 rg", "1 1 0 sc", "/P0 scn"] {
            assert!(text.contains(want), "{want} in {text}");
        }
    }

    /// A 1-bit scan: its /Decode carries the change (black and white swapped by Invert), the
    /// CCITT data left as it is.
    #[test]
    fn a_one_bit_scan_is_changed_through_its_decode() {
        let dict = lopdf::dictionary! { "Subtype" => "Image", "Width" => 8, "Height" => 8, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 1, "Filter" => "CCITTFaxDecode" };
        let mut s = lopdf::Stream::new(dict, vec![1, 2, 3]);
        assert!(adjust_image(&mut s, 1, &Adjust { invert: true, ..Default::default() }));
        let d: Vec<f32> = s.dict.get(b"Decode").unwrap().as_array().unwrap().iter().filter_map(num).collect();
        assert_eq!(d, [1.0, 0.0]);
        assert_eq!(s.content, [1, 2, 3], "the data is not touched");
        // Inverted again: back to [0 1].
        assert!(adjust_image(&mut s, 1, &Adjust { invert: true, ..Default::default() }));
        let d: Vec<f32> = s.dict.get(b"Decode").unwrap().as_array().unwrap().iter().filter_map(num).collect();
        assert_eq!(d, [0.0, 1.0]);
    }

    /// A CMYK JPEG (ImageMagick's, Adobe's inverted CMYK, with the inverted /Decode PDF producers give
    /// it) is inverted too, written back in RGB.
    #[test]
    fn a_cmyk_jpeg_is_changed_too() {
        let dir = tempfile::tempdir().unwrap();
        let jpg = dir.path().join("red.jpg");
        let made = std::process::Command::new("convert").args(["-size", "8x8", "xc:red", "-colorspace", "CMYK"]).arg(&jpg).status();
        assert!(made.is_ok_and(|s| s.success()), "ImageMagick's convert makes the CMYK JPEG");
        let dict = lopdf::dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 8, "Height" => 8, "ColorSpace" => "DeviceCMYK", "BitsPerComponent" => 8, "Filter" => "DCTDecode",
            "Decode" => vec![1.into(), 0.into(), 1.into(), 0.into(), 1.into(), 0.into(), 1.into(), 0.into()] };
        let mut s = lopdf::Stream::new(dict, std::fs::read(&jpg).unwrap());
        assert!(adjust_image(&mut s, 4, &Adjust { invert: true, ..Default::default() }));
        assert_eq!(s.dict.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceRGB");
        assert!(!s.dict.has(b"Decode"), "the CMYK /Decode is dropped with the CMYK data");
        let px = s.decompressed_content().unwrap();
        assert_eq!(px.len(), 8 * 8 * 3);
        // Red inverted: cyan.
        assert!(px[0] < 60 && px[1] > 200 && px[2] > 200, "{:?}", &px[..3]);
    }
}
