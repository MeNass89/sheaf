#![allow(dead_code)]
use sheaf_core::engine::{new_document, pdfium_lock};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

/// Writes a PDF whose page i (1-based) contains the text "Page i".
pub fn fixture_pdf(dir: &Path, name: &str, pages: u32) -> PathBuf {
    let _g = pdfium_lock();
    let mut doc = new_document();
    let font = doc.fonts_mut().helvetica();
    for i in 1..=pages {
        let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4()).unwrap();
        page.objects_mut()
            .create_text_object(PdfPoints::new(72.0), PdfPoints::new(720.0), format!("Page {i}"), font, PdfPoints::new(24.0))
            .unwrap();
    }
    let path = dir.join(name);
    doc.save_to_file(&path).unwrap();
    path
}

/// Text of every page, trimmed, via lopdf (reliable on rotated pages).
pub fn page_texts(path: &Path) -> Vec<String> {
    let d = lopdf::Document::load(path).unwrap();
    (1..=d.get_pages().len() as u32).map(|i| d.extract_text(&[i]).unwrap().trim().to_string()).collect()
}

/// Text of every page as a reader shows it (poppler's pdftotext: inside forms, within the media box), trimmed.
pub fn shown_texts(path: &Path) -> Vec<String> {
    let o = std::process::Command::new("pdftotext").arg("-layout").arg(path).arg("-").output().unwrap();
    let s = String::from_utf8_lossy(&o.stdout);
    let mut pages: Vec<String> = s.split('\x0c').map(|p| p.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    pages.pop();
    pages
}

/// Asserts `qpdf --check` passes with no WARNING.
pub fn qpdf_clean(path: &Path) {
    let o = std::process::Command::new("qpdf").arg("--check").arg(path).output().unwrap();
    let s = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(o.status.success() && !s.contains("WARNING"), "qpdf --check: {s}");
}

/// Page `page` (0-based) of `path` rendered by PdfCraft at 1 px per point: (width, height, rgba).
pub fn render(path: &Path, page: usize) -> (u32, u32, Vec<u8>) {
    let doc = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(path)).unwrap();
    let r = doc.render(page, 1.0).unwrap();
    (r.width, r.height, r.rgba.to_vec())
}

/// Dark pixels (red channel < 100) inside the box x0..x1, y0..y1 (fractions of the page, y down).
pub fn dark_in(img: &(u32, u32, Vec<u8>), x0: f32, x1: f32, y0: f32, y1: f32) -> usize {
    let (w, h, px) = img;
    let (w, h) = (*w as usize, *h as usize);
    let mut n = 0;
    for y in (y0 * h as f32) as usize..((y1 * h as f32) as usize).min(h) {
        for x in (x0 * w as f32) as usize..((x1 * w as f32) as usize).min(w) {
            let p = &px[(y * w + x) * 4..];
            if p[3] > 0 && p[0] < 100 { n += 1; }
        }
    }
    n
}

/// Copies of `src` encrypted the ways other producers do it, each with its password: qpdf
/// AES-256 (R6, owner ≠ user password) always, and MuPDF AES-256 with restricted permissions and
/// /Encrypt inline in the trailer when PyMuPDF is installed (skipped otherwise, with a note).
pub fn foreign_encrypted(src: &Path, dir: &Path) -> Vec<(PathBuf, &'static str)> {
    let stem = src.file_stem().unwrap().to_string_lossy().into_owned();
    let qpdf = dir.join(format!("{stem}-qpdf-aes256.pdf"));
    let o = std::process::Command::new("qpdf").args(["--encrypt", "user", "owner", "256", "--"]).arg(src).arg(&qpdf).output().unwrap();
    assert!(o.status.success(), "qpdf --encrypt: {}", String::from_utf8_lossy(&o.stderr));
    let mut out = vec![(qpdf, "user")];
    let mupdf = dir.join(format!("{stem}-mupdf-aes256.pdf"));
    let script = "import sys, fitz\n\
        fitz.open(sys.argv[1]).save(sys.argv[2], encryption=fitz.PDF_ENCRYPT_AES_256, user_pw='user', owner_pw='owner', \
        permissions=fitz.PDF_PERM_PRINT | fitz.PDF_PERM_COPY)\n";
    match std::process::Command::new("python3").args(["-c", script]).arg(src).arg(&mupdf).output() {
        Ok(o) if o.status.success() => out.push((mupdf, "owner")),
        _ => eprintln!("PyMuPDF not importable: MuPDF-encrypted case skipped"),
    }
    out
}

/// Asserts `path` is not encrypted and passes `qpdf --check` cleanly.
pub fn unprotected_and_clean(path: &Path) {
    let o = std::process::Command::new("qpdf").arg("--show-encryption").arg(path).output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).contains("File is not encrypted"), "{} is still encrypted", path.display());
    qpdf_clean(path);
}
