//! Small JSON settings files under the app's data folder, replaced atomically.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// `~/Library/Application Support/Sheaf` (or the platform equivalent).
pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("Sheaf")
}

/// Writes `bytes` to `target`, replacing it: a temp file in the same folder, fsync, rename.
/// (`sheaf_core::paths::atomic_write` never overwrites, which is right for outputs, not here.)
pub fn replace(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = target.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!("{name}.tmp-{}", std::process::id()));
    let r = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, target)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// Parses `path` as JSON; a missing or unreadable file gives `None`.
pub fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}
