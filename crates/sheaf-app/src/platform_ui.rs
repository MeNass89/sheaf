//! The `PlatformBridge` global (ui/platform.slint): check spelling while typing, the menu bar's
//! shortcuts and Preferences ▸ Keyboard, the tools shown, the interface's direction, Quit.
//!
//! Spelling (Acrobat's Check Spelling, Preview's Check Spelling While Typing): the comment, text
//! box or form field being typed is checked a moment after typing pauses, on a worker thread
//! (the dictionary is read once, on the first check, from the system's Hunspell folders or the
//! `dictionaries` folder of Sheaf's data); the misspelt words come back underlined in a card under
//! the editor, each with its corrections, Ignore and Learn.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use sheaf_app::platform::{self, PlatformPrefs, COMMANDS, TOOLS};
use sheaf_app::{AppWindow, Launcher, PlatformBridge, ShortcutRow, SpellIssue};
use sheaf_core::spell::Speller;
use sheaf_core::tools::ToolId;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::home;

/// How long typing pauses before the text is checked.
const PAUSE: Duration = Duration::from_millis(250);
/// Misspelt words listed under the editor (all are underlined).
const SHOWN: usize = 4;
/// Corrections offered for each.
const CORRECTIONS: usize = 3;

/// The dictionary (or why there is none) and the corrections already found.
struct Spell {
    speller: Result<Speller, String>,
    corrections: HashMap<String, Vec<String>>,
}

static SPELL: Mutex<Option<Spell>> = Mutex::new(None);
/// The latest check asked: an older one finishing late is dropped.
static LATEST: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static TIMER: slint::Timer = slint::Timer::default();
    /// The text last checked and whether it was being typed: checked again after Ignore or Learn.
    static LAST: RefCell<(String, bool)> = RefCell::default();
    /// Preferences ▸ Keyboard's search.
    static QUERY: RefCell<String> = RefCell::default();
    /// The tools' letters (pushed with the keys): looked up on every key pressed on the pages.
    static TOOL_KEYS: RefCell<PlatformPrefs> = RefCell::default();
}

/// Runs `f` with the dictionary, read on first use.
fn with_spell<R>(f: impl FnOnce(&mut Spell) -> R) -> R {
    let mut guard = SPELL.lock().unwrap_or_else(|e| e.into_inner());
    let spell = guard.get_or_insert_with(|| {
        let langs: Vec<String> = sys_locale::get_locales().take(3).collect();
        let dirs = [sheaf_app::store_file::data_dir().join("dictionaries")];
        let mut speller = Speller::load(&langs, &dirs);
        if let Ok(s) = speller.as_mut() {
            for w in &learned_at_start() {
                s.learn(w);
            }
        }
        Spell { speller, corrections: HashMap::new() }
    });
    f(spell)
}

/// The learned words, read on the UI thread when the window starts (the worker cannot reach
/// the preferences).
static LEARNED: OnceLock<Vec<String>> = OnceLock::new();
fn learned_at_start() -> Vec<String> {
    LEARNED.get().cloned().unwrap_or_default()
}

/// Every misspelt word (offset, word), and the first [`SHOWN`] with their corrections.
/// (Grammar findings carry what is wrong; misspelt words an empty note.)
type Checked = (Vec<(usize, String)>, Vec<(usize, String, Vec<String>, &'static str)>);

/// Grammar findings Ignored this session, by what is wrong and the text ("Repeated word|the the").
static IGNORED_GRAMMAR: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn grammar_key(note: &str, text: &str) -> String {
    format!("{note}|{}", text.to_lowercase())
}

/// The misspelt words of `text` and, with `grammar`, its grammar findings (Preview's Check Grammar
/// With Spelling), in reading order; the first [`SHOWN`] with their fixes. While typing, a finding
/// that reaches the end of the text waits until it is finished.
fn check_text(text: &str, typing: bool, grammar: bool) -> Checked {
    let ignored = IGNORED_GRAMMAR.lock().map(|g| g.clone()).unwrap_or_default();
    with_spell(|sp| {
        // The English rules only with an English dictionary (none: English is assumed).
        let english = sp.speller.as_ref().map_or(true, Speller::is_english);
        let found: Vec<sheaf_core::spell::GrammarIssue> = if grammar {
            sheaf_core::spell::grammar(text, english)
                .into_iter()
                .filter(|g| !(typing && g.start + g.text.len() >= text.trim_end().len()))
                .filter(|g| !ignored.contains(&grammar_key(g.note, &g.text)))
                .collect()
        } else {
            Vec::new()
        };
        let misspelt = match &sp.speller {
            Ok(speller) => speller.misspelt(text, typing),
            Err(_) => Vec::new(),
        };
        let mut all: Vec<(usize, String, Option<sheaf_core::spell::GrammarIssue>)> = misspelt.into_iter().map(|i| (i.start, i.word, None)).collect();
        all.extend(found.into_iter().map(|g| (g.start, g.text.clone(), Some(g))));
        all.sort_by_key(|f| f.0);
        let mut shown = Vec::new();
        for (start, word, g) in all.iter().take(SHOWN) {
            match g {
                Some(g) => shown.push((*start, word.clone(), vec![g.fix.clone()], g.note)),
                None => {
                    let Ok(speller) = &sp.speller else { continue };
                    let c = sp.corrections.entry(word.clone()).or_insert_with(|| speller.suggest(word, CORRECTIONS)).clone();
                    shown.push((*start, word.clone(), c, ""));
                }
            }
        }
        (all.into_iter().map(|(s, w, _)| (s, w)).collect(), shown)
    })
}

fn spell_on() -> bool {
    !home::prefs().platform.no_spell_check
}

/// Check Grammar With Spelling (Preview's), on unless turned off.
fn grammar_on() -> bool {
    !home::prefs().platform.no_grammar_check
}

fn clear(ui: &AppWindow) {
    let g = ui.global::<PlatformBridge>();
    g.set_spell_issues(ModelRc::new(VecModel::from(Vec::<SpellIssue>::new())));
    g.set_spell_marked(slint::StyledText::default());
}

/// The card under the editor: the misspelt words of `text`, underlined in the theme's red.
fn show(ui: &AppWindow, text: &str, (all, shown): Checked) {
    if shown.is_empty() {
        return clear(ui);
    }
    let red = ui.global::<sheaf_app::Theme>().get_danger();
    let colour = format!("#{:02x}{:02x}{:02x}", red.red(), red.green(), red.blue());
    let marks: Vec<(usize, &str)> = all.iter().map(|(s, w)| (*s, w.as_str())).collect();
    let md = platform::marked_markdown(text, &marks, &colour);
    let g = ui.global::<PlatformBridge>();
    g.set_spell_marked(slint::StyledText::from_markdown(&md).unwrap_or_else(|_| slint::StyledText::from_plain_text(text)));
    let rows: Vec<SpellIssue> = shown
        .into_iter()
        .map(|(start, word, c, note)| SpellIssue {
            note: note.into(),
            word: word.into(),
            start: start as i32,
            suggestions: ModelRc::new(VecModel::from(c.into_iter().map(SharedString::from).collect::<Vec<_>>())),
        })
        .collect();
    g.set_spell_issues(ModelRc::new(VecModel::from(rows)));
}

/// The editor's text changed: checked once typing pauses, off the UI thread.
fn spell_check(ui: &AppWindow, text: &str, typing: bool) {
    LAST.with(|l| *l.borrow_mut() = (text.to_string(), typing));
    let seq = LATEST.fetch_add(1, Ordering::Relaxed) + 1;
    if !spell_on() || text.trim().is_empty() {
        TIMER.with(slint::Timer::stop);
        return clear(ui);
    }
    let weak = ui.as_weak();
    let text = text.to_string();
    let grammar = grammar_on();
    TIMER.with(|t| {
        t.start(slint::TimerMode::SingleShot, PAUSE, move || {
            let (weak, text) = (weak.clone(), text.clone());
            std::thread::spawn(move || {
                let checked = check_text(&text, typing, grammar);
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    if LATEST.load(Ordering::Relaxed) == seq {
                        show(&ui, &text, checked);
                    }
                });
            });
        });
    });
}

/// The text last checked, checked again (after Ignore, Learn, a dictionary change).
fn check_again(ui: &AppWindow) {
    let (text, typing) = LAST.with(|l| l.borrow().clone());
    spell_check(ui, &text, typing);
}

fn update(ui: &AppWindow, f: impl FnOnce(&mut PlatformPrefs)) {
    home::update_platform(ui, f);
    push(ui);
}

/// Which tools cannot run on this machine, found once.
fn availability() -> (bool, bool) {
    static FOUND: OnceLock<(bool, bool)> = OnceLock::new();
    *FOUND.get_or_init(|| (sheaf_core::office::find_soffice().is_some(), sheaf_core::ocr::available()))
}

/// The tools left off the Tools page and its search (Preferences ▸ Tools, Stirling's).
pub fn hidden_tools() -> Vec<ToolId> {
    if !home::prefs().platform.hide_unavailable_tools {
        return Vec::new();
    }
    let (office, ocr) = availability();
    sheaf_core::tools::catalogue().iter().map(|t| t.id).filter(|&id| platform::unavailable(id, office, ocr).is_some()).collect()
}

/// "Word to PDF, Excel to PDF (needs LibreOffice)": what hiding would leave off.
fn unavailable_text() -> String {
    let (office, ocr) = availability();
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for t in sheaf_core::tools::catalogue() {
        if let Some(why) = platform::unavailable(t.id, office, ocr) {
            match groups.iter_mut().find(|(w, _)| *w == why) {
                Some((_, names)) => names.push(t.name),
                None => groups.push((why, vec![t.name])),
            }
        }
    }
    groups.iter().map(|(why, names)| format!("{} ({why})", names.join(", "))).collect::<Vec<_>>().join("; ")
}

/// The menu bar's shortcuts, the tool launchers' included, off: a shortcut being recorded, or keys
/// sent on by Edit ▸ Copy and the like, go past it. [`push_keys`] puts them back.
fn menu_keys_off(ui: &AppWindow) {
    let g = ui.global::<PlatformBridge>();
    g.set_keys(ModelRc::new(VecModel::from(vec![slint::Keys::default(); COMMANDS.len()])));
    let launchers: Vec<Launcher> = g.get_launchers().iter().map(|l| Launcher { keys: slint::Keys::default(), ..l }).collect();
    g.set_launchers(ModelRc::new(VecModel::from(launchers)));
}

fn push_keys(ui: &AppWindow) {
    let p = home::prefs().platform;
    // Tools ▸ Open Tool: every tool shown on the Tools page, with the keys that open it.
    let hidden = hidden_tools();
    let launchers: Vec<Launcher> = platform::launchers()
        .into_iter()
        .enumerate()
        .filter(|(_, (id, _))| !hidden.iter().any(|h| h.as_str() == *id))
        .map(|(k, (id, name))| Launcher { id: id.into(), name: name.into(), keys: platform::keys(&platform::launch_key(&p, k)) })
        .collect();
    ui.global::<PlatformBridge>().set_launchers(ModelRc::new(VecModel::from(launchers)));
    let keys: Vec<slint::Keys> = (0..COMMANDS.len()).map(|i| platform::keys(&platform::effective(&p, i))).collect();
    let g = ui.global::<PlatformBridge>();
    g.set_keys(ModelRc::new(VecModel::from(keys)));
    g.set_rebound(ModelRc::new(VecModel::from((0..COMMANDS.len()).map(|i| platform::rebound(&p, i)).collect::<Vec<_>>())));
    g.set_key_labels(ModelRc::new(VecModel::from((0..COMMANDS.len()).map(|i| SharedString::from(platform::label(&p, i))).collect::<Vec<_>>())));
    TOOL_KEYS.with(|t| t.borrow_mut().tool_keys = p.tool_keys.clone());
    g.set_tool_letters(ModelRc::new(VecModel::from((0..TOOLS.len()).map(|k| SharedString::from(platform::tool_key(&p, k).to_uppercase())).collect::<Vec<_>>())));
}

fn push_rows(ui: &AppWindow) {
    let p = home::prefs().platform;
    let q = QUERY.with(|q| q.borrow().clone());
    let rows: Vec<ShortcutRow> = platform::shortcut_rows(&p, &q).into_iter().map(|(i, name, keys, custom)| ShortcutRow { index: i as i32, name: name.into(), keys: keys.into(), custom }).collect();
    ui.global::<PlatformBridge>().set_shortcut_rows(ModelRc::new(VecModel::from(rows)));
}

/// Everything the global shows, from the preferences.
fn push(ui: &AppWindow) {
    let p = home::prefs().platform;
    let g = ui.global::<PlatformBridge>();
    g.set_spell_on(!p.no_spell_check);
    g.set_grammar_on(!p.no_grammar_check);
    g.set_learned(ModelRc::new(VecModel::from(p.learned_words.iter().map(|w| SharedString::from(w.as_str())).collect::<Vec<_>>())));
    g.set_hide_unavailable(p.hide_unavailable_tools);
    g.set_direction(i32::from(p.direction));
    g.set_rtl(platform::right_to_left(&p, sys_locale::get_locale().as_deref()));
    push_keys(ui);
    push_rows(ui);
}

/// The dictionary read in the background as the window starts, so the first check is quick and
/// Preferences ▸ Spelling says which one is used (or what is missing).
fn load_dictionary(ui: &AppWindow) {
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let said = with_spell(|sp| match &sp.speller {
            Ok(s) => format!("Checking in {}", s.describe()),
            Err(e) => e.clone(),
        });
        let _ = weak.upgrade_in_event_loop(move |ui| ui.global::<PlatformBridge>().set_spell_dictionary(said.into()));
    });
}

/// A key pressed while Preferences ▸ Keyboard records command `i`'s shortcut: true when the
/// recording ends. Escape alone cancels, Backspace or Delete alone removes the shortcut.
fn record(ui: &AppWindow, i: i32, text: &str, control: bool, alt: bool, shift: bool, meta: bool) -> bool {
    let g = ui.global::<PlatformBridge>();
    let bare = !(control || alt || shift || meta);
    // A tool launcher (rows after the tool letters): keys as a command's.
    if let Some(k) = (i.max(0) as usize).checked_sub(COMMANDS.len() + TOOLS.len()) {
        let parts = match text {
            "\u{1b}" if bare => {
                g.set_shortcut_error("".into());
                return true;
            }
            "\u{8}" | "\u{7f}" if bare => String::new(),
            _ => {
                let p = platform::parts_of_event(text, control, alt, shift, meta);
                if p.is_empty() {
                    return false;
                }
                p
            }
        };
        let mut prefs = home::prefs().platform;
        match platform::assign_launch(&mut prefs, k, &parts) {
            Ok(()) => {
                g.set_shortcut_error("".into());
                update(ui, |p| *p = prefs);
            }
            Err(e) => g.set_shortcut_error(e.into()),
        }
        return true;
    }
    // A tool's letter (rows after the commands): one key alone, as typed on the pages.
    if let Some(k) = (i.max(0) as usize).checked_sub(COMMANDS.len()) {
        let key = match text {
            "\u{1b}" if bare => {
                g.set_shortcut_error("".into());
                return true;
            }
            "\u{8}" | "\u{7f}" if bare => "",
            // A modifier alone: still waiting for the key.
            t if t.chars().next().is_some_and(|c| ('\u{10}'..='\u{18}').contains(&c)) => return false,
            t if bare => t,
            _ => {
                g.set_shortcut_error("A tool's key is one letter or digit alone, without ⌘/Ctrl, Alt or ⇧".into());
                return true;
            }
        };
        let mut prefs = home::prefs().platform;
        match platform::assign_tool(&mut prefs, k, key) {
            Ok(()) => {
                g.set_shortcut_error("".into());
                update(ui, |p| *p = prefs);
            }
            Err(e) => g.set_shortcut_error(e.into()),
        }
        return true;
    }
    let Some(c) = COMMANDS.get(i.max(0) as usize) else { return true };
    let parts = match text {
        "\u{1b}" if bare => {
            g.set_shortcut_error("".into());
            return true;
        }
        "\u{8}" | "\u{7f}" if bare => String::new(),
        _ => {
            let p = platform::parts_of_event(text, control, alt, shift, meta);
            if p.is_empty() {
                return false;
            }
            p
        }
    };
    let mut prefs = home::prefs().platform;
    match platform::assign(&mut prefs, c.id, &parts) {
        Ok(()) => {
            g.set_shortcut_error("".into());
            update(ui, |p| *p = prefs);
        }
        Err(e) => g.set_shortcut_error(e.into()),
    }
    true
}

/// Edit ▸ Undo, Redo, Cut, Copy, Paste, Select All, from the menu or from their keys (which the
/// menu bar takes first): ⌘ (Ctrl), ⇧ for Redo, and `key` sent to whatever has the keyboard, as if
/// typed — a text field undoes, cuts, copies, pastes or selects in its own text; the reader's pages
/// act on the PDF (its history, selection, picked objects or comment; reader.slint's ⌘Z ⌘C ⌘X ⌘V
/// ⌘A). Sent once a menu opened by the mouse has closed and given the keyboard back, with the menu
/// bar's shortcuts off meanwhile so these keys go past it. From the keys themselves (Ctrl or ⌘
/// held: no menu open) they are sent without the wait, so the keys typed next come after them (a
/// letter typed right after ⌘A was replaced by the selection made after it, 27/10). The right-hand Ctrl and Shift
/// are the ones pressed and let go: a left Ctrl the user is holding stays held.
fn edit_key(ui: &AppWindow, key: &str, shift: bool) {
    use slint::platform::{Key, WindowEvent};
    let weak = ui.as_weak();
    let key = SharedString::from(key);
    let send = move || {
        let Some(ui) = weak.upgrade() else { return };
        menu_keys_off(&ui);
        let ctrl = SharedString::from(Key::ControlR);
        let shift_key = SharedString::from(Key::ShiftR);
        // Shift, Alt or Super the user still holds (a shortcut such as Ctrl+Shift+U given to Undo)
        // would change the key sent (⇧⌘Z is Redo): let go for the key, pressed again after it.
        let held = HELD.load(Ordering::Relaxed);
        let in_the_way: Vec<(Key, Key)> = [(HELD_SHIFT, Key::Shift, Key::ShiftR), (HELD_ALT, Key::Alt, Key::AltGr), (HELD_META, Key::Meta, Key::MetaR)]
            .into_iter()
            .filter(|&(bit, left, _)| held & bit != 0 && !(shift && matches!(left, Key::Shift)))
            .map(|(_, left, right)| (left, right))
            .collect();
        let mut events = Vec::new();
        for (left, right) in &in_the_way {
            events.push(WindowEvent::KeyReleased { text: SharedString::from(*left) });
            events.push(WindowEvent::KeyReleased { text: SharedString::from(*right) });
        }
        events.push(WindowEvent::KeyPressed { text: ctrl.clone() });
        if shift {
            events.push(WindowEvent::KeyPressed { text: shift_key.clone() });
        }
        let text: SharedString = if shift { key.to_uppercase().into() } else { key.clone() };
        events.push(WindowEvent::KeyPressed { text: text.clone() });
        events.push(WindowEvent::KeyReleased { text });
        if shift {
            events.push(WindowEvent::KeyReleased { text: shift_key });
        }
        events.push(WindowEvent::KeyReleased { text: ctrl });
        for (left, _) in &in_the_way {
            events.push(WindowEvent::KeyPressed { text: SharedString::from(*left) });
        }
        for ev in events {
            let _ = ui.window().dispatch_event_with_result(ev);
        }
        push_keys(&ui);
    };
    // Not from inside the menu bar's own key handling (Slint's menu is borrowed there): on the next
    // turn of the event loop, before keys typed even a few milliseconds later.
    let wait = if HELD.load(Ordering::Relaxed) & (HELD_CTRL | HELD_META) != 0 { 0 } else { 30 };
    slint::Timer::single_shot(Duration::from_millis(wait), send);
}

/// The modifiers the user holds now (from the window's own events, main.rs): Shift, Alt, Super, Ctrl.
static HELD: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
const HELD_SHIFT: u8 = 1;
const HELD_ALT: u8 = 2;
const HELD_META: u8 = 4;
const HELD_CTRL: u8 = 8;

/// The window's modifiers changed (winit's ModifiersChanged).
pub fn modifiers_changed(shift: bool, alt: bool, meta: bool, ctrl: bool) {
    HELD.store((u8::from(shift) * HELD_SHIFT) | (u8::from(alt) * HELD_ALT) | (u8::from(meta) * HELD_META) | (u8::from(ctrl) * HELD_CTRL), Ordering::Relaxed);
}

/// Edit ▸ Check Spelling… on a document: its texts (comments, replies, form fields), what the
/// pass found in them (scanned once, on a worker thread, when it starts), and the changes made.
struct Pass {
    tab: i32,
    texts: Vec<crate::reader_ui::SpellText>,
    /// `None` while the texts are being scanned.
    check: Option<sheaf_core::spell::CheckPass>,
    changed: usize,
}

thread_local! {
    static PASS: RefCell<Option<Pass>> = const { RefCell::new(None) };
}
/// The finding shown: its corrections, looked up off the UI thread, are kept only if it is still
/// the one shown when they come back.
static SHOWN_SEQ: AtomicU64 = AtomicU64::new(0);

/// The finding the pass is at, shown in the dialog; none left: what was done.
fn check_advance(ui: &AppWindow) {
    let g = ui.global::<PlatformBridge>();
    let shown = PASS.with(|p| {
        let p = p.borrow();
        let pass = p.as_ref()?;
        let f = pass.check.as_ref()?.current()?.clone();
        let (_, where_, text, page) = pass.texts.get(f.text)?;
        Some((pass.tab, where_.clone(), text.clone(), f, *page))
    });
    let scanning = PASS.with(|p| p.borrow().as_ref().is_some_and(|pass| pass.check.is_none()));
    if scanning {
        g.set_check_word("".into());
        g.set_check_done("Checking the comments and form fields…".into());
        return;
    }
    let Some((tab, where_, text, f, page)) = shown else {
        let said = PASS.with(|p| p.borrow().as_ref().map(|pass| (pass.texts.len(), pass.changed)));
        let missing = with_spell(|sp| sp.speller.as_ref().err().cloned());
        let plural = |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { many.to_string() };
        g.set_check_word("".into());
        g.set_check_done(
            match (missing, said) {
                (Some(why), _) => why,
                (None, Some((0, _))) => "This document has no comments, replies or form fields with text to check.".to_string(),
                (None, Some((n, 0))) => format!("Spelling check complete: {n} {} checked, nothing to change.", plural(n, "text", "texts")),
                (None, Some((n, c))) => format!("Spelling check complete: {n} {} checked, {c} {} changed.", plural(n, "text", "texts"), plural(c, "word", "words")),
                (None, None) => String::new(),
            }
            .into(),
        );
        return;
    };
    if let Some(page) = page {
        crate::reader_ui::go_to_page(tab, &(page + 1).to_string());
    }
    let red = ui.global::<sheaf_app::Theme>().get_danger();
    let colour = format!("#{:02x}{:02x}{:02x}", red.red(), red.green(), red.blue());
    let md = platform::marked_markdown(&text, &[(f.start, f.word.as_str())], &colour);
    g.set_check_context(slint::StyledText::from_markdown(&md).unwrap_or_else(|_| slint::StyledText::from_plain_text(&text)));
    g.set_check_where(where_.into());
    g.set_check_note(f.note.unwrap_or("Not in the dictionary").into());
    g.set_check_word(f.word.clone().into());
    let seq = SHOWN_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    match f.fix {
        // A grammar finding brings its own fix.
        Some(fix) => {
            g.set_check_suggestions(ModelRc::new(VecModel::from(vec![SharedString::from(fix)])));
            g.set_check_seq(g.get_check_seq() + 1);
        }
        // A misspelt word: its corrections from the dictionary, off the UI thread.
        None => {
            g.set_check_suggestions(ModelRc::new(VecModel::from(Vec::<SharedString>::new())));
            g.set_check_seq(g.get_check_seq() + 1);
            let weak = ui.as_weak();
            let word = f.word;
            std::thread::spawn(move || {
                let corrections = with_spell(|sp| {
                    let Ok(s) = &sp.speller else { return Vec::new() };
                    sp.corrections.entry(word.clone()).or_insert_with(|| s.suggest(&word, 5)).clone()
                });
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    if SHOWN_SEQ.load(Ordering::Relaxed) != seq {
                        return;
                    }
                    let g = ui.global::<PlatformBridge>();
                    g.set_check_suggestions(ModelRc::new(VecModel::from(corrections.into_iter().map(SharedString::from).collect::<Vec<_>>())));
                    g.set_check_seq(g.get_check_seq() + 1);
                });
            });
        }
    }
}

/// The pass moved on (Next, Ignore, Learn): `step` on its findings, then the next one shown.
fn check_step(ui: &AppWindow, step: impl FnOnce(&mut sheaf_core::spell::CheckPass)) {
    PASS.with(|p| {
        if let Some(check) = p.borrow_mut().as_mut().and_then(|pass| pass.check.as_mut()) {
            step(check);
        }
    });
    check_advance(ui);
}

/// The word shown, if it is a misspelt one (Ignore and Learn act on those).
fn shown_misspelt() -> Option<String> {
    PASS.with(|p| p.borrow().as_ref()?.check.as_ref()?.current().filter(|f| f.note.is_none()).map(|f| f.word.clone()))
}

/// Change: the finding shown becomes `to` in its comment, reply or field (one undo step), and the
/// pass goes on after it.
fn check_change(ui: &AppWindow, to: &str) {
    let to = if to.trim().is_empty() { "" } else { to };
    let put = PASS.with(|p| {
        let mut p = p.borrow_mut();
        let pass = p.as_mut()?;
        let check = pass.check.as_mut()?;
        let f = check.current()?.clone();
        if to.is_empty() || to == f.word {
            check.skip();
            return None;
        }
        let item = pass.texts.get_mut(f.text)?;
        let before = item.2.clone();
        item.2 = sheaf_core::spell::replace_word(&item.2, f.start, &f.word, to);
        if item.2 == before {
            check.skip();
            return None;
        }
        check.changed(to);
        pass.changed += 1;
        Some((pass.tab, item.0.clone(), item.2.clone()))
    });
    if let Some((tab, target, text)) = put {
        crate::reader_ui::spelling_set(ui, tab, &target, &text);
    }
    check_advance(ui);
}

/// Starts the pass on a reader tab: its texts collected, then scanned on a worker thread.
fn check_start(ui: &AppWindow, tab: i32) {
    let texts = crate::reader_ui::spelling_texts(tab);
    let words: Vec<String> = texts.iter().map(|t| t.2.clone()).collect();
    let grammar = grammar_on();
    PASS.with(|p| *p.borrow_mut() = Some(Pass { tab, texts, check: None, changed: 0 }));
    ui.global::<PlatformBridge>().set_check_open(true);
    check_advance(ui);
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        let check = with_spell(|sp| match &sp.speller {
            Ok(s) => sheaf_core::spell::CheckPass::new(s, &refs, grammar),
            Err(_) => sheaf_core::spell::CheckPass::default(),
        });
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let kept = PASS.with(|p| p.borrow_mut().as_mut().filter(|pass| pass.tab == tab && pass.check.is_none()).map(|pass| pass.check = Some(check)).is_some());
            if kept {
                check_advance(&ui);
            }
        });
    });
}

/// Wires the global and fills it.
pub fn install(ui: &AppWindow) {
    let _ = LEARNED.set(home::prefs().platform.learned_words);
    push(ui);
    load_dictionary(ui);
    let g = ui.global::<PlatformBridge>();
    // The GUI smoke (debug builds, SHEAF_READER_DEBUG) clicks at places measured without the menu
    // bar: hidden there unless a step asks for it (SHEAF_TEST_MENU_BAR=1). Its shortcuts work.
    if cfg!(debug_assertions) && std::env::var_os("SHEAF_READER_DEBUG").is_some() && std::env::var("SHEAF_TEST_MENU_BAR").as_deref() != Ok("1") {
        g.set_menu_bar(false);
    }
    macro_rules! on {
        ($on:ident, |$w:ident $(, $a:ident)*| $body:expr) => {{
            let weak = ui.as_weak();
            g.$on(move |$($a),*| {
                if let Some($w) = weak.upgrade() {
                    $body
                }
            });
        }};
    }
    on!(on_spell_check, |ui, text, typing| spell_check(&ui, &text, typing));
    on!(on_spell_reset, |ui| {
        LATEST.fetch_add(1, Ordering::Relaxed);
        TIMER.with(slint::Timer::stop);
        LAST.with(|l| *l.borrow_mut() = Default::default());
        clear(&ui);
    });
    g.on_spell_replace(|text, start, word, correction| sheaf_core::spell::replace_word(&text, start.max(0) as usize, &word, &correction).into());
    on!(on_spell_ignore, |ui, word| {
        with_spell(|sp| if let Ok(s) = sp.speller.as_mut() { s.ignore(&word) });
        check_again(&ui);
    });
    on!(on_spell_learn, |ui, word| {
        with_spell(|sp| if let Ok(s) = sp.speller.as_mut() { s.learn(&word) });
        let word = word.to_string();
        update(&ui, |p| {
            if !p.learned_words.iter().any(|w| w.eq_ignore_ascii_case(&word)) {
                p.learned_words.push(word);
            }
        });
        check_again(&ui);
    });
    on!(on_forget_learned, |ui| {
        with_spell(|sp| if let Ok(s) = sp.speller.as_mut() { s.forget_learned() });
        update(&ui, |p| p.learned_words.clear());
        check_again(&ui);
    });
    on!(on_grammar_ignore, |ui, note, text| {
        if let Ok(mut g) = IGNORED_GRAMMAR.lock() {
            g.push(grammar_key(&note, &text));
        }
        check_again(&ui);
    });
    on!(on_set_grammar_on, |ui, on| {
        update(&ui, |p| p.no_grammar_check = !on);
        check_again(&ui);
    });
    on!(on_set_spell_on, |ui, on| {
        update(&ui, |p| p.no_spell_check = !on);
        check_again(&ui);
    });
    on!(on_shortcut_query, |ui, q| {
        QUERY.with(|s| *s.borrow_mut() = q.to_string());
        push_rows(&ui);
    });
    // While a shortcut is recorded the menu bar has none, so the keys reach the recorder.
    on!(on_recording, |ui, on| {
        if on {
            menu_keys_off(&ui);
        } else {
            push_keys(&ui);
        }
    });
    let weak = ui.as_weak();
    g.on_record_shortcut(move |i, text, control, alt, shift, meta| weak.upgrade().is_none_or(|ui| record(&ui, i, &text, control, alt, shift, meta)));
    g.on_tool_for_key(|text| TOOL_KEYS.with(|t| platform::tool_for_key(&t.borrow(), &text).map_or(-1, |k| k as i32)));
    on!(on_clear_shortcut, |ui, i| {
        if let Some(k) = (i.max(0) as usize).checked_sub(COMMANDS.len() + TOOLS.len()) {
            let mut prefs = home::prefs().platform;
            if platform::assign_launch(&mut prefs, k, "").is_ok() {
                update(&ui, |p| *p = prefs);
            }
            return;
        }
        if let Some(k) = (i.max(0) as usize).checked_sub(COMMANDS.len()) {
            let mut prefs = home::prefs().platform;
            if platform::assign_tool(&mut prefs, k, "").is_ok() {
                update(&ui, |p| *p = prefs);
            }
            return;
        }
        let Some(c) = COMMANDS.get(i.max(0) as usize) else { return };
        let mut prefs = home::prefs().platform;
        if platform::assign(&mut prefs, c.id, "").is_ok() {
            update(&ui, |p| *p = prefs);
        }
    });
    on!(on_reset_shortcut, |ui, i| {
        // A launcher has no keys of its own to go back to: Reset removes them.
        if let Some(k) = (i.max(0) as usize).checked_sub(COMMANDS.len() + TOOLS.len()) {
            let mut prefs = home::prefs().platform;
            if platform::assign_launch(&mut prefs, k, "").is_ok() {
                update(&ui, |p| *p = prefs);
            }
            return;
        }
        if let Some(k) = (i.max(0) as usize).checked_sub(COMMANDS.len()) {
            let mut prefs = home::prefs().platform;
            let own = TOOLS.get(k).map_or("", |t| t.2);
            match platform::assign_tool(&mut prefs, k, own) {
                Ok(()) => {
                    ui.global::<PlatformBridge>().set_shortcut_error("".into());
                    update(&ui, |p| *p = prefs);
                }
                Err(e) => ui.global::<PlatformBridge>().set_shortcut_error(e.into()),
            }
            return;
        }
        let Some(c) = COMMANDS.get(i.max(0) as usize) else { return };
        // The default may be taken by now, by a command or a tool: then it stays as it is, with
        // the reason.
        let mut prefs = home::prefs().platform;
        let g = ui.global::<PlatformBridge>();
        match platform::reset_command(&mut prefs, c.id) {
            Ok(()) => {
                g.set_shortcut_error("".into());
                update(&ui, |p| *p = prefs);
            }
            Err(e) => g.set_shortcut_error(e.into()),
        }
    });
    on!(on_reset_shortcuts, |ui| {
        ui.global::<PlatformBridge>().set_shortcut_error("".into());
        update(&ui, |p| {
            platform::reset(p, None);
            p.tool_keys.clear();
            p.launch_keys.clear();
        });
    });
    ui.global::<PlatformBridge>().set_unavailable_tools(unavailable_text().into());
    on!(on_set_hide_unavailable, |ui, on| {
        update(&ui, |p| p.hide_unavailable_tools = on);
        crate::install_catalogue(&ui);
    });
    on!(on_set_direction, |ui, d| update(&ui, |p| p.direction = d.clamp(0, 2) as u8));
    on!(on_edit_key, |ui, key, shift| edit_key(&ui, &key, shift));
    g.on_shift_held(|| HELD.load(Ordering::Relaxed) & HELD_SHIFT != 0);
    on!(on_check_start, |ui, tab| check_start(&ui, tab));
    on!(on_check_change, |ui, to| check_change(&ui, &to));
    on!(on_check_next, |ui| check_step(&ui, sheaf_core::spell::CheckPass::skip));
    on!(on_check_ignore, |ui| {
        if let Some(word) = shown_misspelt() {
            with_spell(|sp| if let Ok(s) = sp.speller.as_mut() { s.ignore(&word) });
        }
        check_step(&ui, sheaf_core::spell::CheckPass::pass_word);
    });
    on!(on_check_learn, |ui| {
        if let Some(word) = shown_misspelt() {
            with_spell(|sp| if let Ok(s) = sp.speller.as_mut() { s.learn(&word) });
            update(&ui, |p| {
                if !p.learned_words.iter().any(|w| w.eq_ignore_ascii_case(&word)) {
                    p.learned_words.push(word);
                }
            });
        }
        check_step(&ui, sheaf_core::spell::CheckPass::pass_word);
    });
    on!(on_check_close, |ui| {
        PASS.with(|p| *p.borrow_mut() = None);
        let g = ui.global::<PlatformBridge>();
        g.set_check_open(false);
        g.set_check_word("".into());
    });
    // File ▸ Quit: as the window's close button (unsaved documents are asked about first).
    on!(on_quit, |ui| {
        let _ = ui.window().dispatch_event_with_result(slint::platform::WindowEvent::CloseRequested);
    });
}

