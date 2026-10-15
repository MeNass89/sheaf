//! Where a job's outputs go: the output-location preference, or a Save / folder dialog.

use std::path::{Path, PathBuf};

use sheaf_app::jobs;
use sheaf_app::prefs::{OutputMode, Prefs};
use sheaf_app::ToolOptions;
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

/// Tools that write one output from all their inputs.
fn combines(tool: ToolId) -> bool {
    matches!(tool, ToolId::Merge | ToolId::JpgToPdf)
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
    jobs::default_name(tool, &stem(input))
}

/// Whether the run writes into a folder (several outputs) rather than one chosen file.
fn writes_folder(tool: ToolId, inputs: &[PathBuf]) -> bool {
    always_many(tool) || (!combines(tool) && inputs.len() > 1)
}

/// Where the outputs go without asking: the preference's folder, default names.
/// `None` = ask (the preference, or a fixed folder that no longer exists).
pub fn pref_dest(tool: ToolId, inputs: &[PathBuf], opts: &ToolOptions, prefs: &Prefs) -> Option<Dest> {
    let first = inputs.first()?;
    let dir = match prefs.output {
        OutputMode::Ask => return None,
        OutputMode::NextToOriginal if writes_folder(tool, inputs) => return Some(Dest::NextToEach),
        OutputMode::NextToOriginal => parent_dir(first),
        OutputMode::Fixed => prefs.fixed_folder.clone().filter(|d| d.is_dir())?,
    };
    Some(if writes_folder(tool, inputs) { Dest::Folder(dir) } else { Dest::File(dir.join(out_name(tool, first, opts))) })
}

/// The folder holding `p` ("." for a bare name).
pub fn parent_dir(p: &Path) -> PathBuf {
    p.parent().filter(|d| !d.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

pub fn ask_dest(tool: ToolId, inputs: &[PathBuf], opts: &ToolOptions) -> Option<Dest> {
    let first = inputs.first()?;
    let dir = parent_dir(first);
    if writes_folder(tool, inputs) {
        rfd::FileDialog::new().set_title("Choose a folder for the files").set_directory(&dir).pick_folder().map(Dest::Folder)
    } else {
        rfd::FileDialog::new()
            .set_directory(&dir)
            .set_file_name(out_name(tool, first, opts))
            .add_filter("PDF", &["pdf"])
            .save_file()
            .map(Dest::File)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> ToolOptions {
        ToolOptions { level: 1, every: 1, degrees: 90, ..Default::default() }
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
        let dir = tempfile::tempdir().unwrap();
        p.output = OutputMode::Fixed;
        p.fixed_folder = Some(dir.path().to_path_buf());
        assert!(matches!(pref_dest(ToolId::Compress, &one, &opts(), &p), Some(Dest::File(f)) if f == dir.path().join("x-compressed.pdf")));
        p.fixed_folder = Some(dir.path().join("gone"));
        assert!(pref_dest(ToolId::Compress, &one, &opts(), &p).is_none());
    }

}
