use crate::{engine::{pdfium, pdfium_lock}, pages::Input, paths::{all_or_nothing, atomic_write}, OpdfError, Result};
use image::{codecs::jpeg::JpegEncoder, metadata::Orientation, DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageLayout { FitImage, A4 { landscape: bool, margin_pt: f32 } }

/// Renders each page to `<stem>-<n>.jpg` in `out_dir` (n 1-based, zero-padded to the page-count width).
/// All or nothing: if a page fails, the images already written are deleted.
pub fn pdf_to_jpg(input: &Input, dpi: u32, quality: u8, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let _g = pdfium_lock();
    let doc = input.open()?;
    let width = doc.pages().len().to_string().len();
    all_or_nothing(doc.pages().iter().enumerate(), |(i, page)| {
        let px = (page.width().value * dpi as f32 / 72.0).round() as i32;
        let img = page.render_with_config(&PdfRenderConfig::new().set_target_width(px))?.as_image()?;
        let target = out_dir.join(format!("{}-{:0width$}.jpg", input.stem(), i + 1));
        atomic_write(&target, |part| {
            let f = std::fs::File::create(part)?;
            img.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(f, quality))?;
            Ok(())
        })
    })
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
    if let PageLayout::A4 { margin_pt, .. } = layout
        && (!margin_pt.is_finite() || margin_pt < 0.0 || margin_pt >= 595.0 / 2.0)
    {
        return Err(OpdfError::Unsupported(format!("margin {margin_pt} pt leaves no room on an A4 page")));
    }
    let _g = pdfium_lock();
    let mut doc = pdfium().create_new_pdf()?;
    for p in images {
        let src = load_source(p)?;
        let (pxw, pxh) = match &src { Source::Jpeg { w, h } => (*w, *h), Source::Pixels(i) => (i.width(), i.height()) };
        let (iw, ih) = (pxw as f32 * 0.75, pxh as f32 * 0.75); // 96 dpi → pt
        let (pw, ph, m) = match layout {
            PageLayout::FitImage => (iw, ih, 0.0),
            PageLayout::A4 { landscape, margin_pt } => if landscape { (842.0, 595.0, margin_pt) } else { (595.0, 842.0, margin_pt) },
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
