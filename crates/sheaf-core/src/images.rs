use crate::{engine::{pdfium, pdfium_lock}, pages::Input, paths::{all_or_nothing, atomic_write}, OpdfError, Result};
use image::{codecs::jpeg::JpegEncoder, metadata::Orientation, DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

/// How each image is placed: on a page its own size, or centred on a sheet of paper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageLayout { FitImage, Paper { paper: Paper, landscape: bool, margin_pt: f32 } }

/// A sheet of paper, as iLovePDF offers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper { A4, Letter }

impl Paper {
    /// Portrait width and height in points.
    pub fn points(self) -> (f32, f32) {
        match self {
            Paper::A4 => (595.0, 842.0),
            Paper::Letter => (612.0, 792.0),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Paper::A4 => "an A4",
            Paper::Letter => "a Letter",
        }
    }
}

/// Pixels per rendered page at most (72 Mpx: 290 MB as RGBA, A3 at 600 dpi): a bigger page at
/// 600 dpi (A1, A0) is rendered smaller rather than failing to allocate gigabytes.
const MAX_PIXELS: f32 = 72e6;

/// A document closed under the pdfium lock, even when a panic unwinds past it.
struct Locked(Option<PdfDocument<'static>>);

impl Drop for Locked {
    fn drop(&mut self) {
        let _g = pdfium_lock();
        self.0.take();
    }
}

/// Renders each page to `<stem>-<n>.jpg` in `out_dir` (n 1-based, zero-padded to the page-count width)
/// at JPEG `quality`, or to `<stem>-<n>.png` when `quality` is `None`; at `dpi`, or less for a page
/// that would pass [`MAX_PIXELS`].
/// The pdfium lock is held only while a page renders, not while it is encoded, so thumbnails
/// elsewhere keep coming during a long export.
/// All or nothing: if a page fails, the images already written are deleted.
pub fn pdf_to_jpg(input: &Input, dpi: u32, quality: Option<u8>, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let g = pdfium_lock();
    let doc = Locked(Some(input.open()?));
    let n = doc.0.as_ref().map_or(0, |d| d.pages().len());
    drop(g);
    let width = n.to_string().len();
    all_or_nothing(0..n, |i| {
        let img = {
            let _g = pdfium_lock();
            let page = doc.0.as_ref().expect("open until the end").pages().get(i)?;
            let (w, h) = (page.width().value, page.height().value);
            let scale = (dpi as f32 / 72.0).min((MAX_PIXELS / (w * h).max(1.0)).sqrt());
            page.render_with_config(&PdfRenderConfig::new().set_target_width((w * scale).round() as i32))?.as_image()?.to_rgb8()
        };
        let ext = if quality.is_some() { "jpg" } else { "png" };
        let target = out_dir.join(format!("{}-{:0width$}.{ext}", input.stem(), i + 1));
        atomic_write(&target, |part| {
            match quality {
                Some(q) => img.write_with_encoder(JpegEncoder::new_with_quality(std::fs::File::create(part)?, q))?,
                None => img.save_with_format(part, ImageFormat::Png)?,
            }
            Ok(())
        })
    })
}

/// iLovePDF's "Extract images": every picture placed in `input`, as stored (not the page around
/// it), one file each named `<stem>-p<page>-<n>`, a picture repeated on several pages (a logo)
/// written once. JPG at `quality`, else PNG. A PDF without pictures says so.
pub fn extract_images(input: &Input, quality: Option<u8>, out_dir: &Path) -> Result<Vec<PathBuf>> {
    use std::hash::{Hash, Hasher};
    let ext = if quality.is_some() { "jpg" } else { "png" };
    let mut written: Vec<PathBuf> = Vec::new();
    // Each picture is written as soon as it is decoded (Sol, 25/10: a long scan kept gigabytes of
    // pixels before the first write); a failure takes back what was written, as before.
    // The pdfium lock is held while a page's pictures are read, not while they are encoded and
    // written (Sol, 25/10: other tabs' pages waited for the whole document), as `pdf_to_jpg`.
    let result = (|| -> Result<()> {
        let g = pdfium_lock();
        let doc = Locked(Some(input.open()?));
        let n = doc.0.as_ref().map_or(0, |d| d.pages().len());
        drop(g);
        let mut seen = std::collections::HashSet::new();
        for i in 0..n {
            let mut k = 0;
            let mut pictures = Vec::new();
            {
                let _g = pdfium_lock();
                let d = doc.0.as_ref().expect("open until the end");
                let page = d.pages().get(i)?;
                for o in page.objects().iter() {
                    pictures_in(&o, d, &mut pictures);
                }
            }
            for img in pictures {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                (img.width(), img.height(), img.as_bytes()).hash(&mut h);
                if !seen.insert(h.finish()) {
                    continue;
                }
                k += 1;
                let target = out_dir.join(format!("{}-p{}-{k}.{ext}", input.stem(), i + 1));
                written.push(atomic_write(&target, |part| {
                    match quality {
                        Some(q) => img.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(std::fs::File::create(part)?, q))?,
                        None => img.save_with_format(part, ImageFormat::Png)?,
                    }
                    Ok(())
                })?);
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        written.iter().for_each(|p| { let _ = std::fs::remove_file(p); });
        return Err(e);
    }
    if written.is_empty() {
        return Err(OpdfError::Unsupported(format!("{} has no pictures to extract", input.path.display())));
    }
    Ok(written)
}

/// The pictures of page object `o`: itself, or those a form XObject draws, at any depth (Sol,
/// 25/10: pictures inside forms were missed, and a PDF said to have none).
fn pictures_in(o: &PdfPageObject, doc: &PdfDocument, out: &mut Vec<DynamicImage>) {
    if let Some(img) = o.as_image_object() {
        if let Ok(img) = img.get_raw_image().or_else(|_| img.get_processed_image(doc)) {
            out.push(img);
        }
    } else if let Some(form) = o.as_x_object_form_object() {
        for i in 0..form.len() {
            if let Ok(child) = form.get(i) {
                pictures_in(&child, doc, out);
            }
        }
    }
}

/// Renders page `page` (0-based) to a PNG at `dpi`, written to `unique_path(out)`: a look at
/// the page as displayed, for checking a result.
pub fn render_png(input: &Input, page: usize, dpi: u32, out: &Path) -> Result<PathBuf> {
    let _g = pdfium_lock();
    let doc = input.open()?;
    let n = doc.pages().len() as usize;
    let p = doc.pages().get(page as i32).map_err(|_| OpdfError::Unsupported(format!("page {} does not exist (the document has {n} pages)", page + 1)))?;
    let px = (p.width().value * dpi as f32 / 72.0).round() as i32;
    let img = p.render_with_config(&PdfRenderConfig::new().set_target_width(px))?.as_image()?;
    atomic_write(out, |part| {
        img.to_rgba8().save_with_format(part, ImageFormat::Png)?;
        Ok(())
    })
}

/// The first page of PDF `bytes` as a PNG `scale` times its size in points, on a transparent
/// background, its comments drawn: a stamp copied as the picture it shows.
pub fn page_png_bytes(bytes: &[u8], scale: f32) -> Result<Vec<u8>> {
    let _g = pdfium_lock();
    let doc = crate::engine::open_pdf_bytes(bytes, None)?;
    let p = doc.pages().get(0)?;
    let px = (p.width().value * scale).round().clamp(1.0, 4096.0) as i32;
    let cfg = PdfRenderConfig::new().set_target_width(px).render_annotations(true).set_clear_color(PdfColor::new(255, 255, 255, 0));
    let img = p.render_with_config(&cfg)?.as_image()?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.to_rgba8().write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}

/// An input image: an untouched JPEG file (embedded as-is) or decoded, EXIF-oriented pixels.
enum Source { Jpeg { w: u32, h: u32 }, Pixels(DynamicImage) }

/// JPEGs without an EXIF rotation are passed through; everything else is decoded and oriented.
fn load_source(p: &Path) -> Result<Source> {
    let reader = ImageReader::open(p)?.with_guessed_format()?;
    let is_jpeg = reader.format() == Some(ImageFormat::Jpeg);
    let mut dec = reader.into_decoder()?;
    let o = dec.orientation()?;
    if is_jpeg && o == Orientation::NoTransforms {
        let (w, h) = dec.dimensions();
        return Ok(Source::Jpeg { w, h });
    }
    let mut img = DynamicImage::from_decoder(dec)?;
    img.apply_orientation(o);
    Ok(Source::Pixels(img))
}

/// One page per image. Pixels are taken at 96 dpi; images are never upscaled and are centred.
/// JPEGs that need no rotation are embedded without re-encoding.
pub fn images_to_pdf(images: &[PathBuf], layout: PageLayout, out: &Path) -> Result<PathBuf> {
    if images.is_empty() { return Err(OpdfError::Unsupported("no images".into())); }
    if let PageLayout::Paper { paper, margin_pt, .. } = layout
        && (!margin_pt.is_finite() || margin_pt < 0.0 || margin_pt >= paper.points().0 / 2.0)
    {
        return Err(OpdfError::Unsupported(format!("margin {margin_pt} pt leaves no room on {} page", paper.name())));
    }
    let _g = pdfium_lock();
    let mut doc = pdfium().create_new_pdf()?;
    for p in images {
        let src = load_source(p)?;
        let (pxw, pxh) = match &src { Source::Jpeg { w, h } => (*w, *h), Source::Pixels(i) => (i.width(), i.height()) };
        let (iw, ih) = (pxw as f32 * 0.75, pxh as f32 * 0.75); // 96 dpi → pt
        let (pw, ph, m) = match layout {
            PageLayout::FitImage => (iw, ih, 0.0),
            PageLayout::Paper { paper, landscape, margin_pt } => {
                let (w, h) = paper.points();
                if landscape { (h, w, margin_pt) } else { (w, h, margin_pt) }
            }
        };
        let s = ((pw - 2.0 * m) / iw).min((ph - 2.0 * m) / ih).min(1.0);
        let (w, h) = (iw * s, ih * s);
        let (x, y) = ((pw - w) / 2.0, (ph - h) / 2.0);
        let at = doc.pages().len();
        let mut page = doc.pages_mut().create_page_at_index(PdfPagePaperSize::Custom(PdfPoints::new(pw), PdfPoints::new(ph)), at)?;
        match src {
            Source::Jpeg { .. } => {
                let mut obj = PdfPageImageObject::new_from_jpeg_file(&doc, p)?;
                obj.scale(w, h)?;
                obj.translate(PdfPoints::new(x), PdfPoints::new(y))?;
                page.objects_mut().add_image_object(obj)?;
            }
            Source::Pixels(img) => {
                page.objects_mut().create_image_object(PdfPoints::new(x), PdfPoints::new(y), &img, Some(PdfPoints::new(w)), Some(PdfPoints::new(h)))?;
            }
        }
    }
    atomic_write(out, |part| Ok(doc.save_to_file(part)?))
}
