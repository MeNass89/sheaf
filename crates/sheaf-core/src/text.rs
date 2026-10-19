//! The text of pages as Sheaf's tools see it: PdfCraft's text layer (what Redact searches and
//! Compare diffs), in reading order, one string per page.

use crate::pdfcraft::{render, Doc};
use crate::{pages::Input, OpdfError, Result};

/// The text of each of `pages` (0-based; empty means every page), in that order. A page with no
/// text layer gives an empty string.
pub fn page_texts(input: &Input, pages: &[usize]) -> Result<Vec<String>> {
    let doc = Doc::open(input)?;
    let d = doc.document();
    let n = d.info.pages.len();
    if let Some(&p) = pages.iter().find(|&&p| p >= n) {
        return Err(OpdfError::Unsupported(format!("page {} does not exist (the document has {n} pages)", p + 1)));
    }
    let all: Vec<usize> = if pages.is_empty() { (0..n).collect() } else { pages.to_vec() };
    let config = render::RenderConfig { password: d.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
    let mut r = render::PageRenderer::new(d.bytes.clone(), config);
    all.into_iter()
        .map(|page| {
            let res = r.render(render::RenderRequest { page, kind: render::RequestKind::Text, scale: 1.0, ..Default::default() });
            match (res.text, res.error) {
                (Some(t), _) => Ok(t.plain_text()),
                (None, Some(e)) => Err(OpdfError::Engine(format!("page {}: {e}", page + 1))),
                (None, None) => Ok(String::new()),
            }
        })
        .collect()
}
