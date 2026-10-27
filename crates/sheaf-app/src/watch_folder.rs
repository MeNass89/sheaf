//! Stirling's watched folders, locally while Sheaf is open. A PDF must have the same size and
//! modification time on two consecutive polls before processing; outputs live outside the inbox.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use crate::actions::{self, SavedAction};
use sheaf_core::{OpdfError, Result};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp { len: u64, modified: SystemTime }

#[derive(Serialize, Deserialize)]
struct Observed { stamp: Stamp, handled: bool }

pub type PollResults = Vec<(PathBuf, Result<PathBuf>)>;

#[derive(Serialize, Deserialize)]
pub struct FolderWatch {
    input: PathBuf,
    output: PathBuf,
    seen: BTreeMap<PathBuf, Observed>,
    title_names: bool,
    #[serde(default)]
    recursive: bool,
}

impl FolderWatch {
    pub fn new(input: &Path, output: &Path, title_names: bool) -> Result<Self> {
        let input = input.canonicalize()?;
        std::fs::create_dir_all(output)?;
        let output = output.canonicalize()?;
        if !input.is_dir() || input.starts_with(&output) || output.starts_with(&input) {
            return Err(OpdfError::Unsupported("Choose separate input and output folders, neither inside the other".into()));
        }
        Ok(Self { input, output, seen: BTreeMap::new(), title_names, recursive: false })
    }

    pub fn include_subfolders(&mut self, recursive: bool) { self.recursive = recursive; }

    /// One poll. Failed inputs are tried again only once their bytes' size/time changes, so a
    /// locked or damaged file does not flood the user with the same error every second.
    pub fn poll(&mut self, action: &SavedAction, before: &mut dyn FnMut() -> bool) -> Result<PollResults> {
        let output = self.output.clone();
        let title_names = self.title_names;
        self.poll_with(&mut |path| {
            if !before() { return Err(OpdfError::Cancelled); }
            let temp = tempfile::tempdir()?;
            let processed = actions::run_file(action, path, temp.path(), &mut |_, _| before())?;
            let name = if title_names {
                let doc = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(&processed))?;
                doc.document().info.title.clone().filter(|t| !t.trim().is_empty())
            } else { None };
            let fallback = path.file_stem().unwrap_or_default().to_string_lossy();
            let name = safe_name(name.as_deref().unwrap_or(&fallback));
            sheaf_core::paths::atomic_write(&output.join(format!("{name}.pdf")), |part| {
                std::fs::copy(&processed, part)?;
                Ok(())
            })
        })
    }

    fn poll_with(&mut self, process: &mut dyn FnMut(&Path) -> Result<PathBuf>) -> Result<PollResults> {
        let mut current = BTreeMap::new();
        let mut folders = vec![self.input.clone()];
        let mut visited = 0;
        while let Some(folder) = folders.pop() {
            visited += 1;
            if visited > 10_000 { return Err(OpdfError::Unsupported("This watch exceeds 10,000 folders. Choose a smaller inbox.".into())); }
            for entry in std::fs::read_dir(folder)? {
                let entry = entry?;
                let kind = entry.file_type()?;
                // Never follow a symlink into the output or outside the selected inbox.
                if kind.is_dir() && self.recursive { folders.push(entry.path()); }
                if !kind.is_file() { continue; }
                let path = entry.path();
                if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) { continue; }
                let meta = entry.metadata()?;
                if meta.len() == 0 { continue; }
                current.insert(path, Stamp { len: meta.len(), modified: meta.modified()? });
                if current.len() > 100_000 { return Err(OpdfError::Unsupported("This watch exceeds 100,000 PDFs. Choose a smaller inbox.".into())); }
            }
        }
        self.seen.retain(|p, _| current.contains_key(p));
        let mut results = Vec::new();
        for (path, stamp) in current {
            match self.seen.get(&path) {
                Some(old) if old.stamp == stamp && old.handled => continue,
                Some(old) if old.stamp == stamp => {
                    let result = process(&path);
                    // Cancellation leaves this file eligible after a later resume/poll.
                    let handled = !matches!(&result, Err(OpdfError::Cancelled));
                    self.seen.insert(path.clone(), Observed { stamp, handled });
                    results.push((path, result));
                    if !handled { break; }
                }
                _ => { self.seen.insert(path, Observed { stamp, handled: false }); }
            }
        }
        Ok(results)
    }
}

/// The watch and its handled revisions survive a normal restart (Stirling's Folder Scanning).
#[derive(Serialize, Deserialize)]
pub struct SavedWatch {
    version: u32,
    pub action: SavedAction,
    pub watch: FolderWatch,
}

impl SavedWatch {
    pub fn new(action: SavedAction, watch: FolderWatch) -> Result<Self> {
        action.validate()?;
        if action.steps.iter().any(|s| s.prompt) {
            return Err(OpdfError::Unsupported("Folder actions cannot pause".into()));
        }
        Ok(Self { version: 1, action, watch })
    }

    pub fn description(&self) -> String { format!("{} → {} · {}{}", self.watch.input.display(), self.watch.output.display(), self.action.name, if self.watch.recursive { " · subfolders included" } else { "" }) }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let bytes = serde_json::to_vec(self).map_err(|e| OpdfError::Unsupported(e.to_string()))?;
        crate::store_file::replace(&dir.join("folder-watch.json"), &bytes)?;
        Ok(())
    }

    pub fn load(dir: &Path) -> Result<Option<Self>> {
        let bytes = match std::fs::read(dir.join("folder-watch.json")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let saved: Self = serde_json::from_slice(&bytes).map_err(|e| OpdfError::Unsupported(format!("Cannot read saved folder watch: {e}")))?;
        if saved.version != 1 { return Err(OpdfError::Unsupported("Unsupported folder watch version".into())); }
        saved.action.validate()?;
        if saved.action.steps.iter().any(|s| s.prompt) { return Err(OpdfError::Unsupported("Folder actions cannot pause".into())); }
        // Resolve again: the folders must still be separate at resume, even after being moved.
        let watch = FolderWatch::new(&saved.watch.input, &saved.watch.output, saved.watch.title_names)?;
        if watch.input != saved.watch.input || watch.output != saved.watch.output {
            return Err(OpdfError::Unsupported("The watched folder moved. Configure the watch again.".into()));
        }
        Ok(Some(saved))
    }

    pub fn forget(dir: &Path) -> Result<()> {
        match std::fs::remove_file(dir.join("folder-watch.json")) {
            Ok(()) => Ok(()), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), Err(e) => Err(e.into()),
        }
    }
}

/// A title becomes a portable file name, never a path or a Windows device name.
pub fn safe_name(title: &str) -> String {
    let name: String = title.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).take(120).collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    let stem = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"].contains(&stem.as_str());
    if name.is_empty() { "Document".into() } else if reserved { format!("_{name}") } else { name.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watched_pdfs_wait_until_stable_run_once_and_retry_after_change() {
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("in"); let out = t.path().join("out");
        std::fs::create_dir(&input).unwrap();
        let file = input.join("scan.PDF");
        std::fs::write(&file, b"first").unwrap();
        std::fs::write(input.join("upload.pdf.part"), b"not ready").unwrap();
        let mut watch = FolderWatch::new(&input, &out, false).unwrap();
        let mut calls = 0;
        let mut process = |_: &Path| { calls += 1; Err(OpdfError::Unsupported("bad file".into())) };
        assert!(watch.poll_with(&mut process).unwrap().is_empty());
        assert_eq!(watch.poll_with(&mut process).unwrap().len(), 1);
        assert!(watch.poll_with(&mut process).unwrap().is_empty());
        std::fs::write(&file, b"changed size").unwrap();
        assert!(watch.poll_with(&mut process).unwrap().is_empty());
        assert_eq!(watch.poll_with(&mut process).unwrap().len(), 1);
        assert_eq!(calls, 2);
    }

    #[test]
    fn watched_action_uses_the_document_title_and_keeps_existing_outputs() {
        use sheaf_core::pdfcraft::engine::{Session, actions::Step};
        use crate::actions::ActionStep;
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("in"); let output = t.path().join("out");
        std::fs::create_dir(&input).unwrap();
        std::fs::create_dir(&output).unwrap();
        let bytes = Session::new().create_from_text("Scan", "Invoice").unwrap();
        std::fs::write(input.join("scan.pdf"), bytes.as_slice()).unwrap();
        std::fs::write(output.join("Invoice_2026.pdf"), b"keep this existing output").unwrap();
        let mut action = SavedAction::empty();
        action.steps.push(ActionStep::from_engine(&Step::SetTitle("Invoice/2026".into())));
        let mut watch = FolderWatch::new(&input, &output, true).unwrap();
        assert!(watch.poll(&action, &mut || true).unwrap().is_empty());
        let mut results = watch.poll(&action, &mut || true).unwrap();
        assert_eq!(results.len(), 1);
        let path = results.remove(0).1.unwrap();
        assert_eq!(path.file_name().unwrap(), "Invoice_2026 (1).pdf");
        assert_eq!(std::fs::read(output.join("Invoice_2026.pdf")).unwrap(), b"keep this existing output");
        assert!(watch.poll(&action, &mut || true).unwrap().is_empty());
    }

    #[test]
    fn watched_folders_cannot_feed_their_outputs_back_to_the_input() {
        let t = tempfile::tempdir().unwrap();
        assert!(FolderWatch::new(t.path(), t.path(), false).is_err());
        assert!(FolderWatch::new(t.path(), &t.path().join("inside"), false).is_err());
        assert_eq!(safe_name("../A:B\\C?"), ".._A_B_C_");
        assert_eq!(safe_name("CON.txt"), "_CON.txt");
        assert_eq!(safe_name("..."), "Document");
    }
    #[test]
    fn a_saved_recursive_watch_resumes_without_repeating_handled_revisions() {
        use crate::actions::ActionStep;
        use sheaf_core::pdfcraft::engine::actions::Step;
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("in"); let output = t.path().join("out");
        std::fs::create_dir_all(input.join("nested")).unwrap();
        let file = input.join("nested/scan.pdf");
        std::fs::write(&file, b"first revision").unwrap();
        let mut watch = FolderWatch::new(&input, &output, false).unwrap();
        assert!(watch.poll_with(&mut |_| panic!("subfolders disabled")).unwrap().is_empty());
        watch.include_subfolders(true);
        let mut action = SavedAction::empty();
        action.steps.push(ActionStep::from_engine(&Step::FlattenComments));
        let mut saved = SavedWatch::new(action, watch).unwrap();
        assert!(saved.watch.poll_with(&mut |_| panic!("needs stability")).unwrap().is_empty());
        assert_eq!(saved.watch.poll_with(&mut |_| Ok(output.join("done.pdf"))).unwrap().len(), 1);
        saved.save(t.path()).unwrap();
        let mut resumed = SavedWatch::load(t.path()).unwrap().unwrap();
        assert!(resumed.watch.recursive);
        assert!(resumed.watch.poll_with(&mut |_| panic!("already handled")).unwrap().is_empty());
        std::fs::write(&file, b"a newer and longer revision").unwrap();
        assert!(resumed.watch.poll_with(&mut |_| panic!("new revision needs stability")).unwrap().is_empty());
        assert_eq!(resumed.watch.poll_with(&mut |_| Ok(output.join("done2.pdf"))).unwrap().len(), 1);
        SavedWatch::forget(t.path()).unwrap();
        assert!(SavedWatch::load(t.path()).unwrap().is_none());
    }

}
