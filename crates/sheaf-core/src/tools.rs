//! Static catalogue of the tools Sheaf offers. The GUI renders its tools grid from it.

/// Identifies one tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolId {
    Merge,
    Split,
    RemovePages,
    ExtractPages,
    Organize,
    Rotate,
    Compress,
    Repair,
    Ocr,
    JpgToPdf,
    WordToPdf,
    PowerPointToPdf,
    ExcelToPdf,
    HtmlToPdf,
    PdfToJpg,
    PdfToWord,
    PdfToPowerPoint,
    PdfToExcel,
    PdfToPdfA,
    Edit,
    PageNumbers,
    Watermark,
    Crop,
    Sign,
    Protect,
    Unlock,
    Redact,
    Compare,
}

impl ToolId {
    /// Stable snake_case key, for UI models and saved settings.
    pub fn as_str(self) -> &'static str {
        match self {
            ToolId::Merge => "merge",
            ToolId::Split => "split",
            ToolId::RemovePages => "remove_pages",
            ToolId::ExtractPages => "extract_pages",
            ToolId::Organize => "organize",
            ToolId::Rotate => "rotate",
            ToolId::Compress => "compress",
            ToolId::Repair => "repair",
            ToolId::Ocr => "ocr",
            ToolId::JpgToPdf => "jpg_to_pdf",
            ToolId::WordToPdf => "word_to_pdf",
            ToolId::PowerPointToPdf => "powerpoint_to_pdf",
            ToolId::ExcelToPdf => "excel_to_pdf",
            ToolId::HtmlToPdf => "html_to_pdf",
            ToolId::PdfToJpg => "pdf_to_jpg",
            ToolId::PdfToWord => "pdf_to_word",
            ToolId::PdfToPowerPoint => "pdf_to_powerpoint",
            ToolId::PdfToExcel => "pdf_to_excel",
            ToolId::PdfToPdfA => "pdf_to_pdfa",
            ToolId::Edit => "edit",
            ToolId::PageNumbers => "page_numbers",
            ToolId::Watermark => "watermark",
            ToolId::Crop => "crop",
            ToolId::Sign => "sign",
            ToolId::Protect => "protect",
            ToolId::Unlock => "unlock",
            ToolId::Redact => "redact",
            ToolId::Compare => "compare",
        }
    }

    /// The tool whose `as_str` key is `s`.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<ToolId> {
        CATALOGUE.iter().map(|t| t.id).find(|id| id.as_str() == s)
    }
}

/// Group a tool is shown under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Organize,
    Optimize,
    ConvertToPdf,
    ConvertFromPdf,
    Edit,
    Security,
}

impl Category {
    /// Every category, in display order.
    pub const ALL: [Category; 6] = [
        Category::Organize,
        Category::Optimize,
        Category::ConvertToPdf,
        Category::ConvertFromPdf,
        Category::Edit,
        Category::Security,
    ];

    /// Heading shown above the category's tools.
    pub fn label(self) -> &'static str {
        match self {
            Category::Organize => "Organize PDF",
            Category::Optimize => "Optimize PDF",
            Category::ConvertToPdf => "Convert to PDF",
            Category::ConvertFromPdf => "Convert from PDF",
            Category::Edit => "Edit PDF",
            Category::Security => "PDF security",
        }
    }
}

/// Everything the GUI needs to show and launch a tool.
#[derive(Debug)]
pub struct ToolInfo {
    pub id: ToolId,
    pub name: &'static str,
    pub description: &'static str,
    pub category: Category,
    /// Lucide icon name.
    pub icon: &'static str,
    /// Accepted file extensions, lowercase, without the dot.
    pub accepts: &'static [&'static str],
    /// Whether the tool takes several files at once.
    pub multi: bool,
    /// Delivery wave the tool ships in.
    pub wave: u8,
}

const PDF: &[&str] = &["pdf"];

macro_rules! tool {
    ($id:ident, $name:literal, $desc:literal, $cat:ident, $icon:literal, $accepts:expr, $multi:literal, $wave:literal) => {
        ToolInfo {
            id: ToolId::$id,
            name: $name,
            description: $desc,
            category: Category::$cat,
            icon: $icon,
            accepts: $accepts,
            multi: $multi,
            wave: $wave,
        }
    };
}

static CATALOGUE: &[ToolInfo] = &[
    tool!(Merge, "Merge PDF", "Combine several PDFs into one, in the order you choose.", Organize, "combine", PDF, true, 1),
    tool!(Split, "Split PDF", "Cut a PDF into separate files by page ranges.", Organize, "scissors", PDF, false, 1),
    tool!(RemovePages, "Remove pages", "Delete the pages you no longer need from a PDF.", Organize, "file-minus", PDF, false, 1),
    tool!(ExtractPages, "Extract pages", "Pull selected pages out into a new PDF.", Organize, "file-output", PDF, false, 1),
    tool!(Organize, "Organize PDF", "Reorder, duplicate or drop pages by dragging thumbnails.", Organize, "layout-grid", PDF, false, 1),
    tool!(Rotate, "Rotate PDF", "Turn pages a quarter at a time without touching their content.", Organize, "rotate-cw", PDF, true, 1),
    tool!(Compress, "Compress PDF", "Shrink file size while keeping pages readable.", Optimize, "minimize-2", PDF, true, 1),
    tool!(Repair, "Repair PDF", "Rebuild a damaged PDF so it opens again.", Optimize, "wrench", PDF, true, 1),
    tool!(Ocr, "OCR PDF", "Add a searchable text layer to scanned pages.", Optimize, "scan-text", PDF, true, 1),
    tool!(JpgToPdf, "JPG to PDF", "Turn photos and images into a single PDF.", ConvertToPdf, "image", &["jpg", "jpeg", "png", "webp", "tiff", "bmp"], true, 1),
    tool!(WordToPdf, "Word to PDF", "Convert text documents to PDF with their layout intact.", ConvertToPdf, "file-text", &["doc", "docx", "odt", "rtf"], true, 1),
    tool!(PowerPointToPdf, "PowerPoint to PDF", "Save slide decks as PDFs that look the same everywhere.", ConvertToPdf, "presentation", &["ppt", "pptx", "odp"], true, 1),
    tool!(ExcelToPdf, "Excel to PDF", "Turn spreadsheets into printable PDF pages.", ConvertToPdf, "sheet", &["xls", "xlsx", "ods", "csv"], true, 1),
    tool!(HtmlToPdf, "HTML to PDF", "Render a web page file as a PDF.", ConvertToPdf, "code", &["html", "htm"], true, 1),
    tool!(PdfToJpg, "PDF to JPG", "Export each page as an image file.", ConvertFromPdf, "images", PDF, true, 1),
    tool!(PdfToWord, "PDF to Word", "Get an editable document back from a PDF.", ConvertFromPdf, "file-type", PDF, false, 4),
    tool!(PdfToPowerPoint, "PDF to PowerPoint", "Rebuild a PDF as slides you can edit.", ConvertFromPdf, "presentation", PDF, false, 4),
    tool!(PdfToExcel, "PDF to Excel", "Recover tables from a PDF into a spreadsheet.", ConvertFromPdf, "sheet", PDF, false, 4),
    tool!(PdfToPdfA, "PDF to PDF/A", "Save a PDF in the archival format meant to last.", ConvertFromPdf, "archive", PDF, true, 1),
    tool!(Edit, "Edit PDF", "Add text, shapes and images on top of your pages.", Edit, "pen-line", PDF, false, 3),
    tool!(PageNumbers, "Page numbers", "Stamp page numbers where you want them.", Edit, "list-ordered", PDF, true, 1),
    tool!(Watermark, "Add watermark", "Overlay text or an image across your pages.", Edit, "stamp", PDF, true, 1),
    tool!(Crop, "Crop PDF", "Trim the margins or keep only part of each page.", Edit, "crop", PDF, false, 1),
    tool!(Sign, "Sign PDF", "Place your signature on a document.", Security, "signature", PDF, false, 3),
    tool!(Protect, "Protect PDF", "Lock a PDF behind a password.", Security, "lock", PDF, true, 1),
    tool!(Unlock, "Unlock PDF", "Remove the password from a PDF you can open.", Security, "lock-open", PDF, true, 1),
    tool!(Redact, "Redact PDF", "Permanently black out sensitive text and areas.", Security, "eraser", PDF, false, 3),
    tool!(Compare, "Compare PDF", "Spot the differences between two versions of a document.", Security, "git-compare", PDF, false, 3),
];

/// All tools, in display order.
pub fn catalogue() -> &'static [ToolInfo] {
    CATALOGUE
}

/// The catalogue entry for `id`.
pub fn find(id: ToolId) -> &'static ToolInfo {
    CATALOGUE
        .iter()
        .find(|t| t.id == id)
        .expect("catalogue covers every ToolId")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_keys_round_trip() {
        for t in catalogue() {
            assert_eq!(ToolId::from_str(t.id.as_str()), Some(t.id));
        }
        assert_eq!(ToolId::from_str("nope"), None);
    }

    #[test]
    fn tool_keys_are_unique() {
        let keys: std::collections::HashSet<_> = catalogue().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(keys.len(), 28);
        assert_eq!(catalogue().len(), 28);
    }
}
