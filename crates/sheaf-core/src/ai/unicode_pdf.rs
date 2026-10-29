//! Text in any script set as a PDF (PDF Expert's translated PDF keeps the target language's
//! script): each character drawn with the first face that has it, among the bundled craft fonts
//! (Japanese, Arabic, Thai, Hebrew…) and the computer's open-licensed fonts (DejaVu, Noto,
//! Liberation…), each face embedded whole as a CID font with a ToUnicode map, so the text stays
//! searchable and copyable. Arabic and Thai are shaped by PdfCraft's shaper; Arabic and Hebrew
//! lines run right to left. Bold text (slide titles) takes a bold face when one draws the
//! character (BIZ UDPGothic Bold for Japanese, Cyrillic and Greek; the system's DejaVu or Noto
//! Bold), else the regular face stroked as well as filled (synthetic bold, named `Face,Bold`).

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
    /// A bold face (used for bold text only).
    bold: bool,
}

impl Face {
    fn font(&self) -> Option<FontRef<'_>> {
        FontRef::new(&self.bytes).ok()
    }
}

fn face_of(name: &str, bytes: Vec<u8>) -> Option<Face> {
    let f = FontRef::new(&bytes).ok()?;
    let cff = skrifa::raw::TableProvider::glyf(&f).is_err();
    let name = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>();
    Some(Face { name: if name.is_empty() { "Font".into() } else { name }, cff, bytes: std::sync::Arc::new(bytes), bold: false })
}

/// The bold faces tried for bold text, in order: the common Latin families' Bold (DejaVu Sans
/// Bold…), the bundled craft fonts' Bold faces, then the system's other plain Bold faces.
fn bold_faces() -> Vec<Face> {
    let mut system: Vec<&crate::system_fonts::SystemFont> = crate::system_fonts::installed()
        .iter()
        .filter(|f| f.why_not.is_none() && f.name == format!("{} Bold", f.family) && !f.path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttc")))
        .collect();
    let rank = |f: &&crate::system_fonts::SystemFont| {
        let n = f.family.to_ascii_lowercase();
        ["dejavu sans", "liberation sans", "noto sans", "free sans", "freesans"].iter().position(|k| n == *k).unwrap_or(9)
    };
    system.sort_by_key(rank);
    let read = |f: &&crate::system_fonts::SystemFont| std::fs::read(&f.path).ok().and_then(|b| face_of(&format!("{}-Bold", f.family), b));
    let mut out: Vec<Face> = system.iter().take_while(|f| rank(f) < 9).filter_map(read).collect();
    for c in pdfcraft_fonts::CRAFT_FONTS.iter().filter(|c| c.style.eq_ignore_ascii_case("bold")) {
        out.extend(face_of(&format!("{}-Bold", c.family), c.bytes.to_vec()));
    }
    out.extend(system.iter().filter(|f| rank(f) == 9).filter_map(read));
    for f in &mut out {
        f.bold = true;
    }
    out
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

/// One glyph placed: face, glyph id, x and y offset from the pen, the text it shows, and whether
/// it is bold drawn from a regular face (stroked as well as filled).
#[derive(Clone)]
struct Glyph {
    face: usize,
    fake_bold: bool,
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
    /// Whether the bold faces were added to `faces` (the first time bold text is set).
    bold_loaded: bool,
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
    fn glyphs(&mut self, text: &str, size: f64, bold: bool) -> std::result::Result<Vec<Glyph>, char> {
        if bold && !self.bold_loaded {
            self.faces.extend(bold_faces());
            self.bold_loaded = true;
        }
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
                            part.push(Glyph { face: fi, fake_bold: bold, gid: *gid as u16, advance: if k + 1 == n { cl.advance * size } else { 0.0 }, dx: dx * size, dy: dy * size, text: if k == 0 { cl.text.clone() } else { String::new() } });
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
                let sp = self.simple(' ', size, bold).unwrap_or(Glyph { face: 0, fake_bold: false, gid: 0, advance: size * 0.3, dx: 0.0, dy: 0.0, text: String::new() });
                out.push(Glyph { text: " ".into(), ..sp });
            } else {
                out.push(self.simple(c, size, bold).ok_or(c)?);
            }
            i += 1;
        }
        Ok(out)
    }

    /// `c` from the first face that has it: for bold text a bold face, else a regular one drawn
    /// with synthetic bold.
    fn simple(&self, c: char, size: f64, bold: bool) -> Option<Glyph> {
        let found = |want_bold: bool| {
            self.faces.iter().enumerate().filter(|(_, f)| f.bold == want_bold).find_map(|(i, f)| {
                let font = f.font()?;
                let gid = font.charmap().map(c)?;
                if gid.to_u32() == 0 || gid.to_u32() > u16::MAX as u32 {
                    return None;
                }
                let upm = font.metrics(Size::unscaled(), LocationRef::default()).units_per_em.max(1) as f64;
                let adv = font.glyph_metrics(Size::unscaled(), LocationRef::default()).advance_width(gid).unwrap_or(0.0) as f64 / upm;
                Some(Glyph { face: i, fake_bold: false, gid: gid.to_u32() as u16, advance: adv * size, dx: 0.0, dy: 0.0, text: c.to_string() })
            })
        };
        match bold {
            true => found(true).or_else(|| found(false).map(|g| Glyph { fake_bold: true, ..g })),
            false => found(false),
        }
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

/// The display order of a right-to-left line's glyphs, given each glyph's direction class: `R`
/// right to left, `L` left to right (Latin letters, digits), `N` neutral (spaces, punctuation).
/// Runs of `L` read left to right inside the line, and the neutrals between two `L` (the point
/// of 12.50, the colon of 10:30, the space of "New York") belong to that run, so a number keeps
/// its value (Unicode's bidi rules W4-W7 and N1, simplified for a line whose base is RTL).
pub fn visual_order(classes: &[u8]) -> Vec<usize> {
    let n = classes.len();
    let mut cls = classes.to_vec();
    // Neutrals between two L are L.
    let mut i = 0;
    while i < n {
        if cls[i] == b'N' {
            let mut j = i;
            while j < n && cls[j] == b'N' {
                j += 1;
            }
            if i > 0 && j < n && cls[i - 1] == b'L' && cls[j] == b'L' {
                for c in cls.iter_mut().take(j).skip(i) {
                    *c = b'L';
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    // Logical runs, reversed; inside an L run the order is kept.
    let mut runs: Vec<(usize, usize, bool)> = Vec::new();
    let mut k = 0;
    while k < n {
        let l = cls[k] == b'L';
        let mut j = k + 1;
        while j < n && (cls[j] == b'L') == l && l {
            j += 1;
        }
        runs.push((k, j, l));
        k = j;
    }
    runs.iter().rev().flat_map(|&(a, b, _)| a..b).collect()
}

fn class_of(g: &Glyph) -> u8 {
    match g.text.chars().next() {
        Some(c) if rtl_char(c) => b'R',
        Some(c) if c.is_alphanumeric() => b'L',
        // Shaped Arabic glyphs after the first of a cluster carry no text: part of the R run.
        None => b'R',
        _ => b'N',
    }
}

/// A line laid out: its glyphs (logical order) and whether it reads right to left.
pub struct Line {
    glyphs: Vec<Glyph>,
    pub rtl: bool,
}

impl Line {
    pub fn width(&self) -> f64 {
        self.glyphs.iter().map(|g| g.advance).sum()
    }
}

/// Sets text in any script on PDF pages: lines laid out with [`Setter::lines`], drawn into a
/// content stream with [`Setter::draw`], the faces used embedded by [`Setter::fonts`] (as `/F0`,
/// `/F1`… in a page's font resources).
pub struct Setter {
    sh: Shaper,
    /// Glyphs drawn, by face and synthetic bold (a `Face,Bold` font of its own).
    used: BTreeMap<(usize, bool), BTreeMap<u16, (f64, String)>>,
}

impl Default for Setter {
    fn default() -> Self {
        Self::new()
    }
}

impl Setter {
    pub fn new() -> Setter {
        Setter { sh: Shaper { faces: faces(), shaped: BTreeMap::new(), bold_loaded: false }, used: BTreeMap::new() }
    }

    /// `text`'s paragraphs at `size`, wrapped to `width`, bold or not. Err names a character no
    /// face draws.
    pub fn lines(&mut self, text: &str, size: f64, width: f64, bold: bool) -> Result<Vec<Line>> {
        let mut out = Vec::new();
        for para in text.replace("\r\n", "\n").split('\n') {
            let rtl = para.chars().find(|c| c.is_alphabetic()).is_some_and(rtl_char);
            let glyphs = self.sh.glyphs(para, size, bold).map_err(|c| OpdfError::Unsupported(format!("no font on this computer draws “{c}” (U+{:04X}); save the text as a .txt file instead", c as u32)))?;
            for l in wrap(glyphs, width) {
                out.push(Line { glyphs: l, rtl });
            }
        }
        Ok(out)
    }

    /// Content-stream operators (inside BT … ET) drawing `line` at `size` on baseline `y`, from
    /// `left` (a right-to-left line ends at `right`).
    pub fn draw(&mut self, line: &Line, size: f64, left: f64, right: f64, y: f64) -> String {
        let order: Vec<usize> = if line.rtl { visual_order(&line.glyphs.iter().map(class_of).collect::<Vec<_>>()) } else { (0..line.glyphs.len()).collect() };
        let mut x = if line.rtl { right - line.width() } else { left };
        let mut c = String::new();
        for i in order {
            let g = &line.glyphs[i];
            if g.gid != 0 && g.text != " " || g.text.is_empty() {
                let font = self.sh.faces[g.face].font();
                let upm = font.as_ref().map_or(1000.0, |f| f.metrics(Size::unscaled(), LocationRef::default()).units_per_em.max(1) as f64);
                let adv = font.as_ref().and_then(|f| f.glyph_metrics(Size::unscaled(), LocationRef::default()).advance_width(GlyphId::new(g.gid as u32))).unwrap_or(0.0) as f64 / upm * 1000.0;
                let e = self.used.entry((g.face, g.fake_bold)).or_default().entry(g.gid).or_insert((adv, String::new()));
                if e.1.is_empty() {
                    e.1 = g.text.clone();
                }
                if g.fake_bold {
                    // Filled and stroked (in the stroke colour the caller sets to the fill's): synthetic bold.
                    c.push_str(&format!("/B{} {size} Tf 2 Tr {:.2} w 1 0 0 1 {:.2} {:.2} Tm <{:04X}> Tj 0 Tr\n", g.face, size * 0.035, x + g.dx, y + g.dy, g.gid));
                } else {
                    c.push_str(&format!("/F{} {size} Tf 1 0 0 1 {:.2} {:.2} Tm <{:04X}> Tj\n", g.face, x + g.dx, y + g.dy, g.gid));
                }
            }
            x += g.advance;
        }
        c
    }

    /// The faces drawn with, embedded in `doc`: the font resource dictionary.
    pub fn fonts(&self, doc: &mut Document) -> Result<lopdf::Dictionary> {
        let mut fonts = lopdf::Dictionary::new();
        // A face drawn both plain and with synthetic bold is embedded once.
        let mut files: BTreeMap<usize, lopdf::ObjectId> = BTreeMap::new();
        for ((fi, fake_bold), glyphs) in &self.used {
            let f = &self.sh.faces[*fi];
            let font = f.font().ok_or_else(|| OpdfError::Engine("font unreadable".into()))?;
            let m = font.metrics(Size::unscaled(), LocationRef::default());
            let scale = 1000.0 / m.units_per_em.max(1) as f64;
            let file = match files.get(fi) {
                Some(id) => *id,
                None => {
                    let mut file = Stream::new(dictionary! { "Length1" => f.bytes.len() as i64 }, f.bytes.to_vec());
                    if f.cff {
                        file.dict.set("Subtype", "OpenType");
                    }
                    let _ = file.compress();
                    let id = doc.add_object(file);
                    files.insert(*fi, id);
                    id
                }
            };
            // Synthetic bold is named as Acrobat names it ("Face,Bold") and flagged ForceBold.
            let name = if *fake_bold { format!("{},Bold", f.name) } else { f.name.clone() };
            let b = m.bounds.map(|b| [b.x_min, b.y_min, b.x_max, b.y_max]).unwrap_or([0.0, -200.0, 1000.0, 900.0]);
            let mut desc = dictionary! {
                "Type" => "FontDescriptor", "FontName" => Object::Name(name.clone().into_bytes()), "Flags" => if *fake_bold { 4 | 1 << 18 } else { 4 },
                "FontBBox" => b.iter().map(|v| Object::Real(*v * scale as f32)).collect::<Vec<_>>(),
                "ItalicAngle" => 0, "Ascent" => Object::Real(m.ascent * scale as f32), "Descent" => Object::Real(m.descent * scale as f32),
                "CapHeight" => Object::Real(m.cap_height.unwrap_or(m.ascent) * scale as f32), "StemV" => 80,
            };
            desc.set(if f.cff { "FontFile3" } else { "FontFile2" }, file);
            let desc = doc.add_object(desc);
            let widths: Vec<Object> = glyphs.iter().flat_map(|(gid, (adv, _))| [Object::Integer(*gid as i64), Object::Array(vec![Object::Real(*adv as f32)])]).collect();
            let mut cid = dictionary! {
                "Type" => "Font", "Subtype" => if f.cff { "CIDFontType0" } else { "CIDFontType2" }, "BaseFont" => Object::Name(name.clone().into_bytes()),
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
                "Type" => "Font", "Subtype" => "Type0", "BaseFont" => Object::Name(name.into_bytes()), "Encoding" => "Identity-H",
                "DescendantFonts" => vec![cid.into()], "ToUnicode" => to_unicode,
            });
            fonts.set(format!("{}{fi}", if *fake_bold { "B" } else { "F" }), type0);
        }
        Ok(fonts)
    }
}

/// `text` as a PDF (US Letter, 11 pt, 1-inch margins, `title` in its info). Err when some
/// character has no face on this computer, naming it.
pub fn text_pdf(title: &str, text: &str) -> Result<Vec<u8>> {
    let (w, h, margin, size) = (612.0, 792.0, 72.0, 11.0);
    let line_h = size * 1.45;
    let mut set = Setter::new();
    let lines = set.lines(text, size, w - 2.0 * margin, false)?;
    let per_page = (((h - 2.0 * margin) / line_h).floor() as usize).max(1);
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut contents = Vec::new();
    for page in lines.chunks(per_page) {
        let mut c = String::from("BT\n");
        for (k, line) in page.iter().enumerate() {
            c.push_str(&set.draw(line, size, margin, w - margin, h - margin - size - k as f64 * line_h));
        }
        c.push_str("ET\n");
        contents.push(c);
    }
    if contents.is_empty() {
        contents.push(String::from("BT ET\n"));
    }
    let fonts = set.fonts(&mut doc)?;
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

    /// "السعر 12.50 $": the number keeps its digits' order and its point (Sol, round 31: 50.12).
    #[test]
    fn numbers_in_a_right_to_left_line_keep_their_order() {
        // logical: R R _ 1 2 . 5 0 _ R   (a word, a space, 12.50, a space, a word)
        let classes = b"RRNLLNLLNR";
        let order = visual_order(classes);
        assert_eq!(order, vec![9, 8, 3, 4, 5, 6, 7, 2, 1, 0]);
        // Two numbers apart stay apart and each keeps its order.
        assert_eq!(visual_order(b"LLNRNLL"), vec![5, 6, 4, 3, 2, 0, 1]);
    }

    #[test]
    fn right_to_left_lines_are_detected_by_their_first_letter() {
        assert!(rtl_char('ש') && rtl_char('ب') && !rtl_char('a') && !rtl_char('東'));
        assert!(arabic_char('ب') && !arabic_char('ש'));
    }

    #[test]
    fn long_lines_wrap_at_spaces_and_text_without_spaces_anywhere() {
        let g = |t: &str| Glyph { face: 0, fake_bold: false, gid: 1, advance: 10.0, dx: 0.0, dy: 0.0, text: t.into() };
        let words: Vec<Glyph> = "ab cd ef".chars().map(|c| g(&c.to_string())).collect();
        let lines = wrap(words, 55.0);
        let text: Vec<String> = lines.iter().map(|l| l.iter().map(|g| g.text.as_str()).collect()).collect();
        assert_eq!(text, ["ab cd", "ef"]);
        let cjk: Vec<Glyph> = (0..10).map(|_| g("東")).collect();
        assert_eq!(wrap(cjk, 55.0).iter().map(Vec::len).collect::<Vec<_>>(), [5, 5]);
    }
}
