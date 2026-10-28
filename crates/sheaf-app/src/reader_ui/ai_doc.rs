//! What the AI panel (`src/ai_ui.rs`) reads of an open document: its working bytes (unsaved edits
//! included), the selected text and the page count; and the pages' text, read off the UI thread.
use super::*;

/// An open document as the AI panel sees it.
pub struct AiDoc {
    pub bytes: Arc<Vec<u8>>,
    pub password: Option<String>,
    pub selection: String,
    /// The page count.
    pub pages: usize,
    pub path: PathBuf,
}

pub fn ai_doc(tab_id: i32) -> Option<AiDoc> {
    update(tab_id, |r, v| {
        let selection = r.sel.as_ref().and_then(|(p, range)| r.texts.get(p).map(|t| reader::selected_text(t, range.clone()))).unwrap_or_default();
        AiDoc { bytes: r.snap.display.clone(), password: r.snap.password.clone(), selection, pages: v.viewer.page_count(), path: r.path.clone() }
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

/// An AI answer's cited page (0-based) shown in the reader tab (Acrobat's citations open the
/// source).
pub fn ai_goto(ui: &AppWindow, tab_id: i32, page: usize) {
    let shown = update(tab_id, |_, v| {
        if page < v.viewer.page_count() {
            v.show_page(page, None);
            true
        } else {
            false
        }
    });
    if shown == Some(true) {
        crate::viewer_ui::request(ui, tab_id);
    } else {
        crate::flow::toast(ui, &format!("This document has no page {}", page + 1));
    }
}

/// Where a main point's sentence is: its page first, then every page. The sentence as written,
/// else its first eight words (a model may end it differently), case and line breaks ignored.
fn find_quote(r: &mut ReaderUi, pages: usize, page: Option<usize>, quote: &str) -> Option<(usize, std::ops::Range<usize>)> {
    let words: Vec<&str> = quote.split_whitespace().collect();
    let tries = [words.join(" "), words.iter().take(8).copied().collect::<Vec<_>>().join(" ")];
    let order: Vec<usize> = page.filter(|p| *p < pages).into_iter().chain((0..pages).filter(|p| Some(*p) != page)).collect();
    for t in tries.iter().filter(|t| t.chars().count() >= 8) {
        for &p in &order {
            if let Some(m) = r.text(p).and_then(|text| text.find_opts(t, false, false).into_iter().next()) {
                return Some((p, m));
            }
        }
    }
    None
}

/// PDF Expert's and Acrobat's highlight main points: each point's sentence highlighted in the
/// document, the point as the highlight's note, all as one undoable change. Returns how many
/// were found and highlighted.
pub fn ai_highlight(ui: &AppWindow, tab_id: i32, points: &[(Option<usize>, String, String)]) -> usize {
    let made = update(tab_id, |r, v| {
        let n = v.viewer.page_count();
        let mut edits = Vec::new();
        let mut pages = Vec::new();
        for (page, point, quote) in points {
            let Some((p, range)) = find_quote(r, n, *page, quote) else { continue };
            let Some(text) = r.text(p) else { continue };
            let rects = text.line_rects(range);
            let quads = reader::quads(&v.viewer.boxes[p], &rects);
            edits.push(Edit::AddAnnotation(NewAnnotation { page: p, shape: Shape::TextMarkup { kind: Markup::Highlight, quads }, style: edits::style(light_at(r.light), 1.0), contents: point.clone(), author: edits::author() }));
            if !pages.contains(&p) {
                pages.push(p);
            }
        }
        (edits, pages)
    });
    let Some((edits, pages)) = made else { return 0 };
    let count = edits.len();
    if count > 0 {
        edits::apply(ui, tab_id, Edit::Batch { label: "Highlight main points".into(), edits }, &pages, None);
    }
    count
}
