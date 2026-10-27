//! The app's platform side, the parts that can be tested without a window: the menu bar's
//! commands and their keyboard shortcuts (and the user's own, Stirling's Hotkeys), which tools are
//! hidden when they cannot run here, the interface's reading direction (iLovePDF's right-to-left
//! interface), and the misspelt words of a text marked for the editor's spelling line.

use serde::{Deserialize, Serialize};
use sheaf_core::tools::ToolId;

/// What Preferences keeps for these (one field of `Prefs`, `platform`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct PlatformPrefs {
    /// Check Spelling While Typing turned off (Edit menu, Preferences ▸ Spelling).
    pub no_spell_check: bool,
    /// Words taught with Learn Spelling, kept between sessions.
    pub learned_words: Vec<String>,
    /// Shortcuts changed by the user: command id ([`COMMANDS`]) and its keys as parts joined
    /// with "+" ("Control+Shift+K"); "" means the command has none.
    pub shortcuts: Vec<(String, String)>,
    /// Tools that cannot run on this machine (no LibreOffice, no OCR models) are left off the
    /// Tools page (Stirling's hide unavailable tools).
    pub hide_unavailable_tools: bool,
    /// The interface's direction: 0 as the system's language, 1 left to right, 2 right to left.
    pub direction: u8,
}

/// A command of the menu bar: its stable id, menu, title and default keys (parts joined by "+",
/// as `slint::Keys::from_parts` reads them; "" none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub id: &'static str,
    pub menu: &'static str,
    pub title: &'static str,
    pub keys: &'static str,
}

const fn cmd(id: &'static str, menu: &'static str, title: &'static str, keys: &'static str) -> Command {
    Command { id, menu, title, keys }
}

/// Full screen: ⌃⌘F on a Mac (PDF Expert's, Preview's), F11 elsewhere (every Linux and Windows
/// viewer's).
const FULL_SCREEN: &str = if cfg!(target_os = "macos") { "Control+Meta+F" } else { "F11" };

/// The menu bar's commands, in menu order. `ui/main.slint` reads each one's shortcut as
/// `PlatformBridge.keys[<index here>]`: an entry is only ever added at the end (the test
/// `the_menu_bar_reads_each_command_at_its_index` holds the two together).
///
/// Undo, Redo and Select All have no default keys here: the menu bar takes its shortcuts before
/// any field does, and ⌘Z, ⌘A must stay the text field's while one is typed in (the reader's
/// own ⌘Z and ⌘A still work on the pages).
pub const COMMANDS: &[Command] = &[
    cmd("new", "File", "New Blank PDF…", "Control+N"),
    cmd("open", "File", "Open…", "Control+O"),
    cmd("save", "File", "Save", "Control+S"),
    cmd("save-as", "File", "Save As…", "Control+Shift+S"),
    cmd("print", "File", "Print…", "Control+P"),
    cmd("properties", "File", "Document Properties…", "Control+D"),
    cmd("close", "File", "Close Tab", "Control+W"),
    cmd("quit", "File", "Quit Sheaf", "Control+Q"),
    cmd("undo", "Edit", "Undo", ""),
    cmd("redo", "Edit", "Redo", ""),
    cmd("select-all", "Edit", "Select All", ""),
    cmd("find", "Edit", "Find…", "Control+F"),
    cmd("find-next", "Edit", "Find Next", "Control+G"),
    cmd("find-previous", "Edit", "Find Previous", "Control+Shift+G"),
    cmd("spelling", "Edit", "Check Spelling While Typing", ""),
    cmd("preferences", "Edit", "Preferences…", "Control+Comma"),
    cmd("zoom-in", "View", "Zoom In", "Control+Plus"),
    cmd("zoom-out", "View", "Zoom Out", "Control+HyphenMinus"),
    cmd("actual-size", "View", "Actual Size", "Control+Digit0"),
    cmd("fit-page", "View", "Zoom to Fit Page", "Control+Digit8"),
    cmd("fit-width", "View", "Zoom to Fit Width", "Control+Digit9"),
    cmd("panel", "View", "Show or Hide Side Panel", "Control+Alt+Digit0"),
    cmd("two-pages", "View", "Next Page Layout", "Control+Alt+Digit2"),
    cmd("rotate-right", "View", "Rotate View Clockwise", ""),
    cmd("rotate-left", "View", "Rotate View Counterclockwise", ""),
    cmd("full-screen", "View", "Full Screen", FULL_SCREEN),
    cmd("present", "View", "Present", "Control+Shift+P"),
    cmd("previous-page", "Go", "Previous Page", ""),
    cmd("next-page", "Go", "Next Page", ""),
    cmd("first-page", "Go", "First Page", ""),
    cmd("last-page", "Go", "Last Page", ""),
    cmd("go-to-page", "Go", "Go to Page…", "Control+Alt+G"),
    cmd("back", "Go", "Back", "Control+OpenBracket"),
    cmd("forward", "Go", "Forward", "Control+CloseBracket"),
    cmd("all-tools", "Tools", "All Tools", ""),
    cmd("next-tab", "Window", "Next Tab", "Control+Tab"),
    cmd("previous-tab", "Window", "Previous Tab", "Control+Shift+Tab"),
    cmd("home", "Window", "Home", ""),
    cmd("shortcuts", "Help", "Keyboard Shortcuts", "Control+Slash"),
    cmd("customize", "Help", "Customize Shortcuts…", ""),
];

/// The index of a command by id.
pub fn command_index(id: &str) -> Option<usize> {
    COMMANDS.iter().position(|c| c.id == id)
}

/// A command's keys now: the user's, else the default.
pub fn effective(p: &PlatformPrefs, i: usize) -> String {
    let Some(c) = COMMANDS.get(i) else { return String::new() };
    p.shortcuts.iter().find(|(id, _)| id == c.id).map(|(_, k)| k.clone()).unwrap_or_else(|| c.keys.to_string())
}

/// Keys as Slint matches them (none when the parts do not read).
pub fn keys(parts: &str) -> slint::Keys {
    if parts.is_empty() {
        return slint::Keys::default();
    }
    slint::Keys::from_parts(parts.split('+')).unwrap_or_default()
}

/// Keys as the system writes them ("Ctrl+Shift+S", "⇧⌘S").
pub fn display(parts: &str) -> String {
    keys(parts).to_string()
}

/// The parts of a key pressed while a shortcut is recorded, "" when it is a modifier alone
/// (the recording waits for the real key). Letters keep ⇧; digits and punctuation are named
/// without it, since ⇧ changes them with the keyboard's layout (Slint matches them so).
pub fn parts_of_event(text: &str, control: bool, alt: bool, shift: bool, meta: bool) -> String {
    let mut chars = text.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else { return String::new() };
    let named: Option<(&str, bool)> = match c {
        '\u{10}'..='\u{18}' => return String::new(),
        '\t' | '\u{19}' => Some(("Tab", true)),
        '\n' | '\r' => Some(("Return", true)),
        ' ' => Some(("Space", true)),
        '\u{8}' => Some(("Backspace", true)),
        '\u{7f}' => Some(("Delete", true)),
        '\u{1b}' => Some(("Escape", true)),
        '\u{F700}' => Some(("UpArrow", true)),
        '\u{F701}' => Some(("DownArrow", true)),
        '\u{F702}' => Some(("LeftArrow", true)),
        '\u{F703}' => Some(("RightArrow", true)),
        '\u{F729}' => Some(("Home", true)),
        '\u{F72B}' => Some(("End", true)),
        '\u{F72C}' => Some(("PageUp", true)),
        '\u{F72D}' => Some(("PageDown", true)),
        '0'..='9' => None,
        ',' | '<' => Some(("Comma", false)),
        '.' | '>' => Some(("Period", false)),
        '/' | '?' => Some(("Slash", false)),
        '[' | '{' => Some(("OpenBracket", false)),
        ']' | '}' => Some(("CloseBracket", false)),
        '-' | '_' => Some(("HyphenMinus", false)),
        '=' => Some(("Equals", false)),
        '+' => Some(("Plus", false)),
        ';' | ':' => Some(("Semicolon", false)),
        '\'' | '"' => Some(("Quote", false)),
        '`' | '~' => Some(("BackQuote", false)),
        '\\' | '|' => Some(("BackSlash", false)),
        _ => None,
    };
    let fkey = ('\u{F704}'..='\u{F71B}').contains(&c).then(|| format!("F{}", c as u32 - 0xF704 + 1));
    let (key, keep_shift) = match (named, fkey) {
        (_, Some(f)) => (f, true),
        (Some((n, s)), None) => (n.to_string(), s),
        (None, None) if c.is_ascii_digit() => (format!("Digit{c}"), false),
        (None, None) if c.is_ascii_alphabetic() => (c.to_ascii_uppercase().to_string(), true),
        (None, None) => (c.to_lowercase().collect(), true),
    };
    let mut parts: Vec<&str> = Vec::new();
    if control {
        parts.push("Control");
    }
    if alt {
        parts.push("Alt");
    }
    if shift && keep_shift {
        parts.push("Shift");
    }
    if meta {
        parts.push("Meta");
    }
    parts.push(&key);
    parts.join("+")
}

/// Whether keys can be a menu shortcut: a modifier other than ⇧ (a function key alone is
/// fine), not one of the keys text fields use, and readable by Slint. The menu bar takes its
/// keys before any field, so a bare letter or ⌘C would stop typing and copying.
pub fn check_shortcut(parts: &str) -> Result<(), String> {
    if parts.is_empty() {
        return Ok(());
    }
    let p: Vec<&str> = parts.split('+').collect();
    let key = p.last().copied().unwrap_or("");
    let has = |m: &str| p.contains(&m);
    let function_key = key.len() >= 2 && key.starts_with('F') && key[1..].chars().all(|c| c.is_ascii_digit());
    if !(has("Control") || has("Alt") || has("Meta") || function_key) {
        return Err("Add ⌘/Ctrl or Alt: a key alone would stop typing".into());
    }
    let text_keys = ["A", "C", "V", "X", "Z", "Y", "Backspace", "Delete", "LeftArrow", "RightArrow", "UpArrow", "DownArrow", "Home", "End"];
    if has("Control") && !has("Alt") && text_keys.contains(&key) {
        return Err(format!("{} is kept for editing text", display(parts)));
    }
    slint::Keys::from_parts(p.iter().copied()).map(|_| ()).map_err(|e| e.to_string())
}

/// Gives command `id` the keys `parts` ("" removes its shortcut). Refused when the keys cannot
/// be a shortcut or another command has them (Stirling's conflict warning): the reason.
pub fn assign(p: &mut PlatformPrefs, id: &str, parts: &str) -> Result<(), String> {
    let Some(i) = command_index(id) else { return Err(format!("No command {id}")) };
    check_shortcut(parts)?;
    if !parts.is_empty() {
        let wanted = keys(parts);
        if let Some(other) = (0..COMMANDS.len()).filter(|&k| k != i).find(|&k| keys(&effective(p, k)) == wanted) {
            let c = COMMANDS[other];
            return Err(format!("{} is already {} ▸ {}", display(parts), c.menu, c.title.trim_end_matches('…')));
        }
    }
    p.shortcuts.retain(|(c, _)| c != id);
    if parts != COMMANDS[i].keys {
        p.shortcuts.push((id.to_string(), parts.to_string()));
    }
    Ok(())
}

/// Back to the default keys: one command, or all of them (`None`).
pub fn reset(p: &mut PlatformPrefs, id: Option<&str>) {
    match id {
        Some(id) => p.shortcuts.retain(|(c, _)| c != id),
        None => p.shortcuts.clear(),
    }
}

/// A row of Preferences ▸ Keyboard: command index, "Menu ▸ Title", keys shown, changed.
pub type ShortcutRow = (usize, String, String, bool);

/// The rows whose menu, title or keys contain `query` (Stirling's Search tools), all of them
/// when it is empty.
pub fn shortcut_rows(p: &PlatformPrefs, query: &str) -> Vec<ShortcutRow> {
    let q = query.trim().to_lowercase();
    COMMANDS
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let k = effective(p, i);
            (i, format!("{} ▸ {}", c.menu, c.title), display(&k), k != c.keys)
        })
        .filter(|(_, name, k, _)| q.is_empty() || name.to_lowercase().contains(&q) || k.to_lowercase().contains(&q))
        .collect()
}

/// Why a tool cannot run here, if it cannot: the Office conversions need LibreOffice, OCR its
/// models.
pub fn unavailable(tool: ToolId, has_office: bool, has_ocr: bool) -> Option<&'static str> {
    match tool {
        ToolId::WordToPdf | ToolId::PowerPointToPdf | ToolId::ExcelToPdf | ToolId::HtmlToPdf if !has_office => Some("needs LibreOffice"),
        ToolId::Ocr if !has_ocr => Some("needs the OCR models"),
        _ => None,
    }
}

/// Languages written right to left (Arabic, Hebrew, Persian, Urdu, Yiddish, Pashto, Sindhi,
/// Uyghur, Dhivehi, Kurdish Sorani), by the locale's language ("ar-EG", "he_IL.UTF-8").
pub fn is_rtl_language(locale: &str) -> bool {
    let lang = locale.split(['-', '_', '.', '@']).next().unwrap_or("").to_ascii_lowercase();
    matches!(lang.as_str(), "ar" | "he" | "iw" | "fa" | "ur" | "yi" | "ps" | "sd" | "ug" | "dv" | "ckb")
}

/// Whether the interface reads right to left: as chosen, else as the system's language.
pub fn right_to_left(p: &PlatformPrefs, locale: Option<&str>) -> bool {
    match p.direction {
        1 => false,
        2 => true,
        _ => locale.is_some_and(is_rtl_language),
    }
}

/// Markdown escaped so every character shows as typed (Slint's StyledText reads CommonMark).
fn escape_markdown(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The text as Slint's StyledText markdown, each misspelt word (byte offset, word) underlined
/// in `colour` ("#e5322d"): the spelling line under the editor (Acrobat and Preview underline in
/// red). Lines become paragraphs; their leading spaces go (four would make a code block).
pub fn marked_markdown(text: &str, issues: &[(usize, &str)], colour: &str) -> String {
    let mut out = String::new();
    let mut at = 0;
    let mut sorted: Vec<(usize, &str)> = issues.iter().copied().filter(|(s, w)| text.get(*s..s + w.len()) == Some(*w)).collect();
    sorted.sort_by_key(|(s, _)| *s);
    let plain = |s: &str, out: &mut String| {
        let mut first = true;
        for line in s.split('\n') {
            if !first {
                out.push_str("\n\n");
            }
            let line = if first { line } else { line.trim_start() };
            out.push_str(&escape_markdown(line));
            first = false;
        }
    };
    for (s, w) in sorted {
        if s < at {
            continue;
        }
        plain(&text[at..s], &mut out);
        out.push_str(&format!("<u><font color=\"{colour}\">{}</font></u>", escape_markdown(w)));
        at = s + w.len();
    }
    plain(&text[at..], &mut out);
    out.trim_start().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_bar_reads_each_command_at_its_index() {
        // Each command's MenuItem in main.slint has its title and `PlatformBridge.keys[i]`.
        let ui = include_str!("../ui/main.slint");
        for (i, c) in COMMANDS.iter().enumerate() {
            let at = format!("shortcut: PlatformBridge.keys[{i}];");
            let line = ui.lines().find(|l| l.contains(&at)).unwrap_or_else(|| panic!("no menu item reads keys[{i}] ({})", c.id));
            assert!(line.contains(&format!("\"{}\"", c.title)) || line.contains("PlatformBridge.spell-on"), "keys[{i}] is not {}: {line}", c.title);
        }
        // Every default reads as Slint keys and is a valid shortcut; no two are the same.
        for (i, c) in COMMANDS.iter().enumerate() {
            assert_eq!(check_shortcut(c.keys), Ok(()), "{}", c.id);
            if !c.keys.is_empty() {
                assert_ne!(keys(c.keys), slint::Keys::default(), "{}", c.id);
                assert!(COMMANDS[..i].iter().all(|o| o.keys != c.keys), "{} twice", c.keys);
            }
        }
    }

    #[test]
    fn a_pressed_key_becomes_shortcut_parts() {
        assert_eq!(parts_of_event("s", true, false, false, false), "Control+S");
        assert_eq!(parts_of_event("S", true, false, true, false), "Control+Shift+S");
        // ⇧ with a digit or a sign: named without it, the layout decides what ⇧ makes.
        assert_eq!(parts_of_event("+", true, false, true, false), "Control+Plus");
        assert_eq!(parts_of_event("1", true, true, false, false), "Control+Alt+Digit1");
        assert_eq!(parts_of_event("\u{F70E}", false, false, false, false), "F11");
        assert_eq!(parts_of_event("\u{F704}", false, false, true, false), "Shift+F1");
        assert_eq!(parts_of_event("é", true, false, false, false), "Control+é");
        // A modifier alone: still waiting for the key.
        assert_eq!(parts_of_event("\u{11}", true, false, false, false), "");
        assert_eq!(parts_of_event("", true, false, false, false), "");
        for p in ["Control+S", "Control+Shift+S", "Control+Plus", "Control+Alt+Digit1", "F11", "Shift+F1", "Control+é"] {
            assert_ne!(keys(p), slint::Keys::default(), "{p}");
        }
    }

    #[test]
    fn shortcuts_that_would_stop_typing_are_refused() {
        assert!(check_shortcut("K").is_err());
        assert!(check_shortcut("Shift+K").is_err());
        assert!(check_shortcut("Control+C").is_err());
        assert!(check_shortcut("Control+Shift+Z").is_err());
        assert!(check_shortcut("Control+Backspace").is_err());
        assert_eq!(check_shortcut("F5"), Ok(()));
        assert_eq!(check_shortcut("Control+Alt+C"), Ok(()));
        assert_eq!(check_shortcut("Alt+K"), Ok(()));
        assert_eq!(check_shortcut(""), Ok(()));
    }

    #[test]
    fn shortcuts_change_conflict_and_reset() {
        let mut p = PlatformPrefs::default();
        let find = command_index("find").unwrap();
        assert_eq!(effective(&p, find), "Control+F");
        // Taken by Save: refused, with who has it.
        let e = assign(&mut p, "find", "Control+S").unwrap_err();
        assert!(e.contains("File ▸ Save"), "{e}");
        assign(&mut p, "find", "Control+Alt+F").unwrap();
        assert_eq!(effective(&p, find), "Control+Alt+F");
        // Its old keys are free now.
        assign(&mut p, "go-to-page", "Control+F").unwrap();
        // Removed, then back to the default.
        assign(&mut p, "save", "").unwrap();
        assert_eq!(effective(&p, command_index("save").unwrap()), "");
        reset(&mut p, Some("save"));
        assert_eq!(effective(&p, command_index("save").unwrap()), "Control+S");
        // Set back to its default: nothing kept.
        reset(&mut p, None);
        assign(&mut p, "print", "Control+P").unwrap();
        assert!(p.shortcuts.is_empty());
        assert!(assign(&mut p, "nothing", "F5").is_err());
    }

    #[test]
    fn shortcut_rows_are_searched() {
        let mut p = PlatformPrefs::default();
        assign(&mut p, "find", "Control+Alt+F").unwrap();
        let rows = shortcut_rows(&p, "find");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].1, "Edit ▸ Find…");
        assert!(rows[0].3 && !rows[1].3);
        assert_eq!(shortcut_rows(&p, "").len(), COMMANDS.len());
        assert!(shortcut_rows(&p, "zzz").is_empty());
    }

    #[test]
    fn tools_that_cannot_run_here() {
        assert_eq!(unavailable(ToolId::WordToPdf, false, true), Some("needs LibreOffice"));
        assert_eq!(unavailable(ToolId::WordToPdf, true, true), None);
        assert_eq!(unavailable(ToolId::Ocr, true, false), Some("needs the OCR models"));
        assert_eq!(unavailable(ToolId::Merge, false, false), None);
    }

    #[test]
    fn right_to_left_follows_the_language_unless_chosen() {
        assert!(is_rtl_language("ar-EG") && is_rtl_language("he_IL.UTF-8") && is_rtl_language("fa"));
        assert!(!is_rtl_language("fr-FR") && !is_rtl_language("") && !is_rtl_language("arn"));
        let mut p = PlatformPrefs::default();
        assert!(right_to_left(&p, Some("ar")) && !right_to_left(&p, Some("en-US")) && !right_to_left(&p, None));
        p.direction = 2;
        assert!(right_to_left(&p, Some("en-US")));
        p.direction = 1;
        assert!(!right_to_left(&p, Some("ar")));
    }

    #[test]
    fn misspelt_words_are_underlined_and_the_rest_shown_as_typed() {
        let text = "I recieve *two* <b>\n    teh notes";
        let md = marked_markdown(text, &[(2, "recieve"), (24, "teh"), (99, "gone")], "#e5322d");
        assert_eq!(md, "I <u><font color=\"#e5322d\">recieve</font></u> \\*two\\* \\<b\\>\n\n<u><font color=\"#e5322d\">teh</font></u> notes");
        // Slint reads it without an error.
        assert!(slint::StyledText::from_markdown(&md).is_ok());
        assert!(slint::StyledText::from_markdown(&marked_markdown("1. a # b", &[], "#e5322d")).is_ok());
    }
}
