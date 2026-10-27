//! Action Wizard's editor and runner. The saved list and each draft are separate: a suggested
//! action becomes the user's only when saved, and importing never replaces an existing action.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, Sender}};
use slint::{ComponentHandle, ModelRc, VecModel};
use sheaf_app::{ActionBridge, ActionRow, ActionStepRow, ActionCommandRow, AppWindow, actions::{self, ActionStep, SavedAction}, store_file};
use sheaf_core::pdfcraft::engine::actions::Step;

enum Pending { Command(String, i32), Import(SavedAction) }

struct State {
    saved: Vec<SavedAction>,
    baseline: SavedAction,
    pending: Option<Pending>,
    approved: bool,
    draft: SavedAction,
    selected: i32,
    step: i32,
    cancel: Arc<AtomicBool>,
    answer: Option<Sender<Option<String>>>,
    autosave: slint::Timer,
    checkpoint: String,
    watches: Vec<Arc<AtomicBool>>,
    watch_ids: std::collections::HashSet<String>,
}

impl Default for State {
    fn default() -> Self {
        Self { saved: vec![], baseline: SavedAction::empty(), pending: None, approved: false, draft: SavedAction::empty(), selected: -1, step: -1, cancel: Arc::new(AtomicBool::new(false)), answer: None, autosave: slint::Timer::default(), checkpoint: String::new(), watches: Vec::new(), watch_ids: std::collections::HashSet::new() }
    }
}

impl State {
    fn changed(&self) -> bool {
        self.draft != self.baseline || (self.selected < 0 && !self.draft.steps.is_empty())
    }
}

fn guard(ui: &AppWindow, pending: Pending) -> bool {
    let changed = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if std::mem::take(&mut s.approved) || !s.changed() { return false; }
        s.pending = Some(pending);
        true
    });
    if changed { ui.global::<ActionBridge>().set_discard_visible(true); }
    changed
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn status(ui: &AppWindow, message: impl AsRef<str>) { ui.global::<ActionBridge>().set_status(message.as_ref().into()); }

fn rows(ui: &AppWindow) {
    let filter = ui.global::<ActionBridge>().get_filter();
    let query = ui.global::<ActionBridge>().get_query();
    let rows = STATE.with(|state| {
        let s = state.borrow();
        actions::suggested().iter().map(|a| (a, true))
            .chain(s.saved.iter().map(|a| (a, false)))
            .enumerate().filter(|(_, (a, builtin))| match filter { 1 => *builtin, 2 => !*builtin && !a.custom_tool, 3 => !*builtin && a.custom_tool, _ => true })
            .filter(|(_, (a, _))| a.matches_query(&query))
            .map(|(i, (a, builtin))| ActionRow { name: a.name.as_str().into(), description: a.description.as_str().into(), builtin, custom: a.custom_tool, source: i as i32 }).collect::<Vec<_>>()
    });
    ui.global::<ActionBridge>().set_rows(ModelRc::new(VecModel::from(rows)));
    let commands = STATE.with(|state| {
        state.borrow().saved.iter().enumerate().filter(|(_, a)| a.custom_tool).flat_map(|(i, a)| {
            a.steps.iter().enumerate().map(move |(j, step)| ActionCommandRow {
                label: format!("{} · {}", a.name, step.display_label()).into(),
                icon: sheaf_core::tools::ToolId::from_str(&step.argument).filter(|_| step.tool == "open_tool").map(|id| sheaf_app::icons::icon_image(sheaf_core::tools::find(id).icon)).unwrap_or_default(),
                interactive: step.tool == "open_tool",
                separator: step.separator,
                current: step.supports_current_document(),
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
        b.set_builtin(s.selected >= 0 && s.selected < actions::suggested().len() as i32);
        b.set_name(s.draft.name.as_str().into());
        b.set_description(s.draft.description.as_str().into());
        b.set_custom(s.draft.custom_tool);
        b.set_same_folder(s.draft.output.same_folder);
        b.set_output_prefix(s.draft.output.prefix.as_str().into());
        b.set_output_suffix(s.draft.output.suffix.as_str().into());
        b.set_overwrite(s.draft.output.overwrite);
        b.set_output_format(s.draft.output.format.as_str().into());
        b.set_step(s.step);
        b.set_steps(ModelRc::new(VecModel::from(s.draft.steps.iter().map(|step| ActionStepRow {
            label: step.display_label().into(),
            detail: if step.prompt { "Pauses for you" } else if !step.instruction.is_empty() { "Instructions" } else { "" }.into(),
        }).collect::<Vec<_>>())));
        let step = usize::try_from(s.step).ok().and_then(|i| s.draft.steps.get(i));
        b.set_tool_id(step.map(|s| s.tool.as_str()).unwrap_or_default().into());
        let argument = step.map(|s| s.argument.as_str()).unwrap_or_default();
        if step.is_some_and(|s| s.tool == "page_numbers") {
            let fields: Vec<_> = argument.splitn(4, '|').collect();
            b.set_number_position(fields.first().and_then(|v| v.parse().ok()).unwrap_or(4));
            b.set_number_start(fields.get(1).copied().unwrap_or("1").into());
            b.set_number_size(fields.get(2).copied().unwrap_or("8").into());
            b.set_argument(fields.get(3).copied().unwrap_or("Page <<1>> of <<n>>").into());
        } else { b.set_argument(argument.into()); }
        b.set_command_label(step.map(|s| s.label.as_str()).unwrap_or_default().into());
        b.set_separator(step.is_some_and(|s| s.separator));
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
        s.draft.output = actions::OutputOptions { same_folder: b.get_same_folder(), prefix: b.get_output_prefix().to_string(), suffix: b.get_output_suffix().to_string(), overwrite: b.get_overwrite(), format: b.get_output_format().to_string() };
        let index = s.step;
        if let Some(step) = usize::try_from(index).ok().and_then(|i| s.draft.steps.get_mut(i)) {
            step.argument = if step.tool == "protect" { String::new() } else if step.tool == "page_numbers" {
                format!("{}|{}|{}|{}", b.get_number_position(), b.get_number_start(), b.get_number_size(), b.get_argument())
            } else { b.get_argument().to_string() };
            step.instruction = b.get_instruction().to_string();
            step.label = b.get_command_label().to_string();
            step.separator = b.get_separator();
            step.prompt = b.get_pause();
        }
    });
}

pub fn install(ui: &AppWindow) {
    match actions::load(&store_file::data_dir()) {
        Ok(saved) => STATE.with(|s| s.borrow_mut().saved = saved),
        Err(e) => status(ui, format!("{e}. Your saved file has been left unchanged.")),
    }
    ui.global::<ActionBridge>().set_toolbox(ModelRc::new(VecModel::from(sheaf_core::tools::catalogue().iter().map(|t| slint::SharedString::from(t.id.as_str())).collect::<Vec<_>>())));
    ui.global::<ActionBridge>().set_tools(ModelRc::new(VecModel::from(Step::all().iter().map(|s| s.label().into()).collect::<Vec<slint::SharedString>>())));
    match actions::load_draft(&store_file::data_dir()) {
        Ok(Some(recovered)) => { STATE.with(|s| { let mut s = s.borrow_mut(); s.draft = recovered; s.baseline = SavedAction::empty(); }); status(ui, "Recovered your action draft. Save it to keep it in your actions."); }
        Ok(None) => {}, Err(e) => status(ui, format!("Draft recovery: {e}")),
    }
    rows(ui);
    draft(ui);
    let weak = ui.as_weak();
    STATE.with(|s| s.borrow().autosave.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(2), move || {
        let Some(ui) = weak.upgrade() else { return };
        if ui.global::<ActionBridge>().get_busy() { return; }
        flush(&ui);
        let result = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let checkpoint = format!("{}:{}", s.changed(), serde_json::to_string(&s.draft).unwrap_or_default());
            if checkpoint == s.checkpoint { return Ok(()); }
            actions::save_draft(&store_file::data_dir(), s.changed().then_some(&s.draft))?;
            s.checkpoint = checkpoint;
            Ok::<_, sheaf_core::OpdfError>(())
        });
        if let Err(e) = result { status(&ui, format!("Draft could not be saved: {e}")); }
    }));
    match sheaf_app::watch_folder::SavedWatch::load_all(&store_file::data_dir()) {
        Ok(saved) => { for watch in saved { start_watch(ui, watch); } }
        Err(e) => status(ui, format!("Saved folder watches could not resume: {e}")),
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
        if let Some(ui) = weak.upgrade() { let b = ui.global::<ActionBridge>(); b.set_prompt("".into()); if b.get_prompt_password() { b.set_prompt_argument("".into()); } }
    });
}

fn command(ui: &AppWindow, what: &str, index: i32) {
    if what == "keep-draft" || what == "discard-draft" {
        let pending = STATE.with(|s| { let mut s = s.borrow_mut(); s.approved = what == "discard-draft" && s.pending.is_some(); s.pending.take() });
        ui.global::<ActionBridge>().set_discard_visible(false);
        if what == "discard-draft" && let Some(pending) = pending {
            match pending { Pending::Command(what, index) => command(ui, &what, index), Pending::Import(action) => imported(ui, Ok(action)) }
        }
        return;
    }
    if what == "filter" { rows(ui); return; }
    if what == "custom-tools" {
        rows(ui);
        // Acrobat's custom commands follow the reader's stable tab ID, not its list position.
        let reader = crate::flow::active_tool(ui).map(|(id, _)| id).filter(|id| crate::reader_ui::is_reader(*id));
        ui.global::<ActionBridge>().set_current_tab(reader.unwrap_or(-1));
        ui.global::<ActionBridge>().set_tools_visible(true);
        return;
    }
    if matches!(what, "custom-command" | "custom-current") && !ui.global::<ActionBridge>().get_busy() {
        use slint::Model;
        if let Some(row) = ui.global::<ActionBridge>().get_commands().row_data(index as usize) {
            let action = STATE.with(|state| {
                let s = state.borrow();
                let mut action = s.saved.get(row.action as usize)?.clone();
                action.steps = vec![action.steps.get(row.step as usize)?.clone()];
                Some(action)
            });
            if let Some(action) = action {
                if let Some(step) = action.steps.first().filter(|s| s.tool == "open_tool") {
                    let Some(tool) = crate::flow::tool_from_str(&step.argument) else { return status(ui, "This toolbox tool is unavailable"); };
                    ui.global::<ActionBridge>().set_tools_visible(false);
                    if !step.instruction.is_empty() { crate::flow::toast(ui, &step.instruction); }
                    crate::flow::open_tool(ui, tool, Vec::new());
                    return;
                }
                if what == "custom-current" {
                    match crate::reader_ui::custom_tools::apply(ui, ui.global::<ActionBridge>().get_current_tab(), &action.steps[0]) {
                        Ok(()) => ui.global::<ActionBridge>().set_tools_visible(false), Err(e) => status(ui, e),
                    }
                    return;
                }
                ui.global::<ActionBridge>().set_tools_visible(false);
                ui.set_active_tab(-1); ui.set_page(sheaf_app::Page::Actions);
                pick_run(ui, action);
            }
        }
        return;
    }
    if what == "stop-watches" {
        STATE.with(|s| { for cancel in &s.borrow().watches { cancel.store(true, Ordering::Relaxed); } });
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
    if what == "select" && STATE.with(|s| s.borrow().selected == index) { return; }
    if matches!(what, "new" | "new-tool" | "select")
        && guard(ui, Pending::Command(what.into(), index)) { return; }
    match what {
        "paste" => {
            let action = SavedAction::import(ui.global::<ActionBridge>().get_import_text().as_str());
            return imported(ui, action);
        }
        "portfolio" => { ui.global::<sheaf_app::PortfolioBridge>().invoke_open(); return; }
        "watch" => return watch(ui),
        "resume-watch" => {
            match sheaf_app::watch_folder::SavedWatch::load_all(&store_file::data_dir()) {
                Ok(saved) => { for watch in saved { start_watch(ui, watch); } }, Err(e) => status(ui, e.to_string()),
            }
            return;
        }
        "forget-watch" => {
            if ui.global::<ActionBridge>().get_watch_active() > 0 { return status(ui, "Stop the active folder watches before forgetting them."); }
            match sheaf_app::watch_folder::SavedWatch::forget(&store_file::data_dir()) {
                Ok(()) => ui.global::<ActionBridge>().set_saved_watch("".into()), Err(e) => status(ui, e.to_string()),
            }
            return;
        }
        "import" => return import(ui),
        "export" | "export-stirling" => return export(ui, what == "export-stirling"),
        "run-open" => {
            let action = STATE.with(|s| s.borrow().draft.clone());
            if let Err(e) = action.validate() { return status(ui, e.to_string()); }
            if action.steps.iter().any(|s| s.tool == "open_tool") { return status(ui, "Choose an interactive command in Custom tools instead of running this set unattended."); }
            let inputs = match crate::reader_ui::custom_tools::open_inputs() { Ok(inputs) => inputs, Err(e) => return status(ui, e) };
            let files = inputs.iter().map(|i| i.path.clone()).collect();
            // Output copies have an explicit destination; never replace an open reader's file.
            let mut action = action; action.output.same_folder = false; action.output.overwrite = false;
            let d = crate::dialog::new(ui).set_title("Save action results from open PDFs");
            crate::dialog::pick_folder(ui, d, move |ui, out| start_with_snapshots(ui, action, files, out, Some(inputs)));
            return;
        }
        "run-folder" => {
            let action = STATE.with(|s| s.borrow().draft.clone());
            if let Err(e) = action.validate() { return status(ui, e.to_string()); }
            let d = crate::dialog::new(ui).set_title("Choose the input PDF folder");
            crate::dialog::pick_folder(ui, d, move |ui, folder| {
                let recursive = ui.global::<ActionBridge>().get_recursive();
                let cancel = Arc::new(AtomicBool::new(false));
                STATE.with(|s| s.borrow_mut().cancel = cancel.clone());
                ui.global::<ActionBridge>().set_busy(true);
                status(ui, "Finding PDFs in the input folder…");
                let weak = ui.as_weak();
                std::thread::spawn(move || {
                    let files = actions::folder_inputs_cancellable(&folder, recursive, &mut || !cancel.load(Ordering::Relaxed));
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        ui.global::<ActionBridge>().set_busy(false);
                        if cancel.load(Ordering::Relaxed) { status(&ui, "Folder search stopped. No files were processed."); return; }
                        match files { Ok(files) => pick_output(&ui, action, files), Err(e) => status(&ui, e.to_string()) }
                    });
                });
            });
            return;
        }
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
                s.baseline = s.draft.clone();
                s.selected = -1; s.step = -1;
            }
            "select" => {
                if let Some(a) = builtins.iter().chain(s.saved.iter()).nth(index as usize).cloned() {
                    s.baseline = a.clone(); s.draft = a; s.selected = index; s.step = -1;
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
                if next.iter().enumerate().any(|(i, a)| Some(i) != own && a.name.to_lowercase() == s.draft.name.to_lowercase()) {
                    return Err(sheaf_core::OpdfError::Unsupported("An action already has that name. Choose another name.".into()));
                }
                if let Some(i) = own { next[i] = s.draft.clone(); } else { next.push(s.draft.clone()); }
                actions::save(&store_file::data_dir(), &next)?;
                s.selected = own.unwrap_or(next.len() - 1) as i32 + offset;
                s.saved = next; s.baseline = s.draft.clone();
            }
            "delete" if s.selected >= offset => {
                let mut next = s.saved.clone();
                next.remove((s.selected - offset) as usize);
                actions::save(&store_file::data_dir(), &next)?;
                s.saved = next; s.draft = SavedAction::empty(); s.baseline = s.draft.clone(); s.selected = -1; s.step = -1;
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
    flush(ui);
    let action = match action { Ok(a) => a, Err(e) => return status(ui, e.to_string()) };
    if guard(ui, Pending::Import(action.clone())) { return; }
    let mut a = action;
    let result: sheaf_core::Result<()> = STATE.with(|state| {
            let mut s = state.borrow_mut();
            let all: Vec<_> = actions::suggested().into_iter().chain(s.saved.clone()).collect();
            a.name = actions::unused_name(&a.name, &all);
            let mut next = s.saved.clone(); next.push(a.clone());
            actions::save(&store_file::data_dir(), &next)?;
            s.saved = next; s.baseline = a.clone(); s.draft = a; s.selected = (actions::suggested().len() + s.saved.len() - 1) as i32; s.step = -1;
            Ok(())
    });
    match result { Ok(()) => { rows(ui); draft(ui); let note = STATE.with(|s| s.borrow().draft.description.clone()); status(ui, if note.contains("Import notes:") { format!("Action imported. Review its description before running: {note}") } else { "Action imported".into() }); }, Err(e) => status(ui, e.to_string()) }
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
    if action.steps.iter().any(|s| s.tool == "open_tool") { return status(ui, "This set contains interactive toolbox shortcuts. Open Custom tools and choose a command; it cannot run unattended on PDFs."); }
    let d = crate::dialog::new(ui).add_filter("PDF", &["pdf"]);
    crate::dialog::pick_files(ui, d, move |ui, files| {
        pick_output(ui, action, files);
    });
}

fn pick_output(ui: &AppWindow, action: SavedAction, files: Vec<PathBuf>) {
    if action.output.same_folder { return start(ui, action, files, PathBuf::new()); }
    let d = crate::dialog::new(ui).set_title("Choose the output folder");
    crate::dialog::pick_folder(ui, d, move |ui, out| start(ui, action, files, out));
}

fn start(ui: &AppWindow, action: SavedAction, files: Vec<PathBuf>, out: PathBuf) {
    start_with_snapshots(ui, action, files, out, None);
}

fn start_with_snapshots(ui: &AppWindow, action: SavedAction, files: Vec<PathBuf>, out: PathBuf, snapshots: Option<Vec<actions::ActionInput>>) {
    let password = ui.global::<ActionBridge>().get_password().to_string();
    let per_file_password = ui.global::<ActionBridge>().get_per_file_password();
    ui.global::<ActionBridge>().set_password("".into());
    ui.global::<ActionBridge>().set_outputs(ModelRc::new(VecModel::default()));
    ui.global::<ActionBridge>().set_failures(ModelRc::new(VecModel::default()));
    let cancel = Arc::new(AtomicBool::new(false));
    STATE.with(|s| s.borrow_mut().cancel = cancel.clone());
    ui.global::<ActionBridge>().set_busy(true);
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let mut written = 0;
        let mut errors = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) { break; }
            let snapshot = snapshots.as_ref().and_then(|sources| sources.get(file_index));
            let input_bytes = match snapshot {
                Some(snapshot) => snapshot.bytes.clone(),
                None => match std::fs::read(file) { Ok(bytes) => Arc::new(bytes), Err(e) => { errors.push(format!("{}: {e}", file.display())); continue; } },
            };
            let file_password = if per_file_password {
                let (tx, rx) = mpsc::channel();
                let prompt = format!("Password for {} (leave blank for an unprotected PDF)", file.display());
                if weak.upgrade_in_event_loop(move |ui| {
                    STATE.with(|s| s.borrow_mut().answer = Some(tx));
                    let b = ui.global::<ActionBridge>();
                    b.set_prompt_password(true); b.set_prompt_has_argument(true);
                    b.set_prompt_argument("".into()); b.set_prompt(prompt.into());
                }).is_err() { break; }
                let Some(answer) = wait_answer(&rx, &cancel) else { cancel.store(true, Ordering::Relaxed); break };
                answer
            } else { snapshot.and_then(|s| s.password.clone()).unwrap_or_else(|| password.clone()) };
            let r = actions::run_bytes_configured(&action, file, input_bytes, &out, (!file_password.is_empty()).then_some(file_password.as_str()), &mut |i, step| {
                if cancel.load(Ordering::Relaxed) { return false; }
                let message = format!("File {}/{} · Step {}/{} · {}", file_index + 1, files.len(), i + 1, action.steps.len(), step.engine().map(|s| s.label()).unwrap_or("Step"));
                let prompt = if step.prompt { format!("{message}\n{}", step.instruction) } else { String::new() };
                let message = if !step.instruction.is_empty() { format!("{message} · {}", step.instruction) } else { message };
                let (tx, rx) = mpsc::channel();
                let pause = step.prompt;
                let secret = step.tool == "protect";
                let argument = step.argument.clone();
                let has_argument = step.engine().is_ok_and(|s| s.arg().is_some());
                if weak.upgrade_in_event_loop(move |ui| {
                    status(&ui, &message);
                    if pause {
                        STATE.with(|s| s.borrow_mut().answer = Some(tx));
                        ui.global::<ActionBridge>().set_prompt_password(secret);
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
            }, &mut || !cancel.load(Ordering::Relaxed));
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
        let message = format!("{}{} of {} file(s) written · {} failed", if cancel.load(Ordering::Relaxed) { "Stopped. " } else { "" }, written, files.len(), errors.len());
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let b = ui.global::<ActionBridge>();
            b.set_busy(false); b.set_prompt("".into());
            b.set_failures(ModelRc::new(VecModel::from(errors.iter().map(|e| slint::SharedString::from(e.as_str())).collect::<Vec<_>>())));
            status(&ui, message);
        });
    });
}

fn wait_answer(rx: &mpsc::Receiver<Option<String>>, cancel: &AtomicBool) -> Option<String> {
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(answer) if !cancel.load(Ordering::Relaxed) => return answer,
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            Err(mpsc::RecvTimeoutError::Timeout) if cancel.load(Ordering::Relaxed) => return None,
            Err(_) => {},
        }
    }
}

fn watch(ui: &AppWindow) {
    let action = STATE.with(|s| s.borrow().draft.clone());
    if let Err(e) = action.validate() { return status(ui, e.to_string()); }
    if action.steps.iter().any(|s| s.prompt) { return status(ui, "Folder actions cannot pause. Turn off the step prompts first."); }
    let title_names = ui.global::<ActionBridge>().get_title_names();
    let recursive = ui.global::<ActionBridge>().get_recursive();
    let pattern = ui.global::<ActionBridge>().get_watch_pattern().to_string();
    let archive_inputs = ui.global::<ActionBridge>().get_archive_inputs();
    let d = crate::dialog::new(ui).set_title("Choose the folder to watch");
    crate::dialog::pick_folder(ui, d, move |ui, input| {
        let d = crate::dialog::new(ui).set_title("Choose a separate output folder");
        crate::dialog::pick_folder(ui, d, move |ui, output| {
            let mut watch = match sheaf_app::watch_folder::FolderWatch::new(&input, &output, title_names) {
                Ok(watch) => watch, Err(e) => return status(ui, e.to_string()),
            };
            watch.include_subfolders(recursive);
            if let Err(e) = watch.configure_outputs(&pattern, archive_inputs) { return status(ui, e.to_string()); }
            let saved = match sheaf_app::watch_folder::SavedWatch::new(action, watch) {
                Ok(saved) => saved, Err(e) => return status(ui, e.to_string()),
            };
            start_watch(ui, saved);
        });
    });
}

fn start_watch(ui: &AppWindow, mut saved: sheaf_app::watch_folder::SavedWatch) {
    let identity = saved.identity();
    let allowed = STATE.with(|s| { let s = s.borrow(); !s.watch_ids.contains(&identity) });
    if !allowed { status(ui, "This inbox is already active. Stop its watch before starting another action on it."); return; }
    if let Err(e) = saved.save(&store_file::data_dir()) { return status(ui, e.to_string()); }
    let description = saved.description();
    ui.global::<ActionBridge>().set_saved_watch(description.clone().into());
    let cancel = Arc::new(AtomicBool::new(false));
    STATE.with(|s| { let mut s = s.borrow_mut(); s.watches.retain(|c| !c.load(Ordering::Relaxed)); s.watches.push(cancel.clone()); s.watch_ids.insert(identity.clone()); ui.global::<ActionBridge>().set_watch_active(s.watch_ids.len() as i32); });
    status(ui, format!("Watching while Sheaf is open. {description}"));
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let mut written = 0;
        let mut failure = String::new();
        while !cancel.load(Ordering::Relaxed) {
            // One native watch job at a time: many inboxes must not launch many OCR/render jobs.
            static WATCH_WORK: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let work = WATCH_WORK.lock().unwrap_or_else(|e| e.into_inner());
            if cancel.load(Ordering::Relaxed) { break; }
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
            drop(work);
            if let Some(message) = message
                && weak.upgrade_in_event_loop(move |ui| status(&ui, message)).is_err() { break; }
            std::thread::sleep(std::time::Duration::from_millis(if saved.watch.scan_in_progress() { 100 } else { 3000 }));
        }
        let _ = weak.upgrade_in_event_loop(move |ui| {
            STATE.with(|s| { let mut s = s.borrow_mut(); s.watch_ids.remove(&identity); ui.global::<ActionBridge>().set_watch_active(s.watch_ids.len() as i32); });
            status(&ui, format!("Folder watch stopped. {written} file(s) written. {failure}"));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_guard_detects_edits_and_unsaved_copies_but_allows_unchanged_templates() {
        let mut state = State::default();
        assert!(!state.changed());
        state.draft = actions::suggested().remove(0);
        state.baseline = state.draft.clone(); state.selected = 0;
        assert!(!state.changed());
        state.draft.steps[0].instruction = "Review this file".into();
        assert!(state.changed());
        state.draft = state.baseline.clone(); assert!(!state.changed());
        state.selected = -1; assert!(state.changed());
        state.draft = SavedAction::empty(); state.baseline = state.draft.clone();
        assert!(!state.changed());
    }
}
