//! Acrobat's Flattener Preview: compare a captured page before and after real PDF 1.3 conversion.
//! This read-only preview never replaces the working PDF or its undo history.

use crate::{compress::flatten_page_snapshot, pages::Input, pdfcraft::Doc, separations::Plate, OpdfError, Result};

pub fn preview(bytes: &[u8], password: Option<&str>, page: usize, progress: &mut dyn FnMut() -> bool) -> Result<Vec<Plate>> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source.pdf");
    std::fs::write(&source, bytes)?;
    let mut doc = Doc::open(&Input { path: source, password: password.map(str::to_string) })?;
    let info = doc.document().info.pages.get(page).ok_or_else(|| OpdfError::Unsupported("The preview page no longer exists".into()))?;
    let scale = (1200.0 / info.width.max(info.height)).min(2.0);
    if !scale.is_finite() || scale <= 0.0 { return Err(OpdfError::Unsupported("The page has no finite display size".into())); }
    if !progress() { return Err(OpdfError::Cancelled); }
    let before = doc.render(page, scale)?;
    let plain = doc.save_unprotected(&dir.path().join("plain.pdf"))?.0;
    drop(doc);
    let flat = flatten_page_snapshot(std::fs::read(plain)?, page, &mut |_| progress())?;
    let output = dir.path().join("flat.pdf");
    std::fs::write(&output, flat)?;
    let doc = Doc::open(&Input::new(output))?;
    let after = doc.render(0, scale)?;
    Ok(vec![Plate { name: "Original".into(), width: before.width, height: before.height, rgba: before.rgba.to_vec() },
        Plate { name: "Flattened PDF 1.3".into(), width: after.width, height: after.height, rgba: after.rgba.to_vec() }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    #[test]
    fn flattener_preview_renders_the_actual_conversion_and_keeps_source_bytes() {
        let mut pdf = lopdf::Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let content = pdf.add_object(Stream::new(dictionary! {}, b"1 0 0 rg 10 10 80 80 re f /Alpha gs 0 0 1 rg 40 40 50 50 re f".to_vec()));
        let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),100.into(),100.into()], "Contents" => content,
            "Resources" => dictionary! { "ExtGState" => dictionary! { "Alpha" => dictionary! { "ca" => 0.5 } } } });
        let green = pdf.add_object(Stream::new(dictionary! {}, b"0 1 0 rg 0 0 100 100 re f".to_vec()));
        let second = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),100.into(),100.into()], "Contents" => green });
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 2, "Kids" => vec![page.into(), second.into()] }.into());
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages }); pdf.trailer.set("Root", root);
        let mut bytes = Vec::new(); pdf.save_to(&mut bytes).unwrap();
        let before = bytes.clone();
        let images = preview(&bytes, None, 0, &mut || true).unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!((images[0].width, images[0].height), (images[1].width, images[1].height));
        assert_eq!(images[1].name, "Flattened PDF 1.3");
        assert!(images.iter().all(|i| i.rgba.as_chunks::<4>().0.iter().any(|p| p[0] < 250 || p[1] < 250 || p[2] < 250)));
        let second = preview(&bytes, None, 1, &mut || true).unwrap();
        for image in second {
            let pixel = &image.rgba[((image.height / 2 * image.width + image.width / 2) * 4) as usize..][..4];
            assert!(pixel[1] > 200 && pixel[0] < 30 && pixel[2] < 30, "the second page, not the first: {pixel:?}");
        }
        assert_eq!(bytes, before);
        assert!(matches!(preview(&bytes, None, 0, &mut || false), Err(OpdfError::Cancelled)));
    }
}
