//! Acrobat's Generate presentation: the outline an AI wrote (`# Title` then `- ` points) set as
//! 16:9 slides in a PDF, or a PowerPoint file through Sheaf's own PDF to PowerPoint export (one
//! slide per page). Helvetica in WinAnsi: a character it cannot draw is written as `?`.

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

/// The slides as a PDF: a title slide (the first) centred large, then a title band and points.
pub fn pdf(title: &str, slides: &[Slide]) -> Result<Vec<u8>> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let regular = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let bold = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding" });
    let mut kids = Vec::new();
    let fallback = [Slide { title: title.to_string(), points: Vec::new() }];
    let slides = if slides.is_empty() { &fallback[..] } else { slides };
    for (n, s) in slides.iter().enumerate() {
        let mut c: Vec<u8> = Vec::new();
        let text = |c: &mut Vec<u8>, font: &str, size: f64, x: f64, y: f64, rgb: &str, t: &str| {
            c.extend(format!("BT /{font} {size} Tf {rgb} rg {x:.1} {y:.1} Td ").into_bytes());
            c.extend(literal(&win_ansi(t)));
            c.extend_from_slice(b" Tj ET\n");
        };
        if n == 0 {
            c.extend_from_slice(b"0.13 0.17 0.29 rg 0 0 960 540 re f\n");
            let lines = wrap(&s.title, 40.0, W - 160.0);
            let mut y = H / 2.0 + 20.0 + (lines.len() as f64 - 1.0) * 24.0;
            for l in &lines {
                text(&mut c, "F2", 40.0, 80.0, y, "1 1 1", l);
                y -= 48.0;
            }
            for p in s.points.iter().take(3) {
                for l in wrap(p, 20.0, W - 160.0) {
                    text(&mut c, "F1", 20.0, 80.0, y, "0.8 0.85 0.95", &l);
                    y -= 28.0;
                }
            }
        } else {
            c.extend_from_slice(b"0.13 0.17 0.29 rg 0 450 960 90 re f\n");
            let title = wrap(&s.title, 30.0, W - 120.0);
            text(&mut c, "F2", 30.0, 60.0, 485.0, "1 1 1", title.first().map_or("", String::as_str));
            let mut y = 400.0;
            for p in &s.points {
                let lines = wrap(p, 22.0, W - 160.0);
                for (i, l) in lines.iter().enumerate() {
                    if y < 40.0 {
                        break;
                    }
                    if i == 0 {
                        c.extend(format!("0.85 0.2 0.2 rg 66 {:.1} 7 7 re f\n", y + 4.0).into_bytes());
                    }
                    text(&mut c, "F1", 22.0, 90.0, y, "0.15 0.15 0.15", l);
                    y -= 30.0;
                }
                y -= 10.0;
            }
            text(&mut c, "F1", 11.0, W - 60.0, 20.0, "0.5 0.5 0.5", &format!("{}", n + 1));
        }
        let content = doc.add_object(Stream::new(dictionary! {}, c));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => regular, "F2" => bold } },
        });
        kids.push(Object::from(page));
    }
    let count = kids.len() as i64;
    doc.objects.insert(pages_id, dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count, "MediaBox" => vec![0.into(), 0.into(), W.into(), H.into()] }.into());
    let info = doc.add_object(dictionary! { "Title" => Object::string_literal(win_ansi(title)), "Producer" => "Sheaf" });
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
    fn unmappable_characters_are_question_marks() {
        assert_eq!(win_ansi("café 東京 (x)"), b"caf\xe9 ?? (x)".to_vec());
    }
}
