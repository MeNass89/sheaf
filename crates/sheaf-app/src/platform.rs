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
    /// Check Grammar With Spelling turned off (Preview's; Edit menu, Preferences ▸ Spelling).
    pub no_grammar_check: bool,
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
    /// The reader's tool letters changed by the user: tool id ([`TOOLS`]) and its key, a lower-case
    /// letter or digit ("" none).
    pub tool_keys: Vec<(String, String)>,
    /// The spelling dictionaries chosen, in order (language codes, "fr_FR"); none: the system's
    /// language, then English (Acrobat's dictionary list).
    pub dictionaries: Vec<String>,
    /// Keys that open a tool of the Tools page (Stirling's tool hotkeys): its `ToolId::as_str` and
    /// the keys as parts ("Control+Alt+M"). None by default.
    pub launch_keys: Vec<(String, String)>,
    /// The interface's language (iLovePDF's and Acrobat's): 0 the system's, then `i18n::LANGUAGES` from 1.
    pub language: u8,
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
/// The inspector: ⌃⌘I on a Mac (SwiftUI's InspectorCommands), Ctrl+Alt+I elsewhere.
const INSPECTOR: &str = if cfg!(target_os = "macos") { "Control+Meta+I" } else { "Control+Alt+I" };

/// The menu bar's commands, in menu order. `ui/main.slint` reads each one's shortcut as
/// `PlatformBridge.keys[<index here>]`: an entry is only ever added at the end (the test
/// `the_menu_bar_reads_each_command_at_its_index` holds the two together).
///
/// Undo, Redo, Cut, Copy, Paste and Select All show their usual keys and act on what has the
/// keyboard: the menu bar takes the keys first, then sends them on to the focused text field or
/// the reader's pages (`EDIT_KEYS`, `platform_ui::edit_key`).
pub const COMMANDS: &[Command] = &[
    cmd("new", "File", "New Blank PDF…", "Control+N"),
    cmd("open", "File", "Open…", "Control+O"),
    cmd("save", "File", "Save", "Control+S"),
    cmd("save-as", "File", "Save As…", "Control+Shift+S"),
    cmd("print", "File", "Print…", "Control+P"),
    cmd("properties", "File", "Document Properties…", "Control+I"),
    cmd("close", "File", "Close Tab", "Control+W"),
    cmd("quit", "File", "Quit Sheaf", "Control+Q"),
    cmd("undo", "Edit", "Undo", "Control+Z"),
    cmd("redo", "Edit", "Redo", "Control+Shift+Z"),
    cmd("select-all", "Edit", "Select All", "Control+A"),
    cmd("find", "Edit", "Find…", "Control+F"),
    cmd("find-next", "Edit", "Find Next", "Control+G"),
    cmd("find-previous", "Edit", "Find Previous", "Control+Shift+G"),
    cmd("spelling", "Edit", "Check Spelling While Typing", ""),
    cmd("preferences", "Edit", "Settings…", "Control+Comma"),
    cmd("zoom-in", "View", "Zoom In", "Control+Plus"),
    cmd("zoom-out", "View", "Zoom Out", "Control+HyphenMinus"),
    cmd("actual-size", "View", "Actual Size", "Control+Digit0"),
    cmd("fit-page", "View", "Zoom to Fit Page", "Control+Digit8"),
    cmd("fit-width", "View", "Zoom to Fit Width", "Control+Digit9"),
    cmd("panel", "View", "Show Sidebar", "Control+Alt+Digit0"),
    cmd("two-pages", "View", "Next Page Layout", "Control+Alt+L"),
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
    cmd("all-tools", "Tools", "All Tools…", ""),
    cmd("next-tab", "Window", "Next Tab", "Control+Tab"),
    cmd("previous-tab", "Window", "Previous Tab", "Control+Shift+Tab"),
    cmd("home", "Window", "Home", ""),
    cmd("shortcuts", "Help", "Keyboard Shortcuts", "Control+Slash"),
    cmd("customize", "Help", "Customize Shortcuts…", ""),
    // Edit ▸ Cut, Copy, Paste: ⌘X ⌘C ⌘V sent to what has the keyboard (see `EDIT_KEYS`).
    cmd("cut", "Edit", "Cut", "Control+X"),
    cmd("copy", "Edit", "Copy", "Control+C"),
    cmd("paste", "Edit", "Paste", "Control+V"),
    // Acrobat's Edit ▸ Check Spelling ▸ In Comments, Fields, & Editable Text (F7, as Acrobat and Word).
    cmd("check-spelling", "Edit", "Check Spelling…", "F7"),
    // Preview's Edit ▸ Spelling and Grammar ▸ Check Grammar With Spelling.
    cmd("grammar", "Edit", "Check Grammar With Spelling", ""),
    // DESIGN §1 command map (W6): the inspector (⌃⌘I), the sidebar's tabs (⌥⌘1…6), Add Bookmark
    // (⌘D, as PDF Expert), the page themes (⌥⌘D ⌥⌘S ⌥⌘N).
    cmd("inspector", "View", "Show Inspector", INSPECTOR),
    cmd("sidebar-thumbnails", "View", "Thumbnails", "Control+Alt+Digit1"),
    cmd("sidebar-outline", "View", "Outline", "Control+Alt+Digit2"),
    cmd("sidebar-bookmarks", "View", "Bookmarks", "Control+Alt+Digit3"),
    cmd("sidebar-annotations", "View", "Annotations", "Control+Alt+Digit4"),
    cmd("sidebar-attachments", "View", "Attachments", "Control+Alt+Digit5"),
    cmd("sidebar-layers", "View", "Layers", "Control+Alt+Digit6"),
    cmd("add-bookmark", "Go", "Add Bookmark", "Control+D"),
    cmd("theme-day", "View", "Day", "Control+Alt+D"),
    cmd("theme-sepia", "View", "Sepia", "Control+Alt+S"),
    cmd("theme-night", "View", "Night", "Control+Alt+N"),
];

/// Commands that act by sending their usual keys (⌘Z ⇧⌘Z ⌘X ⌘C ⌘V ⌘A) to whatever has the
/// keyboard: a focused text field undoes, cuts, copies, pastes or selects in its own text, else the
/// reader's pages act on the PDF (its history, selection, objects or comments). Those keys keep
/// working whatever the user gives the command (a text field answers them itself, which Slint
/// cannot turn off), and cannot be given to another command. (id, key sent, with ⇧.)
pub const EDIT_KEYS: &[(&str, &str, bool)] = &[("undo", "z", false), ("redo", "z", true), ("cut", "x", false), ("copy", "c", false), ("paste", "v", false), ("select-all", "a", false)];

/// Whether command `id` sends its keys on (an [`EDIT_KEYS`] command).
pub fn is_edit_command(id: &str) -> bool {
    EDIT_KEYS.iter().any(|(e, _, _)| *e == id)
}

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
    // An edit command may have its own usual keys back (⌘C for Copy).
    if parts != COMMANDS[i].keys {
        check_shortcut(parts)?;
    }
    if let Some(who) = taken_by(p, parts, Some(i), None) {
        return Err(format!("{} is already {who}", display(parts)));
    }
    p.shortcuts.retain(|(c, _)| c != id);
    if parts != COMMANDS[i].keys {
        p.shortcuts.push((id.to_string(), parts.to_string()));
    }
    Ok(())
}

/// Who has `parts` already, a command or a tool launcher (leaving out command `command` and launcher
/// `launcher`, the one being given them): "File ▸ Save", "Tools page ▸ Merge PDF".
fn taken_by(p: &PlatformPrefs, parts: &str, command: Option<usize>, launcher: Option<usize>) -> Option<String> {
    if parts.is_empty() {
        return None;
    }
    let wanted = keys(parts);
    if let Some(c) = (0..COMMANDS.len()).filter(|&k| Some(k) != command).find(|&k| keys(&effective(p, k)) == wanted).map(|k| COMMANDS[k]) {
        return Some(format!("{} ▸ {}", c.menu, c.title.trim_end_matches('…')));
    }
    let tools = launchers();
    (0..tools.len()).filter(|&k| Some(k) != launcher).find(|&k| keys(&launch_key(p, k)) == wanted).map(|k| format!("Tools page ▸ {}", tools[k].1))
}

/// The Tools page's tools, each of which can be given keys that open it (Stirling's): id, name.
pub fn launchers() -> Vec<(&'static str, &'static str)> {
    sheaf_core::tools::catalogue().iter().map(|t| (t.id.as_str(), t.name)).collect()
}

/// Launcher `k`'s keys ("" none).
pub fn launch_key(p: &PlatformPrefs, k: usize) -> String {
    let Some((id, _)) = launchers().get(k).copied() else { return String::new() };
    p.launch_keys.iter().find(|(t, _)| t == id).map(|(_, keys)| keys.clone()).unwrap_or_default()
}

/// The tool launcher a key pressed opens, if any (its index): compared as recorded, so the same
/// keys match. Asked only where no text field has the keyboard (main.slint's app scope).
pub fn launcher_for(p: &PlatformPrefs, text: &str, control: bool, alt: bool, shift: bool, meta: bool) -> Option<usize> {
    let parts = parts_of_event(text, control, alt, shift, meta);
    if parts.is_empty() {
        return None;
    }
    let wanted = keys(&parts);
    (0..launchers().len()).find(|&k| {
        let own = launch_key(p, k);
        !own.is_empty() && keys(&own) == wanted
    })
}

/// Gives launcher `k` the keys `parts` ("" none), with the same rules as a command's (a modifier,
/// not a text field's keys, not already someone's).
pub fn assign_launch(p: &mut PlatformPrefs, k: usize, parts: &str) -> Result<(), String> {
    let Some((id, _)) = launchers().get(k).copied() else { return Err(format!("No tool {k}")) };
    check_shortcut(parts)?;
    if let Some(who) = taken_by(p, parts, None, Some(k)) {
        return Err(format!("{} is already {who}", display(parts)));
    }
    p.launch_keys.retain(|(t, _)| t != id);
    if !parts.is_empty() {
        p.launch_keys.push((id.to_string(), parts.to_string()));
    }
    Ok(())
}

/// Reset on one command: back to its default keys, unless a command or a tool launcher has them by
/// now (then nothing changes, and the reason comes back).
pub fn reset_command(p: &mut PlatformPrefs, id: &str) -> Result<(), String> {
    let Some(i) = command_index(id) else { return Err(format!("No command {id}")) };
    let own = COMMANDS[i].keys;
    let mut next = p.clone();
    reset(&mut next, Some(id));
    if let Some(who) = taken_by(&next, own, Some(i), None) {
        return Err(format!("{} is already {who}", display(own)));
    }
    *p = next;
    Ok(())
}

/// The dictionaries as Preferences ▸ Spelling lists them: those chosen first, in their order, then
/// the others installed, by language: (code, chosen).
pub fn dictionary_rows(p: &PlatformPrefs, installed: &[String]) -> Vec<(String, bool)> {
    let chosen = p.dictionaries.iter().filter(|l| installed.contains(l)).map(|l| (l.clone(), true));
    let rest = installed.iter().filter(|l| !p.dictionaries.contains(l)).map(|l| (l.clone(), false));
    chosen.chain(rest).collect()
}

/// A dictionary ticked or unticked (it goes last among those chosen), or moved up (-1) or down (+1)
/// among them.
pub fn dictionary_change(p: &mut PlatformPrefs, lang: &str, step: i32) {
    let at = p.dictionaries.iter().position(|l| l == lang);
    match (at, step) {
        (Some(i), 0) => {
            p.dictionaries.remove(i);
        }
        (None, 0) => p.dictionaries.push(lang.to_string()),
        (Some(i), _) => {
            let to = (i as i32 + step).clamp(0, p.dictionaries.len() as i32 - 1) as usize;
            p.dictionaries.swap(i, to);
        }
        (None, _) => {}
    }
}

/// The modifiers held, as Slint names them (bits): Shift, Alt, Meta, Control. Slint calls a Mac's
/// Command key Control and its Control key Meta: on a Mac Command is restored as the Control the
/// Edit keys are sent with (never let go and pressed again as Meta, which would leave a Control
/// stuck), elsewhere Super (the Windows key) is Meta.
pub const HELD_SHIFT: u8 = 1;
pub const HELD_ALT: u8 = 2;
pub const HELD_META: u8 = 4;
pub const HELD_CTRL: u8 = 8;

/// The bits for the physical keys held (`super_key`: the Windows key, a Mac's Command).
pub fn held_modifiers(shift: bool, alt: bool, control: bool, super_key: bool, mac: bool) -> u8 {
    let (slint_control, slint_meta) = if mac { (super_key, control) } else { (control, super_key) };
    (u8::from(shift) * HELD_SHIFT) | (u8::from(alt) * HELD_ALT) | (u8::from(slint_meta) * HELD_META) | (u8::from(slint_control) * HELD_CTRL)
}

/// Back to the default keys: one command, or all of them (`None`).
pub fn reset(p: &mut PlatformPrefs, id: Option<&str>) {
    match id {
        Some(id) => p.shortcuts.retain(|(c, _)| c != id),
        None => p.shortcuts.clear(),
    }
}

/// Whether the user gave command `i` keys of its own: then the keys the reader and Home answer
/// besides the menu bar's (⌘= for Zoom In, ⇧⌘] for Next Tab, ⌘Z on the pages for Undo) stop
/// (`PlatformBridge.rebound[i]` in reader.slint and main.slint).
pub fn rebound(p: &PlatformPrefs, i: usize) -> bool {
    COMMANDS.get(i).is_some_and(|c| p.shortcuts.iter().any(|(id, _)| id == c.id))
}

/// A command's keys as Preferences and the help sheet write them ("" none). The edit commands
/// (Undo… Select All) keep their usual keys besides any the user gives them.
pub fn label(p: &PlatformPrefs, i: usize) -> String {
    let k = effective(p, i);
    match COMMANDS.get(i).filter(|c| is_edit_command(c.id)) {
        Some(c) if k != c.keys && k.is_empty() => display(c.keys),
        Some(c) if k != c.keys => format!("{}, {}", display(&k), display(c.keys)),
        _ => display(&k),
    }
}

/// The reader's tools by their number (`ReaderBridge.set-tool`), with Acrobat's and PDF Expert's
/// one-letter keys; the last, Marquee Zoom, is not a tool but has its letter too. Pressed on the
/// pages only (in a field they are typed).
pub const TOOLS: &[(&str, &str, &str)] = &[
    ("select", "Select Text", "v"),
    ("highlight", "Highlight", "h"),
    ("underline", "Underline", "u"),
    ("strikethrough", "Strikethrough", "k"),
    ("note", "Note", "n"),
    ("pen", "Pen", "p"),
    ("text-box", "Text Box", "t"),
    ("edit", "Edit Text and Objects", "e"),
    ("image", "Add Image", "i"),
    ("sign", "Sign", "s"),
    ("shapes", "Shapes", "r"),
    ("eraser", "Eraser", "x"),
    ("stamp", "Stamp", "m"),
    ("link", "Link", "l"),
    ("form", "Prepare Form", "f"),
    ("zoom-area", "Marquee Zoom", "z"),
];

/// Tool `k`'s key now ("" none).
pub fn tool_key(p: &PlatformPrefs, k: usize) -> String {
    let Some(t) = TOOLS.get(k) else { return String::new() };
    p.tool_keys.iter().find(|(id, _)| id == t.0).map(|(_, key)| key.clone()).unwrap_or_else(|| t.2.to_string())
}

/// The tool a key pressed on the pages picks (its number), if any. Case counts: ⇧ with a letter is
/// another command (⇧T hides the toolbar).
pub fn tool_for_key(p: &PlatformPrefs, text: &str) -> Option<usize> {
    if text.is_empty() {
        return None;
    }
    (0..TOOLS.len()).find(|&k| tool_key(p, k) == text)
}

/// Gives tool `k` the key `key` (a letter or digit alone, as typed; "" none). Refused when another
/// tool has it, with the reason.
pub fn assign_tool(p: &mut PlatformPrefs, k: usize, key: &str) -> Result<(), String> {
    let Some(t) = TOOLS.get(k) else { return Err(format!("No tool {k}")) };
    let key = key.to_lowercase();
    let mut chars = key.chars();
    let one = matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphanumeric());
    if !key.is_empty() && !one {
        return Err("A tool's key is one letter or digit, without ⌘/Ctrl, Alt or ⇧".into());
    }
    if let Some(other) = (0..TOOLS.len()).find(|&o| o != k && !key.is_empty() && tool_key(p, o) == key) {
        return Err(format!("{} is already Tools ▸ {}", key.to_uppercase(), TOOLS[other].1));
    }
    p.tool_keys.retain(|(id, _)| id != t.0);
    if key != t.2 {
        p.tool_keys.push((t.0.to_string(), key));
    }
    Ok(())
}

/// A row of Preferences ▸ Keyboard: its index (a command's, then `COMMANDS.len()` + a tool's),
/// "Menu ▸ Title", keys shown, changed.
pub type ShortcutRow = (usize, String, String, bool);

/// The rows whose menu, title or keys contain `query` (Stirling's Search tools), all of them
/// when it is empty: the menu bar's commands, then the reader's tool letters.
pub fn shortcut_rows(p: &PlatformPrefs, query: &str) -> Vec<ShortcutRow> {
    let q = query.trim().to_lowercase();
    let commands = COMMANDS.iter().enumerate().map(|(i, c)| (i, format!("{} ▸ {}", c.menu, c.title), label(p, i), rebound(p, i)));
    let tools = TOOLS.iter().enumerate().map(|(k, t)| {
        let key = tool_key(p, k);
        (COMMANDS.len() + k, format!("Tools ▸ {} (on the pages)", t.1), key.to_uppercase(), key != t.2)
    });
    let launch = launchers().into_iter().enumerate().map(|(k, (_, name))| {
        let key = launch_key(p, k);
        (COMMANDS.len() + TOOLS.len() + k, format!("Tools page ▸ {name}"), display(&key), !key.is_empty())
    });
    commands
        .chain(tools)
        .chain(launch)
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
            assert!(check_shortcut(c.keys).is_ok() || is_edit_command(c.id), "{}", c.id);
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
        assert_eq!(shortcut_rows(&p, "").len(), COMMANDS.len() + TOOLS.len() + launchers().len());
        // Undo shows its usual ⌘Z (the menu bar sends it on to the field or the pages).
        let undo = shortcut_rows(&p, "undo");
        assert_eq!(undo[0].2, display("Control+Z"));
        assert_eq!(shortcut_rows(&p, "highlight")[0].2, "H");
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

    #[test]
    fn tool_letters_change_and_conflict() {
        let mut p = PlatformPrefs::default();
        assert_eq!(tool_for_key(&p, "h"), Some(1));
        assert_eq!(tool_for_key(&p, "H"), None);
        assert_eq!(tool_for_key(&p, "z"), Some(TOOLS.len() - 1));
        // Highlight's H given to Note: refused, Highlight has it.
        assert!(assign_tool(&mut p, 4, "H").unwrap_err().contains("Highlight"));
        assert!(assign_tool(&mut p, 4, "ctrl").is_err());
        assign_tool(&mut p, 4, "Q").unwrap();
        assert_eq!((tool_for_key(&p, "q"), tool_for_key(&p, "n")), (Some(4), None));
        // N is free now; none at all is allowed; back to its own: nothing kept.
        assign_tool(&mut p, 1, "n").unwrap();
        assign_tool(&mut p, 2, "").unwrap();
        assert_eq!(tool_for_key(&p, "u"), None);
        assign_tool(&mut p, 4, "n").unwrap_err();
        assign_tool(&mut p, 1, "h").unwrap();
        assign_tool(&mut p, 4, "n").unwrap();
        assign_tool(&mut p, 2, "u").unwrap();
        assert!(p.tool_keys.is_empty());
    }

    #[test]
    fn rebound_commands_stop_their_other_keys() {
        // reader.slint and main.slint guard each such key with `!PlatformBridge.rebound[i] /* id */`:
        // the index must be that command's.
        for ui in [include_str!("../ui/screens/reader.slint"), include_str!("../ui/main.slint")] {
            for part in ui.split("PlatformBridge.rebound[").skip(1) {
                let (i, rest) = part.split_once("] /* ").expect("rebound[i] /* id */");
                let id = rest.split(" */").next().unwrap();
                assert_eq!(COMMANDS[i.parse::<usize>().unwrap()].id, id);
            }
        }
        let mut p = PlatformPrefs::default();
        let zoom = command_index("zoom-in").unwrap();
        assert!(!rebound(&p, zoom));
        assign(&mut p, "zoom-in", "Control+Alt+Plus").unwrap();
        assert!(rebound(&p, zoom));
        // Undo given keys of its own: ⌘Z on the pages stops, and the row shows the new keys.
        assign(&mut p, "undo", "Control+Alt+U").unwrap();
        assert_eq!(label(&p, command_index("undo").unwrap()), format!("{}, {}", display("Control+Alt+U"), display("Control+Z")));
        // Copy keeps ⌘C besides its new keys (fields answer it themselves).
        assign(&mut p, "copy", "F6").unwrap();
        assert!(label(&p, command_index("copy").unwrap()).ends_with(&display("Control+C")));
        assert!(EDIT_KEYS.iter().all(|(id, _, _)| command_index(id).is_some()));
        // Its own ⌘C back: allowed, though no other command may take it.
        assign(&mut p, "copy", "Control+C").unwrap();
        assert!(assign(&mut p, "find", "Control+C").is_err());
    }

    #[test]
    fn every_tool_can_get_keys_that_open_it() {
        let mut p = PlatformPrefs::default();
        let merge = launchers().iter().position(|(id, _)| *id == "merge").unwrap();
        let split = launchers().iter().position(|(id, _)| *id == "split").unwrap();
        assert_eq!(launchers().len(), sheaf_core::tools::catalogue().len());
        assert_eq!(launch_key(&p, merge), "");
        assign_launch(&mut p, merge, "Control+Alt+M").unwrap();
        assert_eq!(launch_key(&p, merge), "Control+Alt+M");
        // Taken by Merge: refused for Split and for a command; a command's keys refused here.
        assert!(assign_launch(&mut p, split, "Control+Alt+M").unwrap_err().contains("Tools page ▸ Merge PDF"));
        assert!(assign(&mut p, "find", "Control+Alt+M").unwrap_err().contains("Merge PDF"));
        assert!(assign_launch(&mut p, split, "Control+S").unwrap_err().contains("File ▸ Save"));
        assert!(assign_launch(&mut p, split, "M").is_err());
        let rows = shortcut_rows(&p, "merge pdf");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, COMMANDS.len() + TOOLS.len() + merge);
        assert!(rows[0].3);
        assign_launch(&mut p, merge, "").unwrap();
        assert!(p.launch_keys.is_empty());
    }

    #[test]
    fn reset_refuses_keys_a_tool_has_taken() {
        let mut p = PlatformPrefs::default();
        let merge = launchers().iter().position(|(id, _)| *id == "merge").unwrap();
        assign(&mut p, "save", "Control+Alt+E").unwrap();
        assign_launch(&mut p, merge, "Control+S").unwrap();
        // Save's own Ctrl+S is Merge PDF's now: Reset says so and changes nothing.
        assert!(reset_command(&mut p, "save").unwrap_err().contains("Merge PDF"));
        assert_eq!(effective(&p, command_index("save").unwrap()), "Control+Alt+E");
        assign_launch(&mut p, merge, "").unwrap();
        reset_command(&mut p, "save").unwrap();
        assert_eq!(effective(&p, command_index("save").unwrap()), "Control+S");
    }

    #[test]
    fn dictionaries_are_chosen_and_ordered() {
        let installed = ["de_DE".to_string(), "en_US".to_string(), "fr_FR".to_string()];
        let mut p = PlatformPrefs::default();
        assert!(dictionary_rows(&p, &installed).iter().all(|(_, on)| !on));
        dictionary_change(&mut p, "fr_FR", 0);
        dictionary_change(&mut p, "en_US", 0);
        assert_eq!(dictionary_rows(&p, &installed), [("fr_FR".into(), true), ("en_US".into(), true), ("de_DE".into(), false)]);
        // English first, then French; French unticked; a dictionary no longer installed is not shown.
        dictionary_change(&mut p, "en_US", -1);
        assert_eq!(p.dictionaries, ["en_US", "fr_FR"]);
        dictionary_change(&mut p, "fr_FR", 0);
        p.dictionaries.push("it_IT".into());
        assert_eq!(dictionary_rows(&p, &installed)[0], ("en_US".into(), true));
        assert_eq!(dictionary_rows(&p, &installed).len(), 3);
    }

    #[test]
    fn a_macs_command_key_is_slints_control_and_stays_held() {
        // Cmd+Shift+U given to Undo on a Mac: Shift is in the way, Command is the Control kept.
        assert_eq!(held_modifiers(true, false, false, true, true), HELD_SHIFT | HELD_CTRL);
        // A Mac's Control key is Slint's Meta (the one let go and pressed again).
        assert_eq!(held_modifiers(false, false, true, false, true), HELD_META);
        // Elsewhere Super is Meta, Control is Control.
        assert_eq!(held_modifiers(false, true, true, true, false), HELD_ALT | HELD_META | HELD_CTRL);
        assert_eq!(held_modifiers(false, false, true, false, false), HELD_CTRL);
    }

    #[test]
    fn a_tools_keys_are_found_from_the_key_pressed() {
        let mut p = PlatformPrefs::default();
        let merge = launchers().iter().position(|(id, _)| *id == "merge").unwrap();
        assign_launch(&mut p, merge, "Control+Alt+M").unwrap();
        assert_eq!(launcher_for(&p, "m", true, true, false, false), Some(merge));
        assert_eq!(launcher_for(&p, "m", true, false, false, false), None);
        assert_eq!(launcher_for(&p, "\u{11}", true, true, false, false), None);
        assert_eq!(launcher_for(&PlatformPrefs::default(), "m", true, true, false, false), None);
        // Where they are asked: main.slint asks only when its own scope or the pages have the keyboard.
        let ui = include_str!("../ui/main.slint");
        assert!(ui.contains("if !PlatformBridge.typing && PlatformBridge.launcher-for("));
    }

    #[test]
    fn every_text_field_says_when_it_has_the_keyboard() {
        // A Tools page tool's keys wait while one has it (main.slint asks `PlatformBridge.typing`).
        for ui in [
            include_str!("../ui/screens/reader-parts.slint"),
            include_str!("../ui/screens/actions.slint"),
            include_str!("../ui/screens/library.slint"),
            include_str!("../ui/screens/reader-form.slint"),
            include_str!("../ui/screens/javascript.slint"),
            include_str!("../ui/screens/reader-options.slint"),
            include_str!("../ui/screens/options-parts.slint"),
            include_str!("../ui/screens/tools.slint"),
            include_str!("../ui/screens/reader-toolbar.slint"),
            include_str!("../ui/screens/reader-sheets.slint"),
            include_str!("../ui/screens/reader-panel.slint"),
            include_str!("../ui/screens/reader.slint"),
            include_str!("../ui/widgets/file-card.slint"),
            include_str!("../ui/widgets/text-field.slint"),
        ] {
            let fields = ui.matches("TextInput {").count() + ui.matches("TextEdit {").count();
            assert_eq!(fields, ui.matches("PlatformBridge.typing = self.has-focus").count());
        }
    }

    /// The positions of every `HorizontalLayout {` of a .slint text not inside a PopupWindow.
    fn rows_outside_popups(ui: &str) -> Vec<usize> {
        let mut open: Vec<bool> = Vec::new();
        let mut out = Vec::new();
        let b = ui.as_bytes();
        let mut i = 0;
        let mut in_string = false;
        while i < b.len() {
            match b[i] {
                b'"' => in_string = !in_string,
                b'/' if !in_string && b.get(i + 1) == Some(&b'/') => {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                    continue;
                }
                b'{' if !in_string => {
                    let head = ui[..i].trim_end();
                    let popup = head.ends_with("PopupWindow") || open.last().copied().unwrap_or(false);
                    if head.ends_with("HorizontalLayout") && !popup {
                        out.push(i);
                    }
                    open.push(popup);
                }
                b'}' if !in_string => {
                    open.pop();
                }
                _ => {}
            }
            i += 1;
        }
        out
    }

    #[test]
    fn right_to_left_screens_have_no_row_that_cannot_reverse() {
        // Mirrored screens use direction-aware FlexboxLayouts; a HorizontalLayout is left only inside
        // a popup menu.
        for (name, ui) in [
            ("screens/reader-sheets", include_str!("../ui/screens/reader-sheets.slint")),
            ("screens/recents", include_str!("../ui/screens/recents.slint")),
            ("screens/tools", include_str!("../ui/screens/tools.slint")),
            ("screens/history", include_str!("../ui/screens/history.slint")),
            ("screens/settings-form", include_str!("../ui/screens/settings-form.slint")),
            ("screens/settings-tools", include_str!("../ui/screens/settings-tools.slint")),
            ("screens/settings-window", include_str!("../ui/screens/settings-window.slint")),
            ("screens/preferences-platform", include_str!("../ui/screens/preferences-platform.slint")),
            ("screens/dialogs", include_str!("../ui/screens/dialogs.slint")),
            ("screens/reader-options", include_str!("../ui/screens/reader-options.slint")),
            ("screens/reader-panel", include_str!("../ui/screens/reader-panel.slint")),
            ("screens/tool-flow", include_str!("../ui/screens/tool-flow.slint")),
            ("screens/options", include_str!("../ui/screens/options.slint")),
            ("screens/options-parts", include_str!("../ui/screens/options-parts.slint")),
            ("screens/result", include_str!("../ui/screens/result.slint")),
            ("screens/spelling-dialog", include_str!("../ui/screens/spelling-dialog.slint")),
            ("screens/reader-toolbar", include_str!("../ui/screens/reader-toolbar.slint")),
            // The tick boxes, radio cards, segments and information boxes every screen above uses.
            ("widgets/check", include_str!("../ui/widgets/check.slint")),
            ("widgets/radio-card", include_str!("../ui/widgets/radio-card.slint")),
        ] {
            assert!(rows_outside_popups(ui).is_empty(), "{name} has a HorizontalLayout outside a popup");
            assert!(ui.contains("PlatformBridge.rtl"), "{name} does not follow the direction");
        }
        // The tick box and the radio circle move to the label's right right to left.
        let check = include_str!("../ui/widgets/check.slint");
        assert!(check.contains("x: PlatformBridge.rtl ? parent.width - self.width : 0;"), "the tick box does not mirror");
        let radio = include_str!("../ui/widgets/radio-card.slint");
        assert!(radio.contains("x: PlatformBridge.rtl ? parent.width - self.width - 12px : 12px;"), "the radio circle does not mirror");
        assert_eq!(rows_outside_popups("Rectangle { HorizontalLayout { } }").len(), 1);
        assert!(rows_outside_popups("menu := PopupWindow { VerticalLayout { HorizontalLayout { } } }").is_empty());
    }
}
