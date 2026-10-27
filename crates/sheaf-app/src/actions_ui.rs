//! Action Wizard's editor and runner. The saved list and each draft are separate: a suggested
//! action becomes the user's only when saved, and importing never replaces an existing action.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, Sender}};
use slint::{ComponentHandle, ModelRc, VecModel};
use sheaf_app::{ActionBridge, ActionRow, ActionStepRow, ActionCommandRow, AppWindow, actions::{self, ActionStep, SavedAction}, store_file};
use sheaf_core::pdfcraft::engine::actions::Step;

struct State {
    saved: Vec<SavedAction>,
    draft: SavedAction,
    selected: i32,
    step: i32,
    cancel: Arc<AtomicBool>,
    answer: Option<Sender<Option<String>>>,
}

impl Default for State {
    fn default() -> Self {
        Self { saved: vec![], draft: SavedAction::empty(), selected: -1, step: -1, cancel: Arc::new(AtomicBool::new(false)), answer: None }
    }
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn status(ui: &AppWindow, message: impl AsRef<str>) { ui.global::<ActionBridge>().set_status(message.as_ref().into()); }

fn rows(ui: &AppWindow) {
    let rows = STATE.with(|state| {
        let s = state.borrow();
        actions::suggested().iter().map(|a| (a, true))
            .chain(s.saved.iter().map(|a| (a, false)))
            .map(|(a, builtin)| ActionRow { name: a.name.as_str().into(), description: a.description.as_str().into(), builtin, custom: a.custom_tool }).collect::<Vec<_>>()
    });
    ui.global::<ActionBridge>().set_rows(ModelRc::new(VecModel::from(rows)));
    let commands = STATE.with(|state| {
        state.borrow().saved.iter().enumerate().filter(|(_, a)| a.custom_tool).flat_map(|(i, a)| {
            a.steps.iter().enumerate().map(move |(j, step)| ActionCommandRow {
                label: format!("{} · {}", a.name, step.engine().map(|s| s.label()).unwrap_or("Step")).into(),
                action: i as i32, step: j as i32,
            })
        }).collect::<Vec<_>>()
    });
    ui.global::<ActionBridge>().set_commands(ModelRc::new(VecModel::from(commands)));
}

fn draft(ui: &AppWindow) {
    STATE.with(|state| {
        let s = state.borrow();
        let b = ui.global::<ActionBridge>();
        b.set_selected(s.selected);
        b.set_name(s.draft.name.as_str().into());
        b.set_description(s.draft.description.as_str().into());
        b.set_custom(s.draft.custom_tool);
        b.set_step(s.step);
        b.set_steps(ModelRc::new(VecModel::from(s.draft.steps.iter().map(|step| ActionStepRow {
            label: step.engine().map(|s| s.label()).unwrap_or("Unknown tool").into(),
            detail: if step.prompt { "Pauses for you" } else if !step.instruction.is_empty() { "Instructions" } else { "" }.into(),
        }).collect::<Vec<_>>())));
        let step = usize::try_from(s.step).ok().and_then(|i| s.draft.steps.get(i));
        b.set_argument(step.map(|s| s.argument.as_str()).unwrap_or_default().into());
        b.set_instruction(step.map(|s| s.instruction.as_str()).unwrap_or_default().into());
        b.set_pause(step.is_some_and(|s| s.prompt));
        b.set_has_argument(step.and_then(|s| s.engine().ok()).is_some_and(|s| s.arg().is_some()));
    });
}

fn flush(ui: &AppWindow) {
    let b = ui.global::<ActionBridge>();
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.draft.name = b.get_name().to_string();
        s.draft.description = b.get_description().to_string();
        s.draft.custom_tool = b.get_custom();
        let index = s.step;
        if let Some(step) = usize::try_from(index).ok().and_then(|i| s.draft.steps.get_mut(i)) {
            step.argument = b.get_argument().to_string();
            step.instruction = b.get_instruction().to_string();
            step.prompt = b.get_pause();
        }
    });
}

pub fn install(ui: &AppWindow) {
    match actions::load(&store_file::data_dir()) {
        Ok(saved) => STATE.with(|s| s.borrow_mut().saved = saved),
        Err(e) => status(ui, format!("{e}. Your saved file has been left unchanged.")),
    }
    ui.global::<ActionBridge>().set_tools(ModelRc::new(VecModel::from(Step::all().iter().map(|s| s.label().into()).collect::<Vec<slint::SharedString>>())));
    rows(ui);
    draft(ui);
    match sheaf_app::watch_folder::SavedWatch::load(&store_file::data_dir()) {
        Ok(Some(saved)) => ui.global::<ActionBridge>().set_saved_watch(saved.description().into()),
        Ok(None) => {}, Err(e) => status(ui, e.to_string()),
    }
    let weak = ui.as_weak();
    ui.global::<ActionBridge>().on_open_page(move |page| {
        if let Some(ui) = weak.upgrade() {
            ui.set_active_tab(-1);
            if page == "library" { ui.set_page(sheaf_app::Page::Library); }
            else { ui.set_page(sheaf_app::Page::Actions); }
        }
    });
    let weak = ui.as_weak();
    ui.global::<ActionBridge>().on_command(move |what, index| {
        if let Some(ui) = weak.upgrade() { command(&ui, &what, index); }
    });
    let weak = ui.as_weak();
    ui.global::<ActionBridge>().on_answer(move |yes| {
        STATE.with(|s| { if let Some(tx) = s.borrow_mut().answer.take() { let argument = weak.upgrade().map(|ui| ui.global::<ActionBridge>().get_prompt_argument().to_string()).unwrap_or_default(); let _ = tx.send(yes.then_some(argument)); } });
        if let Some(ui) = weak.upgrade() { ui.global::<ActionBridge>().set_prompt("".into()); }
    });
}

fn command(ui: &AppWindow, what: &str, index: i32) {
    if what == "custom-tools" {
        rows(ui);
        ui.global::<ActionBridge>().set_tools_visible(true);
        return;
    }
    if what == "custom-command" && !ui.global::<ActionBridge>().get_busy() {
        use slint::Model;
        if let Some(row) = ui.global::<ActionBridge>().get_commands().row_data(index as usize) {
            let action = STATE.with(|state| {
                let s = state.borrow();
                let mut action = s.saved.get(row.action as usize)?.clone();
                action.steps = vec![action.steps.get(row.step as usize)?.clone()];
                Some(action)
            });
            if let Some(action) = action {
                ui.global::<ActionBridge>().set_tools_visible(false);
                ui.set_active_tab(-1); ui.set_page(sheaf_app::Page::Actions);
                pick_run(ui, action);
            }
        }
        return;
    }
    if what == "stop" {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.cancel.store(true, Ordering::Relaxed);
            if let Some(tx) = s.answer.take() { let _ = tx.send(None); }
        });
        ui.global::<ActionBridge>().set_prompt("".into());
        return;
    }
    if what == "open-output" || what == "reveal-output" {
        use slint::Model;
        if let Some(path) = ui.global::<ActionBridge>().get_outputs().row_data(index as usize) {
            let path = PathBuf::from(path.as_str());
            if what == "open-output" { crate::deliver(vec![path]); } else { crate::run::reveal(&path); }
        }
        return;
    }
    if ui.global::<ActionBridge>().get_busy() { return; }
    flush(ui);
    match what {
        "paste" => {
            let action = SavedAction::import(ui.global::<ActionBridge>().get_import_text().as_str());
            return imported(ui, action);
        }
        "portfolio" => { ui.global::<sheaf_app::PortfolioBridge>().invoke_open(); return; }
        "watch" => return watch(ui),
        "resume-watch" => {
            match sheaf_app::watch_folder::SavedWatch::load(&store_file::data_dir()) {
                Ok(Some(saved)) => start_watch(ui, saved), Ok(None) => status(ui, "No saved watch"), Err(e) => status(ui, e.to_string()),
            }
            return;
        }
        "forget-watch" => {
            match sheaf_app::watch_folder::SavedWatch::forget(&store_file::data_dir()) {
                Ok(()) => ui.global::<ActionBridge>().set_saved_watch("".into()), Err(e) => status(ui, e.to_string()),
            }
            return;
        }
        "import" => return import(ui),
        "export" | "export-stirling" => return export(ui, what == "export-stirling"),
        "run" | "run-step" => {
            let mut action = STATE.with(|s| s.borrow().draft.clone());
            if what == "run-step" {
                let Some(step) = usize::try_from(index).ok().and_then(|i| action.steps.get(i)).cloned() else { return };
                action.steps = vec![step];
            }
            return pick_run(ui, action);
        }
        _ => {}
    }
    let result: sheaf_core::Result<()> = STATE.with(|state| {
        let mut s = state.borrow_mut();
        let builtins = actions::suggested();
        let offset = builtins.len() as i32;
        match what {
            "new" | "new-tool" => {
                s.draft = SavedAction::empty(); s.draft.custom_tool = what == "new-tool";
                if s.draft.custom_tool { s.draft.name = "Untitled custom tool".into(); }
                s.selected = -1; s.step = -1;
            }
            "select" => {
                if let Some(a) = builtins.iter().chain(s.saved.iter()).nth(index as usize).cloned() {
                    s.draft = a; s.selected = index; s.step = -1;
                }
            }
            "copy" => {
                let all: Vec<_> = builtins.into_iter().chain(s.saved.clone()).collect();
                s.draft.name = actions::unused_name(&s.draft.name, &all);
                s.selected = -1;
            }
            "save" => {
                s.draft.validate()?;
                if s.selected >= 0 && s.selected < offset {
                    let all: Vec<_> = builtins.iter().cloned().chain(s.saved.clone()).collect();
                    s.draft.name = actions::unused_name(&s.draft.name, &all);
                }
                let mut next = s.saved.clone();
                let own = (s.selected >= offset).then_some((s.selected - offset) as usize);
                if next.iter().enumerate().any(|(i, a)| Some(i) != own && a.name.eq_ignore_ascii_case(&s.draft.name)) {
                    return Err(sheaf_core::OpdfError::Unsupported("An action already has that name. Choose another name.".into()));
                }
                if let Some(i) = own { next[i] = s.draft.clone(); } else { next.push(s.draft.clone()); }
                actions::save(&store_file::data_dir(), &next)?;
                s.selected = own.unwrap_or(next.len() - 1) as i32 + offset;
                s.saved = next;
            }
            "delete" if s.selected >= offset => {
                let mut next = s.saved.clone();
                next.remove((s.selected - offset) as usize);
                actions::save(&store_file::data_dir(), &next)?;
                s.saved = next; s.draft = SavedAction::empty(); s.selected = -1; s.step = -1;
            }
            "add" => {
                if let Some(step) = Step::all().get(index as usize) {
                    if s.draft.steps.len() >= 100 { return Err(sheaf_core::OpdfError::Unsupported("An action supports up to 100 steps".into())); }
                    s.draft.steps.push(ActionStep::from_engine(step));
                    s.step = s.draft.steps.len() as i32 - 1;
                }
            }
            "duplicate" => { let at = usize::try_from(s.step).ok(); if let Some(at) = at { s.draft.duplicate_step(at)?; s.step += 1; } }
            "step" => s.step = index,
            "remove" if s.step >= 0 => { let i = s.step as usize; s.draft.steps.remove(i); s.step = -1; }
            "up" | "down" if s.step >= 0 => {
                let from = s.step as usize;
                let to = s.step + if what == "up" { -1 } else { 1 };
                if to >= 0 && (to as usize) < s.draft.steps.len() { s.draft.steps.swap(from, to as usize); s.step = to; }
            }
            _ => {}
        }
        Ok(())
    });
    match result { Ok(()) => { rows(ui); draft(ui); if what == "save" { status(ui, "Action saved"); } }, Err(e) => status(ui, e.to_string()) }
}

fn imported(ui: &AppWindow, action: sheaf_core::Result<SavedAction>) {
    let result = action.and_then(|mut a| {
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            let all: Vec<_> = actions::suggested().into_iter().chain(s.saved.clone()).collect();
            a.name = actions::unused_name(&a.name, &all);
            let mut next = s.saved.clone(); next.push(a.clone());
            actions::save(&store_file::data_dir(), &next)?;
            s.saved = next; s.draft = a; s.selected = (actions::suggested().len() + s.saved.len() - 1) as i32; s.step = -1;
            Ok(())
        })
    });
    match result { Ok(()) => { rows(ui); draft(ui); status(ui, "Action imported"); }, Err(e) => status(ui, e.to_string()) }
}

fn import(ui: &AppWindow) {
    let d = crate::dialog::new(ui).add_filter("Sheaf or Stirling action JSON", &["json"]);
    crate::dialog::pick_file(ui, d, |ui, path| {
        import_path(ui, &path);
    });
}

pub fn import_path(ui: &AppWindow, path: &std::path::Path) {
    let result = (|| {
        if std::fs::metadata(path)?.len() > 10 * 1024 * 1024 {
            return Err(sheaf_core::OpdfError::Unsupported("An action file must be smaller than 10 MB".into()));
        }
        SavedAction::import(&std::fs::read_to_string(path)?)
    })();
    imported(ui, result);
}

fn export(ui: &AppWindow, stirling: bool) {
    let json = match STATE.with(|s| if stirling { s.borrow().draft.stirling_json() } else { s.borrow().draft.json() }) { Ok(json) => json, Err(e) => return status(ui, e.to_string()) };
    let d = crate::dialog::new(ui).set_file_name(if stirling { "action.automate.json" } else { "action.json" }).add_filter("Action JSON", &["json"]);
    crate::dialog::save_file(ui, d, move |ui, path| {
        let result = sheaf_core::paths::atomic_write(&path, |part| { std::fs::write(part, &json)?; Ok(()) });
        match result { Ok(p) => status(ui, format!("Action exported to {}", p.display())), Err(e) => status(ui, e.to_string()) }
    });
}

fn pick_run(ui: &AppWindow, action: SavedAction) {
    if let Err(e) = action.validate() { return status(ui, e.to_string()); }
    let d = crate::dialog::new(ui).add_filter("PDF", &["pdf"]);
    crate::dialog::pick_files(ui, d, move |ui, files| {
        let d = crate::dialog::new(ui).set_title("Choose the output folder");
        crate::dialog::pick_folder(ui, d, move |ui, out| start(ui, action, files, out));
    });
}

fn start(ui: &AppWindow, action: SavedAction, files: Vec<PathBuf>, out: PathBuf) {
    let password = ui.global::<ActionBridge>().get_password().to_string();
    ui.global::<ActionBridge>().set_password("".into());
    ui.global::<ActionBridge>().set_outputs(ModelRc::new(VecModel::default()));
    let cancel = Arc::new(AtomicBool::new(false));
    STATE.with(|s| s.borrow_mut().cancel = cancel.clone());
    ui.global::<ActionBridge>().set_busy(true);
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let mut written = 0;
        let mut errors = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) { break; }
            let r = actions::run_file_configured(&action, file, &out, (!password.is_empty()).then_some(password.as_str()), &mut |i, step| {
                if cancel.load(Ordering::Relaxed) { return false; }
                let message = format!("File {}/{} · Step {}/{} · {}", file_index + 1, files.len(), i + 1, action.steps.len(), step.engine().map(|s| s.label()).unwrap_or("Step"));
                let prompt = if step.prompt { format!("{message}\n{}", step.instruction) } else { String::new() };
                let message = if !step.instruction.is_empty() { format!("{message} · {}", step.instruction) } else { message };
                let (tx, rx) = mpsc::channel();
                let pause = step.prompt;
                let argument = step.argument.clone();
                let has_argument = step.engine().is_ok_and(|s| s.arg().is_some());
                if weak.upgrade_in_event_loop(move |ui| {
                    status(&ui, &message);
                    if pause {
                        STATE.with(|s| s.borrow_mut().answer = Some(tx));
                        ui.global::<ActionBridge>().set_prompt_argument(argument.into());
                        ui.global::<ActionBridge>().set_prompt_has_argument(has_argument);
                        ui.global::<ActionBridge>().set_prompt(prompt.into());
                    }
                }).is_err() { return false; }
                if !pause { return !cancel.load(Ordering::Relaxed); }
                loop {
                    match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                        Ok(Some(argument)) if !cancel.load(Ordering::Relaxed) => { step.argument = argument; return true; }
                        Ok(_) => return false,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return false,
                        Err(mpsc::RecvTimeoutError::Timeout) if cancel.load(Ordering::Relaxed) => return false,
                        Err(_) => {}
                    }
                }
            });
            match r {
                Ok(path) => {
                    written += 1;
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        use slint::Model;
                        let model = ui.global::<ActionBridge>().get_outputs();
                        if let Some(model) = model.as_any().downcast_ref::<VecModel<slint::SharedString>>() { model.push(path.to_string_lossy().as_ref().into()); }
                    });
                },
                Err(sheaf_core::OpdfError::Cancelled) => { cancel.store(true, Ordering::Relaxed); break; }
                Err(e) => errors.push(format!("{}: {e}", file.display())),
            }
        }
        let message = format!("{}{} file(s) written to {}. {}", if cancel.load(Ordering::Relaxed) { "Stopped. " } else { "" }, written, out.display(), errors.join(" · "));
        let _ = weak.upgrade_in_event_loop(move |ui| { ui.global::<ActionBridge>().set_busy(false); ui.global::<ActionBridge>().set_prompt("".into()); status(&ui, message); });
    });
}

fn watch(ui: &AppWindow) {
    let action = STATE.with(|s| s.borrow().draft.clone());
    if let Err(e) = action.validate() { return status(ui, e.to_string()); }
    if action.steps.iter().any(|s| s.prompt) { return status(ui, "Folder actions cannot pause. Turn off the step prompts first."); }
    let title_names = ui.global::<ActionBridge>().get_title_names();
    let recursive = ui.global::<ActionBridge>().get_recursive();
    let d = crate::dialog::new(ui).set_title("Choose the folder to watch");
    crate::dialog::pick_folder(ui, d, move |ui, input| {
        let d = crate::dialog::new(ui).set_title("Choose a separate output folder");
        crate::dialog::pick_folder(ui, d, move |ui, output| {
            let mut watch = match sheaf_app::watch_folder::FolderWatch::new(&input, &output, title_names) {
                Ok(watch) => watch, Err(e) => return status(ui, e.to_string()),
            };
            watch.include_subfolders(recursive);
            let saved = match sheaf_app::watch_folder::SavedWatch::new(action, watch) {
                Ok(saved) => saved, Err(e) => return status(ui, e.to_string()),
            };
            start_watch(ui, saved);
        });
    });
}

fn start_watch(ui: &AppWindow, mut saved: sheaf_app::watch_folder::SavedWatch) {
    if let Err(e) = saved.save(&store_file::data_dir()) { return status(ui, e.to_string()); }
    let description = saved.description();
    ui.global::<ActionBridge>().set_saved_watch(description.clone().into());
    let cancel = Arc::new(AtomicBool::new(false));
    STATE.with(|s| s.borrow_mut().cancel = cancel.clone());
    ui.global::<ActionBridge>().set_busy(true);
    status(ui, format!("Watching while Sheaf is open. {description}"));
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let mut written = 0;
        let mut failure = String::new();
        while !cancel.load(Ordering::Relaxed) {
            let message = match saved.watch.poll(&saved.action, &mut || !cancel.load(Ordering::Relaxed)) {
                Ok(results) if results.is_empty() => None,
                Ok(results) => {
                    let mut errors = Vec::new();
                    for (path, result) in results {
                        match result { Ok(_) => written += 1, Err(e) => errors.push(format!("{}: {e}", path.display())) }
                    }
                    if let Err(e) = saved.save(&store_file::data_dir()) {
                        failure = format!("Cannot checkpoint this watch: {e}");
                        cancel.store(true, Ordering::Relaxed);
                        errors.push(failure.clone());
                    }
                    Some(format!("Watching · {written} file(s) written. {}", errors.join(" · ")))
                }
                Err(e) => { cancel.store(true, Ordering::Relaxed); failure = e.to_string(); Some(format!("Folder watch stopped: {e}")) }
            };
            if let Some(message) = message
                && weak.upgrade_in_event_loop(move |ui| status(&ui, message)).is_err() { break; }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.global::<ActionBridge>().set_busy(false);
            status(&ui, format!("Folder watch stopped. {written} file(s) written. {failure}"));
        });
    });
}
