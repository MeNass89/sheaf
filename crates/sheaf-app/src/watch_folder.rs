//! Stirling's watched folders, locally while Sheaf is open. A PDF must have the same size and
//! revision on two consecutive polls before processing; outputs live outside the inbox.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use crate::actions::{self, SavedAction};
use sheaf_core::{OpdfError, Result};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    len: u64, modified: SystemTime,
    #[serde(default)]
    changed: Option<(i64, i64)>,
    #[serde(default)]
    fingerprint: Option<u64>,
}

fn revision(path: &Path) -> Result<Stamp> {
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    #[cfg(unix)]
    let (changed, fingerprint) = {
        use std::os::unix::fs::MetadataExt;
        // ctime changes even when a producer restores mtime after a same-size rewrite.
        (Some((metadata.ctime(), metadata.ctime_nsec())), None)
    };
    #[cfg(not(unix))]
    let (changed, fingerprint) = {
        use std::{hash::Hasher, io::Read};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut file = file;
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 { break; }
            hash.write(&buffer[..n]);
        }
        (None, Some(hash.finish()))
    };
    Ok(Stamp { len: metadata.len(), modified: metadata.modified()?, changed, fingerprint })
}

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
    #[serde(default)]
    pattern: String,
    #[serde(default)]
    archive_inputs: bool,
    #[serde(default)]
    sequence: u64,
    #[serde(skip)]
    folders: VecDeque<PathBuf>,
    #[serde(skip)]
    directory: Option<std::fs::ReadDir>,
    #[serde(skip)]
    current: BTreeMap<PathBuf, Stamp>,
    #[serde(skip)]
    visited: usize,
    #[serde(skip)]
    scanning: bool,
}

impl FolderWatch {
    pub fn new(input: &Path, output: &Path, title_names: bool) -> Result<Self> {
        let input = input.canonicalize()?;
        std::fs::create_dir_all(output)?;
        let output = output.canonicalize()?;
        if !input.is_dir() || input.starts_with(&output) || output.starts_with(&input) {
            return Err(OpdfError::Unsupported("Choose separate input and output folders, neither inside the other".into()));
        }
        Ok(Self { input, output, seen: BTreeMap::new(), title_names, recursive: false, pattern: String::new(), archive_inputs: false, sequence: 0, folders: VecDeque::new(), directory: None, current: BTreeMap::new(), visited: 0, scanning: false })
    }

    pub fn include_subfolders(&mut self, recursive: bool) { self.recursive = recursive; }

    pub fn configure_outputs(&mut self, pattern: &str, archive_inputs: bool) -> Result<()> {
        let rest = pattern.replace("{name}", "").replace("{title}", "").replace("{counter}", "");
        if pattern.len() > 256 || rest.contains(['{', '}']) { return Err(OpdfError::Unsupported("Use {name}, {title} and {counter} in the output naming pattern (up to 256 bytes)".into())); }
        self.pattern = pattern.into(); self.archive_inputs = archive_inputs;
        Ok(())
    }

    pub fn scan_in_progress(&self) -> bool { self.scanning }

    /// One poll. Failed inputs are tried again only once their bytes' size/time changes, so a
    /// locked or damaged file does not flood the user with the same error every second.
    pub fn poll(&mut self, action: &SavedAction, before: &mut dyn FnMut() -> bool) -> Result<PollResults> {
        let output = self.output.clone();
        let pattern = if self.pattern.is_empty() { if self.title_names { "{title}" } else { "{name}" } } else { &self.pattern }.to_string();
        let archive_inputs = self.archive_inputs;
        let mut sequence = self.sequence;
        // The watch owns its destination; a saved action's same-folder or overwrite option
        // must never bypass the private staging directory or alter the inbox in place.
        let mut staged_action = action.clone();
        staged_action.output = actions::OutputOptions::default();
        let result = self.poll_with(&mut |path| {
            let original_revision = revision(path)?;
            if !before() { return Err(OpdfError::Cancelled); }
            let temp = tempfile::tempdir()?;
            let processed = actions::run_file(&staged_action, path, temp.path(), &mut |_, _| before())?;
            let title = if pattern.contains("{title}") {
                let doc = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(&processed))?;
                doc.document().info.title.clone().filter(|t| !t.trim().is_empty())
            } else { None };
            let fallback = path.file_stem().unwrap_or_default().to_string_lossy();
            let name = safe_name(&pattern.replace("{name}", &fallback).replace("{title}", title.as_deref().unwrap_or(&fallback)).replace("{counter}", &(sequence + 1).to_string()));
            if !before() { return Err(OpdfError::Cancelled); }
            if revision(path)? != original_revision { return Err(OpdfError::Unsupported("The input changed during processing. It will be retried after it becomes stable.".into())); }
            let bytes = actions::convert_output(std::sync::Arc::new(std::fs::read(&processed)?), None, &action.output.format, before)?;
            if !before() { return Err(OpdfError::Cancelled); }
            if revision(path)? != original_revision { return Err(OpdfError::Unsupported("The input changed during format conversion. It will be retried after it becomes stable.".into())); }
            let published = sheaf_core::paths::atomic_write(&output.join(format!("{name}.{}", action.output.format)), |part| {
                std::fs::write(part, bytes.as_slice())?;
                Ok(())
            })?;
            if archive_inputs {
                let archive = output.join("Processed inputs");
                let archived = (|| {
                    std::fs::create_dir_all(&archive)?;
                    let target = sheaf_core::paths::atomic_write(&archive.join(path.file_name().unwrap_or_default()), |part| { std::fs::copy(path, part)?; Ok(()) })?;
                    if revision(path)? != original_revision {
                        let _ = std::fs::remove_file(target);
                        return Err(OpdfError::Unsupported("The input changed during archiving; the newer input was kept".into()));
                    }
                    if let Err(e) = std::fs::remove_file(path) { let _ = std::fs::remove_file(target); return Err(e.into()); }
                    Ok::<_, OpdfError>(())
                })();
                if let Err(e) = archived {
                    return Err(OpdfError::Unsupported(format!("Output saved to {}, but the input could not be archived: {e}", published.display())));
                }
            }
            sequence += 1;
            Ok(published)
        });
        self.sequence = sequence;
        result
    }

    fn poll_with(&mut self, process: &mut dyn FnMut(&Path) -> Result<PathBuf>) -> Result<PollResults> {
        if !self.scanning {
            self.folders.push_back(self.input.clone());
            self.current.clear(); self.visited = 0; self.scanning = true;
        }
        // Incremental traversal: even a huge flat inbox yields after 512 directory entries.
        // A completed snapshot alone updates stability/handled stamps; partial scans cannot
        // forget already handled inputs or mistake an unfinished upload for a stable file.
        let mut budget: usize = 512;
        while budget > 0 {
            if self.directory.is_none() {
                let Some(folder) = self.folders.pop_front() else { self.scanning = false; break };
                self.visited += 1; budget -= 1;
                if self.visited > 10_000 { return Err(OpdfError::Unsupported("This watch exceeds 10,000 folders. Choose a smaller inbox.".into())); }
                self.directory = match std::fs::read_dir(&folder) {
                    Ok(directory) => Some(directory),
                    // Stirling's inbox can be changed by its producer while a scan is sliced.
                    // Losing a nested folder must not stop the other watched inputs.
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound && folder != self.input => continue,
                    Err(e) => return Err(e.into()),
                };
            }
            let Some(entry) = self.directory.as_mut().and_then(Iterator::next) else { self.directory = None; continue };
            budget = budget.saturating_sub(1);
            let entry = entry?;
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            if kind.is_dir() && self.recursive { self.folders.push_back(entry.path()); }
            if !kind.is_file() { continue; }
            let path = entry.path();
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) { continue; }
            let stamp = match revision(&path) {
                Ok(stamp) => stamp,
                Err(OpdfError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            if stamp.len == 0 { continue; }
            self.current.insert(path, stamp);
            if self.current.len() > 100_000 { return Err(OpdfError::Unsupported("This watch exceeds 100,000 PDFs. Choose a smaller inbox.".into())); }
        }
        if self.scanning { return Ok(vec![]); }
        let current = std::mem::take(&mut self.current);
        self.seen.retain(|p, _| current.contains_key(p));
        let mut results = Vec::new();
        for (path, stamp) in current {
            match self.seen.get(&path) {
                Some(old) if old.stamp == stamp && old.handled => continue,
                Some(old) if old.stamp == stamp => {
                    let latest = match revision(&path) {
                        Ok(stamp) => stamp,
                        Err(OpdfError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => { self.seen.remove(&path); continue; }
                        Err(e) => return Err(e),
                    };
                    if latest != stamp { self.seen.insert(path, Observed { stamp: latest, handled: false }); continue; }
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
        if action.steps.iter().any(|s| s.prompt || s.tool == "open_tool") {
            return Err(OpdfError::Unsupported("Folder actions cannot pause or open interactive tools".into()));
        }
        Ok(Self { version: 1, action, watch })
    }

    pub fn description(&self) -> String { format!("{} → {} · {}{}", self.watch.input.display(), self.watch.output.display(), self.action.name, if self.watch.recursive { " · subfolders included" } else { "" }) }

    pub fn identity(&self) -> String {
        let hash = self.watch.input.to_string_lossy().bytes().fold(0xcbf29ce484222325u64, |n, b| (n ^ u64::from(b)).wrapping_mul(0x100000001b3));
        format!("{hash:016x}")
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        let bytes = serde_json::to_vec(self).map_err(|e| OpdfError::Unsupported(e.to_string()))?;
        crate::store_file::replace(&dir.join(format!("folder-watch-{}.json", self.identity())), &bytes)?;
        Ok(())
    }

    pub fn load(dir: &Path) -> Result<Option<Self>> { Ok(Self::load_all(dir)?.into_iter().next()) }

    pub fn load_all(dir: &Path) -> Result<Vec<Self>> {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]), Err(e) => return Err(e.into()),
        };
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "folder-watch.json" || (name.starts_with("folder-watch-") && name.ends_with(".json")) { paths.push(entry.path()); }
        }
        paths.sort();
        let mut result: Vec<Self> = Vec::new();
        for path in paths {
            let saved: Self = serde_json::from_slice(&std::fs::read(path)?).map_err(|e| OpdfError::Unsupported(format!("Cannot read saved folder watch: {e}")))?;
            if saved.version != 1 { return Err(OpdfError::Unsupported("Unsupported folder watch version".into())); }
            saved.action.validate()?;
            if saved.action.steps.iter().any(|s| s.prompt || s.tool == "open_tool") { return Err(OpdfError::Unsupported("Folder actions cannot pause or open interactive tools".into())); }
            let mut watch = FolderWatch::new(&saved.watch.input, &saved.watch.output, saved.watch.title_names)?;
            watch.configure_outputs(&saved.watch.pattern, saved.watch.archive_inputs)?;
            if watch.input != saved.watch.input || watch.output != saved.watch.output { return Err(OpdfError::Unsupported("The watched folder moved. Configure the watch again.".into())); }
            if result.iter().all(|w| w.identity() != saved.identity()) { result.push(saved); }
        }
        Ok(result)
    }

    pub fn forget(dir: &Path) -> Result<()> {
        // Only remove this feature's settings, retaining action/library stores.
        for saved in Self::load_all(dir)? {
            match std::fs::remove_file(dir.join(format!("folder-watch-{}.json", saved.identity()))) {
                Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e.into()),
            }
        }
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

    #[test]
    fn watches_keep_independent_patterns_archive_inputs_and_scan_incrementally() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("in"); let output = dir.path().join("out");
        std::fs::create_dir(&inbox).unwrap();
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Document title", "Keep").unwrap();
        let original = inbox.join("original.pdf");
        std::fs::write(&original, bytes.as_slice()).unwrap();
        let mut action = actions::suggested().remove(3);
        action.output.same_folder = true; action.output.overwrite = true;
        let mut watch = FolderWatch::new(&inbox, &output, false).unwrap();
        assert!(watch.configure_outputs("{unknown}", false).is_err());
        watch.configure_outputs("{counter}-{name}-{title}", true).unwrap();
        assert!(watch.poll(&action, &mut || true).unwrap().is_empty());
        let results = watch.poll(&action, &mut || true).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.as_ref().unwrap().file_name().unwrap(), "1-original-Document title.pdf");
        assert!(!original.exists());
        assert_eq!(std::fs::read(output.join("Processed inputs/original.pdf")).unwrap(), *bytes);
        let saved = SavedWatch::new(action.clone(), watch).unwrap();
        saved.save(dir.path()).unwrap();
        let second = dir.path().join("second"); std::fs::create_dir(&second).unwrap();
        SavedWatch::new(action, FolderWatch::new(&second, &output, false).unwrap()).unwrap().save(dir.path()).unwrap();
        assert_eq!(SavedWatch::load_all(dir.path()).unwrap().len(), 2);
        for i in 0..600 { std::fs::write(inbox.join(format!("{i}.pdf")), b"partial").unwrap(); }
        let mut incremental = FolderWatch::new(&inbox, &output, false).unwrap();
        assert!(incremental.poll_with(&mut |_| panic!("First snapshot cannot process")).unwrap().is_empty());
        assert!(incremental.scan_in_progress());
        assert!(incremental.seen.is_empty());
        assert!(incremental.poll_with(&mut |_| panic!("Still the first snapshot")).unwrap().is_empty());
        assert_eq!(incremental.seen.len(), 600);
        SavedWatch::forget(dir.path()).unwrap();
        assert!(SavedWatch::load_all(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn a_watch_never_archives_or_publishes_an_input_changed_during_its_run() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("in"); let out = dir.path().join("out"); std::fs::create_dir(&inbox).unwrap();
        let input = inbox.join("input.pdf");
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Original", "Keep").unwrap();
        std::fs::write(&input, bytes.as_slice()).unwrap();
        let mut watch = FolderWatch::new(&inbox, &out, false).unwrap(); watch.configure_outputs("{name}", true).unwrap();
        let action = actions::suggested().remove(3);
        watch.poll(&action, &mut || true).unwrap();
        let mut calls = 0;
        let changed = b"new upload still being written";
        let results = watch.poll(&action, &mut || { calls += 1; if calls == 3 { std::fs::write(&input, changed).unwrap(); } true }).unwrap();
        assert!(results[0].1.is_err());
        assert_eq!(std::fs::read(input).unwrap(), changed);
        assert!(!out.join("input.pdf").exists());
        assert!(!out.join("Processed inputs").exists());
    }

    #[test]
    fn watched_exports_keep_the_selected_format_and_updated_title() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("in"); let out = dir.path().join("out");
        std::fs::create_dir(&inbox).unwrap(); std::fs::create_dir(&out).unwrap();
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Old", "Body").unwrap();
        std::fs::write(inbox.join("source.pdf"), bytes.as_slice()).unwrap();
        let mut action = actions::SavedAction::empty();
        action.steps = vec![actions::ActionStep::from_engine(&sheaf_core::pdfcraft::engine::actions::Step::SetTitle("New title".into()))];
        action.output.format = "txt".into();
        let mut watch = FolderWatch::new(&inbox, &out, true).unwrap();
        assert!(watch.poll(&action, &mut || true).unwrap().is_empty());
        let results = watch.poll(&action, &mut || true).unwrap();
        let path = results[0].1.as_ref().unwrap();
        assert_eq!(path.file_name().unwrap(), "New title.txt");
        assert!(std::fs::read_to_string(path).unwrap().contains("Body"));
    }

    #[test]
    fn watch_detects_same_size_rewrites_with_restored_modification_time() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("in"); let out = dir.path().join("out");
        std::fs::create_dir(&inbox).unwrap(); std::fs::create_dir(&out).unwrap();
        let input = inbox.join("input.pdf"); std::fs::write(&input, b"first revision").unwrap();
        let mtime = std::fs::metadata(&input).unwrap().modified().unwrap();
        let mut watch = FolderWatch::new(&inbox, &out, false).unwrap();
        let mut count = 0;
        let mut process = |_: &Path| { count += 1; Ok(out.join("done.pdf")) };
        assert!(watch.poll_with(&mut process).unwrap().is_empty());
        assert_eq!(watch.poll_with(&mut process).unwrap().len(), 1);
        std::fs::write(&input, b"other revision").unwrap();
        std::fs::File::options().write(true).open(&input).unwrap().set_times(std::fs::FileTimes::new().set_modified(mtime)).unwrap();
        assert!(watch.poll_with(&mut process).unwrap().is_empty());
        assert_eq!(watch.poll_with(&mut process).unwrap().len(), 1);
        assert!(watch.poll_with(&mut process).unwrap().is_empty());
        assert_eq!(count, 2);
        let encoded = serde_json::to_string(&watch).unwrap();
        let mut resumed: FolderWatch = serde_json::from_str(&encoded).unwrap();
        assert!(resumed.poll_with(&mut |_| panic!("handled revision must survive restart")).unwrap().is_empty());
    }

    #[test]
    fn watched_exports_recheck_inputs_after_native_conversion() {
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Original", "Body").unwrap();
        let mut action = actions::suggested().remove(3); action.output.format = "txt".into();
        let watched = |dir: &Path| {
            let (inbox, out) = (dir.join("in"), dir.join("out"));
            std::fs::create_dir(&inbox).unwrap();
            std::fs::write(inbox.join("input.pdf"), bytes.as_slice()).unwrap();
            let mut watch = FolderWatch::new(&inbox, &out, false).unwrap();
            watch.poll(&action, &mut || true).unwrap();
            (watch, inbox.join("input.pdf"), out)
        };
        // A first run counts the checks the job makes; the input then changes at the last, after
        // the conversion to text and before it is published (the step count changes with the
        // action, so it is not written in here).
        let dry = tempfile::tempdir().unwrap();
        let (mut watch, _, _) = watched(dry.path());
        let mut last = 0;
        assert!(watch.poll(&action, &mut || { last += 1; true }).unwrap()[0].1.is_ok());
        let dir = tempfile::tempdir().unwrap();
        let (mut watch, input, out) = watched(dir.path());
        let mut calls = 0;
        let result = watch.poll(&action, &mut || { calls += 1; if calls == last { std::fs::write(&input, b"New input during conversion").unwrap(); } true }).unwrap();
        assert!(result[0].1.is_err()); assert!(!out.join("input.txt").exists());
        assert_eq!(std::fs::read(input).unwrap(), b"New input during conversion");
    }

    #[test]
    fn a_disappearing_upload_does_not_stop_other_files_or_the_next_scan() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("in"); let out = dir.path().join("out");
        std::fs::create_dir(&inbox).unwrap();
        for name in ["a.pdf", "b.pdf", "c.pdf"] { std::fs::write(inbox.join(name), b"ready").unwrap(); }
        let mut watch = FolderWatch::new(&inbox, &out, false).unwrap();
        assert!(watch.poll_with(&mut |_| panic!("wait for stability")).unwrap().is_empty());
        let results = watch.poll_with(&mut |path| {
            if path.ends_with("a.pdf") { std::fs::remove_file(inbox.join("b.pdf")).unwrap(); }
            Ok(out.join(path.file_name().unwrap()))
        }).unwrap();
        assert_eq!(results.iter().map(|(p, _)| p.file_name().unwrap().to_str().unwrap()).collect::<Vec<_>>(), ["a.pdf", "c.pdf"]);
        assert!(results.iter().all(|(_, result)| result.is_ok()));
        assert!(watch.poll_with(&mut |_| panic!("handled inputs stay handled")).unwrap().is_empty());
        std::fs::write(inbox.join("b.pdf"), b"new upload").unwrap();
        assert!(watch.poll_with(&mut |_| panic!("new upload must settle")).unwrap().is_empty());
        assert_eq!(watch.poll_with(&mut |path| Ok(out.join(path.file_name().unwrap()))).unwrap().len(), 1);
    }

    #[test]
    fn a_removed_nested_folder_is_skipped_between_scan_slices() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("in"); let out = dir.path().join("out");
        std::fs::create_dir_all(inbox.join("removed")).unwrap();
        let mut watch = FolderWatch::new(&inbox, &out, false).unwrap(); watch.include_subfolders(true);
        // Resume a scan slice after its producer removed a queued directory.
        watch.scanning = true;
        watch.folders.push_back(inbox.join("removed"));
        std::fs::remove_dir(inbox.join("removed")).unwrap();
        assert!(watch.poll_with(&mut |_| panic!("no files")).unwrap().is_empty());
        assert!(!watch.scan_in_progress());
        std::fs::remove_dir(inbox).unwrap();
        assert!(watch.poll_with(&mut |_| panic!("root folder missing")).is_err());
    }

}
