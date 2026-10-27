//! A file that is not a PDF, opened from outside (Finder's Open With, a drop on the Dock icon, the
//! command line), shown in the reader to browse, as Stirling's non-PDF viewer shows text, Markdown,
//! CSV, JSON, logs and pictures (Sol, 27/10: such files opened a conversion tool, a .txt was
//! refused). The preview is a PDF made in memory and written to a private temporary folder; the
//! original file is only read. Office and HTML files still open their conversion tool: Stirling has
//! no preview for them either, and making one needs LibreOffice or a browser.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Text files shown as their text (Stirling's TextViewer, CsvViewer, JsonViewer and Markdown).
pub const TEXT: [&str; 10] = ["txt", "text", "md", "markdown", "csv", "tsv", "json", "xml", "log", "ini"];
/// Pictures shown one per page (Stirling's ImageViewer), those PdfCraft reads.
pub const IMAGES: [&str; 7] = ["png", "jpg", "jpeg", "tif", "tiff", "gif", "bmp"];
/// Larger files are not previewed (a 16 MB text is some 4,000 pages).
const MAX_BYTES: u64 = 16 * 1024 * 1024;

/// Whether `p` is shown in the reader as a preview.
pub fn previewable(p: &Path) -> bool {
    let e = crate::files::ext(p);
    TEXT.contains(&e.as_str()) || IMAGES.contains(&e.as_str())
}

/// `p` made into a PDF to browse: its text (JSON indented, tabs as spaces) on Letter pages, or its
/// pictures one per page. Written as `<name>.pdf` (so "notes.txt.pdf") in a folder of its own
/// under the system's temporary folder, which is returned.
pub fn preview_pdf(p: &Path) -> Result<PathBuf, String> {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("the file has no name")?;
    let size = std::fs::metadata(p).map_err(|e| e.to_string())?.len();
    if size > MAX_BYTES {
        return Err(format!("{name} is too large to preview ({} MB, at most 16)", size / (1024 * 1024)));
    }
    let data = std::fs::read(p).map_err(|e| e.to_string())?;
    let session = sheaf_core::pdfcraft::engine::Session::new();
    let e = crate::files::ext(p);
    let pdf = if IMAGES.contains(&e.as_str()) {
        session.convert_to_pdf(&name, &Arc::new(data)).map(|(_, b)| b).map_err(|e| e.to_string())?
    } else {
        session.create_from_text(&name, &drawable(&text_of(&e, &data))).map_err(|e| e.to_string())?
    };
    let dir = std::env::temp_dir().join("sheaf-previews").join(format!("{}-{}", std::process::id(), unique()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = dir.join(format!("{name}.pdf"));
    std::fs::write(&out, pdf.as_slice()).map_err(|e| e.to_string())?;
    Ok(out)
}

/// A number no other preview of this run has.
fn unique() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The text a file shows: decoded (UTF-8, a byte order mark, or Windows code pages), JSON indented
/// as Stirling's JSON viewer does, tabs (TSV columns) as four spaces.
fn text_of(ext: &str, data: &[u8]) -> String {
    let text = sheaf_core::pdfcraft::engine::decode_text(data);
    let text = if ext == "json" {
        serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or(text)
    } else {
        text
    };
    text.replace("\r\n", "\n").replace('\t', "    ")
}

/// Characters the preview's Helvetica cannot draw written as `[U+XXXX]`, as PdfCraft's reports do,
/// rather than the whole preview refused.
fn drawable(text: &str) -> String {
    const WIN_ANSI: &str = "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ";
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let u = u32::from(c);
        if c == '\n' || c == '\u{c}' || (0x20..0x7f).contains(&u) || (0xa0..0x100).contains(&u) || WIN_ANSI.contains(c) {
            out.push(c);
        } else if c.is_control() {
            out.push(' ');
        } else {
            out.push_str(&format!("[U+{u:04X}]"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(pdf: &Path) -> String {
        let d = lopdf::Document::load(pdf).unwrap();
        let n = d.get_pages().len() as u32;
        d.extract_text(&(1..=n).collect::<Vec<_>>()).unwrap()
    }

    /// A text, a JSON file, a log with other scripts in it, and a picture each open as a PDF to browse;
    /// the originals are untouched.
    #[test]
    fn text_json_and_pictures_become_a_pdf_to_browse() {
        let dir = tempfile::tempdir().unwrap();
        let txt = dir.path().join("notes.txt");
        std::fs::write(&txt, "Shopping list\r\n\tMilk\nBread").unwrap();
        let pdf = preview_pdf(&txt).unwrap();
        assert!(pdf.ends_with("notes.txt.pdf") && pdf.starts_with(std::env::temp_dir()), "{pdf:?}");
        let t = texts(&pdf);
        assert!(t.contains("Shopping list") && t.contains("Milk") && t.contains("Bread"), "{t}");
        assert_eq!(std::fs::read_to_string(&txt).unwrap(), "Shopping list\r\n\tMilk\nBread");

        let json = dir.path().join("data.json");
        std::fs::write(&json, r#"{"name":"Ada","tags":["a","b"]}"#).unwrap();
        let t = texts(&preview_pdf(&json).unwrap());
        assert!(t.contains("\"name\": \"Ada\""), "indented: {t}");

        let log = dir.path().join("app.log");
        std::fs::write(&log, "Café ok – 東京 ✓").unwrap();
        let t = texts(&preview_pdf(&log).unwrap());
        assert!(t.contains("Café ok") && t.contains("[U+6771]"), "{t}");

        let png = dir.path().join("dot.png");
        image::RgbImage::from_pixel(40, 30, image::Rgb([200, 0, 0])).save(&png).unwrap();
        let pdf = preview_pdf(&png).unwrap();
        assert_eq!(lopdf::Document::load(&pdf).unwrap().get_pages().len(), 1);
        // Two previews of one file do not overwrite each other.
        assert_ne!(preview_pdf(&png).unwrap(), pdf);
    }

    #[test]
    fn what_is_previewed_and_what_is_too_large() {
        for (name, yes) in [("a.TXT", true), ("b.md", true), ("c.csv", true), ("d.png", true), ("e.docx", false), ("f.html", false), ("g.pdf", false)] {
            assert_eq!(previewable(Path::new(name)), yes, "{name}");
        }
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.txt");
        std::fs::File::create(&big).unwrap().set_len(MAX_BYTES + 1).unwrap();
        assert!(preview_pdf(&big).unwrap_err().contains("too large"));
    }
}
