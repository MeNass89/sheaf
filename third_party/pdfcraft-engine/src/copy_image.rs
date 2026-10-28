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
        // A turned picture is copied as it shows ([`Self::stamp_alone_pdf`]): its bitmap alone would
        // lose the turn (Sol, 26/10).
        if drawing(cos, ap) != (1, false, false) || turned(cos, &stamp, ap) {
            return None;
        }
        let picture = picture_in_form(cos, ap)?;
        pdfcraft_create::image_file(cos, picture).ok()
    }

    /// Sheaf: a stamp whose appearance is more than one picture (a picture with text, several
    /// pictures, a drawing: Sol, 26/10, the largest bitmap alone was copied) as a one-page PDF the
    /// stamp's size holding that stamp alone, to be drawn as the picture pasted. None: a lone
    /// picture ([`Self::stamp_picture_file`]) or text alone (XFDF redraws it).
    pub fn stamp_alone_pdf(&self, page: usize, nm: &str, rect: [f64; 4]) -> Option<Vec<u8>> {
        let cos = &self.editor.as_ref()?.cos;
        let mut out = pdfcraft_organize::extract_pages(cos, &[page]).ok()?;
        let p = *pdfcraft_annot::page_refs(&out).ok()?.first()?;
        let annots = out.get(p).as_dict()?.get(b"Annots").map(|a| out.resolve(a))?.as_array()?.clone();
        let is_it = |d: &Dict| {
            let named = d.get(b"NM").map(|n| out.resolve(n)).and_then(|n| n.as_string().map(|s| s.to_text()));
            let r: Vec<f64> = d.get(b"Rect").map(|r| out.resolve(r)).and_then(|r| r.as_array().map(|a| a.iter().filter_map(Object::as_f64).collect())).unwrap_or_default();
            d.name(b"Subtype") == Some(&b"Stamp"[..])
                && if nm.is_empty() { r.len() == 4 && r.iter().zip(rect).all(|(a, b)| (a - b).abs() < 0.5) } else { named.as_deref() == Some(nm) }
        };
        let stamp = annots.into_iter().find(|a| out.resolve(a).as_dict().is_some_and(is_it))?;
        let sd = out.resolve(&stamp).as_dict()?.clone();
        let ap = sd.get(b"AP").map(|a| out.resolve(a))?.as_dict()?.get(b"N")?.as_ref()?;
        // A lone picture not turned, or text alone (XFDF redraws it): not here.
        match drawing(&out, ap) {
            (1, false, false) if !turned(&out, &sd, ap) => return None,
            (0, true, false) => return None,
            _ => {}
        }
        let r = rect.iter().map(|v| Object::Real(*v)).collect::<Vec<_>>();
        out.update_dict(p, |d| {
            for k in [&b"Contents"[..], b"CropBox", b"BleedBox", b"TrimBox", b"ArtBox", b"Rotate"] {
                d.remove(k);
            }
            d.set(b"MediaBox".to_vec(), Object::Array(r));
            d.set(b"Annots".to_vec(), Object::Array(vec![stamp]));
        })
        .ok()?;
        pdfcraft_cos::write_full(&out, &pdfcraft_cos::SaveOptions::default()).ok()
    }
}

impl Document {
    /// Sheaf: objects picked with Edit a PDF on `page` (`targets`, covering user-space `rect`) as a
    /// one-page PDF holding them alone, its crop box `rect`: copied as objects, pasted with
    /// [`crate::Edit::AddArtwork`] (Acrobat's copy of objects; Sol, 26/10: a picture of their box
    /// was copied, with whatever else lay in it).
    pub fn objects_pdf(&self, page: usize, targets: &[pdfcraft_edit::ObjectTarget], rect: [f64; 4]) -> Result<Vec<u8>, String> {
        let cos = &self.editor.as_ref().ok_or("the document can't be read")?.cos;
        let mut out = pdfcraft_organize::extract_pages(cos, &[page]).map_err(|e| e.to_string())?;
        let others: Vec<_> = pdfcraft_edit::editable_objects(&out, 0).map_err(|e| e.to_string())?.into_iter().map(|o| o.target).filter(|t| !targets.contains(t)).collect();
        if !others.is_empty() {
            pdfcraft_edit::delete_objects(&mut out, 0, &others).map_err(|e| e.to_string())?;
        }
        let p = *pdfcraft_annot::page_refs(&out).map_err(|e| e.to_string())?.first().ok_or("no page")?;
        let r = rect.iter().map(|v| Object::Real(*v)).collect::<Vec<_>>();
        out.update_dict(p, |d| {
            d.remove(b"Annots");
            d.set(b"CropBox".to_vec(), Object::Array(r));
        })
        .map_err(|e| e.to_string())?;
        pdfcraft_cos::write_full(&out, &pdfcraft_cos::SaveOptions::default()).map_err(|e| e.to_string())
    }

    /// Sheaf: the paragraphs among `targets` as text to add again, in their order (`None` for what
    /// is not text, or text drawn turned): their words, font, size and colour. Acrobat pastes
    /// copied paragraphs as text, still editable (Sol, 26/10: they became one drawn item). Their
    /// box is the caller's to place.
    pub fn objects_texts(&self, page: usize, targets: &[pdfcraft_edit::ObjectTarget]) -> Vec<Option<pdfcraft_edit::AddedText>> {
        use pdfcraft_edit::{Content, ObjectKind};
        let blocks = self.text_blocks(page);
        let added: Vec<_> = self.editor.as_ref().map(|e| pdfcraft_edit::list_added(&e.cos).into_iter().filter(|a| a.page == page).collect()).unwrap_or_default();
        targets
            .iter()
            .map(|t| match t.kind {
                ObjectKind::Text => blocks.get(t.index).filter(|b| b.rotation.abs() < 0.5 && !b.text.trim().is_empty()).map(|b| pdfcraft_edit::AddedText {
                    rect: b.rect,
                    text: b.text.clone(),
                    family: family_of(&b.base_font),
                    bold: b.bold,
                    italic: b.italic,
                    size: b.size,
                    color: b.color,
                    align: Default::default(),
                    underline: false,
                }),
                ObjectKind::Added => match added.get(t.index).map(|a| &a.content) {
                    Some(Content::Text(x)) => Some(x.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// Sheaf: per target, a paragraph that mixes fonts or styles (a bold word, two faces) as its
    /// runs, each run of one font as text in its box on the page (user space): its own font
    /// resource when that font writes its text again, else `None` and the nearest standard family
    /// in its weight and slant. PDF Expert's formatted paste keeps each run's font (Sol, 28/10: a
    /// mixed paragraph was pasted in one standard font). `None` for a paragraph in one font (see
    /// [`Self::objects_text_fonts`]) and for what is not text.
    pub fn objects_text_runs(&self, page: usize, targets: &[pdfcraft_edit::ObjectTarget]) -> Vec<Option<Vec<(pdfcraft_edit::AddedText, Option<String>)>>> {
        let Some(editor) = self.editor.as_ref() else { return vec![None; targets.len()] };
        let blocks = self.text_blocks(page);
        let lines = self.text_lines(page);
        targets
            .iter()
            .map(|t| {
                if t.kind != pdfcraft_edit::ObjectKind::Text {
                    return None;
                }
                let b = blocks.get(t.index).filter(|b| b.rotation.abs() < 0.5 && !b.invisible)?;
                let runs: Vec<&pdfcraft_edit::TextLine> = b.lines.iter().filter_map(|&i| lines.get(i)).filter(|l| !l.text.trim().is_empty()).collect();
                let first = runs.first()?;
                let style = |l: &pdfcraft_edit::TextLine| (l.font.clone(), l.bold, l.italic);
                if runs.iter().all(|l| style(l) == style(first)) {
                    return None;
                }
                Some(
                    runs.into_iter()
                        .map(|l| {
                            let text = pdfcraft_edit::AddedText {
                                rect: l.rect,
                                text: l.text.trim_end().to_string(),
                                family: family_of(&l.base_font),
                                bold: l.bold,
                                italic: l.italic,
                                size: l.size,
                                color: l.color,
                                align: Default::default(),
                                underline: false,
                            };
                            let own = (l.decodable && pdfcraft_edit::font_writes(&editor.cos, page, &l.font, &text.text)).then(|| l.font.clone());
                            (text, own)
                        })
                        .collect(),
                )
            })
            .collect()
    }

    /// Sheaf: per target, the font resource its paragraph is set in when it can be pasted in that
    /// very font (one font for all its lines, which maps back every character, as an embedded subset
    /// does for the characters it showed), with the paragraph's lines as they break; `None` to paste
    /// it in the nearest standard family. PDF Expert pastes a paragraph in its own font (Sol, 27/10:
    /// pasted paragraphs became Helvetica, Times or Courier).
    pub fn objects_text_fonts(&self, page: usize, targets: &[pdfcraft_edit::ObjectTarget]) -> Vec<Option<(String, String)>> {
        let Some(editor) = self.editor.as_ref() else { return vec![None; targets.len()] };
        let blocks = self.text_blocks(page);
        let lines = self.text_lines(page);
        targets
            .iter()
            .map(|t| {
                if t.kind != pdfcraft_edit::ObjectKind::Text {
                    return None;
                }
                let b = blocks.get(t.index).filter(|b| b.rotation.abs() < 0.5 && !b.invisible)?;
                let mut fonts = b.lines.iter().map(|&i| lines.get(i).filter(|l| l.decodable).map(|l| l.font.clone()));
                let first = fonts.next()??;
                if !fonts.all(|f| f.as_deref() == Some(first.as_str())) {
                    return None;
                }
                pdfcraft_edit::font_writes(&editor.cos, page, &first, &b.editor_text).then(|| (first, b.editor_text.clone()))
            })
            .collect()
    }
}

/// The standard family nearest a font's name.
fn family_of(font: &str) -> pdfcraft_edit::Family {
    use pdfcraft_edit::Family;
    let f = font.to_ascii_lowercase();
    if ["times", "serif", "georgia", "garamond", "roman"].iter().any(|k| f.contains(k)) && !f.contains("sans") {
        Family::Times
    } else if ["courier", "mono", "consol"].iter().any(|k| f.contains(k)) {
        Family::Courier
    } else {
        Family::Helvetica
    }
}

/// Whether a stamp is shown turned: a turn of its own (`/PCPictureRotate`, `/PCStampRotate`) or
/// an appearance matrix that is not upright.
fn turned(cos: &Cos, stamp: &Dict, ap: ObjRef) -> bool {
    let own = [&b"PCPictureRotate"[..], b"PCStampRotate"].iter().any(|k| stamp.get(k).and_then(Object::as_int).is_some_and(|t| t.rem_euclid(360) != 0));
    let matrix = match &*cos.get(ap) {
        Object::Stream(s) => s.dict.get(b"Matrix").and_then(|m| m.as_array().map(|a| a.iter().filter_map(Object::as_f64).collect::<Vec<_>>())),
        _ => None,
    };
    own || matrix.is_some_and(|m| m.len() == 6 && (m[1].abs() > 1e-6 || m[2].abs() > 1e-6 || m[0] < 0.0 || m[3] < 0.0))
}

/// What Form `form` draws, nested Forms included: (raster images, whether it shows text, whether
/// it paints paths: Sol, 26/10, a picture with a drawing was taken for the picture alone).
pub(crate) fn drawing(cos: &Cos, form: ObjRef) -> (usize, bool, bool) {
    fn walk(cos: &Cos, form: ObjRef, depth: usize, seen: &mut HashSet<ObjRef>, acc: &mut (usize, bool, bool)) {
        if depth > MAX_FORM_DEPTH || !seen.insert(form) {
            return;
        }
        if let Object::Stream(s) = &*cos.get(form)
            && let Ok(data) = s.decoded()
        {
            acc.2 |= paints_paths(&data);
        }
        let Some(res) = resources(cos, form) else { return };
        acc.1 |= res.get(b"Font").map(|f| cos.resolve(f)).and_then(|f| f.as_dict().map(|d| !d.is_empty())).unwrap_or(false);
        let Some(xobjects) = res.get(b"XObject").map(|x| cos.resolve(x)).and_then(|x| x.as_dict().cloned()) else { return };
        for (_, v) in xobjects.iter() {
            let Some(r) = v.as_ref() else { continue };
            let sub = match &*cos.get(r) {
                Object::Stream(s) => s.dict.name(b"Subtype").map(<[u8]>::to_vec),
                _ => None,
            };
            match sub.as_deref() {
                Some(b"Image") => acc.0 += 1,
                Some(b"Form") => walk(cos, r, depth + 1, seen, acc),
                _ => {}
            }
        }
    }
    let mut acc = (0, false, false);
    walk(cos, form, 0, &mut HashSet::new(), &mut acc);
    acc
}

/// Sheaf: the raster pictures Form `form` draws (nested Forms included) swapped for an empty Form
/// under the same names, so they no longer show and the group's text and drawings stay (Acrobat's
/// Remove all images; Sol, 27/10: such groups kept their pictures). Returns how many went.
pub(crate) fn blank_pictures(cos: &mut Cos, form: ObjRef) -> usize {
    fn forms(cos: &Cos, form: ObjRef, depth: usize, seen: &mut HashSet<ObjRef>, out: &mut Vec<ObjRef>) {
        if depth > MAX_FORM_DEPTH || !seen.insert(form) {
            return;
        }
        out.push(form);
        let Some(xobjects) = resources(cos, form).and_then(|res| res.get(b"XObject").map(|x| cos.resolve(x))).and_then(|x| x.as_dict().cloned()) else { return };
        for (_, v) in xobjects.iter() {
            if let Some(r) = v.as_ref()
                && matches!(&*cos.get(r), Object::Stream(s) if s.dict.name(b"Subtype") == Some(b"Form"))
            {
                forms(cos, r, depth + 1, seen, out);
            }
        }
    }
    let mut all = Vec::new();
    forms(cos, form, 0, &mut HashSet::new(), &mut all);
    let mut blank: Option<ObjRef> = None;
    let mut gone = 0;
    for f in all {
        let Some(res) = (match &*cos.get(f) { Object::Stream(s) => s.dict.get(b"Resources").cloned(), _ => None }) else { continue };
        let xobj = cos.resolve(&res).as_dict().and_then(|d| d.get(b"XObject").cloned());
        let Some(xobj) = xobj else { continue };
        let Some(mut dict) = cos.resolve(&xobj).as_dict().cloned() else { continue };
        let pictures: Vec<Vec<u8>> = dict
            .iter()
            .filter(|(_, v)| v.as_ref().is_some_and(|r| matches!(&*cos.get(r), Object::Stream(s) if s.dict.name(b"Subtype") == Some(b"Image"))))
            .map(|(k, _)| k.clone())
            .collect();
        if pictures.is_empty() {
            continue;
        }
        let empty = *blank.get_or_insert_with(|| {
            let mut d = Dict::new();
            d.set(b"Type".to_vec(), Object::name("XObject"));
            d.set(b"Subtype".to_vec(), Object::name("Form"));
            d.set(b"BBox".to_vec(), vec![Object::Real(0.0), Object::Real(0.0), Object::Real(0.0), Object::Real(0.0)]);
            cos.add(pdfcraft_cos::Stream::from_raw(d, Vec::new()))
        });
        for k in &pictures {
            dict.set(k.clone(), empty);
        }
        gone += pictures.len();
        // Written back where it lives: its own object, the resources' object, or the Form's dictionary.
        let written = match (xobj.as_ref(), res.as_ref()) {
            (Some(x), _) => cos.update_dict(x, |d| *d = dict),
            (None, Some(r)) => cos.update_dict(r, |d| d.set(b"XObject".to_vec(), dict)),
            (None, None) => cos.update_dict(f, |d| {
                if let Some(Object::Dict(res)) = d.get_mut(b"Resources") {
                    res.set(b"XObject".to_vec(), dict);
                }
            }),
        };
        if written.is_err() {
            gone -= pictures.len();
        }
    }
    gone
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

/// Whether content paints a path (fills or strokes), outside text and inline images.
pub fn paints_paths(content: &[u8]) -> bool {
    let mut in_text = false;
    let text = String::from_utf8_lossy(content);
    let mut words = text.split(|c: char| c.is_ascii_whitespace() || "[]()<>/".contains(c)).filter(|w| !w.is_empty());
    while let Some(w) = words.next() {
        match w {
            "BT" => in_text = true,
            "ET" => in_text = false,
            // An inline image's data is skipped to its end.
            "ID" => {
                for w in words.by_ref() {
                    if w == "EI" {
                        break;
                    }
                }
            }
            "f" | "F" | "f*" | "S" | "s" | "B" | "B*" | "b" | "b*" if !in_text => return true,
            _ => {}
        }
    }
    false
}

