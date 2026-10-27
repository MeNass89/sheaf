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
use sheaf_app::{AppWindow, PlatformBridge, ShortcutRow, SpellIssue};
use sheaf_core::spell::Speller;
use sheaf_core::tools::ToolId;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

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
type Checked = (Vec<(usize, String)>, Vec<(usize, String, Vec<String>)>);

fn check_text(text: &str, typing: bool) -> Checked {
    with_spell(|sp| {
        let Ok(speller) = &sp.speller else { return (Vec::new(), Vec::new()) };
        let issues = speller.misspelt(text, typing);
        let mut shown = Vec::new();
        for i in issues.iter().take(SHOWN) {
            let c = sp.corrections.entry(i.word.clone()).or_insert_with(|| speller.suggest(&i.word, CORRECTIONS)).clone();
            shown.push((i.start, i.word.clone(), c));
        }
        (issues.into_iter().map(|i| (i.start, i.word)).collect(), shown)
    })
}

fn spell_on() -> bool {
    !home::prefs().platform.no_spell_check
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
        .map(|(start, word, c)| SpellIssue {
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
    TIMER.with(|t| {
        t.start(slint::TimerMode::SingleShot, PAUSE, move || {
            let (weak, text) = (weak.clone(), text.clone());
            std::thread::spawn(move || {
                let checked = check_text(&text, typing);
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

fn push_keys(ui: &AppWindow) {
    let p = home::prefs().platform;
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

/// Edit ▸ Cut, Copy, Paste, Select All: ⌘ (Ctrl) and `key` sent to whatever has the keyboard, as
/// if typed — a text field cuts, copies, pastes or selects its own text; the reader's pages their
/// selection, picked objects or comment (reader.slint's ⌘C ⌘X ⌘V ⌘A). Sent once the menu has
/// closed and given the keyboard back. The menu bar has no shortcut on these keys, so they reach
/// the field or the pages.
fn edit_key(ui: &AppWindow, key: &str) {
    use slint::platform::{Key, WindowEvent};
    let weak = ui.as_weak();
    let key = SharedString::from(key);
    slint::Timer::single_shot(Duration::from_millis(30), move || {
        let Some(ui) = weak.upgrade() else { return };
        let ctrl = SharedString::from(Key::Control);
        for ev in [
            WindowEvent::KeyPressed { text: ctrl.clone() },
            WindowEvent::KeyPressed { text: key.clone() },
            WindowEvent::KeyReleased { text: key.clone() },
            WindowEvent::KeyReleased { text: ctrl },
        ] {
            let _ = ui.window().dispatch_event_with_result(ev);
        }
    });
}

/// Edit ▸ Check Spelling… on a document: its texts, the one being checked (`at`), where in it the
/// next word is looked for (`from`, a byte offset), the word shown, and the changes made.
struct Pass {
    tab: i32,
    texts: Vec<crate::reader_ui::SpellText>,
    at: usize,
    from: usize,
    word: Option<(usize, String)>,
    changed: usize,
}

thread_local! {
    static PASS: RefCell<Option<Pass>> = const { RefCell::new(None) };
}

/// The next misspelt word from where the pass is, shown in the dialog; none left: what was done.
fn check_advance(ui: &AppWindow) {
    let g = ui.global::<PlatformBridge>();
    let shown = PASS.with(|p| {
        let mut p = p.borrow_mut();
        let pass = p.as_mut()?;
        pass.word = None;
        let texts: Vec<&str> = pass.texts.iter().map(|t| t.2.as_str()).collect();
        let found = with_spell(|sp| {
            let Ok(s) = &sp.speller else { return None };
            let (k, issue) = s.next_issue(&texts, pass.at, pass.from)?;
            let c = sp.corrections.entry(issue.word.clone()).or_insert_with(|| s.suggest(&issue.word, 5)).clone();
            Some((k, issue, c))
        });
        let Some((k, issue, corrections)) = found else {
            pass.at = pass.texts.len();
            return None;
        };
        pass.at = k;
        pass.word = Some((issue.start, issue.word.clone()));
        let (_, where_, text, page) = &pass.texts[k];
        Some((pass.tab, where_.clone(), text.clone(), issue, corrections, *page))
    });
    match shown {
        Some((tab, where_, text, issue, corrections, page)) => {
            if let Some(page) = page {
                crate::reader_ui::go_to_page(tab, &(page + 1).to_string());
            }
            let red = ui.global::<sheaf_app::Theme>().get_danger();
            let colour = format!("#{:02x}{:02x}{:02x}", red.red(), red.green(), red.blue());
            let md = platform::marked_markdown(&text, &[(issue.start, issue.word.as_str())], &colour);
            g.set_check_context(slint::StyledText::from_markdown(&md).unwrap_or_else(|_| slint::StyledText::from_plain_text(&text)));
            g.set_check_where(where_.into());
            g.set_check_word(issue.word.into());
            g.set_check_suggestions(ModelRc::new(VecModel::from(corrections.into_iter().map(SharedString::from).collect::<Vec<_>>())));
            g.set_check_seq(g.get_check_seq() + 1);
        }
        None => {
            let said = PASS.with(|p| p.borrow().as_ref().map(|pass| (pass.texts.len(), pass.changed)));
            let missing = with_spell(|sp| sp.speller.as_ref().err().cloned());
            g.set_check_word("".into());
            g.set_check_done(
                match (missing, said) {
                    (Some(why), _) => why,
                    (None, Some((0, _))) => "This document has no comments or form fields with text to check.".to_string(),
                    (None, Some((n, 0))) => format!("Spelling check complete: {n} {} checked, nothing to change.", if n == 1 { "text" } else { "texts" }),
                    (None, Some((n, c))) => format!("Spelling check complete: {n} {} checked, {c} {} changed.", if n == 1 { "text" } else { "texts" }, if c == 1 { "word" } else { "words" }),
                    (None, None) => String::new(),
                }
                .into(),
            );
        }
    }
}

/// The word shown is left as it is: the pass goes on after it.
fn check_skip() {
    PASS.with(|p| {
        if let Some(pass) = p.borrow_mut().as_mut()
            && let Some((start, word)) = pass.word.take()
        {
            pass.from = start + word.len();
        }
    });
}

/// Change: the word shown becomes `to` in its comment or field (one undo step), and the pass goes
/// on after it.
fn check_change(ui: &AppWindow, to: &str) {
    let to = to.trim();
    let put = PASS.with(|p| {
        let mut p = p.borrow_mut();
        let pass = p.as_mut()?;
        let (start, word) = pass.word.take()?;
        if to.is_empty() || to == word {
            pass.from = start + word.len();
            return None;
        }
        let item = pass.texts.get_mut(pass.at)?;
        item.2 = sheaf_core::spell::replace_word(&item.2, start, &word, to);
        pass.from = start + to.len();
        pass.changed += 1;
        Some((pass.tab, item.0.clone(), item.2.clone()))
    });
    if let Some((tab, target, text)) = put {
        crate::reader_ui::spelling_set(ui, tab, &target, &text);
    }
    check_advance(ui);
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
            let none: Vec<slint::Keys> = vec![slint::Keys::default(); COMMANDS.len()];
            ui.global::<PlatformBridge>().set_keys(ModelRc::new(VecModel::from(none)));
        } else {
            push_keys(&ui);
        }
    });
    let weak = ui.as_weak();
    g.on_record_shortcut(move |i, text, control, alt, shift, meta| weak.upgrade().is_none_or(|ui| record(&ui, i, &text, control, alt, shift, meta)));
    g.on_tool_for_key(|text| TOOL_KEYS.with(|t| platform::tool_for_key(&t.borrow(), &text).map_or(-1, |k| k as i32)));
    on!(on_clear_shortcut, |ui, i| {
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
        // The default may be taken by now: then it stays as it is, with the reason.
        let mut prefs = home::prefs().platform;
        platform::reset(&mut prefs, Some(c.id));
        let k = platform::keys(c.keys);
        let taken = !c.keys.is_empty() && (0..COMMANDS.len()).any(|o| COMMANDS[o].id != c.id && platform::keys(&platform::effective(&prefs, o)) == k);
        let g = ui.global::<PlatformBridge>();
        if taken {
            g.set_shortcut_error(format!("{} is taken by another command", platform::display(c.keys)).into());
        } else {
            g.set_shortcut_error("".into());
            update(&ui, |p| *p = prefs);
        }
    });
    on!(on_reset_shortcuts, |ui| {
        ui.global::<PlatformBridge>().set_shortcut_error("".into());
        update(&ui, |p| {
            platform::reset(p, None);
            p.tool_keys.clear();
        });
    });
    ui.global::<PlatformBridge>().set_unavailable_tools(unavailable_text().into());
    on!(on_set_hide_unavailable, |ui, on| {
        update(&ui, |p| p.hide_unavailable_tools = on);
        crate::install_catalogue(&ui);
    });
    on!(on_set_direction, |ui, d| update(&ui, |p| p.direction = d.clamp(0, 2) as u8));
    on!(on_edit_key, |ui, key| edit_key(&ui, &key));
    on!(on_check_start, |ui, tab| {
        let texts = crate::reader_ui::spelling_texts(tab);
        PASS.with(|p| *p.borrow_mut() = Some(Pass { tab, texts, at: 0, from: 0, word: None, changed: 0 }));
        ui.global::<PlatformBridge>().set_check_open(true);
        check_advance(&ui);
    });
    on!(on_check_change, |ui, to| check_change(&ui, &to));
    on!(on_check_next, |ui| {
        check_skip();
        check_advance(&ui);
    });
    on!(on_check_ignore, |ui| {
        if let Some(word) = PASS.with(|p| p.borrow().as_ref().and_then(|pass| pass.word.as_ref().map(|w| w.1.clone()))) {
            with_spell(|sp| if let Ok(s) = sp.speller.as_mut() { s.ignore(&word) });
        }
        check_skip();
        check_advance(&ui);
    });
    on!(on_check_learn, |ui| {
        if let Some(word) = PASS.with(|p| p.borrow().as_ref().and_then(|pass| pass.word.as_ref().map(|w| w.1.clone()))) {
            with_spell(|sp| if let Ok(s) = sp.speller.as_mut() { s.learn(&word) });
            update(&ui, |p| {
                if !p.learned_words.iter().any(|w| w.eq_ignore_ascii_case(&word)) {
                    p.learned_words.push(word);
                }
            });
        }
        check_skip();
        check_advance(&ui);
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

