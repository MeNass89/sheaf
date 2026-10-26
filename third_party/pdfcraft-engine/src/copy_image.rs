//! Copy Image: find the picture under a point and hand it to the clipboard, as pixels or as a
//! file in its original format. A picture drawn through grouped
//! Form artwork (as Word, InDesign and many scanners write them) is found inside the group.

use std::collections::HashSet;

use pdfcraft_cos::{Dict, Document as Cos, ObjRef, Object};

use crate::Document;

/// Clipboards copy the whole bitmap; past this many pixels a pixel copy is refused.
const MAX_COPY_PIXELS: u64 = 64_000_000;
/// How deep Form artwork is searched for its picture.
const MAX_FORM_DEPTH: usize = 12;

impl Document {
    /// The topmost picture on `page` under user-space point `at`, as its index in
    /// [`Document::page_images`]: a raster image, or Form artwork that holds one.
    pub fn page_image_at(&self, page: usize, at: [f64; 2]) -> Option<usize> {
        let editor = self.editor.as_ref()?;
        let inside = |r: &[f64; 4]| (r[0]..=r[2]).contains(&at[0]) && (r[1]..=r[3]).contains(&at[1]);
        self.page_images(page).iter().rposition(|i| {
            inside(&i.rect)
                && match (i.is_form, i.object) {
                    (false, Some(_)) => true,
                    (true, Some(form)) => picture_in_form(&editor.cos, form).is_some(),
                    _ => false,
                }
        })
    }

    /// Copy Image in Original Format: picture `index` on `page` as a file (extension, bytes):
    /// a JPEG as stored in the PDF, otherwise a lossless PNG.
    pub fn page_image_copy_file(&self, page: usize, index: usize) -> Result<(&'static str, Vec<u8>), String> {
        let editor = self.editor.as_ref().ok_or("the document can't be read")?;
        let img = self.page_images(page).into_iter().nth(index).ok_or_else(|| format!("page {} has no image {}", page + 1, index + 1))?;
        let object = img.object.ok_or("the image has no object")?;
        let picture = if img.is_form { picture_in_form(&editor.cos, object).ok_or("this artwork holds no picture")? } else { object };
        pdfcraft_create::image_file(&editor.cos, picture)
    }

    /// Copy Image: picture `index` on `page` at its own resolution, as straight (unpremultiplied)
    /// RGBA pixels (width, height, pixels) for the clipboard. Soft masks become alpha.
    pub fn page_image_rgba(&self, page: usize, index: usize) -> Result<(u32, u32, Vec<u8>), String> {
        let (_, bytes) = self.page_image_copy_file(page, index)?;
        let format = image::guess_format(&bytes).map_err(|e| e.to_string())?;
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes.as_slice()), format);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_COPY_PIXELS * 4);
        reader.limits(limits);
        let decoder = reader.into_decoder().map_err(|e| e.to_string())?;
        let (w, h) = image::ImageDecoder::dimensions(&decoder);
        if w == 0 || h == 0 {
            return Err("the image is empty".into());
        }
        if u64::from(w) * u64::from(h) > MAX_COPY_PIXELS {
            return Err(format!("the image is too large to copy ({w} × {h} pixels); use Save Image As"));
        }
        let rgba = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?.into_rgba8();
        Ok((rgba.width(), rgba.height(), rgba.into_raw()))
    }
}

impl Document {
    /// Sheaf: the picture a stamp's appearance draws, as a file (extension, bytes), so that a
    /// copied picture stamp is pasted with its picture (XFDF carries no appearance). The stamp is
    /// found on `page` by its `/NM`, else as the one whose rectangle is `rect`. None: not a
    /// stamp, or its appearance holds no picture (a text stamp, which XFDF redraws).
    pub fn stamp_picture_file(&self, page: usize, nm: &str, rect: [f64; 4]) -> Option<(&'static str, Vec<u8>)> {
        let cos = &self.editor.as_ref()?.cos;
        let p = *pdfcraft_annot::page_refs(cos).ok()?.get(page)?;
        let annots = cos.get(p).as_dict()?.get(b"Annots").map(|a| cos.resolve(a))?.as_array()?.clone();
        let same_rect = |d: &Dict| {
            let r: Vec<f64> = d.get(b"Rect").map(|r| cos.resolve(r)).and_then(|r| r.as_array().map(|a| a.iter().filter_map(Object::as_f64).collect())).unwrap_or_default();
            r.len() == 4 && r.iter().zip(rect).all(|(a, b)| (a - b).abs() < 0.5)
        };
        let stamp = annots.iter().filter_map(|a| cos.resolve(a).as_dict().cloned()).filter(|d| d.name(b"Subtype") == Some(&b"Stamp"[..])).find(|d| {
            let named = d.get(b"NM").map(|n| cos.resolve(n)).and_then(|n| n.as_string().map(|s| s.to_text()));
            if nm.is_empty() { same_rect(d) } else { named.as_deref() == Some(nm) }
        })?;
        let ap = stamp.get(b"AP").map(|a| cos.resolve(a))?.as_dict()?.get(b"N")?.as_ref()?;
        let picture = picture_in_form(cos, ap)?;
        pdfcraft_create::image_file(cos, picture).ok()
    }
}

/// The largest raster image Form `form` draws through its resources (nested Forms included).
fn picture_in_form(cos: &Cos, form: ObjRef) -> Option<ObjRef> {
    let mut seen = HashSet::from([form]);
    let mut best: Option<(u64, ObjRef)> = None;
    let res = resources(cos, form)?;
    collect(cos, &res, 0, &mut seen, &mut best);
    best.map(|(_, r)| r)
}

fn resources(cos: &Cos, stream: ObjRef) -> Option<Dict> {
    match &*cos.get(stream) {
        Object::Stream(s) => s.dict.get(b"Resources").map(|r| cos.resolve(r)).and_then(|r| r.as_dict().cloned()),
        _ => None,
    }
}

fn collect(cos: &Cos, res: &Dict, depth: usize, seen: &mut HashSet<ObjRef>, best: &mut Option<(u64, ObjRef)>) {
    if depth > MAX_FORM_DEPTH {
        return;
    }
    let Some(xobjects) = res.get(b"XObject").map(|x| cos.resolve(x)).and_then(|x| x.as_dict().cloned()) else { return };
    for (_, v) in xobjects.iter() {
        let Some(r) = v.as_ref() else { continue };
        if !seen.insert(r) {
            continue;
        }
        let obj = cos.get(r);
        let Object::Stream(s) = &*obj else { continue };
        match s.dict.name(b"Subtype") {
            Some(b"Image") => {
                let px = |k: &[u8]| u64::try_from(s.dict.int(k).unwrap_or(0)).unwrap_or(0);
                let size = px(b"Width").saturating_mul(px(b"Height"));
                if size > 0 && best.is_none_or(|(b, _)| size > b) {
                    *best = Some((size, r));
                }
            }
            Some(b"Form") => {
                if let Some(inner) = resources(cos, r) {
                    collect(cos, &inner, depth + 1, seen, best);
                }
            }
            _ => {}
        }
    }
}
