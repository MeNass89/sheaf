//! Acrobat's portfolio browser uses the reader's existing attachment open/save paths.
use super::*;
use sheaf_app::{PortfolioBridge, PortfolioRow};

pub fn browse(ui: &AppWindow, id: i32) {
    let Some(files) = update(id, |r, _| {
        r.snap.attachments.iter().map(|a| PortfolioRow {
            name: a.name.as_str().into(), detail: a.description.as_deref().unwrap_or_default().into(),
            description: a.description.as_deref().unwrap_or("—").into(),
            size: a.size.map(|n| format!("{n} bytes")).unwrap_or_else(|| "Unknown".into()).into(),
            kind: std::path::Path::new(&a.name).extension().unwrap_or_default().to_string_lossy().as_ref().into(),
            folder: a.name.rsplit_once('/').map(|(p, _)| p).unwrap_or_default().into(),
        }).collect::<Vec<_>>()
    }) else { return };
    let b = ui.global::<PortfolioBridge>();
    if b.get_busy() { return; }
    b.set_tab(id); b.set_browsing(true); b.set_visible(true); b.set_selected(-1);
    b.set_folder("".into()); b.set_has_preview(false); b.set_preview_busy(false);
    b.set_preview_text("Select Preview to inspect an embedded PDF, image or text file. Originals stay inside this portfolio.".into());
    let folders = files.iter().filter(|f| !f.folder.is_empty()).map(|f| f.folder.to_string()).collect::<std::collections::BTreeSet<_>>();
    b.set_folders(ModelRc::new(VecModel::from(folders.into_iter().map(slint::SharedString::from).collect::<Vec<_>>())));
    b.set_status(format!("{} embedded file(s)", files.len()).into());
    b.set_files(ModelRc::new(VecModel::from(files)));
}


pub fn preview(ui: &AppWindow, id: i32, index: i32) {
    let Some((bytes, password, attachment)) = update(id, |r, _| {
        r.snap.attachments.get(index as usize).cloned().map(|a| (r.snap.bytes.clone(), r.snap.password.clone(), a))
    }).flatten() else { return };
    let b = ui.global::<PortfolioBridge>();
    b.set_selected(index); b.set_has_preview(false);
    if attachment.size.is_none_or(|n| n > 32 * 1024 * 1024) {
        b.set_preview_text("Preview needs a known embedded size up to 32 MB. Use Open or Save copy for this file.".into()); b.set_preview_busy(false); return;
    }
    b.set_preview_busy(true); b.set_preview_text("".into());
    let weak = ui.as_weak();
    sheaf_app::pool::probes().submit(move || {
        let result = embedded_preview(&bytes, password.as_deref(), &attachment);
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let b = ui.global::<PortfolioBridge>();
            if b.get_tab() != id || b.get_selected() != index || !b.get_browsing() { return; }
            b.set_preview_busy(false);
            match result {
                Ok((Some(pixels), _)) => { b.set_preview(slint::Image::from_rgba8(pixels)); b.set_has_preview(true); }
                Ok((None, text)) => b.set_preview_text(text.into()),
                Err(e) => b.set_preview_text(format!("Preview unavailable: {e}. Use Open or Save copy.").into()),
            }
        });
    });
}

type Preview = (Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>, String);

fn embedded_preview(bytes: &[u8], password: Option<&str>, attachment: &render::Attachment) -> Result<Preview, String> {
    let data = render::attachment_data(bytes, password, attachment)?;
    if data.len() > 32 * 1024 * 1024 { return Err("The decoded file exceeds the 32 MB preview limit".into()); }
    let ext = std::path::Path::new(&attachment.name).extension().unwrap_or_default().to_string_lossy().to_lowercase();
    if ext == "pdf" {
        let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let path = temp.path().join("preview.pdf");
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
        return sheaf_app::viewer::render_page(&Input::new(path), 0, 400).map(|pixels| (Some(pixels), String::new()));
    }
    if ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"].contains(&ext.as_str()) {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(&data)).with_guessed_format().map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(12000); limits.max_image_height = Some(12000); limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().map_err(|e| e.to_string())?.thumbnail(500, 500).into_rgba8();
        return Ok((Some(slint::SharedPixelBuffer::clone_from_slice(image.as_raw(), image.width(), image.height())), String::new()));
    }
    if ["txt", "md", "csv", "json", "xml", "log"].contains(&ext.as_str()) {
        let end = data.len().min(64 * 1024);
        let text = String::from_utf8_lossy(&data[..end]);
        return Ok((None, format!("{text}{}", if end < data.len() { "\n[Preview truncated. Save a copy to read the complete file.]" } else { "" })));
    }
    Ok((None, format!("No built-in preview for .{ext} files. Open or Save copy; this preview does not start LibreOffice or another external program.")))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portfolio_browser_uses_embedded_description_size_and_native_text_preview() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.txt"); std::fs::write(&source, b"Original portfolio notes").unwrap();
        let path = sheaf_core::portfolio::create(&[source], "Review", &dir.path().join("portfolio.pdf")).unwrap();
        let doc = sheaf_core::pdfcraft::Doc::open(&Input::new(path)).unwrap();
        let attachment = &doc.document().info.attachments[0];
        assert_eq!(attachment.name, "notes.txt");
        assert_eq!(attachment.description.as_deref(), Some("Portfolio item: notes.txt"));
        assert_eq!(attachment.size, Some(b"Original portfolio notes".len()));
        let (image, text) = embedded_preview(&doc.document().bytes, None, attachment).unwrap();
        assert!(image.is_none()); assert_eq!(text, "Original portfolio notes");
    }
}
