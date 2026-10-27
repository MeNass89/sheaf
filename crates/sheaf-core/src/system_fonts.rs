//! Fonts installed on the computer that a paragraph edited with the Edit tool can be set in
//! (Acrobat's Edit PDF font list, which lists the system's fonts): the TrueType and OpenType files
//! of the system's font folders that PdfCraft may embed, a static face whose own name table
//! declares an open licence (OFL, Apache, Ubuntu, Bitstream Vera, MIT: DejaVu, Liberation, Noto,
//! Ubuntu…). Read once, off the interface thread.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One installed face: its name as shown ("DejaVu Serif Bold") and its file.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemFont {
    pub name: String,
    pub path: PathBuf,
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
    let mut out: Vec<SystemFont> = files.into_iter().take(MAX_FILES).filter_map(|path| Some(SystemFont { name: embeddable_name(&path)?, path })).collect();
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
/// the regular one, from its PostScript name ("DejaVuSerif-BoldItalic" → "DejaVu Serif Bold Italic").
fn embeddable_name(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    pdfcraft_fonts::embedding::validate_document_input(&bytes).ok()?;
    let (family, postscript) = pdfcraft_fonts::embedding::names(&bytes).ok()?;
    let style = postscript.rsplit_once('-').map(|(_, s)| s).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("regular") && !s.eq_ignore_ascii_case("roman"));
    Some(match style {
        Some(s) => format!("{family} {}", spaced(s)),
        None => family,
    })
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

    #[test]
    fn styles_are_spaced_out() {
        assert_eq!(spaced("BoldItalic"), "Bold Italic");
        assert_eq!(spaced("Bold"), "Bold");
    }
}
