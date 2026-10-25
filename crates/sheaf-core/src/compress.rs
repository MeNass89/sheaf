use crate::{pages::Input, paths::atomic_write, Result};
use image::{codecs::jpeg::JpegEncoder, DynamicImage, GrayImage, RgbImage};
use lopdf::{Object, ObjectId, SaveOptions, Stream};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level { Low, Recommended, Extreme }

impl Level {
    /// (max image long side in px, JPEG quality)
    fn params(self) -> (u32, u8) {
        match self { Level::Low => (2400, 85), Level::Recommended => (1600, 70), Level::Extreme => (1000, 50) }
    }
}

#[derive(Debug)]
pub struct CompressReport {
    pub path: PathBuf,
    pub before: u64,
    pub after: u64,
    pub already_optimal: bool,
    pub was_encrypted: bool,
    /// Encrypted input only: even the smaller unencrypted candidate is larger than the original.
    pub grew: bool,
}

/// Re-encodes 8-bit RGB/gray images (Device or Cal colour spaces, the latter kept as they are) as JPEG,
/// downscaled to the level's max long side unless the image has an /SMask, which must keep matching it;
/// then saves with object and xref streams. Never returns a file larger than an unencrypted input:
/// if nothing helps, the input bytes are copied unchanged and `already_optimal` is set.
/// Encrypted input yields an unencrypted output (`was_encrypted` tells the caller to warn the
/// user): the smaller of the recompressed save and a plain decrypted save is written. That can
/// still be larger than the encrypted original (e.g. one packed in object streams); `grew` says so.
pub fn compress(input: &Input, level: Level, out: &Path) -> Result<CompressReport> {
    let original = std::fs::read(&input.path)?;
    let mut doc = input.load_lopdf()?;
    let was_encrypted = doc.was_encrypted();
    let mut plain = was_encrypted.then(|| doc.clone());
    dedupe_streams(&mut doc);
    let (max, q) = level.params();
    let masks = mask_targets(&doc);
    let kinds: HashMap<ObjectId, bool> = doc.objects.iter()
        .filter(|(id, _)| !masks.contains(id))
        .filter_map(|(id, o)| Some((*id, is_gray(&doc, o.as_stream().ok()?)?)))
        .collect();
    for (id, obj) in doc.objects.iter_mut() {
        if let (Some(&gray), Ok(s)) = (kinds.get(id), obj.as_stream_mut()) { recompress(s, gray, max, q); }
    }
    doc.prune_objects();
    doc.compress();
    let mut buf = Vec::new();
    doc.save_with_options(&mut buf, SaveOptions::builder().use_object_streams(true).use_xref_streams(true).build())?;
    let bytes = match plain.as_mut() {
        None if buf.len() >= original.len() => original.clone(),
        None => buf,
        Some(d) => {
            let mut b = Vec::new();
            d.save_to(&mut b)?;
            if b.len() < buf.len() { b } else { buf }
        }
    };
    let (before, after) = (original.len() as u64, bytes.len() as u64);
    let path = atomic_write(out, |part| Ok(std::fs::write(part, &bytes)?))?;
    Ok(CompressReport { path, before, after, already_optimal: after >= before, was_encrypted, grew: after > before })
}

/// Identical streams (the same picture or font program embedded again for each page, as some
/// producers write them) are kept once: every reference goes to the first, the copies are
/// pruned with the other unused objects. Returns how many copies were dropped.
fn dedupe_streams(doc: &mut lopdf::Document) -> usize {
    use std::hash::{Hash, Hasher};
    let mut seen: HashMap<u64, Vec<ObjectId>> = HashMap::new();
    let mut remap: HashMap<ObjectId, ObjectId> = HashMap::new();
    let key = |s: &Stream| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (format!("{:?}", s.dict), &s.content).hash(&mut h);
        h.finish()
    };
    let mut ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    ids.sort();
    for id in ids {
        let Ok(s) = doc.objects[&id].as_stream() else { continue };
        let list = seen.entry(key(s)).or_default();
        // Equal bytes and dictionary, not only an equal hash.
        let same = list.iter().copied().find(|o| doc.objects[o].as_stream().is_ok_and(|t| t.content == s.content && t.dict == s.dict));
        match same {
            Some(o) => {
                remap.insert(id, o);
            }
            None => list.push(id),
        }
    }
    if remap.is_empty() {
        return 0;
    }
    fn walk(o: &mut Object, remap: &HashMap<ObjectId, ObjectId>) {
        match o {
            Object::Reference(r) => {
                if let Some(to) = remap.get(r) {
                    *r = *to;
                }
            }
            Object::Array(a) => a.iter_mut().for_each(|x| walk(x, remap)),
            Object::Dictionary(d) => d.iter_mut().for_each(|(_, x)| walk(x, remap)),
            Object::Stream(s) => s.dict.iter_mut().for_each(|(_, x)| walk(x, remap)),
            _ => {}
        }
    }
    for o in doc.objects.values_mut() {
        walk(o, &remap);
    }
    doc.trailer.iter_mut().for_each(|(_, x)| walk(x, &remap));
    remap.len()
}

/// For an image whose colour space is DeviceGray/CalGray (`Some(true)`) or DeviceRGB/CalRGB (`Some(false)`);
/// `None` for any other colour space (ICCBased, Indexed, CMYK, Lab, …), which we leave alone.
/// Cal spaces only add a colorimetric interpretation of the same 1 or 3 components, so the pixels re-encode as-is.
fn is_gray(doc: &lopdf::Document, s: &Stream) -> Option<bool> {
    if name(s, b"Subtype") != Some(b"Image") { return None; }
    let (_, cs) = doc.dereference(s.dict.get(b"ColorSpace").ok()?).ok()?;
    let family = match cs { Object::Name(n) => n.as_slice(), Object::Array(a) => doc.dereference(a.first()?).ok()?.1.as_name().ok()?, _ => return None };
    match family { b"DeviceGray" | b"CalGray" => Some(true), b"DeviceRGB" | b"CalRGB" => Some(false), _ => None }
}

/// Ids of images used as another image's /SMask or /Mask: alpha and stencil data, never made lossy.
fn mask_targets(doc: &lopdf::Document) -> HashSet<ObjectId> {
    doc.objects.values().filter_map(|o| o.as_stream().ok())
        .flat_map(|s| [b"SMask".as_slice(), b"Mask"].map(|k| s.dict.get(k).and_then(Object::as_reference).ok()))
        .flatten().collect()
}

fn name<'a>(s: &'a Stream, key: &[u8]) -> Option<&'a [u8]> { s.dict.get(key).and_then(Object::as_name).ok() }

/// The stream's single filter, if it has exactly one (a bare name or a one-element array).
fn single_filter(s: &Stream) -> Option<&[u8]> {
    match s.dict.get(b"Filter").ok()? {
        Object::Name(n) => Some(n),
        Object::Array(a) if a.len() == 1 => a[0].as_name().ok(),
        _ => None,
    }
}

/// lopdf applies /DecodeParms only in its dictionary form, to every filter of the chain. Accept
/// only what it decodes faithfully: no parms, or one dictionary with a single filter; and a
/// /Predictor only under a plain /FlateDecode name.
fn decode_parms_supported(s: &Stream) -> bool {
    if let Ok(Object::Array(a)) = s.dict.get(b"Filter")
        && a.len() > 1
    {
        return false;
    }
    match s.dict.get(b"DecodeParms") {
        Err(_) => true,
        Ok(Object::Dictionary(d)) => !d.has(b"Predictor") || matches!(s.dict.get(b"Filter"), Ok(Object::Name(n)) if n == b"FlateDecode"),
        Ok(_) => false,
    }
}

/// True when /Decode is absent or the identity mapping ([0 1] per component).
fn identity_decode(s: &Stream) -> bool {
    match s.dict.get(b"Decode") {
        Err(_) => true,
        Ok(Object::Array(a)) => a.iter().enumerate().all(|(i, v)| v.as_float().ok() == Some((i % 2) as f32)),
        Ok(_) => false,
    }
}

/// Decodes an image XObject we can re-encode faithfully, or `None` to leave it alone:
/// 8-bit, `gray` (1 component) or RGB (3), no /Mask or /ImageMask, identity Decode,
/// Flate-family or plain DCT data, /DecodeParms lopdf reads faithfully. An /SMask is allowed: it is a separate stream we never touch.
fn image_pixels(s: &Stream, gray: bool) -> Option<DynamicImage> {
    if s.dict.has(b"Mask") || s.dict.get(b"ImageMask").and_then(Object::as_bool).unwrap_or(false) { return None; }
    if s.dict.get(b"BitsPerComponent").and_then(Object::as_i64).ok() != Some(8) || !identity_decode(s) { return None; }
    if !decode_parms_supported(s) { return None; }
    let w = u32::try_from(s.dict.get(b"Width").and_then(Object::as_i64).ok()?).ok()?;
    let h = u32::try_from(s.dict.get(b"Height").and_then(Object::as_i64).ok()?).ok()?;
    let img = if single_filter(s) == Some(b"DCTDecode") {
        // CMYK/YCCK JPEGs would be silently converted to RGB by the decoder (and image 0.25's
        // `original_color_type()` reports the converted type), so check the header's component count.
        if jpeg_components(&s.content)? != if gray { 1 } else { 3 } { return None; }
        image::load_from_memory_with_format(&s.content, image::ImageFormat::Jpeg).ok()?
    } else {
        let expected = (w as usize).checked_mul(h as usize)?.checked_mul(if gray { 1 } else { 3 })?;
        let data = s.decompressed_content_with_limit(expected).ok()?;
        if data.len() != expected { return None; }
        if gray { DynamicImage::ImageLuma8(GrayImage::from_raw(w, h, data)?) } else { DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, data)?) }
    };
    // The decoded data must agree with the dictionary, or we would change what the page shows.
    let ok = (img.width(), img.height()) == (w, h)
        && matches!((&img, gray), (DynamicImage::ImageLuma8(_), true) | (DynamicImage::ImageRgb8(_), false));
    ok.then_some(img)
}

/// Number of colour components declared in a JPEG's SOF header (1 gray, 3 YCbCr/RGB, 4 CMYK/YCCK).
fn jpeg_components(data: &[u8]) -> Option<u8> {
    let mut i = 2; // after SOI
    while i + 9 < data.len() {
        if data[i] != 0xFF { return None; }
        let m = data[i + 1];
        if m == 0xFF { i += 1; continue; } // fill byte
        if (0xC0..=0xCF).contains(&m) && ![0xC4, 0xC8, 0xCC].contains(&m) { return Some(data[i + 9]); }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        i += 2 + len;
    }
    None
}

/// Re-encodes `s` as JPEG (quality `q`, long side ≤ `max`, never upscaled) if that makes it smaller.
/// Images with an /SMask keep their size so the mask still lines up pixel for pixel.
/// /ColorSpace is left as it is (a name or the original Cal dictionary).
fn recompress(s: &mut Stream, gray: bool, max: u32, q: u8) -> bool {
    let Some(mut img) = image_pixels(s, gray) else { return false };
    if img.width().max(img.height()) > max && !s.dict.has(b"SMask") { img = img.thumbnail(max, max); }
    let mut jpeg = Vec::new();
    if img.write_with_encoder(JpegEncoder::new_with_quality(&mut jpeg, q)).is_err() || jpeg.len() >= s.content.len() { return false; }
    let (w, h) = (img.width() as i64, img.height() as i64);
    s.set_content(jpeg);
    s.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
    s.dict.remove(b"DecodeParms");
    s.dict.set("Width", w);
    s.dict.set("Height", h);
    s.dict.set("BitsPerComponent", 8);
    true
}
