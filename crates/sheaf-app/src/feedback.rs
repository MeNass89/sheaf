//! Help ▸ Report an Issue (iLovePDF's Contact us dialog: a subject, a message, up to five
//! attached files, sent with the app's details). Sheaf never sends anything itself: the report is
//! saved as one zip, copied, or opened as a new issue in the browser for the user to post.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Where issues are posted (the workspace's `repository`).
pub const ISSUES: &str = "https://github.com/MeNass89/sheaf/issues/new";
/// iLovePDF's limit: five attached files.
pub const MAX_FILES: usize = 5;
/// iLovePDF refuses a file that is too big; Sheaf keeps the zip under 25 MB each.
pub const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
/// The subjects (iLovePDF's Report a problem and Suggest a feature).
pub const SUBJECTS: [&str; 2] = ["Report a problem", "Suggest a feature"];

/// What the user wrote.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// Index in [`SUBJECTS`].
    pub subject: usize,
    pub message: String,
    /// The details of this computer and app ([`diagnostics`]), or None when left out.
    pub diagnostics: Option<String>,
    pub files: Vec<PathBuf>,
}

/// The app's details sent with a problem: version, system, what can run here.
pub fn diagnostics(extra: &[(&str, String)]) -> String {
    let mut s = format!(
        "Sheaf {}\nSystem: {} {} ({})\nLanguage: {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::FAMILY,
        ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty())).unwrap_or_else(|| "unknown".into())
    );
    for (k, v) in extra {
        s.push_str(&format!("{k}: {v}\n"));
    }
    s
}

impl Report {
    /// Why it cannot be sent yet (iLovePDF's "You must include a message before sending."), or
    /// None.
    pub fn problem(&self) -> Option<String> {
        if self.message.trim().is_empty() {
            return Some("Write a message first".into());
        }
        if self.files.len() > MAX_FILES {
            return Some(format!("At most {MAX_FILES} files"));
        }
        for f in &self.files {
            match std::fs::metadata(f) {
                Ok(m) if m.len() > MAX_FILE_BYTES => return Some(format!("{} is too big (25 MB at most)", name(f))),
                Ok(_) => {}
                Err(_) => return Some(format!("{} cannot be read", name(f))),
            }
        }
        None
    }

    /// The issue's title: the subject and the message's first line, cut at 80 characters.
    pub fn title(&self) -> String {
        let first = self.message.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        let mut t = format!("{}: {first}", SUBJECTS[self.subject.min(1)]);
        if t.chars().count() > 80 {
            t = t.chars().take(79).collect::<String>() + "…";
        }
        t
    }

    /// The report as text: the message, the files' names, the details.
    pub fn text(&self) -> String {
        let mut s = format!("{}\n\n{}\n", self.title(), self.message.trim());
        if !self.files.is_empty() {
            s.push_str("\nAttached files:\n");
            for f in &self.files {
                s.push_str(&format!("- {}\n", name(f)));
            }
        }
        if let Some(d) = &self.diagnostics {
            s.push_str("\n---\n");
            s.push_str(d);
        }
        s
    }

    /// The browser's new-issue page with the title and the text filled in (files are attached
    /// there by the user: a web address cannot carry them).
    pub fn issue_url(&self) -> String {
        let label = if self.subject == 1 { "enhancement" } else { "bug" };
        format!("{ISSUES}?title={}&body={}&labels={label}", encode(&self.title()), encode(&self.text()))
    }

    /// One zip holding `report.txt` and the attached files (a name taken twice gets a number).
    pub fn save_zip(&self, to: &Path) -> std::io::Result<()> {
        if let Some(p) = self.problem() {
            return Err(std::io::Error::other(p));
        }
        let mut z = zip::ZipWriter::new(std::fs::File::create(to)?);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        z.start_file("report.txt", opts).map_err(std::io::Error::other)?;
        z.write_all(self.text().as_bytes())?;
        let mut used = vec!["report.txt".to_string()];
        for f in &self.files {
            let base = name(f);
            let mut n = base.clone();
            let mut k = 2;
            while used.contains(&n) {
                n = format!("{k}-{base}");
                k += 1;
            }
            z.start_file(format!("files/{n}"), opts).map_err(std::io::Error::other)?;
            z.write_all(&std::fs::read(f)?)?;
            used.push(n);
        }
        z.finish().map_err(std::io::Error::other)?;
        Ok(())
    }
}

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into())
}

/// Percent-encodes everything but unreserved characters (RFC 3986).
fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_needs_a_message_and_at_most_five_readable_files() {
        let dir = std::env::temp_dir().join(format!("sheaf-feedback-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("b")).unwrap();
        let a = dir.join("a.pdf");
        let a2 = dir.join("b/a.pdf");
        std::fs::write(&a, b"%PDF-1.7 one").unwrap();
        std::fs::write(&a2, b"%PDF-1.7 two").unwrap();
        let mut r = Report { subject: 0, message: "  \n ".into(), diagnostics: None, files: vec![] };
        assert_eq!(r.problem().as_deref(), Some("Write a message first"));
        r.message = "\nSaving freezes\nwith a 300-page file & forms".into();
        assert_eq!(r.problem(), None);
        r.files = vec![a.clone(); 6];
        assert_eq!(r.problem().as_deref(), Some("At most 5 files"));
        r.files = vec![dir.join("missing.pdf")];
        assert_eq!(r.problem().as_deref(), Some("missing.pdf cannot be read"));
        r.files = vec![a.clone(), a2.clone()];
        r.diagnostics = Some(diagnostics(&[("OCR", "available".into())]));
        assert_eq!(r.title(), "Report a problem: Saving freezes");
        let text = r.text();
        assert!(text.contains("Attached files:\n- a.pdf\n- a.pdf\n") && text.contains(&format!("Sheaf {}", env!("CARGO_PKG_VERSION"))) && text.contains("OCR: available\n"), "{text}");
        // The issue page: every reserved character encoded, the subject's label.
        let url = r.issue_url();
        assert!(url.starts_with("https://github.com/MeNass89/sheaf/issues/new?title=Report%20a%20problem%3A%20Saving%20freezes&body="), "{url}");
        assert!(url.contains("%26%20forms") && url.ends_with("&labels=bug") && !url[ISSUES.len() + 1..].contains(' '));
        r.subject = 1;
        assert!(r.issue_url().ends_with("&labels=enhancement"));
        // Long first lines are cut.
        r.message = "x".repeat(200);
        assert_eq!(r.title().chars().count(), 80);
        // The zip: the report and both files, the second renamed.
        let zip_path = dir.join("report.zip");
        r.save_zip(&zip_path).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
        assert_eq!(names, ["report.txt", "files/a.pdf", "files/2-a.pdf"]);
        let mut second = String::new();
        std::io::Read::read_to_string(&mut z.by_name("files/2-a.pdf").unwrap(), &mut second).unwrap();
        assert_eq!(second, "%PDF-1.7 two");
        // Not saved without a message.
        r.message.clear();
        assert!(r.save_zip(&dir.join("no.zip")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
