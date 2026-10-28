//! Text in any script set as a PDF (PDF Expert's translated PDF keeps the target language's
//! script): each character drawn with the first face that has it, among the bundled craft fonts
//! (Japanese, Arabic, Thai, Hebrew…) and the computer's open-licensed fonts (DejaVu, Noto,
//! Liberation…), each face embedded whole as a CID font with a ToUnicode map, so the text stays
//! searchable and copyable. Arabic and Thai are shaped by PdfCraft's shaper; Arabic and Hebrew
//! lines run right to left.

use crate::{OpdfError, Result};
use lopdf::{dictionary, Document, Object, Stream};
use pdfcraft_fonts::ShapedScript;
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::collections::BTreeMap;

/// A face that can be embedded: its bytes and whether its outlines are CFF (OpenType) rather than
/// TrueType.
struct Face {
    name: String,
    bytes: std::sync::Arc<Vec<u8>>,
    cff: bool,
}

impl Face {
    fn font(&self) -> Option<FontRef<'_>> {
        FontRef::new(&self.bytes).ok()
    }
}

fn face_of(name: &str, bytes: Vec<u8>) -> Option<Face> {
    let f = FontRef::new(&bytes).ok()?;
    let cff = skrifa::raw::TableProvider::glyf(&f).is_err();
    let name = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
    Some(Face { name: if name.is_empty() { "Font".into() } else { name }, cff, bytes: std::sync::Arc::new(bytes) })
}

/// The faces tried, in order: bundled craft fonts, then the system's embeddable fonts (common
/// Latin faces first, so Latin text keeps a familiar look). Collections (.ttc) are skipped.
fn faces() -> Vec<Face> {
    let mut out: Vec<Face> = Vec::new();
    let mut system: Vec<&crate::system_fonts::SystemFont> = crate::system_fonts::installed()
        .iter()
        .filter(|f| f.why_not.is_none() && !f.bold && !f.italic && !f.path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttc")))
        .collect();
    let rank = |f: &&crate::system_fonts::SystemFont| {
        let n = f.family.to_ascii_lowercase();
        ["dejavu sans", "liberation sans", "noto sans", "free sans", "freesans"].iter().position(|k| n == *k).unwrap_or(9)
    };
    system.sort_by_key(rank);
    for f in system.iter().take_while(|f| rank(f) < 9) {
        out.extend(std::fs::read(&f.path).ok().and_then(|b| face_of(&f.family, b)));
    }
    for c in pdfcraft_fonts::CRAFT_FONTS.iter().filter(|c| c.style.eq_ignore_ascii_case("regular")) {
        out.extend(face_of(c.family, c.bytes.to_vec()));
    }
    for f in system.iter().filter(|f| rank(f) == 9) {
        out.extend(std::fs::read(&f.path).ok().and_then(|b| face_of(&f.family, b)));
    }
    out
}

/// One glyph placed: face, glyph id, x and y offset from the pen, the text it shows.
#[derive(Clone)]
struct Glyph {
    face: usize,
    gid: u16,
    advance: f64,
    dy: f64,
    dx: f64,
    text: String,
}

struct Shaper {
    faces: Vec<Face>,
    /// The face of PdfCraft's Arabic and Thai shapers, added to `faces` when used.
    shaped: BTreeMap<u8, usize>,
}

fn rtl_char(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

fn arabic_char(c: char) -> bool {
    matches!(c as u32, 0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF)
}

fn thai_char(c: char) -> bool {
    matches!(c as u32, 0x0E00..=0x0E7F)
}

impl Shaper {
    fn shaped_face(&mut self, s: ShapedScript) -> Option<usize> {
        let key = match s {
            ShapedScript::Arabic => 0,
            ShapedScript::Thai => 1,
        };
        if let Some(i) = self.shaped.get(&key) {
            return Some(*i);
        }
        let f = face_of(s.font()?.family, s.font()?.bytes.to_vec())?;
        self.faces.push(f);
        self.shaped.insert(key, self.faces.len() - 1);
        Some(self.faces.len() - 1)
    }

    /// The glyphs of `text` at `size`, in logical order (Arabic runs come back in visual order:
    /// they are reversed again when the line is laid out right to left). Err names a character no
    /// face has.
    fn glyphs(&mut self, text: &str, size: f64) -> std::result::Result<Vec<Glyph>, char> {
        let mut out = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let script = if arabic_char(c) { Some(ShapedScript::Arabic) } else if thai_char(c) { Some(ShapedScript::Thai) } else { None };
            if let Some(s) = script.filter(|s| s.font().is_some()) {
                let mut j = i;
                while j < chars.len() && (if s == ShapedScript::Arabic { arabic_char(chars[j]) } else { thai_char(chars[j]) }) {
                    j += 1;
                }
                let run: String = chars[i..j].iter().collect();
                let clusters = match s {
                    ShapedScript::Arabic => pdfcraft_fonts::shape_arabic(&run, true),
                    ShapedScript::Thai => pdfcraft_fonts::shape_thai(&run),
                };
                if let (Ok(clusters), Some(fi)) = (clusters, self.shaped_face(s)) {
                    let mut part = Vec::new();
                    for cl in clusters {
                        let n = cl.glyphs.len();
                        for (k, (gid, [dx, dy])) in cl.glyphs.iter().enumerate() {
                            part.push(Glyph { face: fi, gid: *gid as u16, advance: if k + 1 == n { cl.advance * size } else { 0.0 }, dx: dx * size, dy: dy * size, text: if k == 0 { cl.text.clone() } else { String::new() } });
                        }
                    }
                    // Shaped right to left: back to logical order, so the line's bidi pass applies once.
                    if s == ShapedScript::Arabic {
                        part.reverse();
                    }
                    out.extend(part);
                    i = j;
                    continue;
                }
            }
            if c.is_whitespace() || c.is_control() {
                let sp = self.simple(' ', size).unwrap_or(Glyph { face: 0, gid: 0, advance: size * 0.3, dx: 0.0, dy: 0.0, text: String::new() });
                out.push(Glyph { text: " ".into(), ..sp });
            } else {
                out.push(self.simple(c, size).ok_or(c)?);
            }
            i += 1;
        }
        Ok(out)
    }

    fn simple(&self, c: char, size: f64) -> Option<Glyph> {
        self.faces.iter().enumerate().find_map(|(i, f)| {
            let font = f.font()?;
            let gid = font.charmap().map(c)?;
            if gid.to_u32() == 0 || gid.to_u32() > u16::MAX as u32 {
                return None;
            }
            let upm = font.metrics(Size::unscaled(), LocationRef::default()).units_per_em.max(1) as f64;
            let adv = font.glyph_metrics(Size::unscaled(), LocationRef::default()).advance_width(gid).unwrap_or(0.0) as f64 / upm;
            Some(Glyph { face: i, gid: gid.to_u32() as u16, advance: adv * size, dx: 0.0, dy: 0.0, text: c.to_string() })
        })
    }
}

/// Lines of glyphs no wider than `width`, broken at spaces (anywhere for text without spaces,
/// such as Chinese and Japanese).
fn wrap(glyphs: Vec<Glyph>, width: f64) -> Vec<Vec<Glyph>> {
    let mut lines = Vec::new();
    let mut cur: Vec<Glyph> = Vec::new();
    let mut w = 0.0;
    let mut last_space: Option<usize> = None;
    for g in glyphs {
        if g.text == " " {
            last_space = Some(cur.len());
        }
        w += g.advance;
        cur.push(g);
        if w > width && cur.len() > 1 {
            let at = last_space.filter(|&s| s > 0).unwrap_or(cur.len() - 1);
            let rest: Vec<Glyph> = cur.split_off(at);
            let rest: Vec<Glyph> = rest.into_iter().skip_while(|g| g.text == " ").collect();
            lines.push(std::mem::take(&mut cur));
            w = rest.iter().map(|g| g.advance).sum();
            cur = rest;
            last_space = None;
        }
    }
    lines.push(cur);
    lines
}

/// `text` as a PDF (US Letter, 11 pt, 1-inch margins, `title` in its info). Err when some
/// character has no face on this computer, naming it.
pub fn text_pdf(title: &str, text: &str) -> Result<Vec<u8>> {
    let (w, h, margin, size) = (612.0, 792.0, 72.0, 11.0);
    let line_h = size * 1.45;
    let mut sh = Shaper { faces: faces(), shaped: BTreeMap::new() };
    let mut lines: Vec<(Vec<Glyph>, bool)> = Vec::new();
    for para in text.replace("\r\n", "\n").split('\n') {
        let rtl = para.chars().find(|c| c.is_alphabetic()).is_some_and(rtl_char);
        let glyphs = sh.glyphs(para, size).map_err(|c| OpdfError::Unsupported(format!("no font on this computer draws “{c}” (U+{:04X}); save the text as a .txt file instead", c as u32)))?;
        for l in wrap(glyphs, w - 2.0 * margin) {
            lines.push((l, rtl));
        }
    }
    let per_page = (((h - 2.0 * margin) / line_h).floor() as usize).max(1);
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    // Glyphs used per face (for widths and ToUnicode).
    let mut used: BTreeMap<usize, BTreeMap<u16, (f64, String)>> = BTreeMap::new();
    let mut contents = Vec::new();
    for page in lines.chunks(per_page) {
        let mut c = String::from("BT\n");
        for (k, (line, rtl)) in page.iter().enumerate() {
            let y = h - margin - size - k as f64 * line_h;
            // Right to left: the line's glyphs from the right margin leftwards, in reverse order;
            // runs of left-to-right text (numbers, Latin words) keep their own order.
            let order: Vec<&Glyph> = if *rtl {
                let mut v: Vec<&Glyph> = Vec::new();
                let mut ltr: Vec<&Glyph> = Vec::new();
                for g in line.iter().rev() {
                    let strong_ltr = g.text.chars().next().is_some_and(|c| c.is_alphanumeric() && !rtl_char(c));
                    if strong_ltr {
                        ltr.push(g);
                    } else {
                        v.extend(ltr.drain(..).rev());
                        v.push(g);
                    }
                }
                v.extend(ltr.drain(..).rev());
                v
            } else {
                line.iter().collect()
            };
            let total: f64 = line.iter().map(|g| g.advance).sum();
            let mut x = if *rtl { w - margin - total } else { margin };
            for g in order {
                if g.gid != 0 && g.text != " " || g.text.is_empty() {
                    let font = sh.faces[g.face].font();
                    let upm = font.as_ref().map_or(1000.0, |f| f.metrics(Size::unscaled(), LocationRef::default()).units_per_em.max(1) as f64);
                    let adv = font.as_ref().and_then(|f| f.glyph_metrics(Size::unscaled(), LocationRef::default()).advance_width(GlyphId::new(g.gid as u32))).unwrap_or(0.0) as f64 / upm * 1000.0;
                    let e = used.entry(g.face).or_default().entry(g.gid).or_insert((adv, String::new()));
                    if e.1.is_empty() {
                        e.1 = g.text.clone();
                    }
                    c.push_str(&format!("/F{} {size} Tf 1 0 0 1 {:.2} {:.2} Tm <{:04X}> Tj\n", g.face, x + g.dx, y + g.dy, g.gid));
                }
                x += g.advance;
            }
        }
        c.push_str("ET\n");
        contents.push(c);
    }
    if contents.is_empty() {
        contents.push(String::from("BT ET\n"));
    }
    let mut fonts = lopdf::Dictionary::new();
    for (fi, glyphs) in &used {
        let f = &sh.faces[*fi];
        let font = f.font().ok_or_else(|| OpdfError::Engine("font unreadable".into()))?;
        let m = font.metrics(Size::unscaled(), LocationRef::default());
        let scale = 1000.0 / m.units_per_em.max(1) as f64;
        let mut file = Stream::new(dictionary! { "Length1" => f.bytes.len() as i64 }, f.bytes.to_vec());
        if f.cff {
            file.dict.set("Subtype", "OpenType");
        }
        let _ = file.compress();
        let file = doc.add_object(file);
        let b = m.bounds.map(|b| [b.x_min, b.y_min, b.x_max, b.y_max]).unwrap_or([0.0, -200.0, 1000.0, 900.0]);
        let mut desc = dictionary! {
            "Type" => "FontDescriptor", "FontName" => Object::Name(f.name.clone().into_bytes()), "Flags" => 4,
            "FontBBox" => b.iter().map(|v| Object::Real(*v * scale as f32)).collect::<Vec<_>>(),
            "ItalicAngle" => 0, "Ascent" => Object::Real(m.ascent * scale as f32), "Descent" => Object::Real(m.descent * scale as f32),
            "CapHeight" => Object::Real(m.cap_height.unwrap_or(m.ascent) * scale as f32), "StemV" => 80,
        };
        desc.set(if f.cff { "FontFile3" } else { "FontFile2" }, file);
        let desc = doc.add_object(desc);
        let widths: Vec<Object> = glyphs.iter().flat_map(|(gid, (adv, _))| [Object::Integer(*gid as i64), Object::Array(vec![Object::Real(*adv as f32)])]).collect();
        let mut cid = dictionary! {
            "Type" => "Font", "Subtype" => if f.cff { "CIDFontType0" } else { "CIDFontType2" }, "BaseFont" => Object::Name(f.name.clone().into_bytes()),
            "CIDSystemInfo" => dictionary! { "Registry" => Object::string_literal("Adobe"), "Ordering" => Object::string_literal("Identity"), "Supplement" => 0 },
            "FontDescriptor" => desc, "W" => widths, "DW" => 0,
        };
        if !f.cff {
            cid.set("CIDToGIDMap", "Identity");
        }
        let cid = doc.add_object(cid);
        let mut cmap = String::from("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CMapName /Sheaf-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
        let mapped: Vec<_> = glyphs.iter().filter(|(_, (_, t))| !t.is_empty()).collect();
        for chunk in mapped.chunks(100) {
            cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
            for (gid, (_, t)) in chunk {
                let hex: String = t.encode_utf16().map(|u| format!("{u:04X}")).collect();
                cmap.push_str(&format!("<{gid:04X}> <{hex}>\n"));
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        let to_unicode = doc.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
        let type0 = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type0", "BaseFont" => Object::Name(f.name.clone().into_bytes()), "Encoding" => "Identity-H",
            "DescendantFonts" => vec![cid.into()], "ToUnicode" => to_unicode,
        });
        fonts.set(format!("F{fi}"), type0);
    }
    let fonts = doc.add_object(fonts);
    let mut kids = Vec::new();
    for c in contents {
        let mut s = Stream::new(dictionary! {}, c.into_bytes());
        let _ = s.compress();
        let content = doc.add_object(s);
        kids.push(Object::from(doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content, "Resources" => dictionary! { "Font" => fonts } })));
    }
    let n = kids.len() as i64;
    doc.objects.insert(pages_id, dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => n, "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()] }.into());
    let title16: Vec<u8> = [0xFE, 0xFF].into_iter().chain(title.encode_utf16().flat_map(u16::to_be_bytes)).collect();
    let info = doc.add_object(dictionary! { "Title" => Object::String(title16, lopdf::StringFormat::Hexadecimal), "Producer" => Object::string_literal("Sheaf") });
    let root = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", root);
    doc.trailer.set("Info", info);
    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_to_left_lines_are_detected_by_their_first_letter() {
        assert!(rtl_char('ש') && rtl_char('ب') && !rtl_char('a') && !rtl_char('東'));
        assert!(arabic_char('ب') && !arabic_char('ש'));
    }

    #[test]
    fn long_lines_wrap_at_spaces_and_text_without_spaces_anywhere() {
        let g = |t: &str| Glyph { face: 0, gid: 1, advance: 10.0, dx: 0.0, dy: 0.0, text: t.into() };
        let words: Vec<Glyph> = "ab cd ef".chars().map(|c| g(&c.to_string())).collect();
        let lines = wrap(words, 55.0);
        let text: Vec<String> = lines.iter().map(|l| l.iter().map(|g| g.text.as_str()).collect()).collect();
        assert_eq!(text, ["ab cd", "ef"]);
        let cjk: Vec<Glyph> = (0..10).map(|_| g("東")).collect();
        assert_eq!(wrap(cjk, 55.0).iter().map(Vec::len).collect::<Vec<_>>(), [5, 5]);
    }
}
