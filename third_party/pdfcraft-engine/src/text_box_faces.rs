//! Sheaf: the fonts text boxes are drawn in, as the reader reads them (pdfcraft-annot's
//! `text_box_font::base_fonts`): bold and italic then pick the font's own faces.

impl crate::Document {
    /// ((page, index in its `/Annots`), `/BaseFont`) of every text box whose appearance holds its font.
    pub fn text_box_base_fonts(&self) -> Vec<((usize, usize), String)> {
        self.editor.as_ref().map(|e| pdfcraft_annot::text_box_font::base_fonts(&e.cos)).unwrap_or_default()
    }
}
