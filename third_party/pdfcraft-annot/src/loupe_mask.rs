//! Sheaf: Preview's Loupe and Mask (Tools ▸ Annotate ▸ Loupe, Mask), as comments any viewer
//! shows.
//!
//! A loupe is a circle comment (`/Subtype /Circle`, `/PCLoupe` its magnification) whose
//! appearance draws, inside the circle, the page under it magnified about the circle's centre:
//! the page's content as it was when the loupe was placed (`/PCLoupeSource`, a form of the page's
//! content and resources), then the ring in the comment's colour. Moved or resized, it shows what
//! is under it there.
//!
//! A mask is a square comment over the whole page (`/PCMask` the hole: `[x0 y0 x1 y1 oval]`)
//! whose appearance dims everything outside the hole, as Preview's Mask shows one area alone.

use pdfcraft_cos::{Dict, Document, ObjRef, Object, PdfString, Stream};

use crate::{AnnotError, Meta, Rgb, annot_dict, annot_ref, annots, base_dict, draw_appearance, n, num_array, redraw_or_restore, rgb, set_annots, touch, unlocked};

/// The dictionary key holding a loupe's magnification, and the page form it magnifies.
pub const LOUPE: &[u8] = b"PCLoupe";
const LOUPE_SOURCE: &[u8] = b"PCLoupeSource";
/// The dictionary key holding a mask's hole.
pub const MASK: &[u8] = b"PCMask";

/// How dark a mask makes what it hides (Preview's is about half).
const MASK_OPACITY: f64 = 0.6;

/// A Bézier circle (four arcs) inscribed in `r`, as path operators, closed.
fn oval(r: [f64; 4]) -> String {
    let (cx, cy, rx, ry) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0, (r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0);
    let k = 0.552_284_75;
    let (kx, ky) = (rx * k, ry * k);
    format!(
        "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h\n",
        n(cx + rx),
        n(cy),
        n(cx + rx),
        n(cy + ky),
        n(cx + kx),
        n(cy + ry),
        n(cx),
        n(cy + ry),
        n(cx - kx),
        n(cy + ry),
        n(cx - rx),
        n(cy + ky),
        n(cx - rx),
        n(cy),
        n(cx - rx),
        n(cy - ky),
        n(cx - kx),
        n(cy - ry),
        n(cx),
        n(cy - ry),
        n(cx + kx),
        n(cy - ry),
        n(cx + rx),
        n(cy - ky),
        n(cx + rx),
        n(cy)
    )
}

fn rect_of(d: &Dict) -> Option<[f64; 4]> {
    let v: Vec<f64> = d.get(b"Rect")?.as_array()?.iter().filter_map(Object::as_f64).collect();
    let r = <[f64; 4]>::try_from(v).ok()?;
    let r = [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])];
    (r.iter().all(|v| v.is_finite()) && r[2] - r[0] >= 1.0 && r[3] - r[1] >= 1.0).then_some(r)
}

fn form(bbox: [f64; 4], content: &[u8], resources: Dict) -> Stream {
    let mut d = Dict::new();
    d.set(b"Type".to_vec(), Object::name("XObject"));
    d.set(b"Subtype".to_vec(), Object::name("Form"));
    d.set(b"BBox".to_vec(), num_array(&bbox));
    d.set(b"Resources".to_vec(), Object::Dict(resources));
    Stream::flate(d, content)
}

/// The page's content streams, decoded and joined.
fn page_content(doc: &Document, p: &pdfcraft_model::Page) -> Vec<u8> {
    let list: Vec<Object> = match p.dict.get(b"Contents").map(|c| doc.resolve(c)) {
        None => Vec::new(),
        Some(c) => match c.as_ref() {
            Object::Array(a) => a.clone(),
            _ => p.dict.get(b"Contents").cloned().into_iter().collect(),
        },
    };
    let mut content = Vec::new();
    for o in list {
        if let Object::Stream(s) = doc.resolve(&o).as_ref() {
            content.extend(s.decoded().unwrap_or_default());
            content.push(b'\n');
        }
    }
    content
}

/// The page's content as a form (its streams joined, its resources), for a loupe to magnify.
fn page_form(doc: &mut Document, page: usize) -> Result<ObjRef, AnnotError> {
    let p = pdfcraft_model::pages(doc).into_iter().nth(page).ok_or(AnnotError::NoSuchPage(page))?;
    let media = p.crop(doc);
    let content = page_content(doc, &p);
    let resources = p.dict.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned()).unwrap_or_default();
    Ok(doc.add(Object::Stream(form(media, &content, resources))))
}

/// The appearance of a loupe or a mask, from its dictionary; `None` for any other comment.
pub(crate) fn appearance(doc: &Document, d: &Dict) -> Option<Stream> {
    if let Some(k) = d.get(LOUPE).and_then(Object::as_f64) {
        let rect = rect_of(d)?;
        let source = d.get(LOUPE_SOURCE)?.clone();
        // The source must be a form (a stale reference draws a plain ring instead).
        let ok = matches!(doc.resolve(&source).as_ref(), Object::Stream(s) if s.dict.name(b"Subtype") == Some(b"Form"));
        let k = k.clamp(1.0, 16.0);
        let (cx, cy) = ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0);
        let col: Vec<f64> = d.get(b"C").and_then(Object::as_array).map(|a| a.iter().filter_map(Object::as_f64).collect()).unwrap_or_default();
        let col = if col.len() == 3 { [col[0], col[1], col[2]] } else { [0.35, 0.35, 0.35] };
        let w = d.get(b"BS").and_then(Object::as_dict).and_then(|b| b.get(b"W")).and_then(Object::as_f64).unwrap_or(2.0).clamp(0.5, 12.0);
        // The ring inside the box, its width within it.
        let inner = [rect[0] + w / 2.0, rect[1] + w / 2.0, rect[2] - w / 2.0, rect[3] - w / 2.0];
        let mut c = String::from("q\n");
        c.push_str(&oval(inner));
        c.push_str("W n\n1 g\n");
        c.push_str(&format!("{} {} {} {} re f\n", n(rect[0]), n(rect[1]), n(rect[2] - rect[0]), n(rect[3] - rect[1])));
        let mut res = Dict::new();
        if ok {
            c.push_str(&format!("{} 0 0 {} {} {} cm\n/PCLoupePage Do\n", n(k), n(k), n(cx * (1.0 - k)), n(cy * (1.0 - k))));
            let mut xo = Dict::new();
            xo.set(b"PCLoupePage".to_vec(), source);
            res.set(b"XObject".to_vec(), Object::Dict(xo));
        }
        c.push_str("Q\n");
        c.push_str(&format!("{} {} {} RG {} w\n", n(col[0]), n(col[1]), n(col[2]), n(w)));
        c.push_str(&oval(inner));
        c.push_str("S\n");
        return Some(form(rect, c.as_bytes(), res));
    }
    let hole: Vec<f64> = d.get(MASK)?.as_array()?.iter().filter_map(Object::as_f64).collect();
    let [x0, y0, x1, y1, round] = <[f64; 5]>::try_from(hole).ok()?;
    let rect = rect_of(d)?;
    let mut gs = Dict::new();
    gs.set(b"Type".to_vec(), Object::name("ExtGState"));
    gs.set(b"ca".to_vec(), Object::Real(MASK_OPACITY));
    gs.set(b"CA".to_vec(), Object::Real(MASK_OPACITY));
    let mut gss = Dict::new();
    gss.set(b"PCMaskGS".to_vec(), Object::Dict(gs));
    let mut res = Dict::new();
    res.set(b"ExtGState".to_vec(), Object::Dict(gss));
    // The page, less the hole (even-odd): dimmed.
    let mut c = format!("/PCMaskGS gs\n0 g\n{} {} {} {} re\n", n(rect[0]), n(rect[1]), n(rect[2] - rect[0]), n(rect[3] - rect[1]));
    let hole = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
    if round != 0.0 {
        c.push_str(&oval(hole));
    } else {
        c.push_str(&format!("{} {} {} {} re\n", n(hole[0]), n(hole[1]), n(hole[2] - hole[0]), n(hole[3] - hole[1])));
    }
    c.push_str("f*\n");
    Some(form(rect, c.as_bytes(), res))
}

fn add(doc: &mut Document, page: usize, d: Dict) -> Result<usize, AnnotError> {
    let p = crate::page_ref(doc, page)?;
    let r = doc.add(Object::Dict(d));
    draw_appearance(doc, r)?;
    let mut list = annots(doc, p);
    let index = list.len();
    list.push(Object::Ref(r));
    set_annots(doc, p, list)?;
    Ok(index)
}

/// Sheaf: a loupe on `page` (0-based): a circle of `radius` about `centre` (user space) showing
/// the page under it `magnify` times as large, ringed in `color` (Preview's Loupe). Returns its
/// index among the page's comments.
pub fn add_loupe(doc: &mut Document, page: usize, (centre, radius): ([f64; 2], f64), magnify: f64, color: Rgb, author: &str, meta: &Meta) -> Result<usize, AnnotError> {
    if !centre.iter().chain([radius, magnify].iter()).chain(color.iter()).all(|v| v.is_finite()) || radius < 4.0 || !(1.1..=16.0).contains(&magnify) {
        return Err(AnnotError::Invalid("a loupe needs a radius of 4 points or more and a magnification from 1.1 to 16".into()));
    }
    let source = page_form(doc, page)?;
    let p = crate::page_ref(doc, page)?;
    let rect = [centre[0] - radius, centre[1] - radius, centre[0] + radius, centre[1] + radius];
    let mut d = base_dict("Circle", rect, p, "Loupe", author, meta);
    d.set(b"Subj".to_vec(), PdfString::text("Loupe"));
    d.set(b"C".to_vec(), rgb(color));
    let mut bs = Dict::new();
    bs.set(b"W".to_vec(), Object::Real(2.0));
    bs.set(b"S".to_vec(), Object::name("S"));
    d.set(b"BS".to_vec(), Object::Dict(bs));
    d.set(LOUPE.to_vec(), Object::Real(magnify));
    d.set(LOUPE_SOURCE.to_vec(), Object::Ref(source));
    add(doc, page, d)
}

/// Sheaf: a mask on `page` (0-based): everything outside `hole` (user space; an oval inscribed
/// in it when `round`) dimmed (Preview's Mask). Returns its index among the page's comments.
pub fn add_mask(doc: &mut Document, page: usize, hole: [f64; 4], round: bool, author: &str, meta: &Meta) -> Result<usize, AnnotError> {
    let hole = [hole[0].min(hole[2]), hole[1].min(hole[3]), hole[0].max(hole[2]), hole[1].max(hole[3])];
    if !hole.iter().all(|v| v.is_finite()) || hole[2] - hole[0] < 2.0 || hole[3] - hole[1] < 2.0 {
        return Err(AnnotError::Invalid("the mask's opening is too small".into()));
    }
    let crop = pdfcraft_model::pages(doc).into_iter().nth(page).ok_or(AnnotError::NoSuchPage(page))?.crop(doc);
    let p = crate::page_ref(doc, page)?;
    let mut d = base_dict("Square", crop, p, "Mask", author, meta);
    d.set(b"Subj".to_vec(), PdfString::text("Mask"));
    d.set(b"C".to_vec(), rgb([0.0, 0.0, 0.0]));
    let mut bs = Dict::new();
    bs.set(b"W".to_vec(), Object::Real(0.0));
    d.set(b"BS".to_vec(), Object::Dict(bs));
    d.set(MASK.to_vec(), num_array(&[hole[0], hole[1], hole[2], hole[3], if round { 1.0 } else { 0.0 }]));
    add(doc, page, d)
}

/// The magnifications a loupe offers (Preview's loupe grows and shrinks; these are its steps).
pub const MAGNIFICATIONS: [f64; 4] = [1.5, 2.0, 3.0, 4.0];

/// Sheaf: loupe `index` on `page` (0-based) shows the page `magnify` times as large.
pub fn set_loupe_magnify(doc: &mut Document, page: usize, index: usize, magnify: f64, meta: &Meta) -> Result<(), AnnotError> {
    if !(1.1..=16.0).contains(&magnify) {
        return Err(AnnotError::Invalid("a loupe magnifies 1.1 to 16 times".into()));
    }
    let (_, r) = annot_ref(doc, page, index)?;
    unlocked(doc, r)?;
    let before = annot_dict(doc, r);
    if before.get(LOUPE).is_none() {
        return Err(AnnotError::Invalid("this comment is not a loupe".into()));
    }
    doc.update_dict(r, |d| {
        d.set(LOUPE.to_vec(), Object::Real(magnify));
        touch(d, meta);
    })?;
    redraw_or_restore(doc, r, before)
}

/// Sheaf: every loupe shows its page as it is now: one whose page's content changed since it was
/// drawn takes a new copy of it and is drawn again (after an edit of the page's content).
pub fn refresh_loupes(doc: &mut Document) {
    let Ok(pages) = crate::page_refs(doc) else { return };
    // The loupes first (most documents have none: nothing more is read).
    let view: &Document = doc;
    let loupes: Vec<(usize, ObjRef, ObjRef)> = pages
        .iter()
        .enumerate()
        .flat_map(|(i, p)| annots(view, *p).into_iter().map(move |e| (i, e)))
        .filter_map(|(i, e)| {
            let r = e.as_ref()?;
            let d = annot_dict(view, r);
            let source = d.reference(LOUPE_SOURCE).filter(|_| d.get(LOUPE).is_some())?;
            Some((i, r, source))
        })
        .collect();
    if loupes.is_empty() {
        return;
    }
    let models = pdfcraft_model::pages(doc);
    for (i, r, source) in loupes {
        let Some(model) = models.get(i) else { continue };
        let now = page_content(doc, model);
        let same = matches!(doc.get(source).as_ref(), Object::Stream(s) if s.decoded().ok().as_deref() == Some(now.as_slice()));
        if same {
            continue;
        }
        let Ok(fresh) = page_form(doc, i) else { continue };
        if doc.update_dict(r, |d| d.set(LOUPE_SOURCE.to_vec(), Object::Ref(fresh))).is_ok() {
            // A loupe that can't be drawn keeps its old appearance.
            let _ = draw_appearance(doc, r);
        }
    }
}

/// Sheaf: every mask's opening: ((page, index in its `/Annots`), the opening in user space), so
/// a click there picks the mask.
pub fn mask_holes(doc: &Document) -> Vec<((usize, usize), [f64; 4])> {
    let Ok(pages) = crate::page_refs(doc) else { return Vec::new() };
    let mut out = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        for (k, entry) in annots(doc, *p).iter().enumerate() {
            let d = doc.resolve(entry);
            let Some(hole) = d.as_dict().and_then(|d| d.get(MASK)).and_then(Object::as_array) else { continue };
            let v: Vec<f64> = hole.iter().filter_map(Object::as_f64).collect();
            if let [x0, y0, x1, y1, _] = v[..] {
                out.push(((i, k), [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)]));
            }
        }
    }
    out
}
