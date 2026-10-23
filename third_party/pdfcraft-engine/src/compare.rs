//! Compare files: the text differences between two open documents, a report, and the
//! differences marked as comments in the newer one.

use std::sync::Arc;

pub use pdfcraft_compare::{Change, Comparison, Kind, Side};

use crate::{DocId, Edit, EditError, Markup, NewAnnotation, NoteIcon, Session, Shape};

/// Acrobat's compare colours: replaced blue, inserted green, deleted red.
pub fn colour(kind: Kind) -> crate::Rgb {
    match kind {
        Kind::Replaced => [0.2, 0.45, 0.95],
        Kind::Inserted => [0.2, 0.75, 0.3],
        Kind::Deleted => [0.9, 0.25, 0.25],
    }
}

impl crate::Document {
    /// Standards ▸ Verify PDF/A: the rules the document breaks for `level`.
    pub fn pdfa_verify(&self, level: pdfcraft_preflight::Level) -> Vec<pdfcraft_preflight::Issue> {
        self.editor.as_ref().map(|e| pdfcraft_preflight::verify(&e.cos, level)).unwrap_or_default()
    }

    /// The standards the document declares (Standards panel).
    pub fn standards(&self) -> pdfcraft_preflight::Declared {
        self.editor.as_ref().map(|e| pdfcraft_preflight::declared(&e.cos)).unwrap_or_default()
    }

    /// Every word of the document in reading order, with page and box.
    pub fn words(&self) -> Vec<pdfcraft_compare::Word> {
        let config = pdfcraft_render::RenderConfig { password: self.password.as_deref().map(Arc::from), ..Default::default() };
        let mut r = pdfcraft_render::PageRenderer::new(self.bytes.clone(), config);
        let mut out = Vec::new();
        for (page, info) in self.info.pages.iter().enumerate() {
            let res = r.render(pdfcraft_render::RenderRequest { page, kind: pdfcraft_render::RequestKind::Text, scale: 1.0, ..Default::default() });
            if let Some(t) = res.text {
                out.extend(crate::js::page_words(&t, info).into_iter().map(|(text, rect)| pdfcraft_compare::Word { text, page, rect }));
            }
        }
        out
    }
}

impl Session {
    /// Compare files: the text differences from document `old` to document `new`.
    pub fn compare(&self, old: DocId, new: DocId) -> Result<Comparison, EditError> {
        let a = self.get(old).ok_or(EditError::NoDocument)?;
        let b = self.get(new).ok_or(EditError::NoDocument)?;
        Ok(pdfcraft_compare::compare(&a.words(), &b.words()))
    }

    /// Visual compare: regions where page n of `new` looks different from page n of `old`
    /// (rendered at `dpi`), as (page, user-space box in `new`).
    pub fn compare_visual(&self, old: DocId, new: DocId, dpi: f32) -> Result<Vec<(usize, [f64; 4])>, EditError> {
        let a = self.get(old).ok_or(EditError::NoDocument)?;
        let b = self.get(new).ok_or(EditError::NoDocument)?;
        let renderer = |d: &crate::Document| {
            pdfcraft_render::PageRenderer::new(
                d.bytes.clone(),
                pdfcraft_render::RenderConfig { password: d.password.as_deref().map(Arc::from), ..Default::default() },
            )
        };
        let (mut ra, mut rb) = (renderer(a), renderer(b));
        let scale = dpi.clamp(18.0, 150.0) / 72.0;
        let mut out = Vec::new();
        for (page, info) in b.info.pages.iter().enumerate().take(a.info.pages.len()) {
            let req = pdfcraft_render::RenderRequest { page, scale, ..Default::default() };
            let (x, y) = (ra.render(req), rb.render(req));
            if x.error.is_some() || y.error.is_some() {
                continue;
            }
            let s = y.width as f32 / info.width.max(1e-3);
            for r in pdfcraft_compare::visual_regions((&y.rgba, y.width, y.height), (&x.rgba, x.width, x.height), 24) {
                let p = info.view_to_user(r[0] as f32 / s, r[1] as f32 / s);
                let q = info.view_to_user(r[2] as f32 / s, r[3] as f32 / s);
                out.push((page, [p[0].min(q[0]) as f64, p[1].min(q[1]) as f64, p[0].max(q[0]) as f64, p[1].max(q[1]) as f64]));
            }
        }
        Ok(out)
    }

    /// The compare report as a new PDF (not opened).
    pub fn compare_report(&self, old: DocId, new: DocId) -> Result<Arc<Vec<u8>>, EditError> {
        let c = self.compare(old, new)?;
        let name = |id| self.get(id).map(|d| d.name.clone()).unwrap_or_default();
        self.create_report("Compare Report", &pdfcraft_compare::report(&c, &name(old), &name(new)))
    }

    /// Mark the differences in `new` as comments: highlights over replaced and inserted text
    /// (blue, green) and a note where text was deleted (red), authored "Compare". One undoable
    /// step; returns how many comments were added.
    pub fn mark_differences(&mut self, old: DocId, new: DocId) -> Result<usize, EditError> {
        let c = self.compare(old, new)?;
        let mut edits = Vec::new();
        for ch in &c.changes {
            let color = colour(ch.kind);
            let contents = match ch.kind {
                Kind::Replaced => format!("Replaced: \"{}\" with \"{}\"", ch.old.text, ch.new.text),
                Kind::Inserted => format!("Inserted: \"{}\"", ch.new.text),
                Kind::Deleted => format!("Deleted: \"{}\"", ch.old.text),
            };
            let shape = if ch.new.rects.is_empty() {
                let Some(r) = ch.new.near else { continue };
                Shape::Note { at: [r[0], r[3]], icon: NoteIcon::Note }
            } else {
                Shape::TextMarkup {
                    kind: Markup::Highlight,
                    quads: ch.new.rects.iter().map(|r| [r[0], r[3], r[2], r[3], r[0], r[1], r[2], r[1]]).collect(),
                }
            };
            let mut style = crate::Style::default_for(&shape);
            style.color = color;
            edits.push(Edit::AddAnnotation(NewAnnotation { page: ch.new.page, shape, style, contents, author: "Compare".into() }));
        }
        let n = edits.len();
        if n > 0 {
            self.apply(new, Edit::Batch { label: "Mark differences".into(), edits })?;
        }
        Ok(n)
    }
}

/// Export a PDF ▸ Word, PowerPoint, HTML or RTF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeFormat {
    Docx,
    Pptx,
    Html,
    Rtf,
}

impl OfficeFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OfficeFormat::Docx => "docx",
            OfficeFormat::Pptx => "pptx",
            OfficeFormat::Html => "html",
            OfficeFormat::Rtf => "rtf",
        }
    }

    pub fn from_extension(ext: &str) -> Option<OfficeFormat> {
        match ext.to_ascii_lowercase().as_str() {
            "docx" => Some(OfficeFormat::Docx),
            "pptx" => Some(OfficeFormat::Pptx),
            "html" | "htm" => Some(OfficeFormat::Html),
            "rtf" => Some(OfficeFormat::Rtf),
            _ => None,
        }
    }
}

impl crate::Document {
    /// The pages as paragraphs and images (for Word, HTML and RTF export), including what
    /// form XObjects draw.
    pub fn export_pages(&self) -> Vec<pdfcraft_export::Page> {
        let Some(cos) = self.editor.as_ref().map(|e| &e.cos) else { return Vec::new() };
        self.info
            .pages
            .iter()
            .enumerate()
            .map(|(i, info)| {
                // Export uses the visible page in points, y up. Raw PDF coordinates include
                // CropBox offsets and precede /Rotate and /UserUnit.
                let visible_rect = |r: [f64; 4]| {
                    let a = info.user_to_view(r[0] as f32, r[1] as f32);
                    let b = info.user_to_view(r[2] as f32, r[3] as f32);
                    [a[0].min(b[0]) as f64, (info.height - a[1].max(b[1])) as f64, a[0].max(b[0]) as f64, (info.height - a[1].min(b[1])) as f64]
                };
                let crop_width = f64::from(info.crop[2] - info.crop[0]);
                let display_width = f64::from(if info.rotation % 180 == 0 { info.width } else { info.height });
                let unit = if crop_width > 0.0 && crop_width.is_finite() && display_width.is_finite() { display_width / crop_width } else { 1.0 };
                // Sheaf: one block per run (a line's piece in one style), with the paragraph it is in:
                // paragraphs as the editor groups them, styles mixed, so a word in bold stays in its
                // sentence. A row break before a run is a space when the row wrapped, a line break
                // ('\n') when the author ended it.
                let (paras, lines) = pdfcraft_edit::reading_paragraphs(cos, i).unwrap_or_default();
                let mut blocks = Vec::new();
                let mut pid = 0usize;
                for tb in paras.iter() {
                    pid += 1;
                    let breaks: Vec<usize> = tb.editor_text.match_indices('\n').map(|(k, _)| k).collect();
                    // A row further below than the paragraph's usual line pitch (by a quarter, or 1.6
                    // lines when there is no usual one) starts a new paragraph: Word's space between
                    // paragraphs that the grouping joined.
                    let tops: Vec<f64> = tb.rows.iter().map(|r| r.iter().filter_map(|i| lines.get(*i)).map(|l| l.rect[3]).fold(f64::MIN, f64::max)).collect();
                    let pitches: Vec<f64> = tops.windows(2).map(|w| w[0] - w[1]).collect();
                    let mut sorted = pitches.clone();
                    sorted.sort_by(|a, b| a.total_cmp(b));
                    let usual = (sorted.len() >= 2).then(|| sorted[sorted.len() / 2]);
                    let split: Vec<bool> = pitches.iter().map(|p| match usual { Some(u) => *p > u * 1.25, None => *p > tb.size * 1.6 }).collect();
                    let mut prev_end: Option<usize> = None;
                    let mut prev_hyphen = false;
                    for (range, li) in &tb.spans {
                        let Some(l) = lines.get(*li) else { continue };
                        let Some(piece) = tb.editor_text.get(range.clone()) else { continue };
                        let mut text = String::new();
                        if let Some(pe) = prev_end {
                            let between = tb.editor_text.get(pe..range.start).unwrap_or_default();
                            let at = breaks.iter().position(|&k| k >= pe && k < range.start);
                            let new_para = at.is_some_and(|k| split.get(k).copied().unwrap_or(false));
                            if new_para {
                                pid += 1;
                            }
                            match at {
                                _ if new_para => {}
                                Some(k) if !tb.soft_breaks.get(k).copied().unwrap_or(true) => text.push('\n'),
                                Some(_) if !prev_hyphen => text.push(' '),
                                Some(_) => {}
                                None if between.chars().any(char::is_whitespace) => text.push(' '),
                                None => {}
                            }
                        }
                        text.push_str(piece);
                        prev_end = Some(range.end);
                        prev_hyphen = piece.ends_with('-');
                        if text.trim().is_empty() && !text.contains('\n') {
                            continue;
                        }
                        let f = l.base_font.to_ascii_lowercase();
                        // Symbol fonts' characters (Word's arrows and bullets in Wingdings or Symbol,
                        // read as private-use code points) as the Unicode characters they draw.
                        let (text, symbols_mapped) = pdfcraft_edit::unicode_symbols(&text, &f);
                        // The family Word knows it by: no subset tag, no style suffix.
                        let family = l.base_font.rsplit('+').next().unwrap_or_default();
                        let family = family.split([',', '-']).next().unwrap_or_default().trim_matches(|c: char| c == '*' || c.is_whitespace()).to_string();
                        blocks.push(pdfcraft_export::Block {
                            font: if symbols_mapped { String::new() } else { family },
                            para: pid,
                            text,
                            rect: visible_rect(l.rect),
                            size: l.size * unit,
                            bold: l.bold || f.contains("bold") || f.contains("black") || f.contains("heavy"),
                            italic: l.italic || f.contains("italic") || f.contains("oblique"),
                            color: l.color,
                            rotation: l.rotation() - f64::from(info.rotation),
                        });
                    }
                }
                let images = pdfcraft_edit::reading_images(cos, i)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|im| {
                        let (ext, bytes) = pdfcraft_create::image_file(cos, im.object?).ok()?;
                        Some(pdfcraft_export::Image {
                            ext: if ext == "jpg" { "jpg" } else { "png" },
                            bytes,
                            rect: visible_rect(im.rect),
                            size: [im.matrix[0].hypot(im.matrix[1]) * unit, im.matrix[2].hypot(im.matrix[3]) * unit],
                            rotation: im.matrix[1].atan2(im.matrix[0]).to_degrees() - f64::from(info.rotation),
                        })
                    })
                    .collect();
                // Sheaf: the page's straight horizontal and vertical lines (table rules, cell borders,
                // box edges), for ruled tables; mapped as the blocks are.
                let rules = crate::measure::snap::geometry(cos, i)
                    .map(|g| g.segments)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|[a, b]| visible_rect([a[0], a[1], b[0], b[1]]))
                    .filter(|r| (r[2] - r[0]).abs() < 0.5 || (r[3] - r[1]).abs() < 0.5)
                    .collect();
                // Sheaf: filled rectangles and their colour, for cell shading.
                let fills = pdfcraft_edit::page_fills(cos, i).unwrap_or_default().into_iter().map(|(r, c)| (visible_rect(r), c)).collect();
                pdfcraft_export::Page { width: info.width as f64, height: info.height as f64, blocks, images, rules, fills }
            })
            .collect()
    }

    /// The document as a Word, PowerPoint, HTML or RTF file.
    pub fn export_office(&self, format: OfficeFormat) -> Vec<u8> {
        let pages = self.export_pages();
        let title = self.info.title.clone().unwrap_or_else(|| self.name.trim_end_matches(".pdf").to_string());
        match format {
            OfficeFormat::Docx => pdfcraft_export::docx(&pages, &title),
            OfficeFormat::Pptx => pdfcraft_export::pptx(&pages, &title),
            OfficeFormat::Html => pdfcraft_export::html(&pages, &title).into_bytes(),
            OfficeFormat::Rtf => pdfcraft_export::rtf(&pages).into_bytes(),
        }
    }
}
