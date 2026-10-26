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
    /// Pages to a sheet: 1, 2, 4, 6, 9 or 16 (Preview's Pages per Sheet, CUPS `number-up`).
    pub per_sheet: u8,
    /// A booklet (Acrobat's Booklet): the pages imposed two to a side in folding order before
    /// printing, both sides, flipped on the short edge.
    pub booklet: bool,
    /// In shades of grey (Acrobat's Print in grayscale, iLovePDF's Grayscale).
    pub gray: bool,
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
    let sides = if job.booklet { "sides=two-sided-short-edge" } else if job.two_sided { "sides=two-sided-long-edge" } else { "sides=one-sided" };
    a.extend(["-o".into(), sides.into()]);
    if job.per_sheet > 1 && !job.booklet {
        a.extend(["-o".into(), format!("number-up={}", job.per_sheet)]);
    }
    if job.gray {
        a.extend(["-o".into(), "print-color-mode=monochrome".into()]);
    }
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

/// A document opened with its password, as the print service can read it: an unprotected copy,
/// when its owner allows printing (Acrobat prints such a file; Sol 25/10: Sheaf refused it).
pub fn unlocked(bytes: &[u8], password: &str) -> Result<std::sync::Arc<Vec<u8>>, String> {
    if sheaf_core::facts::read(bytes, Some(password)).restricted.contains(&"Printing") {
        return Err("its owner does not allow printing it".into());
    }
    sheaf_core::security::printable_copy(bytes, password).map(std::sync::Arc::new).map_err(|e| e.to_string())
}

/// `bytes` as printed with comments off (Acrobat's Comments and Forms ▸ Document): the
/// annotations taken out but for form fields and links.
pub fn without_comments(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut d = lopdf::Document::load_mem(bytes).map_err(|e| e.to_string())?;
    let keep = |d: &lopdf::Document, o: &lopdf::Object| {
        d.dereference(o).ok().and_then(|(_, a)| a.as_dict().ok()?.get(b"Subtype").ok()?.as_name().ok().map(|s| s == b"Widget" || s == b"Link")).unwrap_or(true)
    };
    for page in d.get_pages().into_values() {
        let Some(annots) = d.get_dictionary(page).ok().and_then(|p| p.get(b"Annots").ok()).and_then(|a| d.dereference(a).ok()).and_then(|(_, a)| a.as_array().ok().cloned()) else { continue };
        let kept: Vec<lopdf::Object> = annots.iter().filter(|o| keep(&d, o)).cloned().collect();
        if kept.len() != annots.len()
            && let Ok(p) = d.get_object_mut(page).and_then(lopdf::Object::as_dict_mut)
        {
            p.set("Annots", kept);
        }
    }
    let mut out = Vec::new();
    d.save_to(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// `bytes` imposed as a booklet (`sheaf_core::pages::impose`).
pub fn booklet(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let (from, to) = (dir.path().join("in.pdf"), dir.path().join("booklet.pdf"));
    std::fs::write(&from, bytes).map_err(|e| e.to_string())?;
    sheaf_core::pages::impose(&sheaf_core::pages::Input::new(&from), 2, true, &to).map_err(|e| e.to_string())?;
    std::fs::read(&to).map_err(|e| e.to_string())
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
        let job = Job { printer: "PDF".into(), copies: 2, pages: Some("1-2".into()), fit: true, two_sided: true, media: None, orientation: 0, per_sheet: 1, booklet: false, gray: false };
        let a = lp_args(&job, Path::new("/tmp/x/Book.pdf"));
        assert_eq!(a, ["-d", "PDF", "-n", "2", "-P", "1-2", "-o", "fit-to-page", "-o", "sides=two-sided-long-edge", "-t", "Book", "--", "/tmp/x/Book.pdf"]);
        let job = Job { copies: 0, pages: None, fit: false, two_sided: false, ..job };
        let a = lp_args(&job, Path::new("/tmp/x/Book.pdf"));
        assert!(a.contains(&"print-scaling=none".to_string()) && a.contains(&"sides=one-sided".to_string()) && !a.contains(&"-P".to_string()));
        assert_eq!(a[3], "1");
        // Paper and orientation when chosen.
        let a = lp_args(&Job { media: Some("A4".into()), orientation: 2, ..job.clone() }, Path::new("/tmp/x/Book.pdf"));
        assert!(a.windows(2).any(|w| w == ["-o", "media=A4"]) && a.windows(2).any(|w| w == ["-o", "orientation-requested=4"]), "{a:?}");
        // Pages per sheet and grey; a booklet prints both sides flipped on the short edge, its
        // pages imposed already.
        let a = lp_args(&Job { per_sheet: 4, gray: true, ..job.clone() }, Path::new("/tmp/x/Book.pdf"));
        assert!(a.windows(2).any(|w| w == ["-o", "number-up=4"]) && a.windows(2).any(|w| w == ["-o", "print-color-mode=monochrome"]), "{a:?}");
        let a = lp_args(&Job { per_sheet: 4, booklet: true, ..job }, Path::new("/tmp/x/Book.pdf"));
        assert!(a.windows(2).any(|w| w == ["-o", "sides=two-sided-short-edge"]) && !a.iter().any(|s| s.starts_with("number-up")), "{a:?}");
    }

    /// Comments off: a highlight and a note go, a form field and a link stay.
    #[test]
    fn printing_without_comments_keeps_fields_and_links() {
        use lopdf::{Document, Object, dictionary};
        let mut d = Document::with_version("1.7");
        let annots: Vec<Object> = ["Highlight", "Text", "Widget", "Link"].iter().map(|s| d.add_object(dictionary! { "Type" => "Annot", "Subtype" => *s, "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()] }).into()).collect();
        let pages = d.new_object_id();
        let page = d.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()], "Annots" => annots });
        d.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
        let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        d.trailer.set("Root", cat);
        let mut bytes = Vec::new();
        d.save_to(&mut bytes).unwrap();
        let out = Document::load_mem(&without_comments(&bytes).unwrap()).unwrap();
        let p = out.get_dictionary(*out.get_pages().get(&1).unwrap()).unwrap();
        let kinds: Vec<Vec<u8>> = p.get(b"Annots").unwrap().as_array().unwrap().iter().map(|o| out.dereference(o).unwrap().1.as_dict().unwrap().get(b"Subtype").unwrap().as_name().unwrap().to_vec()).collect();
        assert_eq!(kinds, [b"Widget".to_vec(), b"Link".to_vec()]);
    }

    #[test]
    fn a_protected_pdf_prints_from_an_unprotected_copy_unless_printing_is_forbidden() {
        let t = tempfile::tempdir().unwrap();
        let blank = sheaf_core::pages::blank(false, &t.path().join("b.pdf")).unwrap();
        let qpdf = |extra: &[&str], out: &str| {
            let mut args = vec!["--encrypt", "user", "owner", "256"];
            args.extend_from_slice(extra);
            args.push("--");
            let o = Command::new("qpdf").args(&args).arg(&blank).arg(t.path().join(out)).output().unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
            std::fs::read(t.path().join(out)).unwrap()
        };
        let open = qpdf(&[], "open.pdf");
        let copy = unlocked(&open, "user").unwrap();
        assert!(!String::from_utf8_lossy(&copy).contains("/Encrypt"), "the copy is not encrypted");
        // Changes forbidden, printing allowed: it prints.
        let no_change = qpdf(&["--modify=none", "--extract=n"], "nochange.pdf");
        let copy = unlocked(&no_change, "user").unwrap();
        assert!(!String::from_utf8_lossy(&copy).contains("/Encrypt") && copy.starts_with(b"%PDF"), "a locked-for-changes file prints");
        let no_print = qpdf(&["--print=none"], "noprint.pdf");
        assert!(unlocked(&no_print, "user").unwrap_err().contains("does not allow printing"));
    }
}
