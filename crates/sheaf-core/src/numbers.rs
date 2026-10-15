//! Page numbers: a number stamped in a corner or edge of each page, through PdfCraft's
//! Header & Footer (`Edit::AddHeaderFooter`).

use crate::pdfcraft::engine::{Edit, HeaderFooter};
use crate::pdfcraft::Doc;
use crate::{pages::Input, OpdfError, Result};
use std::path::{Path, PathBuf};

/// What is written: `{n}` is the page number, `{p}` the page count.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum NumberText {
    /// "{n}" (the default).
    #[default]
    Number,
    /// "Page {n}".
    Page,
    /// "Page {n} of {p}".
    PageOfTotal,
    /// Any text with `{n}` and `{p}`.
    Custom(String),
}

impl NumberText {
    /// The template in PdfCraft's tokens (`<<1>>` number, `<<n>>` count).
    fn template(&self) -> String {
        let t = match self {
            NumberText::Number => "{n}",
            NumberText::Page => "Page {n}",
            NumberText::PageOfTotal => "Page {n} of {p}",
            NumberText::Custom(s) => s.as_str(),
        };
        t.replace("{n}", "<<1>>").replace("{p}", "<<n>>")
    }
}

/// Page numbers settings, mirroring the options panel.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberOptions {
    /// Cell of the 3×3 position grid, row by row from the top left (0..=8). Numbers go in the
    /// top or bottom row only: 0, 1, 2, 6, 7 or 8.
    pub position: u8,
    /// Double-sided: even pages mirror the left/right position.
    pub double_sided: bool,
    /// First page is a cover page: never numbered.
    pub cover: bool,
    /// Distance from the page edges, in points (Small 18, Recommended 36, Big 54).
    pub margin: f64,
    /// First and last page to number, 1-based (default: all).
    pub from: Option<u32>,
    pub to: Option<u32>,
    /// The number the first numbered page shows.
    pub start: u32,
    pub text: NumberText,
    pub font_size: f64,
    pub color: [f64; 3],
}

impl Default for NumberOptions {
    fn default() -> Self {
        Self { position: 7, double_sided: false, cover: false, margin: 36.0, from: None, to: None, start: 1, text: NumberText::Number, font_size: 12.0, color: [0.0; 3] }
    }
}

/// Margin choices of the panel: Small, Recommended, Big.
pub const MARGINS: [f64; 3] = [18.0, 36.0, 54.0];

/// The pages numbered (0-based), from `from`/`to` (1-based, inclusive) and the cover option.
pub(crate) fn page_span(from: Option<u32>, to: Option<u32>, skip_first: bool, count: usize) -> Result<std::ops::Range<usize>> {
    let total = count as u32;
    let from = from.unwrap_or(1).max(if skip_first { 2 } else { 1 });
    let to = to.unwrap_or(total);
    if from == 0 || from > to || to > total {
        let range = format!("{from}-{to}");
        return Err(OpdfError::InvalidRange { range, pages: total });
    }
    Ok(from as usize - 1..to as usize)
}

/// Header & footer slot (left, centre, right header, then footer) of a grid cell.
fn slot(position: u8, mirror: bool) -> Result<usize> {
    let s = match position {
        0..=2 => position as usize,
        6..=8 => position as usize - 3,
        p => return Err(OpdfError::Unsupported(format!("page number position {p} (use the top or bottom row)"))),
    };
    Ok(if mirror { s / 3 * 3 + (2 - s % 3) } else { s })
}

/// Stamps page numbers on `input` and writes `out` (never overwriting). Returns the path written
/// and whether the input was password-protected (the output never is).
pub fn number_pages(input: &Input, o: &NumberOptions, out: &Path) -> Result<(PathBuf, bool)> {
    let mut doc = Doc::open(input)?;
    let count = doc.session().get(doc.id()).map(|d| d.info.pages.len()).unwrap_or(0);
    let span = page_span(o.from, o.to, o.cover, count)?;
    slot(o.position, false)?;
    let template = o.text.template();
    if template.trim().is_empty() {
        return Err(OpdfError::Unsupported("the page number text is empty".into()));
    }
    // PdfCraft numbers the pages of one edit start, start + 1, … in list order. Double-sided
    // pages alternate sides, so each page gets its own edit; otherwise one edit covers the span.
    let edit = |pages: Vec<usize>, start_number: u32, mirror: bool| -> Result<Edit> {
        let mut text: [String; 6] = Default::default();
        text[slot(o.position, mirror)?] = template.clone();
        let settings = HeaderFooter { text, font_size: o.font_size, color: o.color, underline: false, margins: [o.margin; 4], start_number };
        Ok(Edit::AddHeaderFooter { pages, settings, replace: false })
    };
    let edits = if o.double_sided {
        span.enumerate()
            .map(|(k, page)| {
                let n = o.start.checked_add(k as u32).ok_or_else(|| OpdfError::Unsupported("the page numbers are too large".into()))?;
                edit(vec![page], n, (page + 1) % 2 == 0)
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        vec![edit(span.collect(), o.start, false)?]
    };
    doc.apply(Edit::Batch { label: "Page numbers".into(), edits })?;
    doc.save_unprotected(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_use_pdfcraft_tokens() {
        assert_eq!(NumberText::Number.template(), "<<1>>");
        assert_eq!(NumberText::PageOfTotal.template(), "Page <<1>> of <<n>>");
        assert_eq!(NumberText::Custom("{n}/{p}".into()).template(), "<<1>>/<<n>>");
    }

    #[test]
    fn slots_and_mirroring() {
        assert_eq!(slot(0, false).unwrap(), 0);
        assert_eq!(slot(0, true).unwrap(), 2);
        assert_eq!(slot(7, true).unwrap(), 4);
        assert_eq!(slot(8, true).unwrap(), 3);
        assert!(slot(4, false).is_err());
    }

    #[test]
    fn spans() {
        assert_eq!(page_span(None, None, false, 5).unwrap(), 0..5);
        assert_eq!(page_span(None, None, true, 5).unwrap(), 1..5);
        assert_eq!(page_span(Some(2), Some(3), false, 5).unwrap(), 1..3);
        assert!(page_span(Some(4), Some(3), false, 5).is_err());
        assert!(page_span(None, None, true, 1).is_err());
    }
}
