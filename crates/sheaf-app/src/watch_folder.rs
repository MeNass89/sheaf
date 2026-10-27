//! Stirling's watched folders, locally while Sheaf is open. A PDF must have the same size and
//! modification time on two consecutive polls before processing; outputs live outside the inbox.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use crate::actions::{self, SavedAction};
use sheaf_core::{OpdfError, Result};

#[derive(Clone, PartialEq, Eq)]
struct Stamp { len: u64, modified: SystemTime }

struct Observed { stamp: Stamp, handled: bool }

pub type PollResults = Vec<(PathBuf, Result<PathBuf>)>;

pub struct FolderWatch {
    input: PathBuf,
    output: PathBuf,
    seen: BTreeMap<PathBuf, Observed>,
    title_names: bool,
}

impl FolderWatch {
    pub fn new(input: &Path, output: &Path, title_names: bool) -> Result<Self> {
        let input = input.canonicalize()?;
        std::fs::create_dir_all(output)?;
        let output = output.canonicalize()?;
        if !input.is_dir() || input.starts_with(&output) || output.starts_with(&input) {
            return Err(OpdfError::Unsupported("Choose separate input and output folders, neither inside the other".into()));
        }
        Ok(Self { input, output, seen: BTreeMap::new(), title_names })
    }

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
        for entry in std::fs::read_dir(&self.input)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() { continue; }
            let path = entry.path();
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) { continue; }
            let meta = entry.metadata()?;
            if meta.len() == 0 { continue; }
            current.insert(path, Stamp { len: meta.len(), modified: meta.modified()? });
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
    fn watched_folders_cannot_feed_their_outputs_back_to_the_input() {
        let t = tempfile::tempdir().unwrap();
        assert!(FolderWatch::new(t.path(), t.path(), false).is_err());
        assert!(FolderWatch::new(t.path(), &t.path().join("inside"), false).is_err());
        assert_eq!(safe_name("../A:B\\C?"), ".._A_B_C_");
        assert_eq!(safe_name("CON.txt"), "_CON.txt");
        assert_eq!(safe_name("..."), "Document");
    }
}
