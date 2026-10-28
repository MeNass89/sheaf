//! The interface's language (Preferences ▸ Interface ▸ Language; iLovePDF's ~30 languages,
//! Acrobat's "Same as the operating system"). The Slint texts are wrapped in `@tr` and their
//! French is bundled at build time from `translations/fr/LC_MESSAGES/sheaf-ui.po`; the texts made
//! in Rust (tool names, descriptions) go through [`tr`], which reads the same catalogue, compiled
//! into the binary: nothing is read from disk and no gettext library is needed.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// The catalogue, as gettext writes it.
const FR_PO: &str = include_str!("../translations/fr/LC_MESSAGES/sheaf-ui.po");

/// The languages offered besides the system's: code ("" English, the source) and name.
pub const LANGUAGES: [(&str, &str); 2] = [("", "English"), ("fr", "Français")];

static CURRENT: RwLock<&'static str> = RwLock::new("");

/// The language a preference means: 0 the system's (French for a French locale, else English),
/// 1 English, 2 French.
pub fn language_for(pref: u8, system: Option<&str>) -> &'static str {
    match pref {
        1 => "",
        2 => "fr",
        _ if system.is_some_and(|l| l.to_ascii_lowercase().starts_with("fr")) => "fr",
        _ => "",
    }
}

pub fn set(lang: &'static str) {
    if let Ok(mut c) = CURRENT.write() {
        *c = lang;
    }
}

pub fn current() -> &'static str {
    CURRENT.read().map(|c| *c).unwrap_or("")
}

fn french() -> &'static HashMap<String, String> {
    static FR: OnceLock<HashMap<String, String>> = OnceLock::new();
    FR.get_or_init(|| parse_po(FR_PO))
}

/// `s` in the interface's language (itself when English, or when the catalogue lacks it).
pub fn tr(s: &str) -> String {
    match current() {
        "fr" => french().get(s).filter(|t| !t.is_empty()).cloned().unwrap_or_else(|| s.to_string()),
        _ => s.to_string(),
    }
}

/// The msgid → msgstr pairs of a .po text (single forms, no contexts: Sheaf's catalogue has none).
pub fn parse_po(po: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let (mut id, mut text, mut field): (Option<String>, Option<String>, u8) = (None, None, 0);
    let mut flush = |id: &mut Option<String>, text: &mut Option<String>| {
        if let (Some(i), Some(t)) = (id.take(), text.take()) {
            if !i.is_empty() {
                map.insert(i, t);
            }
        }
    };
    for line in po.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("msgid ") {
            flush(&mut id, &mut text);
            id = Some(unquote(rest));
            field = 1;
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            text = Some(unquote(rest));
            field = 2;
        } else if line.starts_with('"') {
            match field {
                1 => id.get_or_insert_with(String::new).push_str(&unquote(line)),
                2 => text.get_or_insert_with(String::new).push_str(&unquote(line)),
                _ => {}
            }
        } else if line.is_empty() || line.starts_with('#') {
            field = 0;
        }
    }
    flush(&mut id, &mut text);
    map
}

/// A quoted .po string's text, its escapes read.
fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s);
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The texts of a .slint source wrapped in `@tr("…")`, as Slint sees them (escapes read).
pub fn slint_messages(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("@tr(\"") {
        rest = &rest[i + 5..];
        let mut end = 0;
        let b = rest.as_bytes();
        while end < b.len() && !(b[end] == b'"' && (end == 0 || b[end - 1] != b'\\')) {
            end += 1;
        }
        out.push(unquote(&format!("\"{}\"", &rest[..end])));
        rest = &rest[end.min(rest.len())..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slint_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                slint_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "slint") {
                out.push(p);
            }
        }
    }

    #[test]
    fn every_interface_text_has_its_french() {
        let fr = parse_po(FR_PO);
        let mut files = Vec::new();
        slint_files(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui"), &mut files);
        let mut missing = Vec::new();
        let mut count = 0;
        for f in &files {
            for m in slint_messages(&std::fs::read_to_string(f).unwrap()) {
                count += 1;
                if fr.get(&m).is_none_or(|t| t.is_empty()) {
                    missing.push(format!("{}: {m:?}", f.file_name().unwrap().to_string_lossy()));
                }
            }
        }
        // The texts made in Rust: every `tr("…")` literal of the app's sources.
        let mut rs = Vec::new();
        fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() { rs_files(&p, out) } else if p.extension().is_some_and(|x| x == "rs") { out.push(p) }
            }
        }
        rs_files(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut rs);
        for f in rs.iter().filter(|f| !f.ends_with("i18n.rs")) {
            let src = std::fs::read_to_string(f).unwrap();
            for m in slint_messages(&src.replace("i18n::tr(\"", "@tr(\"")) {
                count += 1;
                if fr.get(&m).is_none_or(|t| t.is_empty()) {
                    missing.push(format!("{}: {m:?}", f.file_name().unwrap().to_string_lossy()));
                }
            }
        }
        // The Tools page's names and sections, made in Rust from the catalogue.
        for t in sheaf_core::tools::catalogue() {
            count += 1;
            if fr.get(t.name).is_none_or(|x| x.is_empty()) {
                missing.push(format!("tool: {:?}", t.name));
            }
        }
        for c in sheaf_core::tools::Category::ALL {
            if fr.get(c.label()).is_none_or(|x| x.is_empty()) {
                missing.push(format!("section: {:?}", c.label()));
            }
        }
        assert!(count > 300, "only {count} texts are translatable");
        assert!(missing.is_empty(), "{} texts without French:\n{}", missing.len(), missing.join("\n"));
        // Placeholders kept: as many {} in the French as in the English.
        for (k, v) in &fr {
            assert_eq!(k.matches("{}").count(), v.matches("{}").count(), "{k:?} → {v:?}");
        }
    }

    #[test]
    fn languages_follow_the_choice_or_the_system_and_texts_fall_back_to_english() {
        assert_eq!(language_for(0, Some("fr-BE")), "fr");
        assert_eq!(language_for(0, Some("de_DE.UTF-8")), "");
        assert_eq!(language_for(0, None), "");
        assert_eq!(language_for(1, Some("fr_FR")), "");
        assert_eq!(language_for(2, Some("en_US")), "fr");
        let po = "msgid \"\"\nmsgstr \"Content-Type: text/plain\\n\"\n\n#: a.slint\nmsgid \"Open\"\nmsgstr \"Ouvrir\"\n\nmsgid \"A \\\"b\\\"\"\nmsgstr \"\"\n\"Un \"\n\"« b »\"\n";
        let m = parse_po(po);
        assert_eq!(m.get("Open").map(String::as_str), Some("Ouvrir"));
        assert_eq!(m.get("A \"b\"").map(String::as_str), Some("Un « b »"));
        assert!(!m.contains_key(""));
        assert_eq!(slint_messages(r#"text: @tr("Open “{}”?", x); t: @tr("a \"q\"");"#), ["Open “{}”?", "a \"q\""]);
        set("fr");
        assert_eq!(tr("Tools"), french().get("Tools").cloned().unwrap_or_else(|| "Tools".into()));
        assert_eq!(tr("No such text here"), "No such text here");
        set("");
        assert_eq!(tr("Tools"), "Tools");
    }
}
