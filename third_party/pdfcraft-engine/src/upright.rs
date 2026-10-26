//! Sheaf: pages turned upright by their text (Acrobat's Auto-Rotate Pages, PDF Expert's): a page
//! whose text mostly runs sideways or upside down as shown gets the quarter turns that set it
//! level. Pages without a text layer are left as they are (a scan needs its text read first).

use crate::Document;

impl Document {
    /// For each page to turn, the clockwise degrees (90, 180 or 270) to add to its rotation.
    pub fn upright_turns(&self) -> Vec<(usize, i64)> {
        let Some(editor) = self.editor.as_ref() else { return Vec::new() };
        let mut out = Vec::new();
        for (page, info) in self.info.pages.iter().enumerate() {
            let Ok(lines) = pdfcraft_edit::text_lines(&editor.cos, page) else { continue };
            // The baseline angles, weighted by how much text runs along each (quarter turns, CCW).
            let mut weight = [0usize; 4];
            for l in &lines {
                let q = ((l.rotation() / 90.0).round() as i64).rem_euclid(4) as usize;
                weight[q] += l.text.chars().filter(|c| !c.is_whitespace()).count();
            }
            let total: usize = weight.iter().sum();
            // Too little text to tell, or no clear majority.
            let Some((q, &w)) = weight.iter().enumerate().max_by_key(|(_, w)| **w) else { continue };
            if total < 20 || w * 3 < total * 2 {
                continue;
            }
            // Shown upright when the page turns clockwise as far as its text turns counterclockwise.
            let turn = (q as i64 * 90 - i64::from(info.rotation)).rem_euclid(360);
            if turn != 0 {
                out.push((page, turn));
            }
        }
        out
    }
}
