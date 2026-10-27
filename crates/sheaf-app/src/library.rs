//! Stirling's local file library: private copies with explicit, immutable versions. Restoring
//! an older version creates a new version, so recovering one never erases subsequent work.

use std::path::{Path, PathBuf};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize};
use sheaf_core::{OpdfError, Result};
use crate::store_file;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Version { pub number: u32, pub created: u64, pub size: u64, pub label: String }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub id: String, pub name: String, pub versions: Vec<Version>,
    #[serde(default)]
    pub folder: String,
}

#[derive(Clone)]
pub struct Library { root: PathBuf }

pub struct UnzipReport { pub imported: Vec<Entry>, pub errors: Vec<String> }

fn invalid(message: impl Into<String>) -> OpdfError { OpdfError::Unsupported(message.into()) }
fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }

/// Filter an already-loaded catalogue. Typing never reopens every entry.json.
pub fn filter_entries(entries: &[Entry], query: &str, folder: &str) -> Vec<Entry> {
    let query = query.to_lowercase();
    entries.iter().filter(|e| (folder.is_empty() || e.folder == folder) &&
        (query.is_empty() || format!("{} {}", e.name, e.folder).to_lowercase().contains(&query))).cloned().collect()
}

/// Stable library sorting uses catalogue values only; no filesystem reads per comparison.
pub fn sort_entries(entries: &mut [Entry], order: i32) {
    entries.sort_by(|a, b| {
        let newest = |e: &Entry| e.versions.last().map_or(0, |v| v.created);
        let size = |e: &Entry| e.versions.last().map_or(0, |v| v.size);
        let names = || a.name.to_lowercase().cmp(&b.name.to_lowercase());
        match order {
            1 => names().reverse(), 2 => newest(b).cmp(&newest(a)),
            3 => newest(a).cmp(&newest(b)), 4 => size(b).cmp(&size(a)),
            5 => size(a).cmp(&size(b)), _ => names(),
        }.then_with(names).then_with(|| a.id.cmp(&b.id))
    });
}

/// Decode raster thumbnails with explicit allocation limits; Office files keep their icon.
pub fn raster_thumbnail(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(12000); limits.max_image_height = Some(12000);
    limits.max_alloc = Some(64 * 1024 * 1024); reader.limits(limits);
    let image = reader.decode().map_err(|e| invalid(format!("Thumbnail unavailable: {e}")))?.thumbnail(120, 120).into_rgba8();
    Ok((image.width(), image.height(), image.into_raw()))
}

/// Include intermediate ZIP folders so a nested import has a navigable tree.
pub fn folders(entries: &[Entry]) -> Vec<String> {
    let mut paths = std::collections::BTreeSet::new();
    for entry in entries {
        let mut path = String::new();
        for part in entry.folder.split('/').filter(|p| !p.is_empty()) {
            if !path.is_empty() { path.push('/'); }
            path.push_str(part); paths.insert(path.clone());
        }
    }
    paths.into_iter().collect()
}

pub struct BatchReport { pub paths: Vec<PathBuf>, pub completed: usize, pub errors: Vec<String> }

impl Library {
    pub fn new(data: &Path) -> Self { Self { root: data.join("library") } }

    fn directory(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) { return Err(invalid("Invalid library file ID")); }
        Ok(self.root.join(id))
    }

    pub fn list(&self) -> Result<Vec<Entry>> {
        let dir = match std::fs::read_dir(&self.root) {
            Ok(dir) => dir,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.into()),
        };
        let mut entries = Vec::new();
        for item in dir {
            let item = item?;
            if !item.file_type()?.is_dir() { continue; }
            let id = item.file_name().to_string_lossy().into_owned();
            entries.push(self.entry(&id)?);
        }
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        Ok(entries)
    }

    /// Move catalogue membership only: immutable file versions stay byte-for-byte unchanged.
    pub fn move_to_folder(&self, id: &str, folder: &str) -> Result<Entry> {
        if folder.len() > 1024 || (!folder.is_empty() && folder.split('/').any(|p| p.is_empty() || p == "." || p == ".." || crate::watch_folder::safe_name(p) != p)) { return Err(invalid("Use folder names separated by / without path characters or ..")); }
        let mut entry = self.entry(id)?; entry.folder = folder.into(); self.save(&entry)?; Ok(entry)
    }

    pub fn entry(&self, id: &str) -> Result<Entry> {
        let bytes = std::fs::read(self.directory(id)?.join("entry.json"))?;
        let entry: Entry = serde_json::from_slice(&bytes).map_err(|e| invalid(format!("Unreadable library entry: {e}")))?;
        if entry.id != id || entry.name != crate::watch_folder::safe_name(&entry.name) || entry.versions.is_empty() {
            return Err(invalid("Invalid library entry"));
        }
        Ok(entry)
    }

    fn save(&self, entry: &Entry) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(entry).map_err(|e| invalid(e.to_string()))?;
        store_file::replace(&self.directory(&entry.id)?.join("entry.json"), &bytes)?;
        Ok(())
    }

    pub fn add(&self, source: &Path) -> Result<Entry> {
        let name = source.file_name().ok_or_else(|| invalid("A library file needs a name"))?.to_string_lossy();
        let name = crate::watch_folder::safe_name(&name);
        std::fs::create_dir_all(&self.root)?;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = loop {
            let id = format!("{}{:06}", now(), NEXT.fetch_add(1, Ordering::Relaxed));
            match std::fs::create_dir(self.root.join(&id)) {
                Ok(()) => break id,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        };
        let mut entry = Entry { id: id.clone(), name, versions: vec![], folder: String::new() };
        let result = self.append(&mut entry, source, "Imported");
        if result.is_err() { let _ = std::fs::remove_dir_all(self.directory(&id)?); }
        result.map(|()| entry)
    }

    fn append(&self, entry: &mut Entry, source: &Path, label: &str) -> Result<()> {
        let number = entry.versions.last().map_or(1, |v| v.number.saturating_add(1));
        let folder = self.directory(&entry.id)?.join(number.to_string());
        std::fs::create_dir(&folder)?;
        let result = (|| {
            let size = std::fs::copy(source, folder.join(&entry.name))?;
            entry.versions.push(Version { number, created: now(), size, label: label.into() });
            self.save(entry)
        })();
        if result.is_err() { let _ = std::fs::remove_dir_all(folder); }
        result
    }

    /// Path of a stored version; zero means the current (last) version.
    pub fn path(&self, id: &str, number: u32) -> Result<PathBuf> {
        let entry = self.entry(id)?;
        let version = if number == 0 { entry.versions.last() } else { entry.versions.iter().find(|v| v.number == number) };
        let version = version.ok_or_else(|| invalid("No such file version"))?;
        Ok(self.directory(id)?.join(version.number.to_string()).join(entry.name))
    }

    pub fn update(&self, id: &str, source: &Path) -> Result<()> {
        let mut entry = self.entry(id)?;
        self.append(&mut entry, source, "New version")
    }

    pub fn restore(&self, id: &str, number: u32) -> Result<()> {
        let mut entry = self.entry(id)?;
        let source = self.path(id, number)?;
        self.append(&mut entry, &source, &format!("Restored version {number}"))
    }

    pub fn export(&self, id: &str, number: u32, out: &Path) -> Result<PathBuf> {
        let source = self.path(id, number)?;
        sheaf_core::paths::atomic_write(out, |part| { std::fs::copy(source, part)?; Ok(()) })
    }

    /// Open an editable working copy. The reader's Save records it as a new library version.
    pub fn checkout(&self, id: &str) -> Result<PathBuf> {
        let entry = self.entry(id)?;
        let root = self.root.parent().unwrap_or(Path::new(".")).join("library-work").join(id);
        std::fs::create_dir_all(&root)?;
        let folder = tempfile::Builder::new().prefix("edit-").tempdir_in(root)?.keep();
        self.export(id, 0, &folder.join(entry.name))
    }

    /// Return false for normal documents. Library work files create an immutable checkpoint
    /// after a successful reader Save, before that save is acknowledged to the user.
    pub fn record_saved(&self, path: &Path) -> Result<bool> {
        let root = self.root.parent().unwrap_or(Path::new(".")).join("library-work");
        let Ok(relative) = path.strip_prefix(root) else { return Ok(false) };
        let parts: Vec<_> = relative.components().collect();
        if parts.len() != 3 { return Ok(false); }
        let id = parts[0].as_os_str().to_string_lossy();
        // A working copy remains an ordinary editable file after its library item was deleted.
        if !self.directory(&id)?.exists() { return Ok(false); }
        let mut entry = self.entry(&id)?;
        if path.file_name().is_none_or(|name| name != std::ffi::OsStr::new(&entry.name)) { return Err(invalid("Library working copy name changed")); }
        self.append(&mut entry, path, "Saved in Sheaf")?;
        Ok(true)
    }

    /// Working files from interrupted sessions remain discoverable; normal tab closure removes
    /// only its own checkout, never a stored version or the imported original.
    pub fn working_copies(&self) -> Result<Vec<PathBuf>> {
        let root = self.root.parent().unwrap_or(Path::new(".")).join("library-work");
        let entries = match std::fs::read_dir(&root) {
            Ok(entries) => entries, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]), Err(e) => return Err(e.into()),
        };
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() { continue; }
            for checkout in std::fs::read_dir(entry.path())? {
                let checkout = checkout?;
                if !checkout.file_type()?.is_dir() { continue; }
                for file in std::fs::read_dir(checkout.path())? {
                    let file = file?;
                    if file.file_type()?.is_file() { paths.push(file.path()); }
                }
            }
        }
        paths.sort();
        Ok(paths)
    }

    pub fn release_checkout(&self, path: &Path) -> Result<bool> {
        let root = self.root.parent().unwrap_or(Path::new(".")).join("library-work");
        let Ok(relative) = path.strip_prefix(&root) else { return Ok(false) };
        let parts: Vec<_> = relative.components().collect();
        if parts.len() != 3 || !parts.iter().all(|p| matches!(p, std::path::Component::Normal(_))) { return Ok(false); }
        if !parts[0].as_os_str().to_string_lossy().bytes().all(|b| b.is_ascii_digit()) { return Ok(false); }
        let folder = path.parent().ok_or_else(|| invalid("Invalid checkout path"))?;
        if root.join(parts[0].as_os_str()).symlink_metadata()?.file_type().is_symlink() || folder.symlink_metadata()?.file_type().is_symlink() || folder.canonicalize()?.parent() != Some(root.join(parts[0].as_os_str()).canonicalize()?.as_path()) {
            return Err(invalid("The checkout folder moved"));
        }
        std::fs::remove_dir_all(folder)?;
        Ok(true)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.entry(id)?;
        std::fs::remove_dir_all(self.directory(id)?)?;
        Ok(())
    }

    pub fn export_many(&self, ids: &[String], folder: &Path) -> BatchReport {
        let mut report = BatchReport { paths: vec![], completed: 0, errors: vec![] };
        for id in ids.iter().collect::<std::collections::BTreeSet<_>>() {
            let result = self.entry(id).and_then(|e| self.export(id, 0, &folder.join(e.name)));
            match result { Ok(path) => { report.paths.push(path); report.completed += 1; }, Err(e) => report.errors.push(format!("{id}: {e}")) }
        }
        report
    }

    pub fn delete_many(&self, ids: &[String]) -> BatchReport {
        let mut report = BatchReport { paths: vec![], completed: 0, errors: vec![] };
        for id in ids.iter().collect::<std::collections::BTreeSet<_>>() {
            match self.delete(id) { Ok(()) => report.completed += 1, Err(e) => report.errors.push(format!("{id}: {e}")) }
        }
        report
    }

    /// Unzip into separate library items, keeping the archive folder path as metadata. A preflight refuses paths
    /// escaping the archive and symlinks before adding anything. Expansion is bounded on disk.
    pub fn unzip(&self, id: &str) -> Result<UnzipReport> {
        let file = std::fs::File::open(self.path(id, 0)?)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| invalid(e.to_string()))?;
        let count = archive.len();
        if count > 10_000 { return Err(invalid("The ZIP contains more than 10000 files")); }
        let mut total = 0u64;
        for i in 0..count {
            let item = archive.by_index(i).map_err(|e| invalid(e.to_string()))?;
            if item.enclosed_name().is_none() || item.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                return Err(invalid("The ZIP contains a path outside its folder or a symbolic link"));
            }
            total = total.saturating_add(item.size());
            if item.size() > 256 * 1024 * 1024 || total > 512 * 1024 * 1024 {
                return Err(invalid("The ZIP exceeds the local library's extraction limit (256 MB per file, 512 MB total, 10000 files)"));
            }
        }
        let mut report = UnzipReport { imported: vec![], errors: vec![] };
        for i in 0..archive.len() {
            let item = archive.by_index(i).map_err(|e| invalid(e.to_string()))?;
            if item.is_dir() { continue; }
            let name = item.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).ok_or_else(|| invalid("Invalid ZIP file name"))?;
            let folder = item.enclosed_name().and_then(|p| p.parent().map(|p| p.to_string_lossy().replace('\\', "/"))).unwrap_or_default();
            let result: Result<Entry> = (|| {
                let temp = tempfile::tempdir()?;
                let path = temp.path().join(crate::watch_folder::safe_name(&name));
                let mut out = std::fs::File::create(&path)?;
                let limit = item.size().min(256 * 1024 * 1024);
                let mut limited = item.take(limit + 1);
                let copied = std::io::copy(&mut limited, &mut out)?;
                out.flush()?;
                if copied > limit { return Err(invalid("ZIP file exceeded its declared size")); }
                let mut entry = self.add(&path)?;
                entry.folder = folder;
                self.save(&entry)?;
                Ok(entry)
            })();
            match result { Ok(entry) => report.imported.push(entry), Err(e) => report.errors.push(format!("{name}: {e}")) }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_versions_keep_private_copies_and_restore_without_losing_history() {
        let t = tempfile::tempdir().unwrap();
        let source = t.path().join("notes.txt");
        std::fs::write(&source, b"one").unwrap();
        let library = Library::new(t.path());
        let item = library.add(&source).unwrap();
        std::fs::write(&source, b"two").unwrap();
        assert_eq!(std::fs::read(library.path(&item.id, 0).unwrap()).unwrap(), b"one");
        library.update(&item.id, &source).unwrap();
        library.restore(&item.id, 1).unwrap();
        assert_eq!(std::fs::read(library.path(&item.id, 0).unwrap()).unwrap(), b"one");
        assert_eq!(std::fs::read(library.path(&item.id, 2).unwrap()).unwrap(), b"two");
        assert_eq!(library.entry(&item.id).unwrap().versions.len(), 3);
        let working = library.checkout(&item.id).unwrap();
        std::fs::write(&working, b"saved in reader").unwrap();
        assert!(library.record_saved(&working).unwrap());
        assert_eq!(std::fs::read(library.path(&item.id, 0).unwrap()).unwrap(), b"saved in reader");
        assert_eq!(std::fs::read(library.path(&item.id, 1).unwrap()).unwrap(), b"one");
        assert!(!library.record_saved(&source).unwrap());
        let exported = library.export(&item.id, 2, &source).unwrap();
        assert_ne!(exported, source);
        assert_eq!(std::fs::read(&source).unwrap(), b"two");
        library.delete(&item.id).unwrap();
        assert!(!library.record_saved(&working).unwrap());
        assert!(library.list().unwrap().is_empty());
        assert_eq!(std::fs::read(source).unwrap(), b"two");
        assert!(library.path("../escape", 0).is_err());
    }

    fn zip_file(path: &Path, names: &[&str]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for name in names {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(b"original archive item").unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn unzip_imports_nested_files_and_rejects_escaping_paths_before_any_import() {
        let t = tempfile::tempdir().unwrap();
        let library = Library::new(t.path());
        let source = t.path().join("bundle.zip");
        zip_file(&source, &["folder/a.txt", "b.txt"]);
        let archive = library.add(&source).unwrap();
        let result = library.unzip(&archive.id).unwrap();
        assert_eq!(result.imported.len(), 2);
        assert!(result.errors.is_empty());
        assert!(result.imported.iter().any(|e| e.folder == "folder"));
        for entry in result.imported {
            assert_eq!(std::fs::read(library.path(&entry.id, 0).unwrap()).unwrap(), b"original archive item");
        }
        zip_file(&source, &["ok.txt", "../escape.txt"]);
        library.update(&archive.id, &source).unwrap();
        let before = library.list().unwrap().len();
        assert!(library.unzip(&archive.id).is_err());
        assert_eq!(library.list().unwrap().len(), before);
        assert!(!t.path().join("escape.txt").exists());
    }
    #[test]
    fn checkout_recovery_and_release_keep_versions_and_other_checkouts() {
        let t = tempfile::tempdir().unwrap();
        let original = t.path().join("notes.txt");
        std::fs::write(&original, b"original").unwrap();
        let library = Library::new(t.path());
        let item = library.add(&original).unwrap();
        let first = library.checkout(&item.id).unwrap();
        let second = library.checkout(&item.id).unwrap();
        assert_eq!(library.working_copies().unwrap().len(), 2);
        std::fs::write(&first, b"saved edit").unwrap();
        assert!(library.record_saved(&first).unwrap());
        assert_eq!(library.entry(&item.id).unwrap().versions.len(), 2);
        assert!(library.release_checkout(&first).unwrap());
        assert!(!first.exists());
        assert_eq!(library.working_copies().unwrap(), [second]);
        assert!(!library.release_checkout(&original).unwrap());
        assert_eq!(std::fs::read(library.path(&item.id, 0).unwrap()).unwrap(), b"saved edit");
        assert_eq!(std::fs::read(original).unwrap(), b"original");
    }

    #[test]
    fn cached_library_filter_tree_and_batch_actions_keep_versions_and_collisions() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::new(dir.path());
        let source = dir.path().join("same.txt"); std::fs::write(&source, b"private original").unwrap();
        let mut first = library.add(&source).unwrap(); first.folder = "Reports/2026".into(); library.save(&first).unwrap();
        let second = library.add(&source).unwrap();
        let entries = library.list().unwrap();
        assert_eq!(folders(&entries), ["Reports", "Reports/2026"]);
        // Catalogue filtering remains valid without another metadata read.
        std::fs::rename(library.directory(&first.id).unwrap().join("entry.json"), dir.path().join("offline.json")).unwrap();
        assert_eq!(filter_entries(&entries, "SAME", "Reports/2026"), [first.clone()]);
        std::fs::rename(dir.path().join("offline.json"), library.directory(&first.id).unwrap().join("entry.json")).unwrap();
        let out = dir.path().join("exports"); std::fs::create_dir(&out).unwrap();
        let ids = vec![first.id.clone(), second.id.clone(), first.id.clone()];
        let report = library.export_many(&ids, &out);
        assert_eq!(report.completed, 2); assert!(report.errors.is_empty());
        assert_ne!(report.paths[0], report.paths[1]);
        let report = library.delete_many(&ids);
        assert_eq!(report.completed, 2); assert!(report.errors.is_empty());
        assert!(library.list().unwrap().is_empty());
        assert_eq!(std::fs::read(source).unwrap(), b"private original");
    }

    #[test]
    fn library_sort_move_and_raster_thumbnail_keep_file_versions() {
        let dir = tempfile::tempdir().unwrap(); let library = Library::new(dir.path());
        let source = dir.path().join("small.png");
        image::RgbaImage::from_pixel(400, 200, image::Rgba([255, 0, 0, 255])).save(&source).unwrap();
        let first = library.add(&source).unwrap();
        let moved = library.move_to_folder(&first.id, "Reports/2026").unwrap();
        assert_eq!(moved.versions, first.versions); assert_eq!(moved.folder, "Reports/2026");
        assert_eq!(std::fs::read(library.path(&first.id, 0).unwrap()).unwrap(), std::fs::read(&source).unwrap());
        assert!(library.move_to_folder(&first.id, "../escape").is_err());
        let (w, h, rgba) = raster_thumbnail(&source).unwrap(); assert_eq!((w, h), (120, 60)); assert_eq!(&rgba[..4], &[255, 0, 0, 255]);
        let mut a = first.clone(); a.name = "Zeta".into(); a.versions.last_mut().unwrap().size = 1;
        let mut b = first; b.name = "alpha".into(); b.versions.last_mut().unwrap().size = 2;
        let mut entries = vec![a, b]; sort_entries(&mut entries, 0); assert_eq!(entries[0].name, "alpha");
        sort_entries(&mut entries, 1); assert_eq!(entries[0].name, "Zeta");
        sort_entries(&mut entries, 4); assert_eq!(entries[0].name, "alpha");
        entries[0].versions.last_mut().unwrap().created = 100;
        entries[1].versions.last_mut().unwrap().created = 1;
        sort_entries(&mut entries, 3); assert_eq!(entries[0].name, "Zeta");
    }

}
