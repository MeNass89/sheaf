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
        let secret = matches!(step, Step::Protect(_));
        Self { tool: step.id().into(), argument: if secret { String::new() } else { step.arg().unwrap_or_default().into() }, instruction: if secret { "Enter an open password for this output PDF. It is used only for this file and is never saved in the action.".into() } else { String::new() }, prompt: secret, label: String::new(), separator: false }
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
    pub format: String,
}

impl Default for OutputOptions {
    fn default() -> Self { Self { same_folder: false, prefix: String::new(), suffix: " - processed".into(), overwrite: false, format: "pdf".into() } }
}

impl OutputOptions {
    fn validate(&self) -> Result<()> {
        if !["pdf", "docx", "pptx", "html", "rtf", "txt", "md", "png.zip", "jpeg.zip"].contains(&self.format.as_str()) { return Err(invalid("Choose a supported output format")); }
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
        Ok(dir.join(format!("{name}.{}", self.format)))
    }
}

/// Folder inputs are bounded and never follow symbolic links, as Folder Scanning does.
pub fn folder_inputs(folder: &Path) -> Result<Vec<PathBuf>> {
    folder_inputs_recursive(folder, false)
}

/// Recursive intake is bounded by entries and depth, and never follows symbolic links.
pub fn folder_inputs_recursive(folder: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut pending = vec![(folder.to_path_buf(), 0)];
    let mut visited = 0;
    while let Some((folder, depth)) = pending.pop() {
        for entry in std::fs::read_dir(folder)? {
            let entry = entry?;
            visited += 1;
            if visited > 100_000 { return Err(invalid("Choose a folder with at most 100000 entries")); }
            let kind = entry.file_type()?;
            if kind.is_file() && entry.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) { paths.push(entry.path()); }
            if recursive && kind.is_dir() {
                if depth >= 64 { return Err(invalid("Folder nesting exceeds 64 levels")); }
                pending.push((entry.path(), depth + 1));
            }
        }
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

pub(crate) const MAX_ACTION_BYTES: usize = 10 * 1024 * 1024;

/// Stirling's shareable pipeline file must be accepted again by its importer. Measure the actual
/// escaped, pretty JSON without allocating another potentially enormous action string.
fn portable_size(action: &SavedAction) -> Result<()> {
    struct Limit(usize);
    impl std::io::Write for Limit {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.checked_sub(bytes.len()).ok_or_else(|| std::io::Error::other("Action JSON exceeds 10 MB"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    serde_json::to_writer_pretty(Limit(MAX_ACTION_BYTES), action)
        .map_err(|_| invalid("Action JSON exceeds 10 MB. Shorten the descriptions, instructions or step text before saving or exporting"))
}

impl SavedAction {
    pub fn empty() -> Self {
        Self { version: 1, name: "Untitled action".into(), description: String::new(), custom_tool: false, output: OutputOptions::default(), steps: vec![] }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 { return Err(invalid("This action needs a different version of Sheaf")); }
        if self.name.trim().is_empty() || self.name.len() > 256 { return Err(invalid("Give the action a name (up to 256 bytes)")); }
        if self.steps.is_empty() || self.steps.len() > 100 { return Err(invalid("An action needs 1–100 steps")); }
        self.output.validate()?;
        for (index, step) in self.steps.iter().enumerate() {
            step.engine()?;
            if step.tool == "protect" && (!step.argument.is_empty() || !step.prompt || index + 1 != self.steps.len() || self.output.format != "pdf") { return Err(invalid("Protection must be the final step of PDF output and ask for a password at runtime; saved actions cannot contain passwords")); }
            if step.tool == "open_tool" && (!self.custom_tool || sheaf_core::tools::ToolId::from_str(&step.argument).is_none() || step.prompt) { return Err(invalid("Toolbox shortcuts belong in custom tool sets and need an existing tool without an action prompt")); }
            if step.label.len() > 256 || step.argument.len() > 1_000_000 || step.instruction.len() > 16_384 { return Err(invalid("A step's text is too long")); }
        }
        portable_size(self)
    }

    /// Duplicate beside the original, retaining its prompt and configuration (Action Wizard).
    pub fn duplicate_step(&mut self, index: usize) -> Result<()> {
        if self.steps.len() >= 100 { return Err(invalid("An action supports up to 100 steps")); }
        let step = self.steps.get(index).cloned().ok_or_else(|| invalid("No such action step"))?;
        self.steps.insert(index + 1, step);
        Ok(())
    }

    pub fn import(text: &str) -> Result<Self> {
        if text.len() > MAX_ACTION_BYTES { return Err(invalid("An action file must be smaller than 10 MB")); }
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
        steps: a.steps.iter().map(|s| { let mut step = ActionStep::from_engine(s); if matches!(s, Step::PrepareAccessibility(_)) { step.prompt = true; step.instruction = "Enter the actual document language (for example fr-FR). Only simple untagged text-only PDFs are supported. Review headings, reading order and accessibility in the output before distributing.".into(); } step }).collect(),
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
        Ok(bytes) if bytes.len() <= MAX_ACTION_BYTES => serde_json::from_slice(&bytes).map(Some).map_err(|e| invalid(e.to_string())),
        Ok(_) => Err(invalid("The saved draft exceeds 10 MB")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None), Err(e) => Err(e.into()),
    }
}

pub fn save_draft(dir: &Path, draft: Option<&SavedAction>) -> Result<()> {
    let path = dir.join("action-draft.json");
    if let Some(draft) = draft {
        let mut draft = draft.clone();
        for step in &mut draft.steps { if step.tool == "protect" { step.argument.clear(); step.prompt = true; } }
        let bytes = serde_json::to_vec(&draft).map_err(|e| invalid(e.to_string()))?;
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
    let bytes = Arc::new(std::fs::read(input)?);
    run_bytes_configured(action, input, bytes, out_dir, password, before, publish)
}

/// A reader's immutable working state, including applied edits not yet saved to its disk file.
pub struct ActionInput { pub path: PathBuf, pub bytes: Arc<Vec<u8>>, pub password: Option<String> }

/// Run from an open document snapshot instead of reopening its older disk file.
pub fn run_bytes_configured(
    action: &SavedAction, input: &Path, bytes: Arc<Vec<u8>>, out_dir: &Path, password: Option<&str>,
    before: &mut dyn FnMut(usize, &mut ActionStep) -> bool,
    publish: &mut dyn FnMut() -> bool,
) -> Result<PathBuf> {
    action.validate()?;
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
    let bytes = convert_output(bytes, password, &action.output.format, publish)?;
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

/// Save to other formats in Rust, after the entire action succeeds. Image sets are one ZIP so
/// cancellation or collisions cannot leave a half-published group of page files.
pub(crate) fn convert_output(bytes: Arc<Vec<u8>>, password: Option<&str>, format: &str, proceed: &mut dyn FnMut() -> bool) -> Result<Arc<Vec<u8>>> {
    use sheaf_core::pdfcraft::engine::{Session, compare::OfficeFormat, export::{Exporter, ImageFormat}};
    if !proceed() { return Err(OpdfError::Cancelled); }
    if format == "pdf" { return Ok(bytes); }
    let mut session = Session::new();
    let id = session.open("Action output.pdf", None, bytes, password).map_err(|e| invalid(e.to_string()))?;
    let doc = session.get(id).ok_or_else(|| invalid("The action output is unavailable"))?;
    if !doc.allows_copying() { return Err(invalid("This PDF does not allow copying or export")); }
    let office = match format {
        "docx" => Some(OfficeFormat::Docx), "pptx" => Some(OfficeFormat::Pptx),
        "html" => Some(OfficeFormat::Html), "rtf" => Some(OfficeFormat::Rtf),
        "txt" => Some(OfficeFormat::Text), "md" => Some(OfficeFormat::Markdown),
        "png.zip" | "jpeg.zip" => None,
        _ => return Err(invalid("Unsupported action output format")),
    };
    if let Some(office) = office { return Ok(Arc::new(doc.export_office(office))); }
    let mut exporter = Exporter::new(doc);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for page in 0..doc.info.pages.len() {
        if !proceed() { return Err(OpdfError::Cancelled); }
        let png = format == "png.zip";
        let data = exporter.image(page, 150.0, if png { ImageFormat::Png } else { ImageFormat::Jpeg { quality: 85 } }).map_err(invalid)?;
        zip.start_file(format!("page-{:04}.{}", page + 1, if png { "png" } else { "jpg" }), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
            .map_err(|e| invalid(e.to_string()))?;
        use std::io::Write;
        zip.write_all(&data)?;
    }
    Ok(Arc::new(zip.finish().map_err(|e| invalid(e.to_string()))?.into_inner()))
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
        assert_eq!(presets.len(), 10);
        let accessibility = presets.iter().find(|a| a.name == "Prepare Accessibility (text-only)").unwrap();
        assert!(accessibility.steps[0].prompt);
        assert!(Step::from_id("prepare_accessibility", "fr--FR").is_none());
        assert!(Step::from_id("open_tool", "bad tool").is_none());
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
        a.output = OutputOptions { same_folder: true, prefix: "review-".into(), suffix: "-done".into(), overwrite: false, ..Default::default() };
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

    #[test]
    fn action_save_to_formats_use_native_writers_and_recursive_inputs_skip_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested"); std::fs::create_dir(&nested).unwrap();
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Source", "Original body").unwrap();
        let input = nested.join("input.pdf"); std::fs::write(&input, bytes.as_slice()).unwrap();
        assert!(folder_inputs(dir.path()).is_err());
        assert_eq!(folder_inputs_recursive(dir.path(), true).unwrap(), std::slice::from_ref(&input));
        #[cfg(unix)] {
            std::os::unix::fs::symlink(dir.path(), nested.join("loop")).unwrap();
            assert_eq!(folder_inputs_recursive(dir.path(), true).unwrap(), std::slice::from_ref(&input));
        }
        let mut action = suggested().remove(3);
        for format in ["txt", "docx", "html", "png.zip", "jpeg.zip"] {
            action.output.format = format.into();
            let output = run_file(&action, &input, dir.path(), &mut |_, _| true).unwrap();
            assert!(output.to_string_lossy().ends_with(format));
            let data = std::fs::read(output).unwrap();
            if format == "txt" || format == "html" { assert!(String::from_utf8(data).unwrap().contains("Original body")); }
            else { assert!(data.starts_with(b"PK")); }
        }
        action.output.format = "unsupported".into(); assert!(action.validate().is_err());
    }

    #[test]
    fn custom_toolbox_shortcuts_validate_real_tools_and_never_run_as_batch_edits() {
        let mut set = SavedAction::empty(); set.custom_tool = true;
        for tool in sheaf_core::tools::catalogue() {
            set.steps = vec![ActionStep::from_engine(&Step::OpenTool(tool.id.as_str().into()))];
            set.validate().unwrap();
            assert_eq!(SavedAction::import(&set.json().unwrap()).unwrap(), set);
            assert!(actions::command_edit(&set.steps[0].engine().unwrap(), 1).is_err());
        }
        set.steps[0].argument = "unknown".into(); assert!(set.validate().is_err());
        set.steps[0].argument = "redact".into(); set.custom_tool = false; assert!(set.validate().is_err());
    }

    #[test]
    fn native_action_exports_stop_before_rendering_cancelled_pages() {
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Title", "Body").unwrap();
        assert!(matches!(convert_output(bytes, None, "png.zip", &mut || false), Err(OpdfError::Cancelled)));
    }

    #[test]
    fn actions_on_open_documents_use_unsaved_working_bytes_and_keep_disk_originals() {
        let dir = tempfile::tempdir().unwrap();
        let session = sheaf_core::pdfcraft::engine::Session::new();
        let disk = session.create_from_text("Saved", "Saved body").unwrap();
        let working = session.create_from_text("Working", "Unsaved body").unwrap();
        let input = dir.path().join("input.pdf"); std::fs::write(&input, disk.as_slice()).unwrap();
        let action = suggested().remove(3);
        let output = run_bytes_configured(&action, &input, working, dir.path(), None, &mut |_, _| true, &mut || true).unwrap();
        let doc = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(output)).unwrap();
        let text = doc.document().export_office(sheaf_core::pdfcraft::engine::compare::OfficeFormat::Text);
        assert!(String::from_utf8(text).unwrap().contains("Unsaved body"));
        assert_eq!(std::fs::read(input).unwrap(), *disk);
    }

    #[test]
    fn password_actions_prompt_per_file_and_never_persist_passwords() {
        let mut action = SavedAction::empty();
        action.steps = vec![ActionStep::from_engine(&Step::Protect("must not persist".into()))];
        assert!(action.steps[0].prompt); assert!(action.steps[0].argument.is_empty());
        assert!(!action.json().unwrap().contains("must not persist"));
        action.steps[0].argument = "secret".into(); assert!(action.json().is_err());
        let dir = tempfile::tempdir().unwrap(); save_draft(dir.path(), Some(&action)).unwrap();
        assert!(!std::fs::read_to_string(dir.path().join("action-draft.json")).unwrap().contains("secret"));
        assert!(load_draft(dir.path()).unwrap().unwrap().steps[0].argument.is_empty());
        action.steps[0].argument.clear(); action.steps[0].prompt = false; assert!(action.validate().is_err());
        action.steps[0].prompt = true; action.output.format = "docx".into(); assert!(action.validate().is_err());
    }

    #[test]
    fn every_exported_action_fits_the_import_limit_including_json_escaping() {
        let mut action = SavedAction::empty();
        action.steps.push(ActionStep::from_engine(&Step::RotateClockwise));
        // Escaping doubles these bytes: measuring only the raw fields would accept a file
        // larger than the importer's limit, including for an action with just one step.
        action.description = "\n".repeat(MAX_ACTION_BYTES / 2);
        assert!(action.validate().unwrap_err().to_string().contains("10 MB"));
        assert!(action.json().is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(save(dir.path(), std::slice::from_ref(&action)).is_err());
        assert!(!dir.path().join("actions.json").exists());
        action.description.truncate(MAX_ACTION_BYTES / 2 - 4096);
        let portable = action.json().unwrap();
        assert!(portable.len() <= MAX_ACTION_BYTES);
        assert_eq!(SavedAction::import(&portable).unwrap(), action);
        let stirling = action.stirling_json().unwrap();
        assert!(stirling.len() <= MAX_ACTION_BYTES);
        assert_eq!(SavedAction::import(&stirling).unwrap(), action);
    }

}
