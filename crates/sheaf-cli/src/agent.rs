//! `--json` output, for agents and scripts.
//!
//! Success: `{"ok": true, "outputs": [paths written], "notes": [summaries and warnings],
//! "data": …}` on stdout, where `data` is the result of `info`, `text` (an array of page texts)
//! or `tools`. Failure: `{"ok": false, "error": message, "code": kind, "notes": […]}`, with
//! the usual exit status (1, or 2 for a password). `code` is one of `password_required`,
//! `wrong_password`, `invalid_range`, `office_missing`, `unsupported`, `usage`, `io`, `engine`.

use serde_json::{json, Value};
use sheaf_core::pages::Input;
use sheaf_core::OpdfError;
use std::path::PathBuf;
use std::sync::Mutex;

struct State {
    json: bool,
    notes: Vec<String>,
    data: Option<Value>,
    text: Option<Vec<String>>,
}

static STATE: Mutex<State> = Mutex::new(State { json: false, notes: Vec::new(), data: None, text: None });

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_json(on: bool) {
    state().json = on;
}

/// A line for the user: printed on stderr now, or kept for the JSON result.
pub fn note(line: String) {
    let mut s = state();
    if s.json { s.notes.push(line) } else { eprintln!("{line}") }
}

pub fn set_data(v: Value) {
    state().data = Some(v);
}

pub fn set_text(pages: Vec<String>) {
    state().text = Some(pages);
}

/// Text mode: `info` and `tools` print their JSON, `text` the pages separated by form feeds.
pub fn print_plain() {
    let s = state();
    if let Some(d) = &s.data {
        println!("{}", serde_json::to_string_pretty(d).unwrap_or_default());
    }
    if let Some(t) = &s.text {
        println!("{}", t.join("\u{c}"));
    }
}

pub fn print_ok(outputs: &[PathBuf], not_conforming: bool) {
    let s = state();
    let mut v = json!({
        "ok": !not_conforming,
        "outputs": outputs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "notes": s.notes,
    });
    if let Some(d) = &s.data {
        v["data"] = d.clone();
    }
    if let Some(t) = &s.text {
        v["data"] = json!(t);
    }
    if not_conforming {
        v["code"] = json!("not_conforming");
        v["error"] = json!("the output was written but does not fully conform to the PDF/A level (see notes)");
    }
    println!("{v}");
}

pub fn print_error(message: &str, code: &str) {
    let s = state();
    println!("{}", json!({ "ok": false, "error": message.trim(), "code": code, "notes": s.notes }));
}

pub fn error_code(e: &OpdfError) -> &'static str {
    match e {
        OpdfError::PasswordRequired(_) => "password_required",
        OpdfError::WrongPassword(_) | OpdfError::CertificatePassword(_) => "wrong_password",
        OpdfError::InvalidRange { .. } | OpdfError::WouldBeEmpty { .. } => "invalid_range",
        OpdfError::OfficeMissing => "office_missing",
        OpdfError::Unsupported(_) => "unsupported",
        OpdfError::Io(_) => "io",
        _ => "engine",
    }
}

/// What `sheaf info` reports.
pub fn info(input: &Input) -> sheaf_core::Result<Value> {
    let doc = sheaf_core::pdfcraft::Doc::open(input)?;
    let i = &doc.document().info;
    let pages: Vec<Value> = i
        .pages
        .iter()
        .enumerate()
        .map(|(n, p)| json!({ "page": n + 1, "label": p.label, "width": p.width, "height": p.height, "rotation": p.rotation }))
        .collect();
    fn outline(items: &[sheaf_core::pdfcraft::render::OutlineItem], out: &mut Vec<Value>, depth: usize) {
        for it in items {
            out.push(json!({ "title": it.title, "page": it.page.map(|p| p + 1), "depth": depth }));
            outline(&it.children, out, depth + 1);
        }
    }
    let mut bookmarks = Vec::new();
    outline(&i.outline, &mut bookmarks, 0);
    Ok(json!({
        "path": input.path.display().to_string(),
        "page_count": i.pages.len(),
        "pages": pages,
        "encrypted": i.encrypted,
        "pdf_version": i.pdf_version,
        "file_size": i.file_size,
        "title": i.title,
        "author": i.author,
        "subject": i.subject,
        "keywords": i.keywords,
        "creator": i.creator,
        "producer": i.producer,
        "tagged": i.tagged,
        "has_javascript": i.has_javascript,
        "form_fields": i.fields.iter().map(|f| json!({ "name": f.name, "kind": format!("{:?}", f.kind).to_lowercase(), "value": f.value, "page": f.page.map(|p| p + 1) })).collect::<Vec<_>>(),
        "bookmarks": bookmarks,
        "annotations": i.annotations.len(),
        "attachments": i.attachments.iter().map(|a| json!({ "name": a.name, "size": a.size })).collect::<Vec<_>>(),
        "warnings": i.warnings,
    }))
}
