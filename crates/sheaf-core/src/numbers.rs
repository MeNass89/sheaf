//! Page numbers: a number stamped in a corner or edge of each page, through PdfCraft's
//! Header & Footer (`Edit::AddHeaderFooter`).

use crate::pdfcraft::engine::{Edit, HeaderFooter};
pub use pdfcraft_fonts::Std14;
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
    /// Any text with `{n}` (page number) and `{p}` (the document's total page count).
    /// `<<` and `>>` are refused: PdfCraft would expand them as its own tokens.
    Custom(String),
}

/// Why a custom page-number text is refused, if it is.
pub fn custom_text_error(text: &str) -> Option<&'static str> {
    (text.contains("<<") || text.contains(">>")).then_some("custom text cannot contain << or >>")
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

/// The standard font of `family` (0 Helvetica, 1 Times, 2 Courier), bold and/or italic, as the
/// options panels and the CLI choose it; `None` for plain Helvetica (the engine's default).
pub fn standard_font(family: u8, bold: bool, italic: bool) -> Option<Std14> {
    use Std14::*;
    const FACES: [[Std14; 4]; 3] = [
        [Helvetica, HelveticaBold, HelveticaOblique, HelveticaBoldOblique],
        [TimesRoman, TimesBold, TimesItalic, TimesBoldItalic],
        [Courier, CourierBold, CourierOblique, CourierBoldOblique],
    ];
    Some(FACES[family.min(2) as usize][bold as usize + 2 * italic as usize]).filter(|f| *f != Helvetica)
}

/// A colour typed as RRGGBB (a leading # allowed), as 0–1 components.
pub fn parse_hex_colour(s: &str) -> Option<[f64; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|b| b as f64 / 255.0);
    Some([v(0)?, v(2)?, v(4)?])
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
    /// The standard font (`None`: Helvetica).
    pub font: Option<Std14>,
    pub underline: bool,
    /// A `to` past the last page means the last page (one range for a batch of files).
    pub clamp_to_last: bool,
}

impl Default for NumberOptions {
    fn default() -> Self {
        Self { position: 7, double_sided: false, cover: false, margin: 36.0, from: None, to: None, start: 1, text: NumberText::Number, font_size: 12.0, color: [0.0; 3], font: None, underline: false, clamp_to_last: false }
    }
}

/// Margin choices of the panel: Small, Recommended, Big.
pub const MARGINS: [f64; 3] = [18.0, 36.0, 54.0];

/// The pages numbered (0-based), from `from`/`to` (1-based, inclusive) and the cover option.
/// `to`, or the last page when `clamp` and `to` is past it.
pub(crate) fn clamp(to: Option<u32>, clamp: bool, count: usize) -> Option<u32> {
    to.map(|t| if clamp { t.min(count as u32) } else { t })
}

pub(crate) fn page_span(from: Option<u32>, to: Option<u32>, skip_first: bool, count: usize) -> Result<std::ops::Range<usize>> {
    let total = count as u32;
    if from == Some(0) {
        return Err(OpdfError::InvalidRange { range: format!("0-{}", to.unwrap_or(total)), pages: total });
    }
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
    if let NumberText::Custom(t) = &o.text
        && let Some(e) = custom_text_error(t)
    {
        return Err(OpdfError::Unsupported(e.into()));
    }
    let mut doc = Doc::open(input)?;
    let count = doc.session().get(doc.id()).map(|d| d.info.pages.len()).unwrap_or(0);
    let span = page_span(o.from, clamp(o.to, o.clamp_to_last, count), o.cover, count)?;
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
        let settings = HeaderFooter { text, font_size: o.font_size, color: o.color, underline: o.underline, margins: [o.margin; 4], start_number, font: o.font };
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
        assert!(page_span(Some(0), None, false, 5).is_err());
        assert!(page_span(Some(0), None, true, 5).is_err());
        assert_eq!(clamp(Some(9), true, 5), Some(5));
        assert_eq!(clamp(Some(9), false, 5), Some(9));
        assert_eq!(clamp(Some(3), true, 5), Some(3));
        assert_eq!(custom_text_error("{n} of {p}"), None);
        assert!(custom_text_error("<<1>>").is_some());
    }
}
