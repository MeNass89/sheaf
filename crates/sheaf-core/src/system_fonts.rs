//! Fonts installed on the computer that a paragraph edited with the Edit tool can be set in
//! (Acrobat's Edit PDF font list, which lists the system's fonts): the TrueType and OpenType files
//! of the system's font folders that PdfCraft may embed, a static face whose own name table
//! declares an open licence (OFL, Apache, Ubuntu, Bitstream Vera, MIT: DejaVu, Liberation, Noto,
//! Ubuntu…). Read once, off the interface thread.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One installed face: its name as shown ("DejaVu Serif Bold"), its file, its family ("DejaVu
/// Serif") and whether it is bold or italic.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemFont {
    pub name: String,
    pub path: PathBuf,
    pub family: String,
    pub bold: bool,
    pub italic: bool,
}

/// The folders fonts are installed in on this system (those that exist).
pub fn font_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        dirs.extend(["/System/Library/Fonts", "/Library/Fonts"].map(PathBuf::from));
        dirs.extend(home.map(|h| h.join("Library/Fonts")));
    } else if cfg!(windows) {
        let windir = std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        dirs.push(windir.join("Fonts"));
        dirs.extend(std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join(r"Microsoft\Windows\Fonts")));
    } else {
        dirs.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
        if let Some(h) = home {
            dirs.push(h.join(".local/share/fonts"));
            dirs.push(h.join(".fonts"));
        }
    }
    dirs.retain(|d| d.is_dir());
    dirs
}

/// Files read at most, and the largest read (PdfCraft embeds up to 16 MiB).
const MAX_FILES: usize = 3_000;
const MAX_BYTES: u64 = 16 << 20;

/// The fonts of `dirs` (searched a few folders deep) PdfCraft may embed, by name, each name once.
pub fn list_fonts(dirs: &[PathBuf]) -> Vec<SystemFont> {
    let mut files = Vec::new();
    for d in dirs {
        font_files(d, 0, &mut files);
    }
    files.sort();
    let mut out: Vec<SystemFont> = files
        .into_iter()
        .take(MAX_FILES)
        .filter_map(|path| {
            let (name, family) = embeddable_name(&path)?;
            let (bold, italic) = style_of(&name);
            Some(SystemFont { name, path, family, bold, italic })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.path.cmp(&b.path)));
    out.dedup_by(|a, b| a.name == b.name);
    out
}

/// The fonts installed on this system ([`list_fonts`] of [`font_dirs`]), read the first time.
pub fn installed() -> &'static [SystemFont] {
    static FONTS: OnceLock<Vec<SystemFont>> = OnceLock::new();
    FONTS.get_or_init(|| list_fonts(&font_dirs()))
}

fn font_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() && depth < 5 {
            font_files(&path, depth + 1, out);
        } else if kind.is_file()
            && path.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf"))
            && e.metadata().is_ok_and(|m| m.len() <= MAX_BYTES)
        {
            out.push(path);
        }
    }
}

/// A font file's name as shown, when PdfCraft may embed it: its family, and its style when not
/// the regular one, from its PostScript name ("DejaVuSerif-BoldItalic" → "DejaVu Serif Bold
/// Italic"); and its family.
fn embeddable_name(path: &Path) -> Option<(String, String)> {
    let bytes = std::fs::read(path).ok()?;
    pdfcraft_fonts::embedding::validate_document_input(&bytes).ok()?;
    let (family, postscript) = pdfcraft_fonts::embedding::names(&bytes).ok()?;
    let style = postscript.rsplit_once('-').map(|(_, s)| s).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("regular") && !s.eq_ignore_ascii_case("roman"));
    Some(match style {
        Some(s) => (format!("{family} {}", spaced(s)), family),
        None => (family.clone(), family),
    })
}

/// Whether a font's name says bold and italic ("DejaVuSerif-BoldItalic", "Arial,Bold",
/// "ABCDEF+NotoSans-SemiBoldOblique").
pub fn style_of(name: &str) -> (bool, bool) {
    let l = name.to_ascii_lowercase();
    let bold = ["bold", "black", "heavy"].iter().any(|w| l.contains(w));
    (bold, l.contains("italic") || l.contains("oblique"))
}

/// Letters and digits of a font name, lower case: "DejaVu Serif" and "DejaVuSerif" alike.
fn key(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).flat_map(|c| c.to_lowercase()).collect()
}

/// The installed face of `family` that is `bold` and `italic` (the format row's B and I after a
/// font of the computer's was chosen).
pub fn face<'a>(fonts: &'a [SystemFont], family: &str, bold: bool, italic: bool) -> Option<&'a SystemFont> {
    fonts.iter().find(|f| f.bold == bold && f.italic == italic && key(&f.family) == key(family))
}

/// The installed face of the font `base_font` is drawn in (a `/BaseFont`, "ABCDEF+DejaVuSerif-Bold":
/// its family is what comes before the style) that is `bold` and `italic`: what the format row's
/// B and I give a text box in a font of the computer's or the file's own, as PDF Expert's
/// typeface menu does.
pub fn variant<'a>(fonts: &'a [SystemFont], base_font: &str, bold: bool, italic: bool) -> Option<&'a SystemFont> {
    let plain = base_font.rsplit('+').next().unwrap_or(base_font);
    let family = key(plain.split(['-', ',']).next().unwrap_or(plain));
    // "ArialMT" is Arial's family as PostScript names it.
    let family = family.strip_suffix("mt").filter(|f| !f.is_empty()).map_or(family.clone(), str::to_string);
    fonts.iter().filter(|f| f.bold == bold && f.italic == italic).find(|f| key(&f.family) == family)
}

/// "BoldItalic" → "Bold Italic".
fn spaced(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if c.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = c.is_lowercase();
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bold and italic pick the same family's faces (Sol, 27/10: a standard family came instead).
    #[test]
    fn bold_and_italic_pick_the_fonts_own_faces() {
        let font = |name: &str, family: &str| {
            let (bold, italic) = style_of(name);
            SystemFont { name: name.into(), path: PathBuf::from(format!("{name}.ttf")), family: family.into(), bold, italic }
        };
        let fonts = [font("DejaVu Serif", "DejaVu Serif"), font("DejaVu Serif Bold", "DejaVu Serif"), font("DejaVu Serif Italic", "DejaVu Serif"), font("DejaVu Sans Bold", "DejaVu Sans")];
        assert_eq!(variant(&fonts, "ABCDEF+DejaVuSerif", true, false).map(|f| f.name.as_str()), Some("DejaVu Serif Bold"));
        assert_eq!(variant(&fonts, "DejaVuSerif-Bold", false, true).map(|f| f.name.as_str()), Some("DejaVu Serif Italic"));
        assert_eq!(variant(&fonts, "DejaVuSerif-Bold", false, false).map(|f| f.name.as_str()), Some("DejaVu Serif"));
        assert!(variant(&fonts, "DejaVuSerif", true, true).is_none(), "no bold italic face installed");
        assert_eq!(style_of("ABCDEF+NotoSans-SemiBoldOblique"), (true, true));
        assert_eq!(face(&fonts, "DejaVu Serif", true, false).map(|f| f.name.as_str()), Some("DejaVu Serif Bold"));
        assert!(face(&fonts, "DejaVu Sans", false, false).is_none(), "no regular DejaVu Sans here");
    }

    #[test]
    fn styles_are_spaced_out() {
        assert_eq!(spaced("BoldItalic"), "Bold Italic");
        assert_eq!(spaced("Bold"), "Bold");
    }
}
