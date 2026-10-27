//! Sheaf: what the reader needs of Preview's Loupe and Mask (pdfcraft-annot's `loupe_mask`).

impl crate::Document {
    /// Every mask's opening: ((page, index in its `/Annots`), the opening in user space), so a
    /// click on it picks the mask.
    pub fn mask_holes(&self) -> Vec<((usize, usize), [f64; 4])> {
        self.editor.as_ref().map(|e| pdfcraft_annot::loupe_mask::mask_holes(&e.cos)).unwrap_or_default()
    }
}

/// The magnifications a loupe offers (Preview's steps): 1.5, 2, 3 and 4 times.
pub fn loupe_magnifications() -> &'static [f64] {
    &pdfcraft_annot::loupe_mask::MAGNIFICATIONS
}
