//! What the AI panel (`src/ai_ui.rs`) reads of an open document: its working bytes (unsaved edits
//! included), the selected text and the page in view; and the pages' text, read off the UI thread.
use super::*;

/// An open document as the AI panel sees it.
pub struct AiDoc {
    pub bytes: Arc<Vec<u8>>,
    pub password: Option<String>,
    pub selection: String,
    /// The page in view (0-based) and the page count.
    pub page: usize,
    pub pages: usize,
    pub path: PathBuf,
}

pub fn ai_doc(tab_id: i32) -> Option<AiDoc> {
    update(tab_id, |r, v| {
        let selection = r.sel.as_ref().and_then(|(p, range)| r.texts.get(p).map(|t| reader::selected_text(t, range.clone()))).unwrap_or_default();
        AiDoc { bytes: r.snap.display.clone(), password: r.snap.password.clone(), selection, page: v.current(), pages: v.viewer.page_count(), path: r.path.clone() }
    })
}

/// The text of `pages` (0-based) of `doc`'s bytes, in that order; `stop` is asked between pages.
pub fn ai_page_texts(bytes: Arc<Vec<u8>>, password: Option<String>, pages: &[usize], stop: &dyn Fn() -> bool) -> Vec<String> {
    let config = render::RenderConfig { password: password.as_deref().map(Arc::from), ..Default::default() };
    let mut r = render::PageRenderer::new(bytes, config);
    let mut out = Vec::with_capacity(pages.len());
    for &page in pages {
        if stop() {
            break;
        }
        let res = r.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
        out.push(res.text.map(|t| t.plain_text()).unwrap_or_default());
    }
    out
}
