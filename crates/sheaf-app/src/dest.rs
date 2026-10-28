//! Where a job's outputs go: the output-location preference, or a Save / folder dialog.

use std::path::{Path, PathBuf};

use sheaf_app::jobs;
use sheaf_app::prefs::{OutputMode, Prefs};
use sheaf_app::{AppWindow, ToolOptions};

use crate::dialog;
use sheaf_core::export::Office;
use sheaf_core::tools::ToolId;

/// Where the outputs go.
pub enum Dest {
    /// One output file, chosen in a Save dialog.
    File(PathBuf),
    /// A folder; each output gets its default name there.
    Folder(PathBuf),
    /// Each output next to its own input, default names ("Save next to the original file").
    /// Tools writing into one folder (Split, PDF to JPG) use the first input's folder.
    NextToEach,
}

/// Tools whose single run always writes several files.
fn always_many(tool: ToolId) -> bool {
    matches!(tool, ToolId::Split | ToolId::PdfToJpg)
}

/// Tools that write one output from all their inputs (JPG to PDF unless "Merge all images" is off).
fn combines(tool: ToolId, opts: &ToolOptions) -> bool {
    matches!(tool, ToolId::Merge | ToolId::Compare) || (tool == ToolId::JpgToPdf && !opts.one_per_image)
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "document".into())
}

/// Default name of the output made from `input`.
pub fn out_name(tool: ToolId, input: &Path, opts: &ToolOptions) -> String {
    if tool == ToolId::Merge && !opts.out_name.trim().is_empty() {
        let n = opts.out_name.trim();
        return if n.to_lowercase().ends_with(".pdf") { n.to_string() } else { format!("{n}.pdf") };
    }
    if tool == ToolId::PdfToText {
        return format!("{}.{}", stem(input), text_format(opts).extension());
    }
    jobs::default_name(tool, &stem(input))
}

/// PDF to HTML or text's format: 0 web page, 1 Markdown, 2 rich text, 3 plain text, 4 XML.
pub fn text_format(opts: &ToolOptions) -> Office {
    [Office::Html, Office::Markdown, Office::Rtf, Office::Text, Office::Xml, Office::Epub][opts.text_format.clamp(0, 5) as usize]
}

/// Why a typed output name cannot be a file name on every system, or `None` when it can (empty
/// is fine: the default name is used).
pub fn file_name_error(name: &str) -> Option<String> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    if let Some(c) = n.chars().find(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()) {
        return Some(if c.is_control() { "A name cannot contain control characters".into() } else { format!("A name cannot contain {c}") });
    }
    if n.starts_with('.') {
        return Some("A name cannot start with a dot".into());
    }
    let stem = n.split('.').next().unwrap_or("").to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && (b'1'..=b'9').contains(&stem.as_bytes()[3]));
    if reserved {
        return Some(format!("{stem} is reserved on Windows"));
    }
    // Systems cap a name at 255 bytes; ".pdf" may still be added.
    if n.len() > 250 {
        return Some("This name is too long".into());
    }
    None
}

/// Whether the run writes into a folder (several outputs) rather than one chosen file.
fn writes_folder(tool: ToolId, inputs: &[PathBuf], opts: &ToolOptions) -> bool {
    always_many(tool) || (!combines(tool, opts) && inputs.len() > 1)
}

/// Where the outputs go without asking: the preference's folder, default names.
/// `None` = ask (the preference, or a fixed folder that no longer exists).
pub fn pref_dest(tool: ToolId, inputs: &[PathBuf], opts: &ToolOptions, prefs: &Prefs) -> Option<Dest> {
    let first = inputs.first()?;
    let dir = match prefs.output {
        OutputMode::Ask => return None,
        OutputMode::NextToOriginal if writes_folder(tool, inputs, opts) => return Some(Dest::NextToEach),
        OutputMode::NextToOriginal => parent_dir(first),
        OutputMode::Fixed => prefs.fixed_folder.clone().filter(|d| d.is_dir())?,
    };
    Some(if writes_folder(tool, inputs, opts) { Dest::Folder(dir) } else { Dest::File(dir.join(out_name(tool, first, opts))) })
}

/// The folder holding `p` ("." for a bare name).
pub fn parent_dir(p: &Path) -> PathBuf {
    p.parent().filter(|d| !d.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

/// The Save dialog's file type for `tool`'s output: (name, extension).
pub fn save_filter(tool: ToolId, opts: &ToolOptions) -> (&'static str, &'static str) {
    match tool {
        ToolId::PdfToText => match text_format(opts) {
            Office::Markdown => ("Markdown", "md"),
            Office::Rtf => ("Rich text", "rtf"),
            Office::Text => ("Plain text", "txt"),
            Office::Xml => ("XML", "xml"),
            Office::Epub => ("Ebook", "epub"),
            _ => ("Web page", "html"),
        },
        ToolId::PdfToWord => ("Word document", "docx"),
        ToolId::PdfToPowerPoint => ("PowerPoint presentation", "pptx"),
        ToolId::PdfToExcel => ("Excel workbook", "xlsx"),
        _ => ("PDF", "pdf"),
    }
}

/// Asks where the outputs go (Save sheet or folder picker); `then` runs once the user confirms.
pub fn ask_dest(ui: &AppWindow, tool: ToolId, inputs: &[PathBuf], opts: &ToolOptions, then: impl FnOnce(&AppWindow, Dest) + 'static) {
    let Some(first) = inputs.first() else { return };
    let dir = parent_dir(first);
    if writes_folder(tool, inputs, opts) {
        let d = dialog::new(ui).set_title(sheaf_app::i18n::tr("Choose a folder for the files")).set_directory(&dir);
        dialog::pick_folder(ui, d, move |ui, p| then(ui, Dest::Folder(p)));
    } else {
        let (name, ext) = save_filter(tool, opts);
        let d = dialog::new(ui).set_directory(&dir).set_file_name(out_name(tool, first, opts)).add_filter(name, &[ext]);
        dialog::save_file(ui, d, move |ui, p| then(ui, Dest::File(p)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> ToolOptions {
        ToolOptions { level: 1, every: 1, degrees: 90, ..Default::default() }
    }

    #[test]
    fn typed_output_names_are_checked() {
        for ok in ["", "  ", "report", "Q3 report.pdf", "rapport été", "COM0", &"é".repeat(125)] {
            assert_eq!(file_name_error(ok), None, "{ok:?}");
        }
        for bad in ["a/b", "a\\b", "x:y", "what?", "..", ".hidden", "con", "COM1.pdf", "tab\tname", &"é".repeat(126)] {
            assert!(file_name_error(bad).is_some(), "{bad:?} accepted");
        }
        assert_eq!(file_name_error("a/b").unwrap(), "A name cannot contain /");
    }

    #[test]
    fn preferences_choose_the_destination_without_a_dialog() {
        let one = [PathBuf::from("/a/x.pdf")];
        let two = [PathBuf::from("/a/x.pdf"), PathBuf::from("/b/y.pdf")];
        let mut p = Prefs::default();
        assert!(pref_dest(ToolId::Compress, &one, &opts(), &p).is_none());
        p.output = OutputMode::NextToOriginal;
        assert!(matches!(pref_dest(ToolId::Compress, &one, &opts(), &p), Some(Dest::File(f)) if f == Path::new("/a/x-compressed.pdf")));
        // A batch keeps each output next to its own input (/a and /b), see run::tests.
        assert!(matches!(pref_dest(ToolId::Compress, &two, &opts(), &p), Some(Dest::NextToEach)));
        assert!(matches!(pref_dest(ToolId::Merge, &two, &opts(), &p), Some(Dest::File(f)) if f == Path::new("/a/x-merged.pdf")));
        assert!(matches!(pref_dest(ToolId::Split, &one, &opts(), &p), Some(Dest::NextToEach)));
        // JPG to PDF: one PDF of all the images, or one each when "Merge all images" is off.
        let imgs = [PathBuf::from("/a/x.png"), PathBuf::from("/b/y.jpg")];
        assert!(matches!(pref_dest(ToolId::JpgToPdf, &imgs, &opts(), &p), Some(Dest::File(f)) if f == Path::new("/a/x.pdf")));
        let each = ToolOptions { one_per_image: true, ..opts() };
        assert!(matches!(pref_dest(ToolId::JpgToPdf, &imgs, &each, &p), Some(Dest::NextToEach)));
        let dir = tempfile::tempdir().unwrap();
        p.output = OutputMode::Fixed;
        p.fixed_folder = Some(dir.path().to_path_buf());
        assert!(matches!(pref_dest(ToolId::Compress, &one, &opts(), &p), Some(Dest::File(f)) if f == dir.path().join("x-compressed.pdf")));
        assert!(matches!(pref_dest(ToolId::PdfToExcel, &one, &opts(), &p), Some(Dest::File(f)) if f == dir.path().join("x.xlsx")));
        p.fixed_folder = Some(dir.path().join("gone"));
        assert!(pref_dest(ToolId::Compress, &one, &opts(), &p).is_none());
        assert_eq!(save_filter(ToolId::PdfToWord, &opts()), ("Word document", "docx"));
        assert_eq!(save_filter(ToolId::Crop, &opts()), ("PDF", "pdf"));
        let mut md = opts();
        md.text_format = 1;
        assert_eq!(save_filter(ToolId::PdfToText, &md), ("Markdown", "md"));
        assert_eq!(out_name(ToolId::PdfToText, Path::new("/a/x.pdf"), &md), "x.md");
        assert_eq!(out_name(ToolId::PdfToText, Path::new("/a/x.pdf"), &opts()), "x.html");
    }
}
