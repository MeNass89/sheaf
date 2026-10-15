#![allow(dead_code)]
use sheaf_core::engine::{new_document, pdfium_lock};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

/// Writes a PDF whose page i (1-based) contains the text "Page i".
pub fn fixture_pdf(dir: &Path, name: &str, pages: u32) -> PathBuf {
    let _g = pdfium_lock();
    let mut doc = new_document();
    let font = doc.fonts_mut().helvetica();
    for i in 1..=pages {
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut()
            .create_text_object(PdfPoints::new(72.0), PdfPoints::new(720.0), format!("Page {i}"), font, PdfPoints::new(24.0))
            .unwrap();
    }
    let path = dir.join(name);
    doc.save_to_file(&path).unwrap();
    path
}

/// Text of every page, trimmed, via lopdf (reliable on rotated pages).
pub fn page_texts(path: &Path) -> Vec<String> {
    let d = lopdf::Document::load(path).unwrap();
    (1..=d.get_pages().len() as u32).map(|i| d.extract_text(&[i]).unwrap().trim().to_string()).collect()
}
