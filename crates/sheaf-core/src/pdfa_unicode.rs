//! iLovePDF's PDF/A-u/a targets require usable Unicode for every text code, not just a
//! ToUnicode entry's presence. Follow page, Form and annotation-appearance resources.
//! Unsupported CMaps are reported conservatively; this is not a complete ISO validator.

use super::PdfAIssue;
use lopdf::{content::Content, Dictionary, Document, Encoding, Object, ObjectId};
use std::{collections::{BTreeSet, HashMap}, rc::Rc};

const MAX_CONTENT: usize = 256 * 1024 * 1024;
const MAX_PAGE: usize = 16 * 1024 * 1024;
const MAX_CMAP: usize = 1024 * 1024;
const MAX_VISITS: usize = 100_000;

enum FontMap {
    Byte(Vec<Option<String>>, String),
    Cmap(Encoding<'static>, usize, String),
    Unknown(String),
}

pub(super) fn issues(pdf: &Document) -> Vec<PdfAIssue> {
    let mut scan = Scan { pdf, fonts: HashMap::new(), problems: BTreeSet::new(), bytes: 0, visits: 0 };
    for (number, id) in pdf.get_pages() {
        if scan.bytes > MAX_CONTENT || scan.visits > MAX_VISITS { break; }
        let resources = match pdf.get_page_resources(id) {
            Ok((direct, inherited)) => direct.cloned().or_else(|| inherited.first().and_then(|id| pdf.get_dictionary(*id).ok().cloned())).unwrap_or_default(),
            Err(_) => { scan.problem(format!("Page {number}: resources could not be checked")); continue; }
        };
        match page_bytes(pdf, id) {
            Some(bytes) => scan.content(&bytes, &resources, None, 0),
            None => scan.problem(format!("Page {number}: text content could not be decoded within the check limit")),
        }
        if let Ok(page) = pdf.get_dictionary(id)
            && let Ok(annots) = page.get_deref(b"Annots", pdf).and_then(Object::as_array)
        {
            for annot in annots {
                if let Ok((_, Object::Dictionary(annot))) = pdf.dereference(annot)
                    && let Ok(ap) = annot.get(b"AP")
                { scan.appearance(ap, &resources, 0); }
            }
        }
    }
    scan.problems.into_iter().map(|message| PdfAIssue { clause: "PDF/A Unicode mapping".into(), message }).collect()
}

/// A validator must not use lopdf's lenient page-content reader: it silently skips bad
/// references and can substitute compressed bytes after a decompression error.
fn page_bytes(pdf: &Document, page: ObjectId) -> Option<Vec<u8>> {
    let page = pdf.get_dictionary(page).ok()?;
    let Some(object) = page.get(b"Contents").ok() else { return Some(Vec::new()) };
    let (_, object) = pdf.dereference(object).ok()?;
    let streams: Vec<&Object> = match object {
        Object::Stream(_) => vec![object],
        Object::Array(streams) => streams.iter().collect(),
        Object::Null => return Some(Vec::new()),
        _ => return None,
    };
    let mut out = Vec::new();
    for object in streams {
        let (_, stream) = pdf.dereference(object).ok()?;
        let stream = stream.as_stream().ok()?;
        let bytes = stream.get_plain_content_with_limit(MAX_PAGE.checked_sub(out.len())?).ok()?;
        out.extend_from_slice(&bytes);
        out.push(b'\n');
    }
    Some(out)
}

struct Scan<'a> {
    pdf: &'a Document,
    fonts: HashMap<ObjectId, Rc<FontMap>>,
    problems: BTreeSet<String>,
    bytes: usize,
    visits: usize,
}

impl Scan<'_> {
    fn problem(&mut self, message: String) {
        if self.problems.len() < 64 { self.problems.insert(message); }
        else { self.problems.insert("Additional Unicode findings were omitted; repair the reported fonts and check again".into()); }
    }

    fn enter(&mut self, depth: usize) -> bool {
        self.visits += 1;
        if depth > 32 || self.visits > MAX_VISITS {
            self.problem("Recursive or excessively complex text appearances could not be checked".into());
            false
        } else { true }
    }

    fn font(&mut self, object: &Object) -> Rc<FontMap> {
        if let Object::Reference(id) = object
            && let Some(found) = self.fonts.get(id)
        { return found.clone(); }
        let font = match self.pdf.dereference(object).ok().and_then(|(_, o)| o.as_dict().ok()) {
            Some(dict) => parse_font(self.pdf, dict),
            None => FontMap::Unknown("The text font dictionary could not be read".into()),
        };
        let font = Rc::new(font);
        if let Object::Reference(id) = object { self.fonts.insert(*id, font.clone()); }
        font
    }

    fn text(&mut self, bytes: &[u8], font: Option<&FontMap>) {
        let (bad, name) = match font {
            Some(FontMap::Byte(map, name)) => (bytes.iter().any(|b| map[*b as usize].as_deref().is_none_or(|s| !valid_unicode(s))), name),
            Some(FontMap::Cmap(encoding, width, name)) => {
                let Encoding::UnicodeMapEncoding(map) = encoding else { self.problem("A Unicode mapping could not be checked".into()); return; };
                let mut codes = bytes.chunks_exact(*width);
                let bad = codes.by_ref().any(|code| {
                    let code = code.iter().fold(0u32, |n, b| n * 256 + u32::from(*b));
                    map.get(code, *width as u8).and_then(|u| String::from_utf16(&u).ok()).as_deref().is_none_or(|s| !valid_unicode(s))
                });
                (bad || !codes.remainder().is_empty(), name)
            }
            Some(FontMap::Unknown(reason)) => { self.problem(reason.clone()); return; }
            None => { self.problem("Text uses no identifiable font; its Unicode mapping could not be checked".into()); return; }
        };
        if bad { self.problem(format!("Font {name}: a displayed text code has no valid Unicode mapping (missing, invalid, replacement or private-use character)")); }
    }

    fn content(&mut self, bytes: &[u8], resources: &Dictionary, mut font: Option<Rc<FontMap>>, depth: usize) {
        if !self.enter(depth) { return; }
        self.bytes = self.bytes.saturating_add(bytes.len());
        if self.bytes > MAX_CONTENT { self.problem("The document exceeded the built-in Unicode check limit; use an external validator".into()); return; }
        let content = match Content::decode(bytes) {
            Ok(content) => content,
            Err(_) => { self.problem("A text content stream could not be decoded".into()); return; }
        };
        let mut stack = Vec::new();
        for op in content.operations {
            match op.operator.as_str() {
                "q" => stack.push(font.clone()),
                "Q" => { if let Some(saved) = stack.pop() { font = saved; } },
                "Tf" => {
                    let found = op.operands.first().and_then(|o| o.as_name().ok()).and_then(|name| {
                        resources.get_deref(b"Font", self.pdf).ok()?.as_dict().ok()?.get(name).ok().cloned()
                    });
                    font = found.as_ref().map(|f| self.font(f));
                }
                "Tj" | "'" | "\"" => {
                    let text = op.operands.last().and_then(|o| o.as_str().ok());
                    match text { Some(text) => self.text(text, font.as_deref()), None => self.problem("A text-showing operator is malformed".into()) }
                }
                "TJ" => match op.operands.first().and_then(|o| o.as_array().ok()) {
                    Some(parts) => for text in parts.iter().filter_map(|o| o.as_str().ok()) { self.text(text, font.as_deref()); },
                    None => self.problem("A text array is malformed".into()),
                },
                "Do" => {
                    let object = op.operands.first().and_then(|o| o.as_name().ok()).and_then(|name| resources.get_deref(b"XObject", self.pdf).ok()?.as_dict().ok()?.get(name).ok()).cloned();
                    if let Some(object) = object {
                        match self.pdf.dereference(&object).ok().and_then(|(_, o)| o.as_stream().ok()) {
                            Some(stream) if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") => {},
                            Some(stream) if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form") => {
                                let own = stream.dict.get_deref(b"Resources", self.pdf).ok().and_then(|o| o.as_dict().ok()).cloned().unwrap_or_else(|| resources.clone());
                                match stream.get_plain_content_with_limit(MAX_CONTENT.saturating_sub(self.bytes)) {
                                    Ok(bytes) => self.content(&bytes, &own, font.clone(), depth + 1),
                                    Err(_) => self.problem("A Form's text could not be decoded within the check budget".into()),
                                }
                            }
                            _ => self.problem("A page XObject could not be checked for text".into()),
                        }
                    } else { self.problem("A page XObject is missing".into()); }
                }
                _ => {},
            }
        }
    }

    fn appearance(&mut self, object: &Object, resources: &Dictionary, depth: usize) {
        if !self.enter(depth) { return; }
        match self.pdf.dereference(object) {
            Ok((_, Object::Stream(stream))) => {
                let own = stream.dict.get_deref(b"Resources", self.pdf).ok().and_then(|o| o.as_dict().ok()).cloned().unwrap_or_else(|| resources.clone());
                match stream.get_plain_content_with_limit(MAX_CONTENT.saturating_sub(self.bytes)) {
                    Ok(bytes) => self.content(&bytes, &own, None, depth + 1),
                    Err(_) => self.problem("An annotation appearance could not be decoded".into()),
                }
            }
            Ok((_, Object::Dictionary(dict))) => for (_, value) in dict { self.appearance(value, resources, depth + 1); },
            _ => self.problem("An annotation appearance could not be checked".into()),
        }
    }
}

fn valid_unicode(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| {
        let n = c as u32;
        !matches!(n, 0 | 0xfffd | 0xe000..=0xf8ff | 0xf0000..=0xffffd | 0x100000..=0x10fffd | 0xfdd0..=0xfdef) && (n & 0xffff) < 0xfffe
    })
}

/// Bound both code width and cumulative range expansion before lopdf builds its reverse map.
/// Repeating overlapping 16-bit ranges must not multiply the parsing work without a limit.
fn bounded_cmap(bytes: &[u8]) -> bool {
    let Ok(content) = Content::decode(bytes) else { return false };
    let source = |o: &Object| o.as_str().is_ok_and(|s| !s.is_empty() && s.len() <= 2);
    let target = |o: &Object| o.as_str().is_ok_and(|s| !s.is_empty() && s.len() <= 64 && s.len() % 2 == 0);
    let mut expanded = 0usize;
    for op in content.operations {
        match op.operator.as_str() {
            "usecmap" => return false,
            "endbfchar" => {
                let (pairs, remainder) = op.operands.as_chunks::<2>();
                if !remainder.is_empty() || !pairs.iter().all(|p| source(&p[0]) && target(&p[1])) { return false; }
                expanded += pairs.len();
            },
            "endbfrange" => {
                let (ranges, remainder) = op.operands.as_chunks::<3>();
                if !remainder.is_empty() { return false; }
                for range in ranges {
                    if !source(&range[0]) || !source(&range[1]) || !(target(&range[2]) || range[2].as_array().is_ok_and(|a| !a.is_empty() && a.iter().all(target))) { return false; }
                    let code = |o: &Object| o.as_str().unwrap_or_default().iter().fold(0usize, |n, b| n * 256 + usize::from(*b));
                    let Some(count) = code(&range[1]).checked_sub(code(&range[0])).map(|n| n + 1) else { return false };
                    if let Ok(target) = range[2].as_str() {
                        let last = target.as_chunks::<2>().0.last().copied().map(u16::from_be_bytes).map(usize::from).unwrap_or(0);
                        if last + count - 1 > usize::from(u16::MAX) { return false; }
                    }
                    expanded += count;
                    if expanded > 100_000 { return false; }
                }
            },
            "endcodespacerange" if op.operands.len() % 2 != 0 || !op.operands.iter().all(source) => return false,
            _ => {},
        }
        if expanded > 100_000 { return false; }
    }
    true
}

fn parse_font(pdf: &Document, font: &Dictionary) -> FontMap {
    let base = font.get(b"BaseFont").and_then(Object::as_name).unwrap_or_default();
    let name: String = String::from_utf8_lossy(base).chars().filter(|c| !c.is_control()).take(80).collect();
    let name = if name.is_empty() { "(unnamed)".into() } else { name };
    let unknown = |why: &str| FontMap::Unknown(format!("Font {name}: {why}"));
    if font.has(b"ToUnicode") {
        let Some(stream) = font.get_deref(b"ToUnicode", pdf).ok().and_then(|o| o.as_stream().ok()) else { return unknown("A ToUnicode mapping is not a readable stream"); };
        let Ok(bytes) = stream.get_plain_content_with_limit(MAX_CMAP) else { return unknown("A Unicode mapping exceeded the built-in check limit or could not be decoded"); };
        // lopdf expands source ranges while parsing. Bound source codes first, while allowing
        // ligatures and supplementary Unicode destinations; keep the compact map afterwards.
        if !bounded_cmap(&bytes) { return unknown("A complex or malformed Unicode CMap needs external validation"); }
        if stream.dict.has(b"UseCMap") { return unknown("An inherited Unicode CMap needs external validation"); }
        let width = if font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0") {
            if !font.get_deref(b"Encoding", pdf).is_ok_and(|e| e.as_name().is_ok_and(|n| matches!(n, b"Identity-H" | b"Identity-V"))) {
                return unknown("This multibyte font encoding needs external Unicode validation");
            }
            2
        } else { 1 };
        let mut mapping = font.clone();
        mapping.remove(b"Encoding");
        return match mapping.get_font_encoding_with_limit(pdf, MAX_CMAP) {
            Ok(Encoding::UnicodeMapEncoding(map)) => {
                let mut compact = Encoding::UnicodeMapEncoding(Default::default());
                if let Encoding::UnicodeMapEncoding(compact_map) = &mut compact { compact_map.bf_ranges = map.bf_ranges; }
                FontMap::Cmap(compact, width, name.clone())
            },
            _ => unknown("A ToUnicode CMap could not be parsed"),
        };
    }
    if !font.get(b"Subtype").is_ok_and(|s| s.as_name().is_ok_and(|n| matches!(n, b"Type1" | b"MMType1" | b"TrueType" | b"Type3"))) {
        return unknown("This font needs an explicit Unicode map before a/u conformance can be checked");
    }
    let builtin = font.get(b"Subtype").and_then(Object::as_name).is_ok_and(|s| matches!(s, b"Type1" | b"MMType1"));
    let symbolic = font.get_deref(b"FontDescriptor", pdf).ok().and_then(|d| d.as_dict().ok()).and_then(|d| d.get(b"Flags").ok()).and_then(|f| f.as_i64().ok()).is_some_and(|f| f & 4 != 0);
    if symbolic { return unknown("A symbolic font needs an explicit Unicode mapping for a/u conformance"); }
    let standard = builtin && matches!(base, b"Helvetica" | b"Helvetica-Bold" | b"Helvetica-Oblique" | b"Helvetica-BoldOblique" | b"Times-Roman" | b"Times-Bold" | b"Times-Italic" | b"Times-BoldItalic" | b"Courier" | b"Courier-Bold" | b"Courier-Oblique" | b"Courier-BoldOblique");
    let named = font.get_deref(b"Encoding", pdf).ok().and_then(|o| o.as_name().ok());
    if !(named.is_some_and(|n| matches!(n, b"WinAnsiEncoding" | b"MacRomanEncoding" | b"StandardEncoding")) || (!font.has(b"Encoding") && standard)) {
        return unknown("A displayed font has no verified Unicode mapping; repair its encoding before choosing a/u conformance");
    }
    match font.get_font_encoding_with_limit(pdf, MAX_CMAP) {
        Ok(Encoding::OneByteEncoding(map)) => {
            let encoding = Encoding::OneByteEncoding(map);
            FontMap::Byte((0..=255u8).map(|b| encoding.bytes_to_string(&[b]).ok().filter(|s| !s.is_empty())).collect(), name.clone())
        }
        _ => unknown("A single-byte font encoding could not be checked"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    fn sample(mapping: &str, text: &str) -> Document {
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let cmap = pdf.add_object(Stream::new(Dictionary::new(), format!("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange {mapping} endcmap CMapName currentdict /CMap defineresource pop end end").into_bytes()));
        let font = pdf.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "Test", "Encoding" => "Identity-H", "ToUnicode" => cmap });
        let content = pdf.add_object(Stream::new(Dictionary::new(), format!("BT /F1 12 Tf <{text}> Tj ET").into_bytes()));
        let page = pdf.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()], "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } }, "Contents" => content });
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()] }.into());
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        pdf.trailer.set("Root", root);
        pdf
    }

    #[test]
    fn unicode_levels_check_every_used_code_not_just_cmap_presence() {
        let pdf = sample("2 beginbfchar <0001> <0041> <0002> <0042> endbfchar", "00010002");
        assert!(issues(&pdf).is_empty(), "{:?}", issues(&pdf));
        for target in ["00660069", "D83DDE00"] {
            let pdf = sample(&format!("1 beginbfchar <0001> <{target}> endbfchar"), "0001");
            assert!(issues(&pdf).is_empty(), "ligatures/supplementary characters: {:?}", issues(&pdf));
        }
        for (mapping, code) in [("1 beginbfchar <0001> <0041> endbfchar", "00010002"), ("1 beginbfchar <0001> <D800> endbfchar", "0001"), ("1 beginbfchar <0001> <E000> endbfchar", "0001")] {
            let pdf = sample(mapping, code);
            assert!(!issues(&pdf).is_empty(), "{mapping}");
        }
    }

    #[test]
    fn unicode_check_visits_form_and_annotation_appearance_text() {
        let mut pdf = sample("1 beginbfchar <0001> <0041> endbfchar", "0001");
        let page = *pdf.get_pages().get(&1).unwrap();
        let resources = pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().clone();
        let form = pdf.add_object(Stream::new(dictionary! { "Subtype" => "Form", "Resources" => resources.clone() }, b"BT /F1 12 Tf <0002> Tj ET".to_vec()));
        let content = pdf.add_object(Stream::new(Dictionary::new(), b"/Form1 Do".to_vec()));
        let p = pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();
        p.set("Contents", content);
        p.get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("XObject", dictionary! { "Form1" => form });
        assert!(!issues(&pdf).is_empty(), "nested Form text is checked");
        let empty = pdf.add_object(Stream::new(Dictionary::new(), Vec::new()));
        let annot = pdf.add_object(dictionary! { "Subtype" => "Stamp", "AP" => dictionary! { "N" => form } });
        let p = pdf.get_object_mut(page).unwrap().as_dict_mut().unwrap();
        p.set("Contents", empty); p.set("Annots", vec![annot.into()]);
        assert!(!issues(&pdf).is_empty(), "annotation appearance text is checked");
    }
    #[test]
    fn cmap_ranges_cannot_expand_a_32_bit_space_or_follow_inherited_maps() {
        let pdf = sample("1 beginbfrange <00000000> <FFFFFFFF> <0041> endbfrange", "0001");
        assert!(issues(&pdf).iter().any(|i| i.message.contains("complex or malformed")));
        let overflowing = sample("1 beginbfrange <0001> <0002> <0066FFFF> endbfrange", "0002");
        assert!(!issues(&overflowing).is_empty(), "a range cannot overflow the mapping parser's final UTF-16 unit");
        let repeated = "1 beginbfrange <0000> <FFFF> <0000> endbfrange\n".repeat(3);
        let pdf = sample(&repeated, "0001");
        assert!(!issues(&pdf).is_empty(), "overlapping ranges share the cumulative expansion budget");
        let pdf = sample("/OtherMap usecmap", "0001");
        assert!(!issues(&pdf).is_empty());
    }

}
