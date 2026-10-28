//! Local scanner acquisition: Acrobat's Scan to PDF and Preview's Insert from Scanner.
//! SANE is also available on macOS; Finder's PATH often omits its installation directory.

use crate::{OpdfError, Result};
use std::path::Path;
use std::process::Command;

/// Scan one page to PNG using local scanner software. The caller owns the temporary path.
/// No destination is accepted until the scanner's output decodes as a nonempty image.
pub fn scan_to_png(to: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let path = to.to_string_lossy().replace('\'', "''");
        let script = format!(
            "$d = New-Object -ComObject WIA.CommonDialog; $i = $d.ShowAcquireImage(1, 1, 0, '{{B96B3CAF-0728-11D3-9D7B-0000F81EF32E}}'); \
             if ($i) {{ $i.SaveFile('{path}'); exit 0 }}; exit 1"
        );
        acquire(Command::new("powershell").args(["-NoProfile", "-STA", "-Command", &script]), to)
    }
    #[cfg(not(windows))]
    {
        let scanner = sane_program();
        sane_to_png(&scanner, to)
    }
}

#[cfg(not(windows))]
fn sane_program() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    let installed = [Path::new("/opt/homebrew/bin/scanimage"), Path::new("/usr/local/bin/scanimage"), Path::new("/opt/local/bin/scanimage")];
    #[cfg(not(target_os = "macos"))]
    let installed = [];
    sane_program_from(std::env::var_os("PATH"), &installed)
}

#[cfg(not(windows))]
fn sane_program_from(path: Option<std::ffi::OsString>, installed: &[&Path]) -> std::path::PathBuf {
    if let Some(found) = path.and_then(|p| std::env::split_paths(&p).map(|p| p.join("scanimage")).find(|p| p.is_file())) {
        return found;
    }
    // Finder's PATH can omit package managers. A local installation still works without a shell.
    installed.iter().find(|p| p.is_file()).map_or_else(|| "scanimage".into(), |p| p.to_path_buf())
}

#[cfg(not(windows))]
fn sane_to_png(scanner: &Path, to: &Path) -> Result<()> {
    acquire(Command::new(scanner).args(["--format=png", "--resolution", "300", "-o"]).arg(to), to)
}

fn acquire(command: &mut Command, to: &Path) -> Result<()> {
    // WIA refuses to overwrite, and a successful driver with no output must not reuse old pixels.
    if to.exists() { std::fs::remove_file(to)?; }
    let result = (|| {
        let out = command.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                #[cfg(windows)]
                let help = "Windows scanner software (WIA) is unavailable";
                #[cfg(target_os = "macos")]
                let help = "No scanner software: install SANE (sane-backends), or scan with Image Capture and open the saved file";
                #[cfg(not(any(windows, target_os = "macos")))]
                let help = "No scanner software: install SANE (sane-utils)";
                OpdfError::Unsupported(help.into())
            } else {
                e.into()
            }
        })?;
        if !out.status.success() {
            let message = String::from_utf8_lossy(&out.stderr).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("No page was scanned").trim().to_string();
            return Err(OpdfError::Conversion(message));
        }
        let image = image::open(to).map_err(|_| OpdfError::Conversion("The scanner did not return a readable page".into()))?;
        if image.width() == 0 || image.height() == 0 {
            return Err(OpdfError::Conversion("The scanner returned an empty page".into()));
        }
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(to); }
    result
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn driver(dir: &Path, script: &str) -> std::path::PathBuf {
        let path = dir.join("scanimage");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[test]
    fn sane_scans_at_300_dpi_to_a_path_with_spaces_and_rejects_driver_failures() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("source.png");
        image::RgbImage::from_pixel(12, 24, image::Rgb([23, 45, 67])).save(&image).unwrap();
        let scanner = driver(dir.path(), "[ \"$1 $2 $3 $4\" = '--format=png --resolution 300 -o' ] || exit 2\ncp \"$(dirname \"$0\")/source.png\" \"$5\"");
        let out = dir.path().join("scanned page.png");
        sane_to_png(&scanner, &out).unwrap();
        assert_eq!(image::open(&out).unwrap().to_rgb8().get_pixel(0, 0).0, [23, 45, 67]);
        let scanner = driver(dir.path(), "echo 'Scanner cover is open' >&2\nprintf partial > \"$5\"\nexit 1");
        let error = sane_to_png(&scanner, &out).unwrap_err();
        assert!(error.to_string().contains("Scanner cover is open"));
        assert!(!out.exists(), "a partial or stale scan is never inserted");
    }

    #[test]
    fn success_without_a_page_or_with_invalid_pixels_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("stale.png");
        image::RgbImage::new(1, 1).save(&out).unwrap();
        let scanner = driver(dir.path(), "exit 0");
        assert!(sane_to_png(&scanner, &out).is_err());
        assert!(!out.exists());
        let scanner = driver(dir.path(), "printf 'not an image' > \"$5\"");
        assert!(sane_to_png(&scanner, &out).is_err());
        assert!(!out.exists());
        let error = sane_to_png(&dir.path().join("missing-driver"), &out).unwrap_err();
        assert!(error.to_string().contains("install SANE"));
        assert!(!out.exists());
    }
    #[test]
    fn finder_without_a_shell_path_finds_an_installed_scanner() {
        let dir = tempfile::tempdir().unwrap();
        let installed = driver(dir.path(), "exit 0");
        assert_eq!(sane_program_from(None, &[&installed]), installed);
        let missing = dir.path().join("missing");
        assert_eq!(sane_program_from(Some(missing.into_os_string()), &[&installed]), installed);
        assert_eq!(sane_program_from(None, &[]), Path::new("scanimage"));
    }

}
