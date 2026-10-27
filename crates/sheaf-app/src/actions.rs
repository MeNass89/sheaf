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
}

impl ActionStep {
    pub fn from_engine(step: &Step) -> Self {
        Self { tool: step.id().into(), argument: step.arg().unwrap_or_default().into(), instruction: String::new(), prompt: false }
    }

    pub fn engine(&self) -> Result<Step> {
        Step::from_id(&self.tool, &self.argument).ok_or_else(|| invalid(format!("Unknown action tool: {}", self.tool)))
    }
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
}

fn invalid(message: impl Into<String>) -> OpdfError { OpdfError::Unsupported(message.into()) }

impl SavedAction {
    pub fn empty() -> Self {
        Self { version: 1, name: "Untitled action".into(), description: String::new(), custom_tool: false, steps: vec![] }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 { return Err(invalid("This action needs a different version of Sheaf")); }
        if self.name.trim().is_empty() || self.name.len() > 256 { return Err(invalid("Give the action a name (up to 256 bytes)")); }
        if self.steps.is_empty() || self.steps.len() > 100 { return Err(invalid("An action needs 1–100 steps")); }
        for step in &self.steps {
            step.engine()?;
            if step.argument.len() > 1_000_000 || step.instruction.len() > 16_384 { return Err(invalid("A step's text is too long")); }
        }
        Ok(())
    }

    pub fn import(text: &str) -> Result<Self> {
        if text.len() > 10 * 1024 * 1024 { return Err(invalid("An action file must be smaller than 10 MB")); }
        let action: Self = serde_json::from_str(text).map_err(|e| invalid(format!("Invalid action JSON: {e}")))?;
        action.validate()?;
        Ok(action)
    }

    pub fn json(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| invalid(e.to_string()))
    }
}

/// Offline suggested actions: copying creates an editable, independent action.
pub fn suggested() -> Vec<SavedAction> {
    actions::builtin().into_iter().map(|a| SavedAction {
        version: 1, name: a.name, description: a.description, custom_tool: false,
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

/// Give a copied/imported action a name that leaves the existing ones alone.
pub fn unused_name(name: &str, all: &[SavedAction]) -> String {
    let taken = |name: &str| all.iter().any(|a| a.name.eq_ignore_ascii_case(name));
    if !taken(name) { return name.into(); }
    (2..).map(|n| format!("{name} ({n})")).find(|n| !taken(n)).unwrap()
}

/// Run steps sequentially, pausing before each prompted step. No output is published until
/// every step succeeds; a failure leaves the input untouched. `before` returns false to cancel.
pub fn run_file(action: &SavedAction, input: &Path, out_dir: &Path, before: &mut dyn FnMut(usize, &ActionStep) -> bool) -> Result<PathBuf> {
    action.validate()?;
    let mut bytes = Arc::new(std::fs::read(input)?);
    let name = input.file_name().ok_or_else(|| invalid("The input needs a file name"))?.to_string_lossy().into_owned();
    for (index, step) in action.steps.iter().enumerate() {
        if !before(index, step) { return Err(OpdfError::Cancelled); }
        let one = Action { name: action.name.clone(), description: String::new(), steps: vec![step.engine()?], builtin: false };
        bytes = actions::run_on(&one, &name, bytes, |_, _| {}).map_err(OpdfError::Engine)?.bytes;
    }
    std::fs::create_dir_all(out_dir)?;
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    sheaf_core::paths::atomic_write(&out_dir.join(format!("{stem} - processed.pdf")), |part| {
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
    fn suggested_copies_and_custom_tools_are_independent() {
        let original = suggested();
        let mut copy = original[0].clone();
        copy.name = unused_name(&copy.name, &original);
        copy.custom_tool = true;
        copy.steps.reverse();
        assert_ne!(copy.steps, original[0].steps);
        assert_eq!(copy.name, "Prepare for Distribution (2)");
        assert_eq!(SavedAction::import(&copy.json().unwrap()).unwrap(), copy);
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
    fn cancelled_action_does_not_publish_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.pdf");
        std::fs::write(&input, b"not opened before the pause").unwrap();
        let out = dir.path().join("outputs");
        assert!(matches!(run_file(&suggested()[0], &input, &out, &mut |_, _| false), Err(OpdfError::Cancelled)));
        assert!(!out.exists());
        assert_eq!(std::fs::read(input).unwrap(), b"not opened before the pause");
    }
}
