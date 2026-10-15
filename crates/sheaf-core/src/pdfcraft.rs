//! Bridge to the PdfCraft engine (<https://github.com/storytold/pdfcraft>, Apache-2.0).
//!
//! A [`Doc`] owns one `pdfcraft_engine::Session` holding one document. A session is not
//! thread-safe, so each job opens its own `Doc` on its worker thread and shares nothing.
//! Outputs go through [`atomic_write`]: the engine's bytes are written verbatim (never
//! re-saved), which keeps incremental saves such as signatures valid.

use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The PdfCraft engine crate, so later tools and the app use one path to it.
pub use pdfcraft_engine as engine;
/// PdfCraft's renderer crate (render requests, rendered pages, open errors).
pub use pdfcraft_render as render;

use engine::{DocId, Edit, Session};
use render::OpenError;

/// One document open in its own PdfCraft session.
pub struct Doc {
    session: Session,
    id: DocId,
}

impl Doc {
    /// Reads `input.path` and opens it with `input.password` (tried as user or owner password).
    /// A missing password is [`OpdfError::PasswordRequired`], a wrong one
    /// [`OpdfError::WrongPassword`]; any other failure is [`OpdfError::Engine`].
    pub fn open(input: &Input) -> Result<Doc> {
        let bytes = Arc::new(std::fs::read(&input.path)?);
        let name = input.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "document.pdf".into());
        let mut session = Session::new();
        let id = session.open(name, None, bytes, input.password.as_deref()).map_err(|e| match e {
            OpenError::NeedsPassword | OpenError::WrongPassword if input.password.is_none() => {
                OpdfError::PasswordRequired(input.path.clone())
            }
            OpenError::NeedsPassword | OpenError::WrongPassword => OpdfError::WrongPassword(input.path.clone()),
            e => OpdfError::Engine(format!("{}: {e}", input.path.display())),
        })?;
        Ok(Doc { session, id })
    }

    /// Applies one edit to the document (in memory; nothing is written until a save).
    pub fn apply(&mut self, edit: Edit) -> Result<()> {
        self.session.apply(self.id, edit).map_err(engine_err)
    }

    /// The session, for tools that need engine methods beyond [`Edit`]s.
    pub fn session(&self) -> &Session { &self.session }
    pub fn session_mut(&mut self) -> &mut Session { &mut self.session }
    /// The document's id within [`Self::session`].
    pub fn id(&self) -> DocId { self.id }

    fn doc(&self) -> &engine::Document {
        self.session.get(self.id).expect("a Doc's document stays open in its session")
    }

    /// Whether the input file was encrypted (it was opened with a password).
    pub fn was_encrypted(&self) -> bool { self.doc().info.encrypted }

    /// Renders page `page` (0-based) at `scale` device pixels per point, as premultiplied RGBA8.
    pub fn render(&self, page: usize, scale: f32) -> Result<render::RenderedPage> {
        let doc = self.doc();
        let config = render::RenderConfig { password: doc.password.as_deref().map(Arc::from), ..Default::default() };
        let mut r = render::PageRenderer::new(doc.display.clone(), config);
        let out = r.render(render::RenderRequest { page, scale, ..Default::default() });
        match &out.error {
            Some(e) => Err(OpdfError::Engine(format!("page {}: {e}", page + 1))),
            None => Ok(out),
        }
    }

    /// Writes a full, compacted rewrite of the document to `unique_path(out)`. An encrypted
    /// input stays encrypted: apply `Edit::RemoveProtection` first for an unprotected output.
    pub fn save(&self, out: &Path) -> Result<PathBuf> {
        let bytes = self.session.save_full_bytes(self.id).map_err(engine_err)?;
        write_verbatim(&bytes, out)
    }

    /// Writes the document as an incremental update of the file as opened (the original bytes
    /// followed by the changes), to `unique_path(out)`. Signatures need this: a full rewrite
    /// would invalidate earlier ones.
    pub fn save_incremental(&self, out: &Path) -> Result<PathBuf> {
        let bytes = self.session.save_bytes(self.id).map_err(engine_err)?;
        write_verbatim(&bytes, out)
    }
}

fn engine_err(e: engine::EditError) -> OpdfError { OpdfError::Engine(e.to_string()) }

fn write_verbatim(bytes: &[u8], out: &Path) -> Result<PathBuf> {
    atomic_write(out, |part| Ok(std::fs::write(part, bytes)?))
}
