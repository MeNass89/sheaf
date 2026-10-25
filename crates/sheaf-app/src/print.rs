//! Printing through the system's print service (CUPS `lp`, on macOS and Linux).
//!
//! The reader writes the document as it is now (unsaved edits included) to a temporary file and
//! hands it to `lp` with the options chosen in the print sheet; `lp` copies it to the spool, so
//! the temporary file is removed as soon as `lp` returns.

use std::path::Path;
use std::process::Command;

/// What the print sheet asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub printer: String,
    pub copies: u32,
    /// Normalised page list for `lp -P` ("1-3,5"); `None` prints every page.
    pub pages: Option<String>,
    /// Shrink or grow each page to the paper (PDF Expert's "Fit"); else print at actual size.
    pub fit: bool,
    pub two_sided: bool,
    /// Paper: `None` the printer's own, else a CUPS media name ("A4", "Letter", "Legal").
    pub media: Option<String>,
    /// 0 as each page is (CUPS turns landscape pages), 1 portrait, 2 landscape.
    pub orientation: u8,
}

/// The paper sizes the print sheet offers after the printer's own (index 1, 2, 3).
pub const PAPERS: [&str; 3] = ["A4", "Letter", "Legal"];

/// The printers the system knows, and the default one when set.
pub fn printers() -> (Vec<String>, Option<String>) {
    let out = |args: &[&str]| Command::new("lpstat").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    let list = out(&["-e"]).map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()).unwrap_or_default();
    // "system default destination: NAME"
    let default = out(&["-d"]).and_then(|s| s.split_once(':').map(|(_, n)| n.trim().to_string())).filter(|n| !n.is_empty());
    (list, default)
}

/// A typed page range ("1-3, 5, 8-") checked against `count` pages and normalised for `lp`
/// ("1-3,5,8-12"). Pages are 1-based; an open end runs to the last page.
pub fn page_range(text: &str, count: usize) -> Result<String, String> {
    let bad = || format!("Pages: type numbers from 1 to {count}, like 1-3, 5");
    let mut parts = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let num = |s: &str, empty: usize| -> Result<usize, String> {
            let s = s.trim();
            if s.is_empty() {
                return Ok(empty);
            }
            s.parse::<usize>().ok().filter(|n| (1..=count).contains(n)).ok_or_else(bad)
        };
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (num(a, 1)?, num(b, count)?),
            None => {
                let n = num(part, 0)?;
                (n, n)
            }
        };
        if a == 0 || a > b {
            return Err(bad());
        }
        parts.push(if a == b { a.to_string() } else { format!("{a}-{b}") });
    }
    if parts.is_empty() {
        return Err(bad());
    }
    Ok(parts.join(","))
}

/// The `lp` arguments for `job` printing `file`.
pub fn lp_args(job: &Job, file: &Path) -> Vec<String> {
    let mut a = vec!["-d".into(), job.printer.clone(), "-n".into(), job.copies.max(1).to_string()];
    if let Some(p) = &job.pages {
        a.extend(["-P".into(), p.clone()]);
    }
    a.extend(["-o".into(), if job.fit { "fit-to-page".into() } else { "print-scaling=none".into() }]);
    a.extend(["-o".into(), if job.two_sided { "sides=two-sided-long-edge".into() } else { "sides=one-sided".into() }]);
    if let Some(m) = &job.media {
        a.extend(["-o".into(), format!("media={m}")]);
    }
    // IPP orientation-requested: 3 portrait, 4 landscape.
    if job.orientation > 0 {
        a.extend(["-o".into(), format!("orientation-requested={}", if job.orientation == 2 { 4 } else { 3 })]);
    }
    a.extend(["-t".into(), file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()]);
    a.push("--".into());
    a.push(file.to_string_lossy().into_owned());
    a
}

/// Prints `bytes` (a PDF) named `title`; returns `lp`'s request id ("PDF-12").
pub fn submit(job: &Job, title: &str, bytes: &[u8]) -> Result<String, String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let file = dir.path().join(format!("{}.pdf", title.replace(['/', '\\'], "_")));
    std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
    let out = Command::new("lp").args(lp_args(job, &file)).output().map_err(|e| format!("The print service is not available ({e})"))?;
    if !out.status.success() {
        let e = String::from_utf8_lossy(&out.stderr);
        return Err(e.trim().trim_start_matches("lp: ").to_string());
    }
    // "request id is PDF-12 (1 file(s))"
    let s = String::from_utf8_lossy(&out.stdout);
    Ok(s.split_whitespace().nth(3).unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_checked_and_normalised() {
        assert_eq!(page_range("1-3, 5", 10).unwrap(), "1-3,5");
        assert_eq!(page_range("8-", 10).unwrap(), "8-10");
        assert_eq!(page_range("-2", 10).unwrap(), "1-2");
        assert_eq!(page_range("4-4", 10).unwrap(), "4");
        for bad in ["", "0", "11", "3-2", "a", "1-x", ","] {
            assert!(page_range(bad, 10).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn lp_gets_every_option() {
        let job = Job { printer: "PDF".into(), copies: 2, pages: Some("1-2".into()), fit: true, two_sided: true, media: None, orientation: 0 };
        let a = lp_args(&job, Path::new("/tmp/x/Book.pdf"));
        assert_eq!(a, ["-d", "PDF", "-n", "2", "-P", "1-2", "-o", "fit-to-page", "-o", "sides=two-sided-long-edge", "-t", "Book", "--", "/tmp/x/Book.pdf"]);
        let job = Job { copies: 0, pages: None, fit: false, two_sided: false, ..job };
        let a = lp_args(&job, Path::new("/tmp/x/Book.pdf"));
        assert!(a.contains(&"print-scaling=none".to_string()) && a.contains(&"sides=one-sided".to_string()) && !a.contains(&"-P".to_string()));
        assert_eq!(a[3], "1");
        // Paper and orientation when chosen.
        let a = lp_args(&Job { media: Some("A4".into()), orientation: 2, ..job }, Path::new("/tmp/x/Book.pdf"));
        assert!(a.windows(2).any(|w| w == ["-o", "media=A4"]) && a.windows(2).any(|w| w == ["-o", "orientation-requested=4"]), "{a:?}");
    }
}
