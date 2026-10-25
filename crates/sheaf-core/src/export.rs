//! PDF to Word, PowerPoint, Excel, HTML and text: PdfCraft reads each page as paragraphs and
//! images, and writes them as an editable document. The layout is approximate.

use crate::ocr::{self, OcrOptions};
use crate::paths::atomic_write;
use crate::pdfcraft::engine::compare::OfficeFormat;
use crate::pdfcraft::Doc;
use crate::{pages::Input, Result};
use std::path::{Path, PathBuf};

/// Which Office file to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Office {
    /// .docx
    Word,
    /// .pptx, one slide per page
    PowerPoint,
    /// .html, one web page with the pictures inside it (Acrobat's Export ▸ HTML Web Page)
    Html,
    /// .rtf
    Rtf,
    /// .txt, a paragraph per line
    Text,
    /// .md, headings and tables kept
    Markdown,
}

impl Office {
    /// The file extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Office::Word => "docx",
            Office::PowerPoint => "pptx",
            Office::Html => "html",
            Office::Rtf => "rtf",
            Office::Text => "txt",
            Office::Markdown => "md",
        }
    }
}

/// Export settings shared by Word, PowerPoint and Excel.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExportOptions {
    /// Recognise the text of scanned pages first (in memory; the PDF is not changed).
    pub ocr: bool,
    /// The OCR settings when `ocr` is set.
    pub ocr_options: OcrOptions,
    /// Word: each page kept as laid out (positioned text boxes) instead of flowing paragraphs and
    /// tables (Acrobat's "Retain page layout").
    pub exact_layout: bool,
}

/// What an export wrote.
#[derive(Debug, Clone)]
pub struct ExportReport {
    pub path: PathBuf,
}

/// Opens `input` and, with `opts.ocr`, adds the recognised text in memory. `progress` gets
/// 0..=1 during OCR; returning `false` stops with [`crate::OpdfError::Cancelled`].
pub(crate) fn open_for_export(input: &Input, opts: &ExportOptions, progress: &mut dyn FnMut(f32) -> bool) -> Result<Doc> {
    let mut doc = Doc::open(input)?;
    if opts.ocr {
        ocr::recognize(&mut doc, &opts.ocr_options, progress)?;
    }
    Ok(doc)
}

/// Exports `input` as a Word, PowerPoint, web page or text file at `out` (never overwriting).
pub fn to_office(input: &Input, format: Office, opts: &ExportOptions, out: &Path, progress: &mut dyn FnMut(f32) -> bool) -> Result<ExportReport> {
    let doc = open_for_export(input, opts, progress)?;
    let engine = match format {
        Office::Word if opts.exact_layout => OfficeFormat::DocxLayout,
        Office::Word => OfficeFormat::Docx,
        Office::PowerPoint => OfficeFormat::Pptx,
        Office::Html => OfficeFormat::Html,
        Office::Rtf => OfficeFormat::Rtf,
        Office::Text => OfficeFormat::Text,
        Office::Markdown => OfficeFormat::Markdown,
    };
    let bytes = doc.document().export_office(engine);
    let path = atomic_write(out, |part| Ok(std::fs::write(part, &bytes)?))?;
    progress(1.0);
    Ok(ExportReport { path })
}
