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
/// Merge takes PDFs, images and Office or HTML documents, as iLovePDF's Merge.
const MERGE: &[&str] = &["pdf", "jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp", "gif", "doc", "docx", "odt", "rtf", "ppt", "pptx", "odp", "xls", "xlsx", "ods", "csv", "html", "htm"];

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
    tool!(Merge, "Merge PDF", "Combine PDFs, images and documents into one PDF, in the order you choose.", Organize, "combine", MERGE, true, 1),
    tool!(Split, "Split PDF", "Cut a PDF into separate files by page ranges.", Organize, "scissors", PDF, false, 1),
    tool!(RemovePages, "Remove pages", "Delete the pages you no longer need from a PDF.", Organize, "file-minus", PDF, false, 1),
    tool!(ExtractPages, "Extract pages", "Pull selected pages out into a new PDF.", Organize, "file-output", PDF, false, 1),
    tool!(Organize, "Organize PDF", "Reorder, duplicate or drop pages by dragging thumbnails.", Organize, "layout-grid", PDF, false, 1),
    tool!(Rotate, "Rotate PDF", "Turn pages a quarter at a time without touching their content.", Organize, "rotate-cw", PDF, true, 1),
    tool!(Compress, "Compress PDF", "Shrink file size while keeping pages readable.", Optimize, "minimize-2", PDF, true, 1),
    tool!(Repair, "Repair PDF", "Rebuild a damaged PDF so it opens again.", Optimize, "wrench", PDF, true, 1),
    tool!(Ocr, "OCR PDF", "Add a searchable text layer to scanned pages.", Optimize, "scan-text", PDF, true, 1),
    tool!(JpgToPdf, "JPG to PDF", "Turn photos and images into a PDF.", ConvertToPdf, "image", &["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp", "gif"], true, 1),
    tool!(WordToPdf, "Word to PDF", "Convert text documents to PDF with their layout intact.", ConvertToPdf, "file-text", &["doc", "docx", "odt", "rtf"], true, 1),
    tool!(PowerPointToPdf, "PowerPoint to PDF", "Save slide decks as PDFs that look the same everywhere.", ConvertToPdf, "presentation", &["ppt", "pptx", "odp"], true, 1),
    tool!(ExcelToPdf, "Excel to PDF", "Turn spreadsheets into printable PDF pages.", ConvertToPdf, "sheet", &["xls", "xlsx", "ods", "csv"], true, 1),
    tool!(HtmlToPdf, "HTML to PDF", "Render a web page file as a PDF.", ConvertToPdf, "code", &["html", "htm"], true, 1),
    tool!(PdfToJpg, "PDF to JPG", "Export each page as an image file.", ConvertFromPdf, "images", PDF, true, 1),
    tool!(PdfToWord, "PDF to Word", "Get an editable document back from a PDF.", ConvertFromPdf, "file-type", PDF, true, 1),
    tool!(PdfToPowerPoint, "PDF to PowerPoint", "Rebuild a PDF as slides you can edit.", ConvertFromPdf, "presentation", PDF, true, 1),
    tool!(PdfToExcel, "PDF to Excel", "Recover tables from a PDF into a spreadsheet.", ConvertFromPdf, "sheet", PDF, true, 1),
    tool!(PdfToPdfA, "PDF to PDF/A", "Save a PDF in the archival format meant to last.", ConvertFromPdf, "archive", PDF, true, 1),
    tool!(Edit, "Edit PDF", "Add text and images on top of your pages.", Edit, "pen-line", PDF, false, 1),
    tool!(PageNumbers, "Page numbers", "Stamp page numbers where you want them.", Edit, "list-ordered", PDF, true, 1),
    tool!(Watermark, "Add watermark", "Overlay text or an image across your pages.", Edit, "stamp", PDF, true, 1),
    tool!(Crop, "Crop PDF", "Trim the margins or keep only part of each page.", Edit, "crop", PDF, false, 1),
    tool!(Sign, "Sign PDF", "Place your signature on a document.", Security, "signature", PDF, false, 1),
    tool!(Protect, "Protect PDF", "Lock a PDF behind a password.", Security, "lock", PDF, true, 1),
    tool!(Unlock, "Unlock PDF", "Remove the password from a PDF you can open.", Security, "lock-open", PDF, true, 1),
    tool!(Redact, "Redact PDF", "Permanently black out sensitive text and areas.", Security, "eraser", PDF, false, 1),
    tool!(Compare, "Compare PDF", "Spot the differences between two versions of a document.", Security, "git-compare", PDF, true, 1),
];

/// Tools matching what is typed in the Tools search, best first: a name starting with it, a name
/// containing it, a word of the description starting with it, then the letters in order in the
/// name ("cmprs" finds Compress, as Stirling's tool search). Nothing typed: nothing found.
pub fn search(query: &str) -> Vec<&'static ToolInfo> {
    let q: String = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let score = |t: &ToolInfo| {
        let name = t.name.to_lowercase();
        let in_order = |hay: &str| {
            let mut letters = hay.chars().filter(|c| !c.is_whitespace());
            q.chars().filter(|c| !c.is_whitespace()).all(|c| letters.any(|h| h == c))
        };
        if name.starts_with(&q) {
            Some(0)
        } else if name.contains(&q) {
            Some(1)
        } else if t.description.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&q)) {
            Some(2)
        } else if in_order(&name) {
            Some(3)
        } else {
            None
        }
    };
    let mut found: Vec<(u8, usize, &'static ToolInfo)> = catalogue().iter().enumerate().filter_map(|(i, t)| score(t).map(|s| (s, i, t))).collect();
    found.sort_by_key(|(s, i, _)| (*s, *i));
    found.into_iter().map(|(.., t)| t).collect()
}

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

/// `n` and `word`, with an "s" unless `n` is 1 ("1 page", "3 pages").
pub fn count(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod summary_tests {
    use crate::pdfa::PdfALevel;

    #[test]
    fn result_lines_are_one_wording_for_cli_and_app() {
        assert_eq!((super::count(1, "page"), super::count(0, "page"), super::count(3, "page")), ("1 page".into(), "0 pages".into(), "3 pages".into()));
        assert_eq!(crate::excel::summary(0), "No tables found: the text is in column A");
        assert_eq!(crate::excel::summary(1), "1 table found");
        assert_eq!(crate::excel::summary(4), "4 tables found");
        assert_eq!(crate::ocr::summary(1, 12, 0), "1 page read, 12 words added");
        assert_eq!(crate::ocr::summary(2, 1, 1), "2 pages read, 1 word added, 1 page already had text");
        use crate::redact::{summary, MarkCounts};
        assert_eq!(summary(MarkCounts { texts: 0, pages: 0, areas: 1 }), "1 area redacted");
        assert_eq!(summary(MarkCounts { texts: 1, pages: 0, areas: 0 }), "1 text passage redacted");
        assert_eq!(summary(MarkCounts { texts: 2, pages: 1, areas: 1 }), "2 text passages, 1 page and 1 area redacted");
        assert_eq!(crate::pdfa::summary(PdfALevel::A2b, 0), "Conforms to PDF/A-2b");
        assert_eq!(crate::pdfa::summary(PdfALevel::A3b, 2), "2 files do not fully conform to PDF/A-3b");
        let c = crate::compare::CompareReport { path: Default::default(), added: 1, removed: 0, replaced: 2, regions: 1, was_encrypted: false };
        assert_eq!(c.summary(), "1 added, 0 removed, 2 changed; 1 other changed area outlined");
        let e = crate::edit::EditReport { path: Default::default(), added: 1, was_encrypted: false };
        assert_eq!(e.summary(), "1 item added");
        let s = crate::sign::SignReport { path: Default::default(), signer: Some("A".into()) };
        assert_eq!(s.summary(), "Signed digitally (certificate: A)");
    }
}
