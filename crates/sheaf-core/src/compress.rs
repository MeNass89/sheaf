use crate::{pages::Input, paths::atomic_write, Result};
use image::{codecs::jpeg::JpegEncoder, DynamicImage, GrayImage, RgbImage};
use lopdf::{Object, ObjectId, SaveOptions, Stream};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[path = "optimizer_post.rs"]
mod post;
pub(crate) use post::page_preview as flatten_page_snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Low,
    Recommended,
    Extreme,
    /// A file this many bytes or smaller (iLovePDF's and Smallpdf's target size): stronger settings
    /// are tried in turn ([`TARGET_STEPS`]) until one fits; none fitting, the smallest is kept.
    Target(u64),
}

/// The settings a target size tries, gentlest first: Low, Recommended, Extreme, then two beyond.
const TARGET_STEPS: [(u32, u8); 5] = [(2400, 85), (1600, 70), (1000, 50), (800, 40), (600, 30)];

impl Level {
    /// (max image long side in px, JPEG quality)
    fn params(self) -> (u32, u8) {
        match self {
            Level::Low => (2400, 85),
            Level::Recommended | Level::Target(_) => (1600, 70),
            Level::Extreme => (1000, 50),
        }
    }
}

/// Acrobat's PDF Optimizer, on top of a level: the images' largest resolution and their JPEG
/// quality chosen, and parts of the file that do not show taken out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Advanced {
    /// Images above this many pixels per inch are brought down to it (the level's otherwise).
    pub max_ppi: Option<u32>,
    /// JPEG quality, 1–100 (the level's otherwise).
    pub quality: Option<u8>,
    /// Page thumbnails (`/Thumb`), which readers make themselves.
    pub discard_thumbnails: bool,
    /// The document's structure tags (accessibility, reflow): `/StructTreeRoot`, `/MarkInfo`.
    pub discard_tags: bool,
    /// Its information (title, author…) and XMP metadata.
    pub discard_metadata: bool,
    /// Every colour made grey ([`crate::grayscale`]; Stirling's Convert to grayscale).
    pub grayscale: bool,
    /// The programs of the standard faces every reader has, taken out (Acrobat's Unembed fonts).
    pub unembed_standard: bool,
    /// Acrobat's transparency flattener: an explicit PDF 1.3 conversion using local Ghostscript.
    pub flatten_transparency: bool,
    /// Acrobat's Fast Web View: qpdf lays out the first page for incremental delivery.
    pub linearize: bool,
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
    /// [`Level::Target`]: even the strongest settings left the file larger than asked.
    pub missed_target: bool,
    /// Convert to grayscale: the colour settings and pictures left in colour (spot colours,
    /// sampled gradients, pictures it cannot read), said to the user.
    pub gray_kept: usize,
}

/// Re-encodes 8-bit RGB/gray images (Device or Cal colour spaces, the latter kept as they are) as JPEG,
/// downscaled to the level's max long side unless the image has an /SMask, which must keep matching it;
/// then saves with object and xref streams. Never returns a file larger than an unencrypted input:
/// if nothing helps, the input bytes are copied unchanged and `already_optimal` is set.
/// Encrypted input yields an unencrypted output (`was_encrypted` tells the caller to warn the
/// user): the smaller of the recompressed save and a plain decrypted save is written. That can
/// still be larger than the encrypted original (e.g. one packed in object streams); `grew` says so.
pub fn compress(input: &Input, level: Level, out: &Path) -> Result<CompressReport> {
    compress_with(input, level, out, &mut |_| true)
}

/// [`compress`], `progress` told 0..=1 as the pictures are re-encoded (most of the time); returning
/// `false` stops with [`crate::OpdfError::Cancelled`], nothing written (Sol, 25/10: a large file
/// showed no progress and Cancel waited for its end).
pub fn compress_with(input: &Input, level: Level, out: &Path, progress: &mut dyn FnMut(f32) -> bool) -> Result<CompressReport> {
    compress_advanced(input, level, &Advanced::default(), out, progress)
}

/// [`compress_with`], with [`Advanced`] settings.
pub fn compress_advanced(input: &Input, level: Level, adv: &Advanced, out: &Path, progress: &mut dyn FnMut(f32) -> bool) -> Result<CompressReport> {
    let post_factor = if adv.flatten_transparency || adv.linearize { 0.9 } else { 1.0 };
    let (bytes, before, was_encrypted, missed_target, gray_kept) = match level {
        Level::Target(limit) => {
            let n = TARGET_STEPS.len() as f32;
            let mut best: Option<(Vec<u8>, u64, bool, usize)> = None;
            for (k, (max, q)) in TARGET_STEPS.into_iter().enumerate() {
                let tried = squeeze(input, max, q, adv, &mut |p| progress((k as f32 + p) / n * 0.95 * post_factor))?;
                let fits = tried.0.len() as u64 <= limit;
                if best.as_ref().is_none_or(|b| tried.0.len() < b.0.len()) {
                    best = Some(tried);
                }
                if fits {
                    break;
                }
            }
            let (bytes, before, enc, kept) = best.ok_or_else(|| crate::OpdfError::Unsupported("nothing to compress".into()))?;
            let missed = bytes.len() as u64 > limit;
            (bytes, before, enc, missed, kept)
        }
        level => {
            let (max, q) = level.params();
            // Pixels per inch to the long side the level allows: 150 ppi on A4's 11.7 in is 1,755 px.
            let max = adv.max_ppi.map_or(max, |ppi| (ppi.clamp(36, 1200) as f64 * 11.7) as u32);
            let q = adv.quality.map_or(q, |q| q.clamp(1, 100));
            let (bytes, before, enc, kept) = squeeze(input, max, q, adv, &mut |p| progress(p * post_factor))?;
            (bytes, before, enc, false, kept)
        }
    };
    let bytes = post::finish(bytes, adv, progress)?;
    let missed_target = if let Level::Target(limit) = level { bytes.len() as u64 > limit } else { missed_target };
    let after = bytes.len() as u64;
    if !progress(0.95) { return Err(crate::OpdfError::Cancelled); }
    let path = atomic_write(out, |part| Ok(std::fs::write(part, &bytes)?))?;
    progress(1.0);
    Ok(CompressReport { path, before, after, already_optimal: after >= before && *adv == Advanced::default(), was_encrypted, grew: after > before, missed_target, gray_kept })
}

/// The file recompressed with pictures at most `max` pixels on their long side and JPEG quality
/// `q`, as bytes, with the original's size and whether it was encrypted; `progress` told 0..0.9.
/// The parts [`Advanced`] takes out; what they leave behind is pruned with the rest.
fn discard(doc: &mut lopdf::Document, adv: &Advanced) {
    let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
    for id in pages {
        if let Ok(p) = doc.get_object_mut(id).and_then(Object::as_dict_mut) {
            if adv.discard_thumbnails {
                p.remove(b"Thumb");
            }
            if adv.discard_tags {
                p.remove(b"StructParents");
            }
            if adv.discard_metadata {
                p.remove(b"Metadata");
                p.remove(b"PieceInfo");
            }
        }
    }
    if let Ok(root) = doc.trailer.get(b"Root").and_then(Object::as_reference)
        && let Ok(c) = doc.get_object_mut(root).and_then(Object::as_dict_mut)
    {
        if adv.discard_tags {
            c.remove(b"StructTreeRoot");
            c.remove(b"MarkInfo");
        }
        if adv.discard_metadata {
            c.remove(b"Metadata");
            c.remove(b"PieceInfo");
        }
    }
    if adv.discard_metadata {
        doc.trailer.remove(b"Info");
    }
    if adv.unembed_standard {
        unembed_standard(doc);
    }
}

/// Whether a font's base name is a standard face a reader draws without its program.
fn standard_face(base: &[u8]) -> bool {
    let b = String::from_utf8_lossy(base);
    let family = b.split([',', '-']).next().unwrap_or("");
    matches!(family, "Helvetica" | "Times" | "Courier" | "Symbol" | "ZapfDingbats" | "Arial" | "ArialMT" | "TimesNewRoman" | "TimesNewRomanPSMT" | "CourierNew" | "CourierNewPSMT")
}

/// The embedded programs of whole (not subset), simple (Type1 or TrueType) standard faces taken out
/// of their descriptors; returns how many. A subset or a composite font keeps its program: its
/// glyphs may not be where a reader's own face has them.
pub fn unembed_standard(doc: &mut lopdf::Document) -> usize {
    let fonts: Vec<(ObjectId, ObjectId)> = doc
        .objects
        .iter()
        .filter_map(|(id, o)| {
            let d = o.as_dict().ok()?;
            let simple = matches!(d.get(b"Subtype").and_then(Object::as_name).ok()?, b"Type1" | b"TrueType");
            let base = d.get(b"BaseFont").and_then(Object::as_name).ok()?;
            let whole = base.get(6) != Some(&b'+');
            (d.get(b"Type").and_then(Object::as_name).ok() == Some(b"Font") && simple && whole && standard_face(base)).then(|| Some((*id, d.get(b"FontDescriptor").and_then(Object::as_reference).ok()?)))?
        })
        .collect();
    let mut n = 0;
    for (_, fd) in fonts {
        if let Ok(d) = doc.get_object_mut(fd).and_then(Object::as_dict_mut) {
            let had = [&b"FontFile"[..], b"FontFile2", b"FontFile3"].iter().filter(|k| d.remove(k).is_some()).count();
            n += usize::from(had > 0);
        }
    }
    n
}

fn squeeze(input: &Input, max: u32, q: u8, adv: &Advanced, progress: &mut dyn FnMut(f32) -> bool) -> Result<(Vec<u8>, u64, bool, usize)> {
    let original = std::fs::read(&input.path)?;
    let mut doc = input.load_lopdf()?;
    let was_encrypted = doc.was_encrypted();
    let mut plain = was_encrypted.then(|| doc.clone());
    discard(&mut doc, adv);
    let gray_kept = if adv.grayscale { crate::grayscale::to_gray(&mut doc).kept } else { 0 };
    dedupe_streams(&mut doc);
    let masks = mask_targets(&doc);
    let kinds: HashMap<ObjectId, bool> = doc.objects.iter()
        .filter(|(id, _)| !masks.contains(id))
        .filter_map(|(id, o)| Some((*id, is_gray(&doc, o.as_stream().ok()?)?)))
        .collect();
    // Progress: the pictures to 60 %, then each stream compressed to 90 % (Sol, 25/10: a file
    // without pictures stayed at 0 % with Cancel unheard until the end).
    if !progress(0.02) { return Err(crate::OpdfError::Cancelled); }
    let (total, mut done) = (kinds.len().max(1) as f32, 0.0);
    for (id, obj) in doc.objects.iter_mut() {
        if let (Some(&gray), Ok(s)) = (kinds.get(id), obj.as_stream_mut()) {
            recompress(s, gray, max, q);
            done += 1.0;
            if !progress(0.02 + 0.58 * done / total) { return Err(crate::OpdfError::Cancelled); }
        }
    }
    doc.prune_objects();
    // `Document::compress`, a stream at a time with a word to Cancel every 64.
    let streams = doc.objects.values().filter(|o| o.as_stream().is_ok()).count().max(1) as f32;
    for (k, obj) in doc.objects.values_mut().enumerate() {
        if let Object::Stream(s) = obj
            && s.allows_compression
        {
            let _ = s.compress();
        }
        if k % 64 == 63 && !progress(0.6 + 0.3 * (k as f32 / streams).min(1.0)) {
            return Err(crate::OpdfError::Cancelled);
        }
    }
    if !progress(0.9) { return Err(crate::OpdfError::Cancelled); }
    let mut buf = Vec::new();
    doc.save_with_options(&mut buf, SaveOptions::builder().use_object_streams(true).use_xref_streams(true).build())?;
    let bytes = match plain.as_mut() {
        // Nothing gained: the file as it was, unless parts were asked out.
        None if buf.len() >= original.len() && *adv == Advanced::default() => original.clone(),
        None => buf,
        // The parts asked out, or the colours made grey, are only in the recompressed save.
        Some(_) if *adv != Advanced::default() => buf,
        Some(d) => {
            let mut b = Vec::new();
            d.save_to(&mut b)?;
            if b.len() < buf.len() { b } else { buf }
        }
    };
    Ok((bytes, original.len() as u64, was_encrypted, gray_kept))
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
pub(crate) fn single_filter(s: &Stream) -> Option<&[u8]> {
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
pub(crate) fn image_pixels(s: &Stream, gray: bool) -> Option<DynamicImage> {
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

/// A CMYK JPEG's pixels as RGB the way PDF viewers show them, for the tools that recolour
/// pictures; `None` for any other picture, a mask, or a /Decode other than the identity or the
/// fully inverted `[1 0 1 0 1 0 1 0]`.
/// PDF viewers (PDFium, Poppler, Ghostscript, checked 27/10) take the JPEG's stored components as
/// they are, then apply /Decode. Adobe's CMYK JPEGs store their components inverted, so producers
/// pair them with the inverted /Decode. The JPEG decoder always assumes Adobe's inversion
/// (r = c·k/255), so its RGB is right for the inverted /Decode only; with the identity the stored
/// components are read raw and converted here.
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
pub(crate) fn image_pixels(s: &Stream, gray: bool) -> Option<DynamicImage> {
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

/// A CMYK JPEG's pixels as RGB the way PDF viewers show them, for the tools that recolour
/// pictures; `None` for any other picture, a mask, or a /Decode other than the identity or the
/// fully inverted `[1 0 1 0 1 0 1 0]`.
/// PDF viewers (PDFium, Poppler, Ghostscript, checked 27/10) take the JPEG's stored components as
/// they are, then apply /Decode. Adobe's CMYK JPEGs store their components inverted, so producers
/// pair them with the inverted /Decode. The JPEG decoder always assumes Adobe's inversion
/// (r = c·k/255), so its RGB is right for the inverted /Decode only; with the identity the stored
/// components are read raw and converted here.
pub(crate) fn cmyk_jpeg_rgb(s: &Stream) -> Option<RgbImage> {
    if s.dict.has(b"Mask") || single_filter(s) != Some(b"DCTDecode") || jpeg_components(&s.content)? != 4 {
        return None;
    }
    let w = u32::try_from(s.dict.get(b"Width").and_then(Object::as_i64).ok()?).ok()?;
    let h = u32::try_from(s.dict.get(b"Height").and_then(Object::as_i64).ok()?).ok()?;
    let img = if identity_decode(s) {
<<<<<<< HEAD
        use zune_core::{colorspace::ColorSpace, options::DecoderOptions};
        let mut dec = zune_jpeg::JpegDecoder::new(std::io::Cursor::new(&s.content[..]));
        dec.decode_headers().ok()?;
        let space = dec.input_colorspace()?;
        if !matches!(space, ColorSpace::CMYK | ColorSpace::YCCK) { return None; }
        // Acrobat applies PDF Decode after the JPEG colour transform. Requesting CMYK from
        // a YCCK decoder fails; read its own four channels, then undo that transform here.
        dec.set_options(DecoderOptions::default().jpeg_set_out_colorspace(space));
        let raw = dec.decode().ok()?;
=======
        use zune_core::colorspace::ColorSpace;
        // The components as stored: CMYK as it is, or YCCK (Adobe's transform 2) taken back to CMYK
        // with the JPEG colour transform, as PDF viewers' DCT decoders do.
        let mut dec = zune_jpeg::JpegDecoder::new(std::io::Cursor::new(&s.content[..]));
        dec.decode_headers().ok()?;
        let input = dec.input_colorspace()?;
        if !matches!(input, ColorSpace::CMYK | ColorSpace::YCCK) { return None; }
        let opts = zune_core::options::DecoderOptions::default().jpeg_set_out_colorspace(input);
        let mut dec = zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(&s.content[..]), opts);
        let mut raw = dec.decode().ok()?;
>>>>>>> dev/round29
        if raw.len() != (w as usize).checked_mul(h as usize)?.checked_mul(4)? { return None; }
        if input == ColorSpace::YCCK {
            for p in raw.as_chunks_mut::<4>().0 {
                let (y, cb, cr) = (f32::from(p[0]), f32::from(p[1]) - 128.0, f32::from(p[2]) - 128.0);
                let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                [p[0], p[1], p[2]] = [c(y + 1.402 * cr), c(y - 0.344_136 * cb - 0.714_136 * cr), c(y + 1.772 * cb)];
            }
        }
        let ink = |v: u8, k: u8| ((255 - u16::from(v)) * (255 - u16::from(k)) / 255) as u8;
        RgbImage::from_fn(w, h, |x, y| {
            let p = &raw[(y as usize * w as usize + x as usize) * 4..][..4];
            let [c, m, y] = if space == ColorSpace::YCCK {
                let (luma, cb, cr) = (f32::from(p[0]), f32::from(p[1]) - 128.0, f32::from(p[2]) - 128.0);
                [luma + 1.402 * cr, luma - 0.344136 * cb - 0.714136 * cr, luma + 1.772 * cb]
                    .map(|v| 255 - v.round().clamp(0.0, 255.0) as u8)
            } else { [p[0], p[1], p[2]] };
            image::Rgb([ink(c, p[3]), ink(m, p[3]), ink(y, p[3])])
        })
    } else if inverted_cmyk_decode(s) {
        image::load_from_memory_with_format(&s.content, image::ImageFormat::Jpeg).ok()?.to_rgb8()
    } else {
        return None;
    };
    ((img.width(), img.height()) == (w, h)).then_some(img)
}

/// True when /Decode is `[1 0 1 0 1 0 1 0]`, the array Adobe's inverted CMYK JPEGs come with.
fn inverted_cmyk_decode(s: &Stream) -> bool {
    matches!(s.dict.get(b"Decode"), Ok(Object::Array(a)) if a.len() == 8
        && a.iter().enumerate().all(|(i, v)| v.as_float().ok() == Some(((i + 1) % 2) as f32)))
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
