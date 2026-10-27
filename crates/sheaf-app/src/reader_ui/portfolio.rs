//! Acrobat's portfolio browser uses the reader's existing attachment open/save paths.
use super::*;
use sheaf_app::{PortfolioBridge, PortfolioRow};

pub fn browse(ui: &AppWindow, id: i32) {
    let Some(mut files) = update(id, |r, _| {
        r.snap.attachments.iter().enumerate().map(|(i, a)| PortfolioRow {
            source: i as i32,
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
    b.set_preview_page(0); b.set_preview_pages(1); b.set_sort(0); b.set_descending(false);
    b.set_preview_text("Select Preview to inspect an embedded PDF, image or text file. Originals stay inside this portfolio.".into());
    let folders = files.iter().filter(|f| !f.folder.is_empty()).map(|f| f.folder.to_string()).collect::<std::collections::BTreeSet<_>>();
    b.set_folders(ModelRc::new(VecModel::from(folders.into_iter().map(slint::SharedString::from).collect::<Vec<_>>())));
    b.set_status(format!("{} embedded file(s)", files.len()).into());
    sort_rows(&mut files, 0, false);
    b.set_files(ModelRc::new(VecModel::from(files)));
}


pub fn sort(ui: &AppWindow, key: i32) {
    use slint::Model;
    let b = ui.global::<PortfolioBridge>();
    let descending = b.get_sort() == key && !b.get_descending();
    b.set_sort(key); b.set_descending(descending);
    let mut rows = b.get_files().iter().collect::<Vec<_>>();
    sort_rows(&mut rows, key, descending);
    b.set_files(ModelRc::new(VecModel::from(rows)));
}

fn sort_rows(rows: &mut [PortfolioRow], key: i32, descending: bool) {
    rows.sort_by(|a, b| {
        let bytes = |s: &str| s.split_whitespace().next().and_then(|v| v.parse::<u64>().ok());
        let order = match key {
            1 => a.description.to_lowercase().cmp(&b.description.to_lowercase()),
            2 => bytes(&a.size).cmp(&bytes(&b.size)),
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        };
        (if descending { order.reverse() } else { order }).then(a.source.cmp(&b.source))
    });
}

pub fn preview(ui: &AppWindow, id: i32, index: i32) {
    let b = ui.global::<PortfolioBridge>();
    b.set_preview_page(0); b.set_preview_pages(1);
    preview_at(ui, id, index, 0);
}

pub fn preview_page(ui: &AppWindow, id: i32, page: i32) {
    let b = ui.global::<PortfolioBridge>();
    if page < 0 || page >= b.get_preview_pages() || b.get_preview_busy() { return; }
    b.set_preview_page(page);
    preview_at(ui, id, b.get_selected(), page as usize);
}

fn preview_at(ui: &AppWindow, id: i32, index: i32, page: usize) {
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
        let result = embedded_preview(&bytes, password.as_deref(), &attachment, page);
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let b = ui.global::<PortfolioBridge>();
            if b.get_tab() != id || b.get_selected() != index || !b.get_browsing() || b.get_preview_page() != page as i32 { return; }
            b.set_preview_busy(false);
            match result {
                Ok((Some(pixels), _, pages)) => { b.set_preview_pages(pages as i32); b.set_preview(slint::Image::from_rgba8(pixels)); b.set_has_preview(true); }
                Ok((None, text, pages)) => { b.set_preview_pages(pages as i32); b.set_preview_text(text.into()); }
                Err(e) => b.set_preview_text(format!("Preview unavailable: {e}. Use Open or Save copy.").into()),
            }
        });
    });
}

type Preview = (Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>, String, usize);

fn embedded_preview(bytes: &[u8], password: Option<&str>, attachment: &render::Attachment, page: usize) -> Result<Preview, String> {
    let data = render::attachment_data(bytes, password, attachment)?;
    if data.len() > 32 * 1024 * 1024 { return Err("The decoded file exceeds the 32 MB preview limit".into()); }
    let ext = std::path::Path::new(&attachment.name).extension().unwrap_or_default().to_string_lossy().to_lowercase();
    if ext == "pdf" {
        let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let path = temp.path().join("preview.pdf");
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
        let input = Input::new(path);
        let pages = sheaf_core::pdfcraft::Doc::open(&input).map_err(|e| e.to_string())?.document().info.pages.len();
        if page >= pages { return Err("This preview page is outside the embedded PDF".into()); }
        return sheaf_app::viewer::render_page(&input, page, 400).map(|pixels| (Some(pixels), String::new(), pages));
    }
    if ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"].contains(&ext.as_str()) {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(&data)).with_guessed_format().map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(12000); limits.max_image_height = Some(12000); limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().map_err(|e| e.to_string())?.thumbnail(500, 500).into_rgba8();
        return Ok((Some(slint::SharedPixelBuffer::clone_from_slice(image.as_raw(), image.width(), image.height())), String::new(), 1));
    }
    if ["txt", "md", "csv", "json", "xml", "log"].contains(&ext.as_str()) {
        let end = data.len().min(64 * 1024);
        let text = String::from_utf8_lossy(&data[..end]);
        return Ok((None, format!("{text}{}", if end < data.len() { "\n[Preview truncated. Save a copy to read the complete file.]" } else { "" }), 1));
    }
    Ok((None, format!("No built-in preview for .{ext} files. Open or Save copy; this preview does not start LibreOffice or another external program."), 1))
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
        let (image, text, pages) = embedded_preview(&doc.document().bytes, None, attachment, 0).unwrap();
        assert_eq!(pages, 1);
        assert!(image.is_none()); assert_eq!(text, "Original portfolio notes");
    }
    #[test]
    fn portfolio_sort_keeps_attachment_identity_and_previews_every_pdf_page() {
        let mut rows = vec![PortfolioRow { name: "Zeta".into(), size: "9 bytes".into(), source: 0, ..Default::default() }, PortfolioRow { name: "alpha".into(), size: "100 bytes".into(), source: 1, ..Default::default() }];
        sort_rows(&mut rows, 0, false); assert_eq!(rows[0].source, 1);
        sort_rows(&mut rows, 2, false); assert_eq!(rows[0].source, 0);
        sort_rows(&mut rows, 2, true); assert_eq!(rows[0].source, 1);
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("three.pdf");
        let mut session = sheaf_core::pdfcraft::engine::Session::new();
        let bytes = session.create_from_text("First", "First page").unwrap();
        let id = session.open("three.pdf", None, bytes, None).unwrap();
        session.apply(id, Edit::InsertBlankPage { at: 1, width: 400.0, height: 500.0 }).unwrap();
        let bytes = session.save_full_bytes(id).unwrap(); std::fs::write(&source, bytes.as_slice()).unwrap();
        let portfolio = sheaf_core::portfolio::create(&[source], "Review", &dir.path().join("portfolio.pdf")).unwrap();
        let doc = sheaf_core::pdfcraft::Doc::open(&Input::new(portfolio)).unwrap();
        let attachment = &doc.document().info.attachments[0];
        let (image, _, pages) = embedded_preview(&doc.document().bytes, None, attachment, 1).unwrap();
        assert!(image.is_some()); assert_eq!(pages, 2);
        assert!(embedded_preview(&doc.document().bytes, None, attachment, 2).is_err());
    }

}
