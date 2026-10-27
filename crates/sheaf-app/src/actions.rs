//! Saved guided actions (Acrobat's Action Wizard): ordered steps, instructions and pauses.
//! The portable JSON contains no machine paths; importing validates every step before saving.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use sheaf_core::pdfcraft::engine::actions::{self, Action, Step};
use sheaf_core::{OpdfError, Result};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActionStep {
    pub tool: String,
    #[serde(default)]
    pub argument: String,
    #[serde(default)]
    pub instruction: String,
    #[serde(default)]
    pub prompt: bool,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub separator: bool,
}

impl ActionStep {
    pub fn from_engine(step: &Step) -> Self {
        Self { tool: step.id().into(), argument: step.arg().unwrap_or_default().into(), instruction: String::new(), prompt: false, label: String::new(), separator: false }
    }

    pub fn engine(&self) -> Result<Step> {
        Step::from_id(&self.tool, &self.argument).ok_or_else(|| invalid(format!("Unknown action tool: {}", self.tool)))
    }
}

/// Acrobat's output options travel with the action, without storing a machine-specific folder.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct OutputOptions {
    pub same_folder: bool,
    pub prefix: String,
    pub suffix: String,
    pub overwrite: bool,
}

impl Default for OutputOptions {
    fn default() -> Self { Self { same_folder: false, prefix: String::new(), suffix: " - processed".into(), overwrite: false } }
}

impl OutputOptions {
    fn validate(&self) -> Result<()> {
        for part in [&self.prefix, &self.suffix] {
            if part.len() > 100 || part.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)) {
                return Err(invalid("Output prefix and suffix need up to 100 bytes without path characters"));
            }
        }
        Ok(())
    }

    fn path(&self, input: &Path, out: &Path) -> Result<PathBuf> {
        self.validate()?;
        let dir = if self.same_folder { input.parent().unwrap_or(Path::new(".")) } else { out };
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        let name = crate::watch_folder::safe_name(&format!("{}{stem}{}", self.prefix, self.suffix));
        Ok(dir.join(format!("{name}.pdf")))
    }
}

/// Folder inputs are bounded and never follow symbolic links, as Folder Scanning does.
pub fn folder_inputs(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            paths.push(entry.path());
        }
        if paths.len() > 100_000 { return Err(invalid("Choose a folder with at most 100000 PDFs")); }
    }
    paths.sort();
    if paths.is_empty() { return Err(invalid("This folder has no PDF files")); }
    Ok(paths)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SavedAction {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Custom tool sets expose each command separately as well as the whole sequence.
    #[serde(default)]
    pub custom_tool: bool,
    pub steps: Vec<ActionStep>,
    #[serde(default)]
    pub output: OutputOptions,
}

fn invalid(message: impl Into<String>) -> OpdfError { OpdfError::Unsupported(message.into()) }

impl SavedAction {
    pub fn empty() -> Self {
        Self { version: 1, name: "Untitled action".into(), description: String::new(), custom_tool: false, output: OutputOptions::default(), steps: vec![] }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 { return Err(invalid("This action needs a different version of Sheaf")); }
        if self.name.trim().is_empty() || self.name.len() > 256 { return Err(invalid("Give the action a name (up to 256 bytes)")); }
        if self.steps.is_empty() || self.steps.len() > 100 { return Err(invalid("An action needs 1–100 steps")); }
        self.output.validate()?;
        for step in &self.steps {
            step.engine()?;
            if step.label.len() > 256 || step.argument.len() > 1_000_000 || step.instruction.len() > 16_384 { return Err(invalid("A step's text is too long")); }
        }
        Ok(())
    }

    /// Duplicate beside the original, retaining its prompt and configuration (Action Wizard).
    pub fn duplicate_step(&mut self, index: usize) -> Result<()> {
        if self.steps.len() >= 100 { return Err(invalid("An action supports up to 100 steps")); }
        let step = self.steps.get(index).cloned().ok_or_else(|| invalid("No such action step"))?;
        self.steps.insert(index + 1, step);
        Ok(())
    }

    pub fn import(text: &str) -> Result<Self> {
        if text.len() > 10 * 1024 * 1024 { return Err(invalid("An action file must be smaller than 10 MB")); }
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| invalid(format!("Invalid action JSON: {e}")))?;
        if value.get("steps").is_none() { return crate::action_import::stirling(value); }
        let action: Self = serde_json::from_value(value).map_err(|e| invalid(format!("Invalid action JSON: {e}")))?;
        action.validate()?;
        Ok(action)
    }

    pub fn stirling_json(&self) -> Result<String> { crate::action_import::export(self) }

    pub fn json(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| invalid(e.to_string()))
    }
}

/// Offline suggested actions: copying creates an editable, independent action.
pub fn suggested() -> Vec<SavedAction> {
    actions::builtin().into_iter().map(|a| SavedAction {
        version: 1, name: a.name, description: a.description, custom_tool: false, output: OutputOptions::default(),
        steps: a.steps.iter().map(ActionStep::from_engine).collect(),
    }).collect()
}

pub fn load(dir: &Path) -> Result<Vec<SavedAction>> {
    let path = dir.join("actions.json");
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    let all: Vec<SavedAction> = serde_json::from_slice(&bytes).map_err(|e| invalid(format!("Could not read saved actions: {e}")))?;
    for action in &all { action.validate()?; }
    Ok(all)
}

pub fn save(dir: &Path, all: &[SavedAction]) -> Result<()> {
    // Do not replace an unreadable store with the empty list shown after a load failure.
    load(dir)?;
    for action in all { action.validate()?; }
    let bytes = serde_json::to_vec_pretty(all).map_err(|e| invalid(e.to_string()))?;
    crate::store_file::replace(&dir.join("actions.json"), &bytes)?;
    Ok(())
}

/// Recover partial drafts, including a new action with no steps yet.
pub fn load_draft(dir: &Path) -> Result<Option<SavedAction>> {
    match std::fs::read(dir.join("action-draft.json")) {
        Ok(bytes) if bytes.len() <= 10 * 1024 * 1024 => serde_json::from_slice(&bytes).map(Some).map_err(|e| invalid(e.to_string())),
        Ok(_) => Err(invalid("The saved draft exceeds 10 MB")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None), Err(e) => Err(e.into()),
    }
}

pub fn save_draft(dir: &Path, draft: Option<&SavedAction>) -> Result<()> {
    let path = dir.join("action-draft.json");
    if let Some(draft) = draft {
        let bytes = serde_json::to_vec(draft).map_err(|e| invalid(e.to_string()))?;
        crate::store_file::replace(&path, &bytes)?;
    } else {
        match std::fs::remove_file(path) { Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e.into()) }
    }
    Ok(())
}

/// Give a copied/imported action a name that leaves the existing ones alone.
pub fn unused_name(name: &str, all: &[SavedAction]) -> String {
    let taken = |name: &str| all.iter().any(|a| a.name.to_lowercase() == name.to_lowercase());
    if !taken(name) { return name.into(); }
    (2..).map(|n| {
        let suffix = format!(" ({n})");
        let mut end = name.len().min(256 - suffix.len());
        while !name.is_char_boundary(end) { end -= 1; }
        format!("{}{suffix}", &name[..end])
    }).find(|n| !taken(n)).unwrap()
}

/// Run steps sequentially, pausing before each prompted step. No output is published until
/// every step succeeds; a failure leaves the input untouched. `before` returns false to cancel.
pub fn run_file(action: &SavedAction, input: &Path, out_dir: &Path, before: &mut dyn FnMut(usize, &ActionStep) -> bool) -> Result<PathBuf> {
    run_file_configured(action, input, out_dir, None, &mut |i, step| before(i, step), &mut || true)
}

/// Password and prompted edits apply only to this run, leaving the saved template untouched.
pub fn run_file_configured(
    action: &SavedAction, input: &Path, out_dir: &Path, password: Option<&str>,
    before: &mut dyn FnMut(usize, &mut ActionStep) -> bool,
    publish: &mut dyn FnMut() -> bool,
) -> Result<PathBuf> {
    action.validate()?;
    let bytes = Arc::new(std::fs::read(input)?);
    let name = input.file_name().ok_or_else(|| invalid("The input needs a file name"))?.to_string_lossy().into_owned();
    let engine = Action { name: action.name.clone(), description: action.description.clone(),
        steps: action.steps.iter().map(ActionStep::engine).collect::<Result<Vec<_>>>()?, builtin: false };
    let mut cancelled = false;
    let result = actions::run_guided(&engine, &name, bytes, password, |i, _| {
        let mut step = action.steps[i].clone();
        if !before(i, &mut step) { cancelled = true; return None; }
        // The caller can edit the argument, never replace a tool with an unvalidated command.
        if step.argument.len() > 1_000_000 { return None; }
        step.engine().ok()
    });
    if cancelled { return Err(OpdfError::Cancelled); }
    let bytes = result.map_err(OpdfError::Engine)?.bytes;
    if !publish() { return Err(OpdfError::Cancelled); }
    let target = action.output.path(input, out_dir)?;
    std::fs::create_dir_all(target.parent().unwrap_or(Path::new(".")))?;
    if action.output.overwrite {
        // NamedTempFile uses an exclusive temporary name and atomic replacement on each OS.
        use std::io::Write;
        let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap_or(Path::new(".")))?;
        temp.write_all(bytes.as_slice())?;
        temp.as_file().sync_all()?;
        if let Ok(meta) = std::fs::metadata(&target) { temp.as_file().set_permissions(meta.permissions())?; }
        temp.persist(&target).map_err(|e| e.error)?;
        return Ok(target);
    }
    sheaf_core::paths::atomic_write(&target, |part| {
        std::fs::write(part, bytes.as_slice())?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_round_trip_keeps_order_arguments_instructions_and_prompts() {
        let mut a = suggested().remove(3);
        a.steps[0].instruction = "Check the watermark text before continuing".into();
        a.steps[0].prompt = true;
        a.steps.push(ActionStep::from_engine(&Step::SetTitle("Review copy".into())));
        assert_eq!(SavedAction::import(&a.json().unwrap()).unwrap(), a);
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), std::slice::from_ref(&a)).unwrap();
        assert_eq!(load(dir.path()).unwrap(), [a]);
    }

    #[test]
    fn import_rejects_unknown_tools_versions_and_empty_actions() {
        let mut a = suggested().remove(0);
        a.steps[0].tool = "unknown_tool".into();
        assert!(SavedAction::import(&serde_json::to_string(&a).unwrap()).is_err());
        a.steps.clear();
        assert!(a.validate().is_err());
        a = suggested().remove(0);
        a.version = 2;
        assert!(a.validate().is_err());
        assert!(SavedAction::import("{}").is_err());
    }

    #[test]
    fn suggested_actions_are_valid_offline_templates_and_survive_copy_edits() {
        let mut presets = suggested();
        assert_eq!(presets.len(), 9);
        for preset in &presets { preset.validate().unwrap(); }
        let original = presets[0].clone();
        let mut copy = presets.remove(0);
        copy.steps.clear();
        copy.steps.push(ActionStep::from_engine(&Step::SetTitle("My workflow".into())));
        assert_eq!(suggested()[0], original);
        assert_ne!(copy.steps, suggested()[0].steps);
    }

    #[test]
    fn suggested_copies_and_custom_tools_are_independent() {
        let original = suggested();
        let mut copy = original[0].clone();
        copy.name = unused_name(&copy.name, &original);
        copy.custom_tool = true;
        copy.steps[0].label = "Prepare recipient copy".into();
        copy.steps[0].separator = true;
        copy.steps.reverse();
        assert_ne!(copy.steps, original[0].steps);
        assert_eq!(copy.name, "Prepare for Distribution (2)");
        assert_eq!(SavedAction::import(&copy.json().unwrap()).unwrap(), copy);
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), std::slice::from_ref(&copy)).unwrap();
        let mut tools = load(dir.path()).unwrap();
        assert!(tools[0].custom_tool);
        tools[0].name = "My commands".into();
        tools[0].steps.remove(0);
        save(dir.path(), &tools).unwrap();
        assert_eq!(load(dir.path()).unwrap(), tools);
        save(dir.path(), &[]).unwrap();
        assert!(load(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn copied_names_remain_valid_at_the_unicode_name_limit() {
        let mut a = suggested().remove(0);
        a.name = "é".repeat(128);
        let name = unused_name(&a.name, std::slice::from_ref(&a));
        assert!(name.len() <= 256); assert!(name.ends_with(" (2)"));
        a.name = name;
        a.validate().unwrap();
        let second = unused_name(&a.name.to_uppercase(), std::slice::from_ref(&a));
        assert_ne!(second.to_lowercase(), a.name.to_lowercase());
    }

    #[test]
    fn duplicate_step_keeps_configuration_and_enforces_the_step_limit() {
        let mut a = suggested().remove(3);
        a.steps[0].prompt = true;
        a.steps[0].instruction = "Review text".into();
        a.duplicate_step(0).unwrap();
        assert_eq!(a.steps[0], a.steps[1]);
        a.steps[1].argument = "Copy".into();
        assert_eq!(a.steps[0].argument, "CONFIDENTIAL");
        assert!(a.duplicate_step(99).is_err());
        while a.steps.len() < 100 { a.duplicate_step(0).unwrap(); }
        assert!(a.duplicate_step(0).is_err());
        assert_eq!(a.steps.len(), 100);
    }

    #[test]
    fn unreadable_saved_actions_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("actions.json"), b"broken JSON").unwrap();
        assert!(save(dir.path(), &suggested()).is_err());
        assert_eq!(std::fs::read(dir.path().join("actions.json")).unwrap(), b"broken JSON");
    }

    #[test]
    fn saved_action_runs_in_order_and_never_replaces_originals_or_outputs() {
        use sheaf_core::{pages::Input, pdfcraft::{Doc, engine::Session}};
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.pdf");
        let bytes = Session::new().create_from_text("Original", "Keep this page").unwrap();
        std::fs::write(&input, bytes.as_slice()).unwrap();
        let mut a = SavedAction::empty();
        a.steps = vec![ActionStep::from_engine(&Step::SetTitle("First".into())), ActionStep::from_engine(&Step::SetTitle("Last".into()))];
        a.steps[1].prompt = true;
        let out = dir.path().join("outputs");
        let mut visited = Vec::new();
        let first = run_file(&a, &input, &out, &mut |i, step| { visited.push((i, step.prompt)); true }).unwrap();
        assert_eq!(visited, [(0, false), (1, true)]);
        let second = run_file(&a, &input, &out, &mut |_, _| true).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&input).unwrap(), *bytes);
        assert_eq!(Doc::open(&Input::new(first)).unwrap().document().info.title.as_deref(), Some("Last"));
    }

    #[test]
    fn prompted_settings_affect_this_run_without_changing_the_saved_action() {
        use sheaf_core::{pages::Input, pdfcraft::{Doc, engine::Session}};
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.pdf");
        let bytes = Session::new().create_from_text("Original", "Keep").unwrap();
        std::fs::write(&input, bytes.as_slice()).unwrap();
        let mut action = SavedAction::empty();
        action.steps = vec![ActionStep::from_engine(&Step::SetTitle("Template title".into()))];
        action.steps[0].prompt = true;
        let output = run_file_configured(&action, &input, dir.path(), None, &mut |_, step| {
            step.argument = "Title entered at the pause".into(); true
        }, &mut || true).unwrap();
        assert_eq!(Doc::open(&Input::new(output)).unwrap().document().info.title.as_deref(), Some("Title entered at the pause"));
        assert_eq!(action.steps[0].argument, "Template title");
        assert_eq!(std::fs::read(input).unwrap(), *bytes);
    }

    #[test]
    fn stopping_during_the_last_step_does_not_publish_its_output() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.pdf");
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Original", "Keep").unwrap();
        std::fs::write(&input, bytes.as_slice()).unwrap();
        let mut action = SavedAction::empty();
        action.steps.push(ActionStep::from_engine(&Step::SetTitle("Changed".into())));
        let output = dir.path().join("outputs");
        assert!(matches!(run_file_configured(&action, &input, &output, None, &mut |_, _| true, &mut || false), Err(OpdfError::Cancelled)));
        assert!(!output.exists());
        assert_eq!(std::fs::read(input).unwrap(), *bytes);
    }

    #[test]
    fn cancelled_action_does_not_publish_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.pdf");
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Original", "Keep").unwrap();
        std::fs::write(&input, bytes.as_slice()).unwrap();
        let out = dir.path().join("outputs");
        assert!(matches!(run_file(&suggested()[0], &input, &out, &mut |_, _| false), Err(OpdfError::Cancelled)));
        assert!(!out.exists());
        assert_eq!(std::fs::read(input).unwrap(), *bytes);
    }
    #[test]
    fn partial_action_drafts_recover_without_becoming_saved_actions() {
        let dir = tempfile::tempdir().unwrap();
        let mut draft = SavedAction::empty();
        draft.name = "Still writing".into();
        save_draft(dir.path(), Some(&draft)).unwrap();
        assert_eq!(load_draft(dir.path()).unwrap(), Some(draft));
        assert!(load(dir.path()).unwrap().is_empty());
        save_draft(dir.path(), None).unwrap();
        assert_eq!(load_draft(dir.path()).unwrap(), None);
    }

    #[test]
    fn output_options_keep_originals_unless_explicitly_overwritten_and_accept_folder_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.PDF");
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Original", "Keep").unwrap();
        std::fs::write(&input, bytes.as_slice()).unwrap();
        std::fs::write(dir.path().join("skip.txt"), b"skip").unwrap();
        assert_eq!(folder_inputs(dir.path()).unwrap(), std::slice::from_ref(&input));
        let mut a = suggested().remove(3);
        a.output = OutputOptions { same_folder: true, prefix: "review-".into(), suffix: "-done".into(), overwrite: false };
        let out = run_file(&a, &input, &dir.path().join("unused"), &mut |_, _| true).unwrap();
        assert_eq!(out, dir.path().join("review-input-done.pdf"));
        assert_eq!(std::fs::read(&input).unwrap(), *bytes);
        assert_ne!(run_file(&a, &input, dir.path(), &mut |_, _| true).unwrap(), out);
        a.output.overwrite = true;
        assert_eq!(run_file(&a, &input, dir.path(), &mut |_, _| true).unwrap(), out);
        assert_eq!(SavedAction::import(&a.json().unwrap()).unwrap(), a);
        a.output.prefix = "../".into();
        assert!(a.validate().is_err());
    }

    #[test]
    fn suggested_archive_form_and_sharing_actions_are_native_editable_copies() {
        let all = suggested();
        for (name, tool) in [("Archive Documents (PDF/A-2b)", "pdfa"), ("Archive Documents (PDF/A-3b)", "pdfa"), ("Prepare a Form", "detect_form_fields"), ("Optimize for Sharing", "compress")] {
            let template = all.iter().find(|a| a.name == name).unwrap();
            assert_eq!(template.steps[0].tool, tool);
            assert_eq!(SavedAction::import(&template.json().unwrap()).unwrap(), *template);
            let mut copy = template.clone();
            copy.steps[0].instruction = "Review output".into();
            copy.steps[0].prompt = true;
            assert_ne!(copy, *template);
            assert!(suggested().contains(template));
        }
        assert!(!all.iter().any(|a| a.name == "Make Accessible"));
    }

}
