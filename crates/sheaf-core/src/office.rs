//! Office / HTML → PDF through LibreOffice, run headless as a child process.
//!
//! LibreOffice is an external dependency: detected at runtime, never bundled.
//! Every conversion gets its own throwaway profile and output directory, so
//! parallel conversions never collide with each other or with a LibreOffice
//! the user already has open.

use crate::paths::atomic_write;
use crate::{OpdfError, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(100);

/// Locates `soffice`: `$SHEAF_SOFFICE`, then the standard install location (macOS, Windows),
/// then `PATH`.
pub fn find_soffice() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SHEAF_SOFFICE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    #[cfg(target_os = "macos")]
    {
        let p = Path::new("/Applications/LibreOffice.app/Contents/MacOS/soffice");
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    #[cfg(windows)]
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(d) = std::env::var_os(var) {
            // soffice.com is the console entry point: it waits for the conversion to finish.
            let p = Path::new(&d).join("LibreOffice/program/soffice.com");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let name = if cfg!(windows) { "soffice.com" } else { "soffice" };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Converts `input` to PDF with the default 120 s timeout. Returns the path
/// actually written (`out`, or `out (1).pdf`… if `out` already exists).
pub fn office_to_pdf(input: &Path, out: &Path) -> Result<PathBuf> {
    office_to_pdf_with(None, input, out, DEFAULT_TIMEOUT)
}

/// Converts `input` to PDF with an explicit `soffice` (or the detected one when
/// `None`) and timeout. A missing `soffice` yields [`OpdfError::OfficeMissing`]
/// before anything is spawned.
pub fn office_to_pdf_with(soffice: Option<&Path>, input: &Path, out: &Path, timeout: Duration) -> Result<PathBuf> {
    // Markdown and e-mail as a web page set here, PostScript through Ghostscript (`textdocs`).
    if crate::textdocs::kind(input).is_some() {
        return crate::textdocs::to_pdf(input, out, |html, out| office_to_pdf_with(soffice, html, out, timeout));
    }
    let soffice = match soffice {
        Some(p) if p.is_file() => p.to_path_buf(),
        Some(_) => return Err(OpdfError::OfficeMissing),
        None => find_soffice().ok_or(OpdfError::OfficeMissing)?,
    };
    // Windows: canonicalize gives a `\\?\` path LibreOffice does not read.
    let input = if cfg!(windows) { std::path::absolute(input)? } else { std::fs::canonicalize(input)? };

    let tmp = tempfile::tempdir()?;
    let outdir = tmp.path().join("out");
    std::fs::create_dir(&outdir)?;
    let profile = file_url(&tmp.path().join("profile"));

    let mut child = Command::new(&soffice)
        .arg(format!("-env:UserInstallation={profile}"))
        .args(["--headless", "--norestore", "--nologo", "--convert-to", "pdf", "--outdir"])
        .arg(&outdir)
        .arg(&input)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    // Drain stderr on a thread so a chatty soffice can never block on a full pipe.
    let mut pipe = child.stderr.take().expect("stderr is piped");
    let drain = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    });

    let start = Instant::now();
    let status = loop {
        let polled = match child.try_wait() {
            Ok(s) => s,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
        };
        if let Some(st) = polled {
            break st;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            // The drain thread is left detached: a grandchild may still hold the pipe.
            return Err(OpdfError::Conversion(format!("timed out after {}s", timeout.as_secs())));
        }
        std::thread::sleep(POLL);
    };
    let stderr = drain.join().unwrap_or_default();

    let pdf = produced_pdf(&outdir, &input);
    match pdf {
        Some(pdf) if status.success() => atomic_write(out, |part| {
            std::fs::copy(&pdf, part)?;
            Ok(())
        }),
        _ => {
            let text = String::from_utf8_lossy(&stderr);
            let last = text.lines().map(str::trim).rfind(|l| !l.is_empty());
            let msg = match (status.success(), last) {
                (true, Some(l)) => format!("LibreOffice produced no PDF (exit 0): {l}"),
                (true, None) => "LibreOffice produced no PDF (exit 0)".to_string(),
                (false, Some(l)) => l.to_string(),
                (false, None) => format!("LibreOffice produced no PDF ({status})"),
            };
            Err(OpdfError::Conversion(msg))
        }
    }
}

/// `<outdir>/<input stem>.pdf`, or — if soffice rewrote the name — the single
/// `.pdf` in `outdir`.
fn produced_pdf(outdir: &Path, input: &Path) -> Option<PathBuf> {
    if let Some(stem) = input.file_stem() {
        let mut name = stem.to_os_string();
        name.push(".pdf");
        let p = outdir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    let pdfs: Vec<PathBuf> = std::fs::read_dir(outdir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf")) && p.is_file())
        .collect();
    match <[PathBuf; 1]>::try_from(pdfs) {
        Ok([p]) => Some(p),
        Err(_) => None,
    }
}

/// `file://` URL for an absolute path, percent-encoding every byte outside
/// RFC 3986 unreserved characters and `/` (Windows: `file:///C:/…`).
fn file_url(path: &Path) -> String {
    #[cfg(unix)]
    let bytes = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()).to_vec();
    #[cfg(not(unix))]
    let bytes = format!("/{}", path.to_string_lossy().replace('\\', "/")).into_bytes();
    let mut url = String::from("file://");
    for &b in &bytes {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) || (cfg!(windows) && b == b':') {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn file_url_encodes_spaces_and_non_ascii() {
        assert_eq!(file_url(Path::new("/tmp/a b/é")), "file:///tmp/a%20b/%C3%A9");
        assert_eq!(file_url(Path::new("/var/folders/x_y/T/.tmp1-2~")), "file:///var/folders/x_y/T/.tmp1-2~");
    }
}
