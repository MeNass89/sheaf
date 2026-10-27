//! Locate an optional packaged Office converter (iLovePDF's offline Office conversion).
//!
//! The packager supplies the runtime and its licence files; probing never downloads or installs it.

use std::path::{Path, PathBuf};

/// A runtime beside the executable, before falling back to the machine's installation.
pub fn bundled_soffice(executable: &Path) -> Option<PathBuf> {
    let dir = executable.parent()?;
    bundled_candidates(dir).into_iter().find(|p| p.is_file())
}

fn bundled_candidates(dir: &Path) -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![dir.join("../Frameworks/LibreOffice.app/Contents/MacOS/soffice")]
    } else if cfg!(windows) {
        vec![dir.join("libreoffice/program/soffice.com")]
    } else {
        vec![dir.join("libreoffice/program/soffice"), dir.join("../lib/sheaf/libreoffice/program/soffice")]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packaged_office_runtime_is_found_without_a_system_installation() {
        let t = tempfile::tempdir().unwrap();
        let bin = t.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let exe = bin.join("sheaf");
        assert_eq!(bundled_soffice(&exe), None);
        let runtime = bundled_candidates(&bin).remove(0);
        std::fs::create_dir_all(runtime.parent().unwrap()).unwrap();
        // No child is run: detection must work even when PATH has no system LibreOffice.
        std::fs::write(&runtime, b"packaged runtime").unwrap();
        assert_eq!(bundled_soffice(&exe), Some(runtime.clone()));
        std::fs::remove_file(&runtime).unwrap();
        assert_eq!(bundled_soffice(&exe), None);
    }
}
