mod common;
use sheaf_core::{engine::{open_pdf, pdfium_lock}, OpdfError};

#[test]
fn fixture_roundtrips_text() {
    let t = tempfile::tempdir().unwrap();
    let p = common::fixture_pdf(t.path(), "a.pdf", 3);
    assert_eq!(common::page_texts(&p), ["Page 1", "Page 2", "Page 3"]);
    let _g = pdfium_lock();
    assert_eq!(open_pdf(&p, None).unwrap().pages().len(), 3);
}

#[test]
fn missing_file_is_io_or_pdfium_error() {
    let _g = pdfium_lock();
    let err = open_pdf(std::path::Path::new("/nonexistent.pdf"), None).err().unwrap();
    assert!(matches!(err, OpdfError::Pdfium(_) | OpdfError::Io(_)), "{err:?}");
}
