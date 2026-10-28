//! Acrobat's Generate presentation: the outline an AI wrote (`# Title` then `- ` points) set as
//! 16:9 slides in a PDF, or a PowerPoint file through Sheaf's own PDF to PowerPoint export (one
//! slide per page). Western text in Helvetica; any other script in an embedded font that draws it.

use crate::export::{self, ExportOptions, Office};
use crate::pages::Input;
use crate::paths::atomic_write;
use crate::Result;
use lopdf::{dictionary, Document, Object, Stream};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Slide {
    pub title: String,
    pub points: Vec<String>,
}

/// The slides of an outline: a line starting `#` opens a slide, a line starting `-`, `*`, `•` or
/// a number adds a point; other text before the first title is ignored, after it is a point.
pub fn parse(outline: &str) -> Vec<Slide> {
    let mut slides: Vec<Slide> = Vec::new();
    for line in outline.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("```")) {
        if let Some(t) = line.strip_prefix('#') {
            let t = t.trim_start_matches('#').trim().trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ':').trim();
            let t = t.strip_prefix("Slide").map_or(t, |r| r.trim_start_matches(|c: char| c.is_ascii_digit() || c == ':' || c == ' ' || c == '-'));
            slides.push(Slide { title: t.trim().trim_matches('*').to_string(), points: Vec::new() });
        } else if let Some(s) = slides.last_mut() {
            let p = line.trim_start_matches(['-', '*', '•']).trim();
            let p = match p.split_once(". ") {
                Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest,
                _ => p,
            };
            if !p.is_empty() {
                s.points.push(p.replace("**", ""));
            }
        }
    }
    slides
}

const W: f64 = 960.0;
const H: f64 = 540.0;

fn win_ansi(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let mut buf = [0; 4];
        let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(ch.encode_utf8(&mut buf));
        if unmappable { out.push(b'?') } else { out.extend_from_slice(&bytes) }
    }
    out
}

fn literal(bytes: &[u8]) -> Vec<u8> {
    let mut o = vec![b'('];
    for &b in bytes {
        if matches!(b, b'(' | b')' | b'\\') {
            o.push(b'\\');
        }
        o.push(b);
    }
    o.push(b')');
    o
}

/// Lines of at most `width` points at `size` (Helvetica's average advance, half the size).
fn wrap(text: &str, size: f64, width: f64) -> Vec<String> {
    let per = ((width / (size * 0.5)).floor() as usize).max(8);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > per {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// How slide text is drawn: Helvetica for text it can draw (no font embedded), else the
/// any-script path ([`super::unicode_pdf::Setter`]: Japanese, Arabic… set in a font that has them).
enum Ink {
    Helvetica,
    Any(Box<super::unicode_pdf::Setter>),
}

impl Ink {
    /// Draws `text` wrapped to `right - x` from baseline `y` down, at most `max` lines `gap` apart;
    /// returns the baseline after the last line.
    #[allow(clippy::too_many_arguments)]
    fn para(&mut self, c: &mut Vec<u8>, bold: bool, size: f64, x: f64, right: f64, mut y: f64, gap: f64, rgb: &str, text: &str, max: usize, bullet: Option<f64>) -> Result<f64> {
        match self {
            Ink::Helvetica => {
                for (i, l) in wrap(text, size, right - x).iter().take(max).enumerate() {
                    if i == 0 && let Some(bx) = bullet {
                        c.extend(format!("0.85 0.2 0.2 rg {bx:.1} {:.1} 7 7 re f\n", y + 4.0).into_bytes());
                    }
                    c.extend(format!("BT /{} {size} Tf {rgb} rg {x:.1} {y:.1} Td ", if bold { "F2" } else { "F1" }).into_bytes());
                    c.extend(literal(&win_ansi(l)));
                    c.extend_from_slice(b" Tj ET\n");
                    y -= gap;
                }
            }
            Ink::Any(set) => {
                for (i, l) in set.lines(text, size, right - x)?.iter().take(max).enumerate() {
                    if i == 0 && let Some(bx) = bullet {
                        // A right-to-left point has its bullet on the right.
                        let bx = if l.rtl { right + (x - bx) - 7.0 } else { bx };
                        c.extend(format!("0.85 0.2 0.2 rg {bx:.1} {:.1} 7 7 re f\n", y + 4.0).into_bytes());
                    }
                    c.extend(format!("BT {rgb} rg\n").into_bytes());
                    c.extend(set.draw(l, size, x, right, y).into_bytes());
                    c.extend_from_slice(b"ET\n");
                    y -= gap;
                }
            }
        }
        Ok(y)
    }
}

/// The slides as a PDF: a title slide (the first) centred large, then a title band and points.
pub fn pdf(title: &str, slides: &[Slide]) -> Result<Vec<u8>> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let fallback = [Slide { title: title.to_string(), points: Vec::new() }];
    let slides = if slides.is_empty() { &fallback[..] } else { slides };
    let western = slides.iter().flat_map(|s| std::iter::once(&s.title).chain(&s.points)).all(|t| pdfcraft_fonts::first_undrawable(t).is_none());
    let mut ink = if western { Ink::Helvetica } else { Ink::Any(Box::default()) };
    let mut pages = Vec::new();
    for (n, s) in slides.iter().enumerate() {
        let mut c: Vec<u8> = Vec::new();
        if n == 0 {
            c.extend_from_slice(b"0.13 0.17 0.29 rg 0 0 960 540 re f\n");
            let y = ink.para(&mut c, true, 40.0, 80.0, W - 80.0, H / 2.0 + 44.0, 48.0, "1 1 1", &s.title, 3, None)?;
            let mut y = y - 8.0;
            for p in s.points.iter().take(3) {
                y = ink.para(&mut c, false, 20.0, 80.0, W - 80.0, y, 28.0, "0.8 0.85 0.95", p, 2, None)?;
            }
        } else {
            c.extend_from_slice(b"0.13 0.17 0.29 rg 0 450 960 90 re f\n");
            ink.para(&mut c, true, 30.0, 60.0, W - 60.0, 485.0, 36.0, "1 1 1", &s.title, 1, None)?;
            let mut y: f64 = 400.0;
            for p in &s.points {
                if y < 40.0 {
                    break;
                }
                let lines = ((y - 40.0) / 30.0).floor().max(1.0) as usize;
                y = ink.para(&mut c, false, 22.0, 90.0, W - 70.0, y, 30.0, "0.15 0.15 0.15", p, lines, Some(66.0))? - 10.0;
            }
            ink.para(&mut c, false, 11.0, W - 60.0, W - 20.0, 20.0, 12.0, "0.5 0.5 0.5", &format!("{}", n + 1), 1, None)?;
        }
        pages.push(c);
    }
    let fonts = match &ink {
        Ink::Helvetica => {
            let regular = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
            let bold = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding" });
            dictionary! { "F1" => regular, "F2" => bold }
        }
        Ink::Any(set) => set.fonts(&mut doc)?,
    };
    let fonts = doc.add_object(fonts);
    let mut kids = Vec::new();
    for c in pages {
        let content = doc.add_object(Stream::new(dictionary! {}, c));
        kids.push(Object::from(doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content, "Resources" => dictionary! { "Font" => fonts } })));
    }
    let count = kids.len() as i64;
    doc.objects.insert(pages_id, dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count, "MediaBox" => vec![0.into(), 0.into(), W.into(), H.into()] }.into());
    let title16: Vec<u8> = [0xFE, 0xFF].into_iter().chain(title.encode_utf16().flat_map(u16::to_be_bytes)).collect();
    let info = doc.add_object(dictionary! { "Title" => Object::String(title16, lopdf::StringFormat::Hexadecimal), "Producer" => "Sheaf" });
    let root = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", root);
    doc.trailer.set("Info", info);
    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(out)
}

/// The deck written at `out`: a PowerPoint file when it ends in `.pptx`, a PDF otherwise. Never
/// overwrites (the path written is returned).
pub fn write(title: &str, slides: &[Slide], out: &Path) -> Result<PathBuf> {
    let bytes = pdf(title, slides)?;
    if out.extension().is_some_and(|e| e.eq_ignore_ascii_case("pptx")) {
        let dir = tempfile::tempdir()?;
        let tmp = dir.path().join("slides.pdf");
        std::fs::write(&tmp, &bytes)?;
        return Ok(export::to_office(&Input::new(&tmp), Office::PowerPoint, &ExportOptions::default(), out, &mut |_| true)?.path);
    }
    atomic_write(out, |part| Ok(std::fs::write(part, &bytes)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_outline_becomes_slides() {
        let s = parse("Here is your deck:\n# Slide 1: Annual report\n- Sales up 12%\n\n## 2. Costs\n* Rent\n1. Staff\n**bold** remark\n```\n");
        assert_eq!(s, vec![
            Slide { title: "Annual report".into(), points: vec!["Sales up 12%".into()] },
            Slide { title: "Costs".into(), points: vec!["Rent".into(), "Staff".into(), "bold remark".into()] },
        ]);
    }

    #[test]
    fn western_text_is_written_in_win_ansi() {
        assert_eq!(win_ansi("café (x)"), b"caf\xe9 (x)".to_vec());
    }
}
