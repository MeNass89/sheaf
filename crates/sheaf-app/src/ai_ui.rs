//! The AI panel on a document (PDF Expert's AI Copilot panel, Acrobat's AI Assistant) and
//! Preferences ▸ AI: `ui/ai.slint` (the `AiBridge` global), `ui/screens/ai-panel.slint`,
//! `ui/screens/preferences-ai.slint`. Requests run on a worker thread (`sheaf_core::ai`), so the
//! window never waits on a provider; the answer, or a plain error, comes back as a message.
//! Before the first request to a provider the panel says that only the document's text is sent,
//! and to whom, and offers to turn AI off.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sheaf_app::ai_store::{self, AiPrefs};
use sheaf_app::{AiBridge, AiMessage, AppWindow};
use sheaf_core::ai::spaces::{self, Space};
use sheaf_core::ai::tasks::{self, Source, Summary, ToSort};
use sheaf_core::ai::{self, deck, AiError, Config, Provider};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::reader_ui;

#[derive(Default)]
struct Msg {
    mine: bool,
    text: String,
    error: bool,
    saved: Option<PathBuf>,
    /// The request to send again (Try again under a network, rate-limit or provider error, or
    /// after Stop).
    retry: Option<Request>,
    /// The error is fixed in Preferences ▸ AI (no provider, no key, key refused, AI off).
    settings: bool,
    /// A podcast script: Save audio… speaks it.
    podcast: bool,
}

#[derive(Default)]
struct Panel {
    messages: Vec<Msg>,
    /// Questions asked and their answers, sent back with the next question.
    history: Vec<(String, String)>,
    busy: bool,
    status: String,
    cancel: Arc<AtomicBool>,
    /// The answer as it is being written (streamed), shown as the last message until it ends.
    live: Option<String>,
}

/// A request from the panel: what, its text, the scope (0 whole, 1 selection, 2 pages), the pages.
#[derive(Clone)]
struct Request {
    action: String,
    arg: String,
    scope: i32,
    pages: String,
}

#[derive(Default)]
struct State {
    open: Option<i32>,
    panels: HashMap<i32, Panel>,
    keys: BTreeMap<String, String>,
    spaces: Vec<Space>,
    /// The space in use: 0 this document, k the space k - 1.
    space: usize,
    /// The provider that answers ("" until known) and whether its note is still to be read.
    provider: String,
    notice: Option<String>,
    /// A request held until the note is read.
    pending: Option<(i32, Request)>,
    /// A provider answers (false: the panel shows how to set one up).
    configured: bool,
    /// The new name typed for the space in use (Rename), until applied.
    renaming: Option<String>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn dir() -> PathBuf {
    sheaf_app::store_file::data_dir()
}

pub fn install(ui: &AppWindow) {
    let keys = ai_store::load_keys(&dir()).unwrap_or_else(|e| {
        eprintln!("sheaf: {e}");
        BTreeMap::new()
    });
    with(|s| {
        s.keys = keys;
        s.spaces = spaces::load(&dir());
    });
    let g = ui.global::<AiBridge>();
    g.set_languages(ModelRc::new(VecModel::from(tasks::LANGUAGES.iter().map(|l| SharedString::from(*l)).collect::<Vec<_>>())));
    g.set_providers(ModelRc::new(VecModel::from(std::iter::once("Automatic: a model on this computer".into()).chain(Provider::ALL.iter().map(|p| SharedString::from(p.name()))).collect::<Vec<_>>())));
    let weak = ui.as_weak();
    g.on_toggle(move |tab| {
        if let Some(ui) = weak.upgrade() {
            toggle(&ui, tab);
        }
    });
    let weak = ui.as_weak();
    g.on_act(move |tab, action, arg, scope, pages| {
        if let Some(ui) = weak.upgrade() {
            act(&ui, tab, Request { action: action.into(), arg: arg.into(), scope, pages: pages.into() });
        }
    });
    let weak = ui.as_weak();
    g.on_space_act(move |action, k| {
        if let Some(ui) = weak.upgrade() {
            space_act(&ui, &action, k);
        }
    });
    let weak = ui.as_weak();
    g.on_space_rename(move |name| {
        if let Some(ui) = weak.upgrade() {
            with(|s| s.renaming = Some(name.to_string()));
            space_act(&ui, "rename", 0);
        }
    });
    let weak = ui.as_weak();
    g.on_pref(move |field, value| {
        if let Some(ui) = weak.upgrade() {
            pref(&ui, &field, &value);
        }
    });
    push_prefs(ui);
}

/// PDF Expert opens its Copilot with each document when asked to (Preferences ▸ AI).
pub fn auto_open(ui: &AppWindow, tab: i32) {
    if crate::home::prefs().ai.auto_open && with(|s| s.open != Some(tab)) {
        toggle(ui, tab);
    }
}

pub fn toggle(ui: &AppWindow, tab: i32) {
    let opening = with(|s| {
        if s.open == Some(tab) {
            s.open = None;
            false
        } else {
            s.open = Some(tab);
            s.panels.entry(tab).or_default();
            true
        }
    });
    if opening {
        resolve(ui);
        watch_selection(ui);
    }
    push(ui);
}

/// Finds who answers (a local server is looked for off the UI thread) for the header and the note.
fn resolve(ui: &AppWindow) {
    let prefs = crate::home::prefs().ai;
    let keys = with(|s| s.keys.clone());
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let r = prefs.config(&keys, ai::detect_local);
        let _ = weak.upgrade_in_event_loop(move |ui| {
            with(|s| match &r {
                Ok(c) => {
                    s.configured = true;
                    s.provider = c.label();
                    s.notice = (!prefs.notice_seen.iter().any(|p| p == c.provider.id())).then(|| notice(c));
                }
                Err(e) => {
                    s.configured = false;
                    s.provider = e.to_string();
                    s.notice = None;
                }
            });
            push(&ui);
        });
    });
}

fn notice(c: &Config) -> String {
    if c.provider.is_local() {
        format!("Sheaf sends only this document's text, never the file, to {}. It stays on this computer.", c.label())
    } else {
        format!("Sheaf sends only this document's text, never the file, to {}, under your API key and its terms. Nothing is sent until you ask.", c.label())
    }
}

fn push(ui: &AppWindow) {
    let g = ui.global::<AiBridge>();
    let p = crate::home::prefs().ai;
    with(|s| {
        g.set_open(s.open.is_some());
        g.set_tab(s.open.unwrap_or(-1));
        g.set_provider(if s.provider.is_empty() { "Looking for a provider…".into() } else { s.provider.as_str().into() });
        g.set_show_notice(s.notice.is_some());
        g.set_notice(s.notice.clone().unwrap_or_default().into());
        g.set_off(p.off);
        g.set_configured(s.configured || s.provider.is_empty());
        let lang = p.language.as_deref().and_then(|l| tasks::LANGUAGES.iter().position(|x| *x == l)).unwrap_or(0);
        g.set_language(lang as i32);
        let names: Vec<SharedString> = std::iter::once("This document".into()).chain(s.spaces.iter().map(|sp| SharedString::from(format!("Space: {} ({} PDF{})", sp.name, sp.files.len(), if sp.files.len() == 1 { "" } else { "s" })))).collect();
        g.set_spaces(ModelRc::new(VecModel::from(names)));
        s.space = s.space.min(s.spaces.len());
        g.set_space(s.space as i32);
        let files: Vec<SharedString> = s.space.checked_sub(1).and_then(|k| s.spaces.get(k)).map(|sp| sp.files.iter().map(|f| SharedString::from(f.file_name().unwrap_or_default().to_string_lossy().as_ref())).collect()).unwrap_or_default();
        g.set_space_files(ModelRc::new(VecModel::from(files)));
        g.set_space_name(s.space.checked_sub(1).and_then(|k| s.spaces.get(k)).map(|sp| sp.name.as_str()).unwrap_or_default().into());
        let panel = s.open.and_then(|t| s.panels.get(&t));
        let msgs: Vec<AiMessage> = panel
            .map(|p| {
                let mut v: Vec<AiMessage> = p.messages.iter().map(|m| AiMessage { mine: m.mine, text: m.text.as_str().into(), error: m.error, saved: m.saved.as_ref().map(|p| p.display().to_string()).unwrap_or_default().into(), live: false, retry: m.retry.is_some(), settings: m.settings, audio: m.podcast }).collect();
                if let Some(t) = p.live.as_ref().filter(|t| !t.is_empty()) {
                    v.push(AiMessage { mine: false, text: format!("{t}▍").into(), error: false, saved: SharedString::new(), live: true, retry: false, settings: false, audio: false });
                }
                v
            })
            .unwrap_or_default();
        g.set_messages(ModelRc::new(VecModel::from(msgs)));
        g.set_busy(panel.is_some_and(|p| p.busy));
        g.set_status(panel.map(|p| p.status.as_str()).unwrap_or_default().into());
    });
    let sel = s_selection(&with(|s| s.open)).unwrap_or_default();
    g.set_selection(sel.into());
}

/// The selection shortened for the panel ("" none).
fn s_selection(tab: &Option<i32>) -> Option<String> {
    let doc = reader_ui::ai_doc((*tab)?)?;
    let one: String = doc.selection.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(if one.chars().count() > 80 { format!("{}…", one.chars().take(80).collect::<String>()) } else { one })
}

thread_local! {
    static WATCH: slint::Timer = slint::Timer::default();
}

/// While the panel is open, the selection made on the pages is followed (its chips and scope).
fn watch_selection(ui: &AppWindow) {
    let weak = ui.as_weak();
    WATCH.with(|t| {
        t.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(400), move || {
            let Some(ui) = weak.upgrade() else { return };
            let open = with(|s| s.open);
            if open.is_none() {
                return WATCH.with(slint::Timer::stop);
            }
            let sel: SharedString = s_selection(&open).unwrap_or_default().into();
            let g = ui.global::<AiBridge>();
            if g.get_selection() != sel {
                g.set_selection(sel);
            }
        });
    });
}

fn panel<R>(tab: i32, f: impl FnOnce(&mut Panel) -> R) -> R {
    with(|s| f(s.panels.entry(tab).or_default()))
}

fn say(ui: &AppWindow, tab: i32, mine: bool, text: String, error: bool) {
    panel(tab, |p| p.messages.push(Msg { mine, text, error, ..Default::default() }));
    push(ui);
}

fn act(ui: &AppWindow, tab: i32, req: Request) {
    match req.action.as_str() {
        "clear" => {
            panel(tab, |p| {
                p.messages.clear();
                p.history.clear();
                p.status.clear();
            });
            return push(ui);
        }
        "cancel" => {
            panel(tab, |p| {
                p.cancel.store(true, Ordering::Relaxed);
                p.status = "Stopping…".into();
            });
            return push(ui);
        }
        "notice-ok" => {
            // The note is read for the provider that answers now.
            let prefs = crate::home::prefs().ai;
            let keys = with(|s| s.keys.clone());
            let weak = ui.as_weak();
            std::thread::spawn(move || {
                let id = prefs.config(&keys, ai::detect_local).ok().map(|c| c.provider.id().to_string());
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    if let Some(id) = id {
                        crate::home::update_ai(&ui, |a| if !a.notice_seen.contains(&id) { a.notice_seen.push(id) });
                    }
                    with(|s| s.notice = None);
                    let pending = with(|s| s.pending.take());
                    push(&ui);
                    if let Some((tab, req)) = pending {
                        act(&ui, tab, req);
                    }
                });
            });
            return;
        }
        "notice-off" => {
            crate::home::update_ai(ui, |a| a.off = true);
            with(|s| {
                s.notice = None;
                s.pending = None;
            });
            push_prefs(ui);
            return push(ui);
        }
        "turn-on" => {
            crate::home::update_ai(ui, |a| a.off = false);
            push_prefs(ui);
            resolve(ui);
            return push(ui);
        }
        "copy" | "save" | "open" => return message_act(ui, tab, &req),
        "audio" => return save_audio(ui, tab, &req.arg),
        "retry" => {
            let again = req.arg.parse::<usize>().ok().and_then(|k| panel(tab, |p| p.messages.get(k).and_then(|m| m.retry.clone())));
            if let Some(again) = again {
                act(ui, tab, again);
            }
            return;
        }
        "settings" => {
            ui.set_active_tab(-1);
            ui.set_page(sheaf_app::Page::Preferences);
            return;
        }
        _ => {}
    }
    if panel(tab, |p| p.busy) {
        return;
    }
    if with(|s| s.notice.is_some()) {
        with(|s| s.pending = Some((tab, req)));
        panel(tab, |p| p.status = "Read the note above, then OK sends the request.".into());
        return push(ui);
    }
    if req.action == "classify" && with(|s| s.space) == 0 {
        let d = crate::dialog::new(ui).set_title("PDFs to classify").add_filter("PDF", &["pdf"]);
        let req2 = req.clone();
        crate::dialog::pick_files(ui, d, move |ui, files| start(ui, tab, req2, Some(files)));
        return;
    }
    start(ui, tab, req, None);
}

/// What the user asked, as their message.
fn asked(req: &Request, space: Option<&Space>) -> String {
    let what = match req.action.as_str() {
        "ask" => req.arg.clone(),
        "summarize" => "Summarize".into(),
        "points" => "Main points".into(),
        "translate" => format!("Translate into {}", req.arg),
        "podcast" => "Podcast script".into(),
        "slides-pptx" => "Slides (PowerPoint)".into(),
        "slides-pdf" => "Slides (PDF)".into(),
        "classify" if req.arg.trim().is_empty() => "Classify".into(),
        "classify" => format!("Classify into: {}", req.arg.trim()),
        a => a.into(),
    };
    match (space, req.scope) {
        (Some(sp), _) => format!("{what} · space {}", sp.name),
        (None, 1) => format!("{what} · the selection"),
        (None, 2) => format!("{what} · pages {}", req.pages.trim()),
        _ => what,
    }
}

fn start(ui: &AppWindow, tab: i32, req: Request, picked: Option<Vec<PathBuf>>) {
    let Some(doc) = reader_ui::ai_doc(tab) else { return };
    let space = with(|s| s.space.checked_sub(1).and_then(|k| s.spaces.get(k).cloned()));
    // What is read: the selection, some pages or every page of this document; or a space.
    let pages: Vec<usize> = match req.scope {
        2 if space.is_none() => match sheaf_core::ranges::parse_ranges(req.pages.trim(), doc.pages as u32) {
            Ok(r) if !req.pages.trim().is_empty() => r.into_iter().flatten().map(|p| p as usize - 1).collect(),
            Ok(_) => return say(ui, tab, false, "Type the pages to read, for example 1-3, 5.".into(), true),
            Err(e) => return say(ui, tab, false, e.to_string(), true),
        },
        _ => (0..doc.pages).collect(),
    };
    if req.scope == 1 && space.is_none() && doc.selection.trim().is_empty() {
        return say(ui, tab, false, "Select some text on the pages first.".into(), true);
    }
    if req.action == "translate" {
        let lang = req.arg.clone();
        crate::home::update_ai(ui, |a| a.language = Some(lang));
    }
    let files = picked.or_else(|| if req.action == "classify" { space.as_ref().map(|s| s.files.clone()) } else { None });
    panel(tab, |p| {
        p.messages.push(Msg { mine: true, text: asked(&req, space.as_ref()), ..Default::default() });
        p.busy = true;
        p.status = "Reading the text…".into();
        p.cancel = Arc::new(AtomicBool::new(false));
    });
    push(ui);
    let cancel = panel(tab, |p| p.cancel.clone());
    let history = panel(tab, |p| p.history.clone());
    let prefs = crate::home::prefs().ai;
    let keys = with(|s| s.keys.clone());
    let weak = ui.as_weak();
    let title = doc.path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    std::thread::spawn(move || {
        let status = |text: String| {
            let _ = weak.upgrade_in_event_loop(move |ui| {
                panel(tab, |p| if p.busy { p.status = text });
                push(&ui);
            });
        };
        // Answers stream into the panel as the provider writes them (PDF Expert's and Acrobat's
        // answers appear word by word), at most about 12 updates a second.
        let sink = {
            let weak = weak.clone();
            let cancel = cancel.clone();
            let pending = Arc::new(std::sync::Mutex::new(String::new()));
            let last = std::cell::Cell::new(std::time::Instant::now() - std::time::Duration::from_secs(1));
            move |d: &str| {
                pending.lock().unwrap_or_else(|e| e.into_inner()).push_str(d);
                if last.get().elapsed() >= std::time::Duration::from_millis(80) {
                    last.set(std::time::Instant::now());
                    let text = std::mem::take(&mut *pending.lock().unwrap_or_else(|e| e.into_inner()));
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        panel(tab, |p| if p.busy { p.live.get_or_insert_with(String::new).push_str(&text) });
                        push(&ui);
                    });
                }
                !cancel.load(Ordering::Relaxed)
            }
        };
        let result = tasks::streaming(sink, || (|| -> Result<String, AiError> {
            let cfg = prefs.config(&keys, ai::detect_local)?;
            let stop = || cancel.load(Ordering::Relaxed);
            let mut progress = |i: usize, n: usize| {
                status(if n > 1 { format!("Asking {} · part {i} of {n}…", cfg.provider.name()) } else { format!("Asking {}…", cfg.provider.name()) });
                !stop()
            };
            if req.action == "classify" {
                let files = files.unwrap_or_default();
                let docs: Vec<ToSort> = files
                    .iter()
                    .map(|f| ToSort { name: f.file_name().unwrap_or_default().to_string_lossy().into(), text: sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(f), &[]).map(|t| t.join("\n")).unwrap_or_default() })
                    .collect();
                if docs.is_empty() {
                    return Err(AiError::NoText);
                }
                let cats: Vec<String> = req.arg.split(',').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
                let got = tasks::classify(&cfg, &docs, &cats, &mut progress)?;
                // Grouped by category, as Stirling lists its results.
                let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
                for (d, c) in docs.iter().zip(&got) {
                    groups.entry(c.as_str()).or_default().push(d.name.as_str());
                }
                return Ok(groups.iter().map(|(c, names)| format!("{c}: {}", names.join(", "))).collect::<Vec<_>>().join("\n"));
            }
            let mut notes = Vec::new();
            let sources: Vec<Source> = if let Some(sp) = &space {
                let (src, failed) = spaces::sources(&sp.files);
                notes = failed;
                src
            } else if req.scope == 1 {
                vec![Source { label: "the selection".into(), text: doc.selection.clone() }]
            } else {
                let texts = reader_ui::ai_page_texts(doc.bytes.clone(), doc.password.clone(), &pages, &stop);
                pages.iter().zip(texts).filter(|(_, t)| !t.trim().is_empty()).map(|(p, t)| Source { label: format!("p. {}", p + 1), text: t.trim().to_string() }).collect()
            };
            if stop() {
                return Err(AiError::Cancelled);
            }
            let answer = match req.action.as_str() {
                "ask" if req.scope == 1 && space.is_none() => tasks::ask(&cfg, &[], Some(&doc.selection), &req.arg, &history, &mut progress)?,
                "ask" => tasks::ask(&cfg, &sources, None, &req.arg, &history, &mut progress)?,
                "summarize" => tasks::summarize(&cfg, &sources, Summary::Paragraphs, &mut progress)?,
                "points" => tasks::summarize(&cfg, &sources, Summary::MainPoints, &mut progress)?,
                "translate" => tasks::translate(&cfg, &sources, &req.arg, &mut progress)?,
                "podcast" => tasks::podcast(&cfg, &sources, &mut progress)?,
                _ => tasks::slides(&cfg, &sources, &mut progress)?,
            };
            Ok(if notes.is_empty() { answer } else { format!("{answer}\n\nNot read: {}", notes.join("; ")) })
        })());
        let _ = weak.upgrade_in_event_loop(move |ui| finish(&ui, tab, req, result, title));
    });
}

fn finish(ui: &AppWindow, tab: i32, req: Request, result: Result<String, AiError>, title: String) {
    let live = panel(tab, |p| {
        p.busy = false;
        p.status.clear();
        p.live.take()
    });
    match result {
        Ok(text) => {
            if req.action == "ask" {
                panel(tab, |p| p.history.push((req.arg.clone(), text.clone())));
            }
            let podcast = req.action == "podcast";
            panel(tab, |p| p.messages.push(Msg { text: text.clone(), podcast, ..Default::default() }));
            push(ui);
            if req.action.starts_with("slides") {
                save_deck(ui, tab, &title, &text, req.action == "slides-pptx");
            }
        }
        Err(e) => {
            // Stop keeps what was already written; every error says what to do next.
            let settings = matches!(e, AiError::NotConfigured | AiError::MissingKey(_) | AiError::MissingBase | AiError::BadKey(..) | AiError::Off);
            let retry = matches!(e, AiError::Network(..) | AiError::RateLimited(..) | AiError::Provider(..) | AiError::BadResponse(_) | AiError::Cancelled).then(|| req.clone());
            let text = match (&e, live) {
                (AiError::Cancelled, Some(t)) if !t.is_empty() => format!("{t}\n\n(Stopped.)"),
                _ => e.to_string(),
            };
            let error = !matches!(e, AiError::Cancelled);
            panel(tab, |p| p.messages.push(Msg { text, error, retry, settings, ..Default::default() }));
            push(ui);
        }
    }
}

/// Acrobat's Generate presentation: the outline set as slides, saved where the user says.
fn save_deck(ui: &AppWindow, tab: i32, title: &str, outline: &str, pptx: bool) {
    let slides = deck::parse(outline);
    if slides.is_empty() {
        return say(ui, tab, false, "The answer has no slides in it (no \"# Title\" lines). Ask again, or try another model.".into(), true);
    }
    let ext = if pptx { "pptx" } else { "pdf" };
    let d = crate::dialog::new(ui).set_title("Save the slides").set_file_name(format!("{title} slides.{ext}")).add_filter(if pptx { "PowerPoint" } else { "PDF" }, &[ext]);
    let title = format!("{title} slides");
    crate::dialog::save_file(ui, d, move |ui, out| {
        panel(tab, |p| {
            p.busy = true;
            p.status = "Writing the slides…".into();
        });
        push(ui);
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let r = deck::write(&title, &slides, &out);
            let _ = weak.upgrade_in_event_loop(move |ui| {
                panel(tab, |p| {
                    p.busy = false;
                    p.status.clear();
                    match r {
                        Ok(path) => p.messages.push(Msg { text: format!("{} slides saved: {}", slides.len(), path.display()), saved: Some(path), ..Default::default() }),
                        Err(e) => p.messages.push(Msg { text: format!("The slides could not be saved: {e}"), error: true, ..Default::default() }),
                    }
                });
                push(&ui);
            });
        });
    });
}

/// Acrobat's podcast, spoken: the script read by the computer's voices, one per host, into a
/// WAV file the user names; then opened from its message.
fn save_audio(ui: &AppWindow, tab: i32, k: &str) {
    let Ok(k) = k.parse::<usize>() else { return };
    let Some(script) = panel(tab, |p| p.messages.get(k).map(|m| m.text.clone())) else { return };
    if panel(tab, |p| p.busy) {
        return;
    }
    let title = reader_ui::ai_doc(tab).map(|d| d.path.file_stem().unwrap_or_default().to_string_lossy().to_string()).unwrap_or_else(|| "Podcast".into());
    let d = crate::dialog::new(ui).set_title("Save the podcast audio").set_file_name(format!("{title} podcast.wav")).add_filter("WAV audio", &["wav"]);
    crate::dialog::save_file(ui, d, move |ui, out| {
        let cancel = Arc::new(AtomicBool::new(false));
        panel(tab, |p| {
            p.busy = true;
            p.status = "Recording the podcast…".into();
            p.cancel = cancel.clone();
        });
        push(ui);
        let voice = crate::home::prefs().reading_voice;
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let voices = ai::audio::host_voices(voice.as_deref(), &crate::run::voices());
            let status = weak.clone();
            let r = ai::audio::podcast_wav(&script, &ai::audio::SystemSynth, &voices, &out, &mut |i, n| {
                let _ = status.upgrade_in_event_loop(move |ui| {
                    panel(tab, |p| if p.busy { p.status = format!("Recording the podcast · turn {i} of {n}…") });
                    push(&ui);
                });
                !cancel.load(Ordering::Relaxed)
            });
            let _ = weak.upgrade_in_event_loop(move |ui| {
                panel(tab, |p| {
                    p.busy = false;
                    p.status.clear();
                    p.messages.push(match r {
                        Ok(path) => Msg { text: format!("Podcast audio saved: {}", path.display()), saved: Some(path), ..Default::default() },
                        Err(e) => Msg { text: format!("The podcast could not be recorded: {e}"), error: true, ..Default::default() },
                    });
                });
                push(&ui);
            });
        });
    });
}

fn message_act(ui: &AppWindow, tab: i32, req: &Request) {
    let Ok(k) = req.arg.parse::<usize>() else { return };
    let Some((text, saved)) = panel(tab, |p| p.messages.get(k).map(|m| (m.text.clone(), m.saved.clone()))) else { return };
    match req.action.as_str() {
        "copy" => {
            let ok = arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).is_ok();
            crate::flow::toast(ui, if ok { "Copied" } else { "The clipboard is not available" });
        }
        "open" => {
            if let Some(path) = saved {
                if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
                    crate::deliver(vec![path]);
                } else {
                    crate::run::open_detached(&path);
                }
            }
        }
        _ => {
            let title = reader_ui::ai_doc(tab).map(|d| d.path.file_stem().unwrap_or_default().to_string_lossy().to_string()).unwrap_or_else(|| "Answer".into());
            let d = crate::dialog::new(ui).set_title("Save the answer").set_file_name(format!("{title} AI.txt")).add_filter("Text", &["txt", "md"]).add_filter("PDF", &["pdf"]);
            crate::dialog::save_file(ui, d, move |ui, out| match ai::write_text(&format!("{title} AI"), &text, &out) {
                Ok(p) => {
                    panel(tab, |pa| if let Some(m) = pa.messages.get_mut(k) { m.saved = Some(p.clone()) });
                    crate::flow::toast(ui, &format!("Saved {}", p.file_name().unwrap_or_default().to_string_lossy()));
                    push(ui);
                }
                Err(e) => say(ui, tab, false, e.to_string(), true),
            });
        }
    }
}

/// The name of the AI space a file dropped on reader tab `tab` goes into: the panel is open there
/// on a space (Acrobat adds files dropped on a space). None: the drop does what it always does.
pub fn drop_space(tab: i32) -> Option<String> {
    with(|s| {
        if s.open != Some(tab) {
            return None;
        }
        s.space.checked_sub(1).and_then(|i| s.spaces.get(i)).map(|sp| sp.name.clone())
    })
}

/// A file dropped while [`drop_space`]: a PDF joins the space, anything else is refused.
pub fn drop_into_space(ui: &AppWindow, path: PathBuf) {
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        return crate::flow::toast(ui, "Only PDFs go in an AI space");
    }
    let name = with(|s| {
        let i = s.space.checked_sub(1)?;
        let sp = s.spaces.get_mut(i)?;
        spaces::add(sp, [path]);
        Some(sp.name.clone())
    });
    if let Some(name) = name {
        save_spaces(ui);
        crate::flow::toast(ui, &format!("Added to the space “{name}”"));
    }
    push(ui);
}

fn save_spaces(ui: &AppWindow) {
    if let Err(e) = with(|s| spaces::save(&dir(), &s.spaces)) {
        crate::flow::toast(ui, &format!("Spaces not saved: {e}"));
    }
}

/// Acrobat's AI spaces: a named group of PDFs, the open document first, asked together.
fn space_act(ui: &AppWindow, action: &str, k: i32) {
    let k = usize::try_from(k).unwrap_or(0);
    match action {
        "select" => with(|s| s.space = k.min(s.spaces.len())),
        "new" => {
            let doc = with(|s| s.open).and_then(reader_ui::ai_doc);
            with(|s| {
                let mut sp = Space { name: spaces::fresh_name(&s.spaces, "Space"), files: Vec::new() };
                spaces::add(&mut sp, doc.map(|d| d.path));
                s.spaces.push(sp);
                s.space = s.spaces.len();
            });
            save_spaces(ui);
        }
        "add" => {
            let d = crate::dialog::new(ui).set_title("Add PDFs to the space").add_filter("PDF", &["pdf"]);
            crate::dialog::pick_files(ui, d, move |ui, files| {
                with(|s| {
                    if let Some(sp) = s.space.checked_sub(1).and_then(|i| s.spaces.get_mut(i)) {
                        spaces::add(sp, files);
                    }
                });
                save_spaces(ui);
                push(ui);
            });
        }
        "open" => {
            // A space's file opens in its own tab (Acrobat opens a space's sources).
            if let Some(f) = with(|s| s.space.checked_sub(1).and_then(|i| s.spaces.get(i)).and_then(|sp| sp.files.get(k).cloned())) {
                crate::deliver(vec![f]);
            }
            return;
        }
        "rename" => {
            // Acrobat's Rename space: the name typed, trimmed, unique among the spaces.
            let name = with(|s| s.renaming.take()).unwrap_or_default();
            with(|s| {
                if let Some(i) = s.space.checked_sub(1).filter(|i| *i < s.spaces.len()) {
                    let others: Vec<Space> = s.spaces.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, sp)| sp.clone()).collect();
                    let n = name.trim();
                    if !n.is_empty() && n != s.spaces[i].name {
                        s.spaces[i].name = spaces::fresh_name(&others, n);
                    }
                }
            });
            save_spaces(ui);
        }
        "remove" => {
            with(|s| {
                if let Some(sp) = s.space.checked_sub(1).and_then(|i| s.spaces.get_mut(i))
                    && k < sp.files.len()
                {
                    sp.files.remove(k);
                }
            });
            save_spaces(ui);
        }
        "delete" => {
            with(|s| {
                if let Some(i) = s.space.checked_sub(1).filter(|i| *i < s.spaces.len()) {
                    s.spaces.remove(i);
                    s.space = 0;
                }
            });
            save_spaces(ui);
        }
        _ => {}
    }
    push(ui);
}

// ---- Preferences ▸ AI ------------------------------------------------------------------------

fn chosen(p: &AiPrefs) -> Option<Provider> {
    p.provider.as_deref().and_then(Provider::from_id)
}

fn push_prefs(ui: &AppWindow) {
    let g = ui.global::<AiBridge>();
    let p = crate::home::prefs().ai;
    let c = chosen(&p);
    g.set_pref_provider(c.and_then(|c| Provider::ALL.iter().position(|x| *x == c)).map_or(0, |i| i as i32 + 1));
    g.set_pref_model(c.and_then(|c| p.models.get(c.id()).cloned()).unwrap_or_default().into());
    g.set_pref_model_hint(c.map_or("", |c| c.default_model()).into());
    g.set_pref_base(c.and_then(|c| p.base_urls.get(c.id()).cloned()).unwrap_or_default().into());
    g.set_pref_base_hint(c.map_or("", |c| if c == Provider::Compatible { "https://… /v1 (required)" } else { c.default_base() }).into());
    g.set_pref_needs_key(c.is_some_and(|c| !c.is_local()));
    g.set_pref_has_key(c.is_some_and(|c| with(|s| s.keys.contains_key(c.id()))));
    g.set_pref_auto_open(p.auto_open);
    g.set_off(p.off);
}

fn pref(ui: &AppWindow, field: &str, value: &str) {
    let p = crate::home::prefs().ai;
    let c = chosen(&p);
    let status = |t: &str| ui.global::<AiBridge>().set_pref_status(t.into());
    match field {
        "provider" => {
            let id = value.parse::<usize>().ok().and_then(|i| i.checked_sub(1)).and_then(|i| Provider::ALL.get(i)).map(|p| p.id().to_string());
            crate::home::update_ai(ui, |a| a.provider = id);
            status("");
        }
        "model" | "base" => {
            let Some(c) = c else { return };
            let v = value.trim().to_string();
            crate::home::update_ai(ui, |a| {
                let map = if field == "model" { &mut a.models } else { &mut a.base_urls };
                if v.is_empty() { map.remove(c.id()) } else { map.insert(c.id().into(), v) };
            });
        }
        "key" | "forget-key" => {
            let Some(c) = c else { return };
            let keys = with(|s| {
                if field == "key" && !value.trim().is_empty() {
                    s.keys.insert(c.id().into(), value.trim().into());
                } else {
                    s.keys.remove(c.id());
                }
                s.keys.clone()
            });
            match ai_store::save_keys(&dir(), &keys) {
                Ok(()) => status(if field == "key" { "Key saved, encrypted on this computer." } else { "Key removed." }),
                Err(e) => status(&format!("The key could not be saved: {e}")),
            }
        }
        "off" => crate::home::update_ai(ui, |a| a.off = value == "1"),
        "auto-open" => crate::home::update_ai(ui, |a| a.auto_open = value == "1"),
        "test" => {
            status("Testing…");
            let keys = with(|s| s.keys.clone());
            let weak = ui.as_weak();
            std::thread::spawn(move || {
                let r = p.config(&keys, ai::detect_local).and_then(|cfg| ai::chat(&cfg, &[ai::Message::user("Reply with the single word OK.")]).map(|a| (cfg.label(), a)));
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    let t = match r {
                        Ok((label, a)) => format!("Connected to {label}: “{}”", a.trim().chars().take(40).collect::<String>()),
                        Err(e) => e.to_string(),
                    };
                    ui.global::<AiBridge>().set_pref_status(t.into());
                });
            });
            return;
        }
        _ => {}
    }
    push_prefs(ui);
    resolve(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file dropped on a document goes into the AI space only while the panel shows a space on
    /// that very tab; otherwise the drop opens it as before.
    #[test]
    fn drops_go_into_the_space_shown_on_that_tab() {
        with(|s| {
            s.open = Some(3);
            s.spaces = vec![Space { name: "Contracts".into(), files: Vec::new() }];
            s.space = 1;
        });
        assert_eq!(drop_space(3).as_deref(), Some("Contracts"));
        assert_eq!(drop_space(4), None);
        with(|s| s.space = 0);
        assert_eq!(drop_space(3), None);
        with(|s| {
            s.space = 1;
            s.open = None;
        });
        assert_eq!(drop_space(3), None);
    }
}
