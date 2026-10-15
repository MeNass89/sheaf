use crate::error::{OpdfError, Result};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static PDFIUM: OnceLock<Pdfium> = OnceLock::new();
static PDFIUM_LOCK: parking_lot::ReentrantMutex<()> = parking_lot::const_reentrant_mutex(());

/// Process-wide lock serialising all pdfium work. pdfium shares global state across
/// documents, and pdfium-render's `thread_safe` feature only locks individual calls, so
/// concurrent operations crash (SIGSEGV). Hold the guard from before the first pdfium call
/// until every `PdfDocument`/page created under it is dropped. Re-entrant, so public
/// operations that call each other (remove → organize) can each take it.
pub fn pdfium_lock() -> parking_lot::ReentrantMutexGuard<'static, ()> { PDFIUM_LOCK.lock() }

fn candidate_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(d) = std::env::var("SHEAF_PDFIUM_DIR") { v.push(PathBuf::from(d)); }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        v.push(dir.join("../Frameworks"));
    }
    if cfg!(debug_assertions) {
        v.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../vendor/pdfium/lib")));
    }
    v
}

/// The process-wide pdfium instance. Panics with a clear message if no library is found.
pub(crate) fn pdfium() -> &'static Pdfium {
    PDFIUM.get_or_init(|| {
        let mut last_err = None;
        for dir in candidate_dirs() {
            let lib = Pdfium::pdfium_platform_library_name_at_path(&dir);
            if lib.exists() {
                match Pdfium::bind_to_library(&lib) {
                    Ok(b) => return Pdfium::new(b),
                    Err(e) => last_err = Some((lib, e)),
                }
            }
        }
        match last_err {
            Some((lib, e)) => panic!(
                "libpdfium.dylib could not be loaded: {}: {e}; run scripts/fetch-pdfium.sh or set SHEAF_PDFIUM_DIR",
                lib.display()
            ),
            None => panic!("libpdfium.dylib not found; run scripts/fetch-pdfium.sh or set SHEAF_PDFIUM_DIR"),
        }
    })
}

/// A new empty document. The caller must hold `pdfium_lock()` for the document's whole lifetime.
#[doc(hidden)]
pub fn new_document() -> PdfDocument<'static> {
    pdfium().create_new_pdf().expect("pdfium could not create a document")
}

/// Whether an open document uses a security handler (user or owner password, any revision).
/// The caller holds `pdfium_lock()`.
pub fn is_protected(doc: &PdfDocument) -> bool {
    !matches!(doc.permissions().security_handler_revision(), Ok(PdfSecurityHandlerRevision::Unprotected))
}

/// Opens a PDF and maps pdfium's password error to our variants.
/// The caller must hold `pdfium_lock()` for the document's whole lifetime.
pub fn open_pdf(path: &Path, password: Option<&str>) -> Result<PdfDocument<'static>> {
    match pdfium().load_pdf_from_file(path, password) {
        Ok(d) => Ok(d),
        Err(PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError)) => Err(match password {
            None => OpdfError::PasswordRequired(path.to_path_buf()),
            Some(_) => OpdfError::WrongPassword(path.to_path_buf()),
        }),
        Err(e) => Err(e.into()),
    }
}
