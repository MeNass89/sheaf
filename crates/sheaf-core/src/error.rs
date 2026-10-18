use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum OpdfError {
    #[error("{0}: a password is required")]
    PasswordRequired(PathBuf),
    #[error("{0}: wrong password")]
    WrongPassword(PathBuf),
    /// The password of a digital ID (`.p12`), not of a PDF.
    #[error("{0}: wrong certificate password")]
    CertificatePassword(PathBuf),
    #[error("invalid page range \"{range}\": the document has {pages} pages")]
    InvalidRange { range: String, pages: u32 },
    #[error("cannot remove every page (the document has {pages} pages)")]
    WouldBeEmpty { pages: u32 },
    #[error("LibreOffice is not installed (needed for Office → PDF)")]
    OfficeMissing,
    #[error("conversion failed: {0}")]
    Conversion(String),
    #[error("unsupported input: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Engine(String),
    #[error("cancelled")]
    Cancelled,
    #[error("pdfium: {0}")]
    Pdfium(#[from] pdfium_render::prelude::PdfiumError),
    #[error("pdf structure: {0}")]
    Lopdf(#[from] lopdf::Error),
    #[error("image: {0}")]
    Image(#[from] image::ImageError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, OpdfError>;
