//! Stirling's Automate and Folder Scanning files use the same operations with different names.
//! Translate only equivalent settings; refusing unsupported ones prevents a lossy import.

use serde_json::{Value, Map, json};
use sheaf_core::{OpdfError, Result, pdfcraft::engine::actions::Step};
use crate::actions::{SavedAction, ActionStep};

fn invalid(message: impl Into<String>) -> OpdfError { OpdfError::Unsupported(message.into()) }

fn parameters(value: &Value, allowed: &[&str]) -> Result<Map<String, Value>> {
    let p = match value.get("parameters") {
        None => Map::new(), Some(Value::Object(p)) => p.clone(), _ => return Err(invalid("Operation parameters must be a JSON object")),
    };
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
    let mut warnings = Vec::new();
    for (i, operation) in operations.iter().enumerate() {
        if let Some(format) = terminal_format(operation)? {
            if i + 1 != operations.len() { return Err(invalid(format!("Stirling step {}: format conversion must be last", i + 1))); }
            action.output.format = format.into();
            if action.steps.is_empty() { action.steps.push(ActionStep::from_engine(&Step::KeepPages("all".into()))); }
            warnings.push("Conversion uses Sheaf's native Rust writers; editable Office layout and bytes differ from LibreOffice. Image exports are 150 dpi page files inside one ZIP".into());
            continue;
        }
        let result = step(operation, &mut warnings).map_err(|e| invalid(format!("Stirling step {}: {e}", i + 1)))?;
        action.steps.extend(result.iter().map(ActionStep::from_engine));
    }
    if !warnings.is_empty() {
        action.description = format!("{}\nImport notes: {}", action.description, warnings.join("; "));
    }
    action.validate()?;
    Ok(action)
}

fn terminal_format(operation: &Value) -> Result<Option<&'static str>> {
    let id = operation.get("operation").and_then(Value::as_str).unwrap_or_default();
    let word = ["pdfToWord", "/api/v1/convert/pdf/word"].contains(&id);
    let text = ["pdfToText", "pdfToTextOrRTF", "/api/v1/convert/pdf/text"].contains(&id);
    if !word && !text { return Ok(None); }
    let p = parameters(operation, &["outputFormat"])?;
    let format = p.get("outputFormat").and_then(Value::as_str).ok_or_else(|| invalid("Conversion needs outputFormat"))?;
    match (word, format) {
        (true, "docx") => Ok(Some("docx")), (false, "txt") => Ok(Some("txt")), (false, "rtf") => Ok(Some("rtf")),
        _ => Err(invalid(format!("Sheaf cannot retain Stirling's {format} conversion; use docx, txt or rtf"))),
    }
}

fn step(operation: &Value, warnings: &mut Vec<String>) -> Result<Vec<Step>> {
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
        "compress" | "/api/v1/misc/compress-pdf" => {
            let p = parameters(operation, &["optimizeLevel", "grayscale", "lineArt", "linearize", "expectedOutputSize"])?;
            for key in ["grayscale", "lineArt", "linearize"] {
                if p.get(key).is_some_and(|v| v != &Value::Bool(false)) { return Err(invalid(format!("Native compression cannot retain {key}; use the compression tool manually"))); }
            }
            if p.get("expectedOutputSize").is_some_and(|v| !v.is_null() && v.as_str() != Some("")) { return Err(invalid("Target-size compression is not an action step yet")); }
            let level = p.get("optimizeLevel").and_then(Value::as_i64).ok_or_else(|| invalid("Compression needs optimizeLevel 1–9"))?;
            let native = match level { 1..=3 => "low", 4..=6 => "recommended", 7..=9 => "extreme", _ => return Err(invalid("Compression level must be 1–9")) };
            warnings.push(format!("Stirling compression {level} translated to Sheaf {native}; native Rust compression preserves content but image sizes and bytes differ from Ghostscript/qpdf"));
            Ok(vec![Step::Compress(native.into())])
        }
        "ocr" | "/api/v1/misc/ocr-pdf" => {
            let p = parameters(operation, &["languages", "ocrType", "ocrRenderType", "sidecar", "deskew", "clean", "cleanFinal", "removeImagesAfter"])?;
            for key in ["sidecar", "deskew", "clean", "cleanFinal", "removeImagesAfter"] {
                if p.get(key).is_some_and(|v| v != &Value::Bool(false)) { return Err(invalid(format!("Native OCR cannot retain {key}"))); }
            }
            if p.get("languages") != Some(&json!(["eng"])) || p.get("ocrType").and_then(Value::as_str) != Some("skip-text") {
                return Err(invalid("OCR actions currently support English skip-text only"));
            }
            if p.get("ocrRenderType").is_some_and(|v| !["hocr", "sandwich"].contains(&v.as_str().unwrap_or_default())) { return Err(invalid("Unsupported OCR rendering mode")); }
            warnings.push("OCR uses Sheaf's local recognition model at 300 dpi and adds a hidden text layer; recognition differs from Tesseract, and the local model must be installed".into());
            Ok(vec![Step::RecognizeText])
        }
        "addPageNumbers" | "/api/v1/misc/add-page-numbers" => {
            let p = parameters(operation, &["position", "fontSize", "startingNumber", "customText", "pagesToNumber", "zeroPad", "fontType", "customMargin"])?;
            if p.get("pagesToNumber").is_some_and(|v| v.as_str() != Some("")) || p.get("zeroPad").is_some_and(|v| v.as_u64() != Some(0)) {
                return Err(invalid("Numbering selected pages or zero-padding is not supported by action import"));
            }
            for key in ["position", "startingNumber", "zeroPad"] {
                if p.get(key).is_some_and(|v| v.as_u64().is_none()) { return Err(invalid(format!("{key} must be a nonnegative whole number"))); }
            }
            if p.get("fontSize").is_some_and(|v| v.as_f64().is_none()) || p.get("customText").is_some_and(|v| v.as_str().is_none()) { return Err(invalid("Font size must be a number and page-number text must be text")); }
            if p.get("fontType").is_some_and(|v| !["Times", "Helvetica", "Courier", "times", "helvetica", "courier"].contains(&v.as_str().unwrap_or_default())) || p.get("customMargin").is_some_and(|v| !["small", "medium", "large", "x-large"].contains(&v.as_str().unwrap_or_default())) { return Err(invalid("Unknown page-number font or margin")); }
            let position = match p.get("position").and_then(Value::as_u64).unwrap_or(8) { 1 => 0, 2 => 1, 3 => 2, 7 => 3, 8 => 4, 9 => 5, _ => return Err(invalid("Only header/footer page-number positions can be imported")) };
            let start = p.get("startingNumber").and_then(Value::as_u64).unwrap_or(1);
            let size = p.get("fontSize").and_then(Value::as_f64).unwrap_or(12.0);
            let format = p.get("customText").and_then(Value::as_str).unwrap_or_default();
            let format = if format.is_empty() { "<<1>>".into() } else { format.replace("{n}", "<<1>>").replace("{total}", "<<n>>") };
            if format.contains('{') || format.contains('}') { return Err(invalid("Unknown page-number token")); }
            let value = format!("{position}|{start}|{size}|{format}");
            let step = Step::from_id("page_numbers", &value).ok_or_else(|| invalid("Invalid page-number settings"))?;
            warnings.push("Page numbering keeps position, start, size and text; uses Sheaf's Helvetica font and 0.5/1 inch margins. Review the settings before running".into());
            Ok(vec![step])
        }
        "removePages" | "/api/v1/general/remove-pages" | "extractPages" | "/api/v1/general/extract-pages" => {
            let p = parameters(operation, &["pageNumbers"])?;
            let pages = p.get("pageNumbers").and_then(Value::as_str).ok_or_else(|| invalid("Page selection needs pageNumbers"))?;
            let tool = if id.contains("remove") || id == "removePages" { "delete_pages" } else { "keep_pages" };
            if tool == "keep_pages" {
                let mut last = 0;
                for part in pages.split(',') {
                    let (start, end) = part.trim().split_once('-').unwrap_or((part.trim(), part.trim()));
                    let start = start.trim().parse::<usize>().map_err(|_| invalid("Invalid extract page range"))?;
                    let end = end.trim().parse::<usize>().map_err(|_| invalid("Invalid extract page range"))?;
                    if start <= last || end < start { return Err(invalid("Reordered or duplicate extraction ranges cannot be imported yet")); }
                    last = end;
                }
            }
            Ok(vec![Step::from_id(tool, pages).ok_or_else(|| invalid("Use page ranges such as 1-3,5"))?])
        }
        "changeMetadata" | "/api/v1/misc/update-metadata" => {
            let p = parameters(operation, &["deleteAll", "title", "author", "subject", "keywords", "creator", "producer", "creationDate", "modificationDate", "trapped", "allRequestParams"])?;
            if p.get("deleteAll").is_some_and(|v| !v.is_boolean()) { return Err(invalid("deleteAll must be true or false")); }
            if p.get("allRequestParams").is_some_and(|v| !v.is_null() && v.as_object().is_none_or(|o| !o.is_empty())) { return Err(invalid("Custom metadata map entries are not imported yet")); }
            if p.get("deleteAll") == Some(&Value::Bool(true)) { return Ok(vec![Step::RemoveMetadata]); }
            let mut result = Vec::new();
            for (key, native) in [("title", "Title"), ("author", "Author"), ("subject", "Subject"), ("keywords", "Keywords"), ("creator", "Creator"), ("producer", "Producer"), ("creationDate", "CreationDate"), ("modificationDate", "ModDate"), ("trapped", "Trapped")] {
                let value = match p.get(key) { None | Some(Value::Null) => "", Some(Value::String(v)) => if v == "undefined" { "" } else { v }, _ => return Err(invalid(format!("{key} must be text"))) };
                let arg = format!("{native}|{value}");
                result.push(Step::from_id("set_metadata", &arg).ok_or_else(|| invalid(format!("Cannot retain Stirling {key}; date and trapped values can only be cleared")))?);
            }
            warnings.push("Metadata import replaces all standard fields, clearing omitted fields as Stirling does; unrelated custom fields stay intact".into());
            Ok(result)
        }
        _ => Err(invalid(format!("Unsupported Stirling operation: {id}. No steps have been saved."))),
    }
}

pub fn export(action: &SavedAction) -> Result<String> {
    action.validate()?;
    if action.output != crate::actions::OutputOptions::default() { return Err(invalid("Stirling cannot preserve Sheaf output naming and overwrite options. Use Sheaf JSON.")); }
    if action.custom_tool || action.steps.iter().any(|s| s.prompt || !s.instruction.is_empty() || !s.label.is_empty() || s.separator) {
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
        assert_eq!(stirling(make("changeMetadata", json!({"deleteAll":true}))).unwrap().steps[0].tool, "remove_metadata");
        assert_eq!(stirling(make("changeMetadata", json!({"title":"Replace standard metadata"}))).unwrap().steps.len(), 9);
        assert!(stirling(make("rotate", json!({"angle":45}))).is_err());
    }
    #[test]
    fn stirling_native_import_keeps_order_and_discloses_conversion_limits() {
        let a = stirling(json!({"name":"Prepare", "operations":[
            {"operation":"compress", "parameters":{"optimizeLevel":7,"grayscale":false,"lineArt":false,"linearize":false}},
            {"operation":"addPageNumbers", "parameters":{"position":9,"startingNumber":3,"fontSize":12,"customText":"Page {n} of {total}","pagesToNumber":"","zeroPad":0}},
            {"operation":"ocr", "parameters":{"languages":["eng"],"ocrType":"skip-text","ocrRenderType":"hocr","sidecar":false}}
        ]})).unwrap();
        assert_eq!(a.steps.iter().map(|s| s.tool.as_str()).collect::<Vec<_>>(), ["compress", "page_numbers", "recognize_text"]);
        assert_eq!(a.steps[0].argument, "extreme");
        assert_eq!(a.steps[1].argument, "5|3|12|Page <<1>> of <<n>>");
        assert!(a.description.contains("Import notes:"));
        assert!(a.description.contains("Ghostscript"));
        assert!(a.description.contains("Helvetica"));
        assert_eq!(SavedAction::import(&a.json().unwrap()).unwrap(), a);
        let unsupported = json!({"name":"No loss", "operations":[{"operation":"compress","parameters":{"optimizeLevel":5,"linearize":true}}]});
        assert!(stirling(unsupported).unwrap_err().to_string().contains("step 1"));
        for parameters in [json!({"startingNumber":-1}), json!({"fontSize":"large"}), json!({"customText":5}), json!("not an object")] {
            assert!(stirling(json!({"name":"Bad settings", "operations":[{"operation":"addPageNumbers", "parameters":parameters}]})).is_err());
        }
        let mut exact = SavedAction::empty();
        exact.steps.push(ActionStep::from_engine(&Step::RotateClockwise));
        exact.output.same_folder = true;
        assert!(export(&exact).is_err());
    }

    #[test]
    fn stirling_page_metadata_and_terminal_conversions_preserve_or_refuse_settings() {
        let action = stirling(json!({"name":"Publish", "operations":[
            {"operation":"removePages", "parameters":{"pageNumbers":"2-3"}},
            {"operation":"changeMetadata", "parameters":{"title":"Review","author":"Owner"}},
            {"operation":"pdfToWord", "parameters":{"outputFormat":"docx"}}
        ]})).unwrap();
        assert_eq!(action.steps[0].tool, "delete_pages"); assert_eq!(action.steps[0].argument, "2-3");
        assert_eq!(action.steps[1].argument, "Title|Review");
        assert_eq!(action.steps[3].argument, "Subject|"); assert_eq!(action.output.format, "docx");
        let convert = json!({"operation":"pdfToText", "parameters":{"outputFormat":"txt"}});
        assert!(stirling(json!({"name":"Wrong order", "operations":[convert.clone(), {"operation":"rotate", "parameters":{"angle":90}}]})).is_err());
        let only = stirling(json!({"name":"Text", "operations":[convert]})).unwrap();
        assert_eq!(only.output.format, "txt"); assert_eq!(only.steps[0].argument, "all");
        assert!(stirling(json!({"name":"Unsupported date", "operations":[{"operation":"changeMetadata","parameters":{"creationDate":"2024/01/01 12:00:00"}}]})).is_err());
        assert!(stirling(json!({"name":"No reordering", "operations":[{"operation":"extractPages","parameters":{"pageNumbers":"3,1"}}]})).is_err());
    }

}
