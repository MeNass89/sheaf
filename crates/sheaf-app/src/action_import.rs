//! Stirling's Automate and Folder Scanning files use the same operations with different names.
//! Translate only equivalent settings; refusing unsupported ones prevents a lossy import.

use serde_json::{Value, Map, json};
use sheaf_core::{OpdfError, Result, pdfcraft::engine::actions::Step};
use crate::actions::{SavedAction, ActionStep};

fn invalid(message: impl Into<String>) -> OpdfError { OpdfError::Unsupported(message.into()) }

fn parameters(value: &Value, allowed: &[&str]) -> Result<Map<String, Value>> {
    let p = value.get("parameters").and_then(Value::as_object).cloned().unwrap_or_default();
    for key in p.keys() {
        if key == "fileInput" && p[key] == "automated" { continue; }
        if !allowed.contains(&key.as_str()) { return Err(invalid(format!("Unsupported Stirling setting: {key}"))); }
    }
    Ok(p)
}

pub fn stirling(value: Value) -> Result<SavedAction> {
    let mut action = SavedAction::empty();
    action.name = value.get("name").and_then(Value::as_str).ok_or_else(|| invalid("The automation needs a name"))?.into();
    action.description = value.get("description").and_then(Value::as_str).unwrap_or_default().into();
    let operations = value.get("operations").or_else(|| value.get("pipeline")).and_then(Value::as_array)
        .ok_or_else(|| invalid("Expected Sheaf steps, Stirling operations or a Folder Scanning pipeline"))?;
    if operations.len() > 100 { return Err(invalid("An action supports up to 100 steps")); }
    for (i, operation) in operations.iter().enumerate() {
        let result = step(operation).map_err(|e| invalid(format!("Stirling step {}: {e}", i + 1)))?;
        action.steps.extend(result.iter().map(ActionStep::from_engine));
    }
    action.validate()?;
    Ok(action)
}

fn step(operation: &Value) -> Result<Vec<Step>> {
    let id = operation.get("operation").and_then(Value::as_str).ok_or_else(|| invalid("Missing operation name"))?;
    match id {
        "rotate" | "/api/v1/general/rotate-pdf" => {
            let p = parameters(operation, &["angle"])?;
            let angle = p.get("angle").and_then(Value::as_i64).ok_or_else(|| invalid("Rotation needs an angle in whole degrees"))?;
            let step = match angle.rem_euclid(360) {
                0 => return Ok(vec![]), 90 => Step::RotateClockwise, 180 => Step::RotateHalfTurn, 270 => Step::RotateCounterclockwise,
                _ => return Err(invalid("Rotation must be a multiple of 90 degrees")),
            };
            Ok(vec![step])
        }
        "flatten" | "/api/v1/misc/flatten" => {
            let p = parameters(operation, &["flattenOnlyForms"])?;
            if p.get("flattenOnlyForms").and_then(Value::as_bool) != Some(true) {
                return Err(invalid("Only form flattening is equivalent; Stirling's whole-page rasterization is not imported"));
            }
            Ok(vec![Step::FlattenFields])
        }
        "changeMetadata" | "/api/v1/misc/update-metadata" => Err(invalid(
            "Stirling replaces all standard metadata, including omitted fields. Use a Sheaf metadata action to choose what to keep."
        )),
        _ => Err(invalid(format!("Unsupported Stirling operation: {id}. No steps have been saved."))),
    }
}

pub fn export(action: &SavedAction) -> Result<String> {
    action.validate()?;
    if action.custom_tool || action.steps.iter().any(|s| s.prompt || !s.instruction.is_empty()) {
        return Err(invalid("Stirling cannot preserve custom tools, prompts or instructions. Use Sheaf JSON."));
    }
    let operations = action.steps.iter().map(|s| {
        let (operation, parameters) = match s.engine()? {
            Step::RotateClockwise => ("rotate", json!({"angle":90})),
            Step::RotateCounterclockwise => ("rotate", json!({"angle":270})),
            Step::RotateHalfTurn => ("rotate", json!({"angle":180})),
            Step::FlattenFields => ("flatten", json!({"flattenOnlyForms":true})),
            _ => return Err(invalid(format!("{} has no lossless Stirling export. Use Sheaf JSON.", s.engine()?.label()))),
        };
        Ok(json!({"operation":operation, "parameters":parameters}))
    }).collect::<Result<Vec<_>>>()?;
    serde_json::to_string_pretty(&json!({"name":action.name,"description":action.description,"operations":operations}))
        .map_err(|e| invalid(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stirling_export_import_preserves_equivalent_settings_and_order() {
        let mut a = SavedAction::empty();
        a.name = "Prepare forms".into();
        a.steps = [Step::RotateClockwise, Step::RotateCounterclockwise, Step::FlattenFields].iter().map(ActionStep::from_engine).collect();
        let json = export(&a).unwrap();
        assert_eq!(SavedAction::import(&json).unwrap(), a);
        // Folder Scanning is the same canonical export with endpoint names and automated input.
        let mut value: Value = serde_json::from_str(&json).unwrap();
        let mut operations = value.as_object_mut().unwrap().remove("operations").unwrap();
        for op in operations.as_array_mut().unwrap() {
            op["operation"] = match op["operation"].as_str().unwrap() {
                "rotate" => "/api/v1/general/rotate-pdf", "flatten" => "/api/v1/misc/flatten", _ => "/api/v1/misc/update-metadata",
            }.into();
            op["parameters"]["fileInput"] = "automated".into();
        }
        value["pipeline"] = operations;
        assert_eq!(stirling(value).unwrap(), a);
    }

    #[test]
    fn stirling_import_refuses_unknown_tools_settings_and_rasterization() {
        let make = |operation: &str, parameters: Value| json!({"name":"Unsupported", "operations":[{"operation":operation, "parameters":parameters}]});
        assert!(stirling(make("merge", json!({}))).is_err());
        assert!(stirling(make("rotate", json!({"angle":90,"pages":"1"}))).is_err());
        assert!(stirling(make("flatten", json!({"flattenOnlyForms":false}))).is_err());
        assert!(stirling(make("changeMetadata", json!({"deleteAll":true}))).is_err());
        assert!(stirling(make("changeMetadata", json!({"title":"Keep other metadata"}))).is_err());
        assert!(stirling(make("rotate", json!({"angle":45}))).is_err());
    }
}
