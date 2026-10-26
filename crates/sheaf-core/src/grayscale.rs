//! Compress ▸ Convert to grayscale (Stirling's compress option, Acrobat's Convert Colors to
//! Gray): the document's colours as greys, text and drawings still vector.
//!
//! Colours set in the pages' and forms' content (`rg`, `RG`, `k`, `K`, and `sc`/`scn` in an RGB or
//! CMYK space chosen by `cs`/`CS`) are written as the grey of the same lightness (Rec. 601 luma;
//! CMYK through its RGB). RGB and CMYK pictures (8 bits, Flate or JPEG) are stored again in
//! DeviceGray, and gradients (shadings drawn by exponential or stitched functions) run between
//! greys. Left as they are, and counted: sampled gradients, spot and indexed colours, and
//! pictures this cannot decode faithfully.

use crate::compress::{image_pixels, single_filter};
use image::codecs::jpeg::JpegEncoder;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

/// What the conversion did: streams of content rewritten, pictures made grey, and the colour
/// settings and pictures left in colour.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GrayReport {
    pub contents: usize,
    pub images: usize,
    pub kept: usize,
}

/// Every page's content, every form XObject (appearances included) and every picture of `doc`
/// made grey.
pub fn to_gray(doc: &mut Document) -> GrayReport {
    let mut report = GrayReport::default();
    // Content streams with the resources their colour space names are looked up in.
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
        let (ops, changed, kept) = gray_ops(doc, res.as_ref(), content.operations);
        report.kept += kept;
        if changed {
            let Ok(bytes) = (Content { operations: ops }).encode() else { continue };
            if let Ok(Object::Stream(s)) = doc.get_object_mut(id) {
                s.set_plain_content(bytes);
                let _ = s.compress();
                report.contents += 1;
            }
        }
    }
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for &id in &ids {
        match gray_shading(doc, id) {
            Some(true) => report.contents += 1,
            Some(false) => report.kept += 1,
            None => {}
        }
    }
    for id in ids {
        let comps = match doc.get_object(id).ok().and_then(|o| o.as_stream().ok()) {
            Some(s) if s.dict.get(b"Subtype").and_then(Object::as_name).is_ok_and(|n| n == b"Image") => image_components(doc, s),
            _ => continue,
        };
        match comps {
            Some(1) => {}
            Some(n @ (3 | 4)) => {
                if let Ok(Object::Stream(s)) = doc.get_object_mut(id)
                    && gray_image(s, n)
                {
                    report.images += 1;
                } else {
                    report.kept += 1;
                }
            }
            _ => report.kept += 1,
        }
    }
    report
}

/// Object `id` when it is a shading (a gradient, alone or a pattern's): made grey when its colours
/// are RGB or CMYK given by exponential functions (or stitched ones of those); `Some(false)` when
/// left in colour, `None` when not a shading or grey already.
fn gray_shading(doc: &mut Document, id: ObjectId) -> Option<bool> {
    let dict = match doc.get_object(id).ok()? {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &s.dict,
        _ => return None,
    };
    dict.get(b"ShadingType").ok()?;
    let n = components(doc, dict.get(b"ColorSpace").ok()?)?;
    if n == 1 {
        return None;
    }
    if n != 3 && n != 4 {
        return Some(false);
    }
    let function = dict.get(b"Function").ok()?.clone();
    let background = dict.get(b"Background").ok().and_then(|b| b.as_array().ok()).and_then(|b| gray_of(b, n));
    let mut funcs = Vec::new();
    if !function_ids(doc, &function, &mut funcs) {
        return Some(false);
    }
    for f in funcs {
        let Ok(d) = doc.get_object_mut(f).and_then(Object::as_dict_mut) else { continue };
        for key in [&b"C0"[..], b"C1"] {
            let g = d.get(key).ok().and_then(|c| c.as_array().ok()).and_then(|c| gray_of(c, n)).unwrap_or(if key == b"C0" { 0.0 } else { 1.0 });
            d.set(key, vec![Object::Real(g)]);
        }
    }
    let d = match doc.get_object_mut(id).ok()? {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &mut s.dict,
        _ => return None,
    };
    d.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
    if let Some(g) = background {
        d.set("Background", vec![Object::Real(g)]);
    }
    Some(true)
}

/// The exponential functions (type 2) `f` is made of, as objects of their own; false when one is
/// of another type or written inline.
fn function_ids(doc: &Document, f: &Object, out: &mut Vec<ObjectId>) -> bool {
    let Ok(id) = f.as_reference() else { return false };
    let Ok(d) = doc.get_dictionary(id) else { return false };
    match d.get(b"FunctionType").and_then(Object::as_i64) {
        Ok(2) => {
            if !out.contains(&id) {
                out.push(id);
            }
            true
        }
        Ok(3) => d.get(b"Functions").and_then(Object::as_array).is_ok_and(|fs| fs.iter().all(|f| function_ids(doc, f, out))),
        _ => false,
    }
}

pub(crate) fn page_resources(doc: &Document, page: ObjectId) -> Option<Dictionary> {
    let (own, inherited) = doc.get_page_resources(page).ok()?;
    let mut res = own.cloned().unwrap_or_default();
    for id in inherited {
        if let Ok(d) = doc.get_dictionary(id) {
            for (k, v) in d.iter() {
                if !res.has(k) {
                    res.set(k.clone(), v.clone());
                }
            }
        }
    }
    Some(res)
}

/// The number of components a colour space gives (1 grey, 3 RGB, 4 CMYK), for the families
/// converted; None for the others (patterns, indexed, spot colours, Lab).
pub(crate) fn components(doc: &Document, cs: &Object) -> Option<u8> {
    let cs = doc.dereference(cs).ok()?.1;
    match cs {
        Object::Name(n) => match n.as_slice() {
            b"DeviceGray" | b"G" | b"CalGray" => Some(1),
            b"DeviceRGB" | b"RGB" => Some(3),
            b"DeviceCMYK" | b"CMYK" => Some(4),
            _ => None,
        },
        Object::Array(a) => match doc.dereference(a.first()?).ok()?.1.as_name().ok()? {
            b"CalGray" => Some(1),
            b"CalRGB" => Some(3),
            b"ICCBased" => {
                let (_, s) = doc.dereference(a.get(1)?).ok()?;
                match s.as_stream().ok()?.dict.get(b"N").and_then(Object::as_i64).ok()? {
                    n @ (1 | 3 | 4) => Some(n as u8),
                    _ => None,
                }
            }
            _ => None,
        },
        _ => None,
    }
}

/// A picture's components when it is one this converts or is grey already.
fn image_components(doc: &Document, s: &Stream) -> Option<u8> {
    if s.dict.get(b"ImageMask").and_then(Object::as_bool).unwrap_or(false) {
        return Some(1);
    }
    components(doc, s.dict.get(b"ColorSpace").ok()?)
}

pub(crate) fn luma(r: f32, g: f32, b: f32) -> f32 {
    (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 1.0)
}

fn cmyk_gray(c: f32, m: f32, y: f32, k: f32) -> f32 {
    luma((1.0 - c) * (1.0 - k), (1.0 - m) * (1.0 - k), (1.0 - y) * (1.0 - k))
}

pub(crate) fn num(o: &Object) -> Option<f32> {
    o.as_float().ok().or_else(|| o.as_i64().ok().map(|i| i as f32))
}

/// The grey of `vals` read in a space of `n` components.
fn gray_of(vals: &[Object], n: u8) -> Option<f32> {
    let v: Vec<f32> = vals.iter().map(num).collect::<Option<_>>()?;
    match (n, v.as_slice()) {
        (3, [r, g, b]) => Some(luma(*r, *g, *b)),
        (4, [c, m, y, k]) => Some(cmyk_gray(*c, *m, *y, *k)),
        _ => None,
    }
}

fn op(name: &str, operands: Vec<Object>) -> Operation {
    Operation::new(name, operands)
}

/// `ops` with their colours made grey; whether any changed, and how many colour settings were
/// left (a space this does not convert).
fn gray_ops(doc: &Document, res: Option<&Dictionary>, ops: Vec<Operation>) -> (Vec<Operation>, bool, usize) {
    let spaces = res.and_then(|r| r.get(b"ColorSpace").ok()).and_then(|c| doc.dereference(c).ok()).and_then(|(_, c)| c.as_dict().ok().cloned());
    // The components of the fill's and the stroke's space when converted (else 0), saved by q.
    let (mut fill, mut stroke) = (0u8, 0u8);
    let mut saved: Vec<(u8, u8)> = Vec::new();
    let (mut out, mut changed, mut kept) = (Vec::with_capacity(ops.len()), false, 0);
    for o in ops {
        let gray = |o: &Operation, n: u8, to: &str| gray_of(&o.operands, n).map(|g| op(to, vec![Object::Real(g)]));
        let new = match o.operator.as_str() {
            "q" => {
                saved.push((fill, stroke));
                None
            }
            "Q" => {
                (fill, stroke) = saved.pop().unwrap_or((0, 0));
                None
            }
            "rg" => gray(&o, 3, "g"),
            "RG" => gray(&o, 3, "G"),
            "k" => gray(&o, 4, "g"),
            "K" => gray(&o, 4, "G"),
            cs @ ("cs" | "CS") => {
                let n = o.operands.first().and_then(|name| {
                    let by_name = name.as_name().ok().and_then(|k| spaces.as_ref()?.get(k).ok().cloned());
                    components(doc, by_name.as_ref().unwrap_or(name))
                });
                let conv = match n {
                    Some(n @ (3 | 4)) => n,
                    Some(_) => 0,
                    None => {
                        kept += 1;
                        0
                    }
                };
                if cs == "cs" { fill = conv } else { stroke = conv }
                (conv != 0).then(|| op(cs, vec![Object::Name(b"DeviceGray".to_vec())]))
            }
            s @ ("sc" | "scn") if fill != 0 => gray(&o, fill, s),
            s @ ("SC" | "SCN") if stroke != 0 => gray(&o, stroke, s),
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

/// Picture `s` of `n` components (3 RGB, 4 CMYK) stored again in DeviceGray: a JPEG as a grey
/// JPEG, other data losslessly; false when it cannot be read faithfully.
fn gray_image(s: &mut Stream, n: u8) -> bool {
    let jpeg = single_filter(s) == Some(b"DCTDecode");
    let pixels: Option<image::GrayImage> = if n == 3 {
        image_pixels(s, false).and_then(|i| {
            let rgb = i.to_rgb8();
            let g = rgb.pixels().map(|p| (luma(p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0) * 255.0).round() as u8).collect();
            image::GrayImage::from_raw(rgb.width(), rgb.height(), g)
        })
    } else if !jpeg && s.dict.get(b"BitsPerComponent").and_then(Object::as_i64).ok() == Some(8) && !s.dict.has(b"Decode") {
        let w = s.dict.get(b"Width").and_then(Object::as_i64).ok().and_then(|v| u32::try_from(v).ok());
        let h = s.dict.get(b"Height").and_then(Object::as_i64).ok().and_then(|v| u32::try_from(v).ok());
        w.zip(h).and_then(|(w, h)| {
            let data = s.decompressed_content().ok()?;
            (data.len() == w as usize * h as usize * 4).then_some(())?;
            let g = data.as_chunks::<4>().0.iter().map(|p| (cmyk_gray(p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0) * 255.0).round() as u8).collect();
            image::GrayImage::from_raw(w, h, g)
        })
    } else {
        None
    };
    let Some(px) = pixels else { return false };
    if jpeg {
        let mut out = Vec::new();
        if image::DynamicImage::ImageLuma8(px).write_with_encoder(JpegEncoder::new_with_quality(&mut out, 90)).is_err() {
            return false;
        }
        s.set_content(out);
    } else {
        s.set_plain_content(px.into_raw());
        s.dict.remove(b"Filter");
        let _ = s.compress();
    }
    s.dict.remove(b"DecodeParms");
    s.dict.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
    s.dict.set("BitsPerComponent", 8);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn colours_in_content_become_the_grey_of_their_lightness() {
        let doc = Document::with_version("1.7");
        let ops = Content::decode(b"1 0 0 rg 0 0 1 RG 0 0 0 1 k /DeviceRGB cs 0 1 0 sc /Pattern cs /P0 scn q /DeviceCMYK CS Q 1 1 1 SC").unwrap().operations;
        let (out, changed, kept) = gray_ops(&doc, None, ops);
        assert!(changed);
        assert_eq!(kept, 1, "the pattern is left");
        let text = String::from_utf8(Content { operations: out }.encode().unwrap()).unwrap();
        assert!(text.contains("0.299 g") && text.contains("0.114 G") && text.contains("0 g"), "{text}");
        assert!(text.contains("/DeviceGray cs") && text.contains("0.587 sc"), "{text}");
        assert!(text.contains("/P0 scn"), "the pattern's colour stays: {text}");
        // CS inside q … Q: the stroke's space back to grey after Q, so SC is left as it is.
        assert!(text.contains("1 1 1 SC"), "{text}");
    }

    #[test]
    fn a_gradient_runs_between_greys() {
        let mut doc = Document::with_version("1.7");
        let f = doc.add_object(dictionary! { "FunctionType" => 2, "Domain" => vec![0.into(), 1.into()], "C0" => vec![1.into(), 0.into(), 0.into()], "C1" => vec![0.into(), 0.into(), 1.into()], "N" => 1 });
        let sh = doc.add_object(dictionary! { "ShadingType" => 2, "ColorSpace" => "DeviceRGB", "Coords" => vec![0.into(), 0.into(), 100.into(), 0.into()], "Function" => f });
        let r = to_gray(&mut doc);
        assert_eq!(r.contents, 1);
        let d = doc.get_dictionary(sh).unwrap();
        assert_eq!(d.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceGray");
        let c0 = doc.get_dictionary(f).unwrap().get(b"C0").unwrap().as_array().unwrap()[0].as_float().unwrap();
        assert!((c0 - 0.299).abs() < 1e-4, "red's lightness: {c0}");
    }

    #[test]
    fn an_rgb_picture_is_stored_again_in_grey() {
        let mut doc = Document::with_version("1.7");
        let px: Vec<u8> = [255u8, 0, 0].repeat(4);
        let mut s = Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 2, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8 }, px);
        let _ = s.compress();
        let id = doc.add_object(s);
        let r = to_gray(&mut doc);
        assert_eq!(r.images, 1);
        let s = doc.get_object(id).unwrap().as_stream().unwrap();
        assert_eq!(s.dict.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceGray");
        assert_eq!(s.decompressed_content().unwrap(), vec![76u8; 4], "red's lightness");
    }
}
