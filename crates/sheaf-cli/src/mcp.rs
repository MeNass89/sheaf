//! `sheaf mcp`: every subcommand as a Model Context Protocol tool, over JSON-RPC on stdio
//! (one message per line).
//!
//! The tools are generated from the clap definitions, so they never drift from the CLI: a
//! call becomes `sheaf <command> <args> --json` run as a child process, and the tool result is
//! that JSON. `render` also returns the page as an image, so an agent can look at a result.

use base64::Engine as _;
use clap::{Arg, ArgAction, CommandFactory};
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};

/// Commands that are not tools.
const SKIP: &[&str] = &["tools", "mcp", "help", "actions"];

const PROTOCOL: &str = "2025-06-18";

/// The JSON type of one argument's values.
fn value_schema(a: &Arg) -> Value {
    let pv: Vec<String> = a.get_possible_values().iter().filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect();
    if !pv.is_empty() {
        return json!({ "type": "string", "enum": pv });
    }
    let t = a.get_value_parser().type_id();
    let int = [std::any::TypeId::of::<u32>(), std::any::TypeId::of::<u8>(), std::any::TypeId::of::<i64>(), std::any::TypeId::of::<i32>()];
    if int.iter().any(|i| t == *i) {
        json!({ "type": "integer" })
    } else if t == std::any::TypeId::of::<f64>() {
        json!({ "type": "number" })
    } else {
        json!({ "type": "string" })
    }
}

fn arg_schema(a: &Arg) -> Value {
    let mut s = match a.get_action() {
        ArgAction::SetTrue => json!({ "type": "boolean" }),
        ArgAction::Append => json!({ "type": "array", "items": value_schema(a) }),
        _ if a.get_num_args().is_some_and(|n| n.max_values() > 1) => json!({ "type": "array", "items": value_schema(a) }),
        _ if a.get_value_delimiter().is_some() => json!({ "type": "array", "items": value_schema(a) }),
        _ => value_schema(a),
    };
    // Values the custom parsers in main.rs accept, which clap cannot list.
    match a.get_id().as_str() {
        "rotate" => s = json!({ "type": "integer", "enum": [0, 45, 90, 180, 270, -45] }),
        "transparency" => s = json!({ "type": "integer", "enum": [0, 25, 50, 75] }),
        "degrees" => s = json!({ "type": "integer", "enum": [90, 180, 270, -90, -180, -270] }),
        "width" => {
            let item = json!({ "type": "number", "minimum": 10, "maximum": 2000 });
            if s["type"] == "array" { s["items"] = item } else { s = item }
        }
        "font_size" => s = json!({ "type": ["number", "string"] }),
        "dpi" => s = json!({ "type": "integer", "minimum": 18, "maximum": 600 }),
        "quality" => s = json!({ "type": "integer", "minimum": 1, "maximum": 100 }),
        "page" | "start" | "from" | "to" | "every" => {
            let item = json!({ "type": "integer", "minimum": 1 });
            if s["type"] == "array" { s["items"] = item } else { s = item }
        }
        _ => {}
    }
    let mut help = a.get_help().map(|h| h.to_string()).unwrap_or_default();
    if a.get_id() == "out" {
        help = "Output path (a directory for split and to-jpg). An existing file is never overwritten: a free name such as \"out (1).pdf\" is used, and the path written is returned in outputs".into();
    }
    if a.get_value_parser().type_id() == std::any::TypeId::of::<(f64, f64)>() {
        help.push_str(" (\"X,Y\" in points, e.g. \"72,600\")");
    }
    if !help.is_empty() {
        s["description"] = json!(help);
    }
    if let Some(d) = a.get_default_values().first().filter(|_| !matches!(a.get_action(), ArgAction::SetTrue)) {
        let d = d.to_string_lossy();
        s["default"] = match s["type"].as_str() {
            Some("integer") => d.parse::<i64>().map(Value::from).unwrap_or(json!(d)),
            Some("number") => d.parse::<f64>().map(Value::from).unwrap_or(json!(d)),
            _ => json!(d),
        };
    }
    s
}

/// Every tool: name, description and input schema.
pub fn tools() -> Vec<Value> {
    let cli = crate::Cli::command();
    let password = cli.get_arguments().find(|a| a.get_id() == "password").map(arg_schema).unwrap_or(json!({ "type": "string" }));
    cli.get_subcommands()
        .filter(|c| !SKIP.contains(&c.get_name()))
        .map(|c| {
            let mut props = Map::new();
            let mut required = Vec::new();
            if c.get_name() == "edit" {
                props.insert("input".into(), json!({ "type": "string", "description": "The PDF to edit" }));
                props.insert("items".into(), edit_items_schema());
                required.extend(["input", "items", "out"].map(Value::from));
            }
            for a in c.get_arguments() {
                let id = a.get_id().as_str();
                if ["help", "version", "json", "password", "items"].contains(&id) || (c.get_name() == "edit" && id != "out") {
                    continue;
                }
                props.insert(id.into(), arg_schema(a));
                if a.is_required_set() && !required.contains(&json!(id)) {
                    required.push(json!(id));
                }
            }
            if c.get_name() != "from-images" && c.get_name() != "office" {
                props.insert("password".into(), password.clone());
            }
            let mut description = c.get_about().map(|s| s.to_string()).unwrap_or_default();
            for g in c.get_groups().filter(|g| g.is_required_set()) {
                let ids: Vec<String> = g.get_args().map(|a| a.to_string()).collect();
                description.push_str(&format!(". Give exactly one of: {}", ids.join(", ")));
            }
            if let Some(after) = c.get_after_help() {
                description.push_str(&format!(". {}", after.to_string().replace('\n', " ")));
            }
            json!({
                "name": tool_name(c.get_name()),
                "description": description,
                "inputSchema": { "type": "object", "properties": props, "required": required, "additionalProperties": false },
            })
        })
        .collect()
}

fn edit_items_schema() -> Value {
    json!({
        "type": "array",
        "minItems": 1,
        "description": "Things to add, each a text or an image",
        "items": {
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to add (\\n starts a new line)" },
                "image": { "type": "string", "description": "A PNG or JPEG image to add" },
                "page": { "type": "integer", "description": "Page (1-based, default 1)" },
                "at": { "type": "string", "description": "Top-left corner, \"X,Y\" in points from the top-left of the page as displayed" },
                "size": { "type": "number", "description": "Font size of a text in points (default 12)" },
                "color": { "type": "string", "description": "Colour of a text, #RRGGBB (default black)" },
                "width": { "type": "number", "description": "Width of an image in points (its height follows the image)" }
            },
            "required": ["at"],
            "additionalProperties": false
        }
    })
}

fn tool_name(command: &str) -> String {
    command.replace('-', "_")
}

/// One JSON value as a command-line value.
fn scalar(v: &Value) -> Result<String, String> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        _ => Err(format!("expected a string or a number, got {v}")),
    }
}

/// The `sheaf` arguments for a tool call (the command name first).
pub fn argv(name: &str, args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let cli = crate::Cli::command();
    let cmd = cli.get_subcommands().find(|c| tool_name(c.get_name()) == name && !SKIP.contains(&c.get_name())).ok_or_else(|| format!("unknown tool \"{name}\""))?;
    let mut out = vec![cmd.get_name().to_string()];
    let mut positional = Vec::new();
    if let Some(pw) = args.get("password") {
        out.extend(["--password".into(), scalar(pw)?]);
    }
    for (key, v) in args {
        if key == "password" || v.is_null() {
            continue;
        }
        if cmd.get_name() == "edit" && key == "items" {
            for it in v.as_array().ok_or("items must be an array")? {
                let it = it.as_object().ok_or("each item must be an object")?;
                for k in ["text", "image", "page", "at", "size", "color", "width"] {
                    if let Some(x) = it.get(k).filter(|x| !x.is_null()) {
                        out.push(format!("--{k}={}", scalar(x)?));
                    }
                }
                if let Some(k) = it.keys().find(|k| !["text", "image", "page", "at", "size", "color", "width"].contains(&k.as_str())) {
                    return Err(format!("unknown item field \"{k}\""));
                }
            }
            continue;
        }
        let a = cmd.get_arguments().find(|a| a.get_id() == key.as_str()).ok_or_else(|| format!("unknown argument \"{key}\" for {name}"))?;
        let values: Vec<String> = match v {
            Value::Array(xs) => xs.iter().map(scalar).collect::<Result<_, _>>()?,
            Value::Bool(b) if matches!(a.get_action(), ArgAction::SetTrue) => {
                if *b {
                    vec![]
                } else {
                    continue;
                }
            }
            v => vec![scalar(v)?],
        };
        if a.is_positional() {
            positional.push((a.get_index().unwrap_or(0), values));
            continue;
        }
        let flag = format!("--{}", a.get_long().unwrap_or(key));
        if matches!(a.get_action(), ArgAction::SetTrue) {
            out.push(flag);
        } else if a.get_value_delimiter().is_some() {
            out.push(format!("{flag}={}", values.join(",")));
        } else {
            for x in values {
                // `=` keeps values such as "-90" or "-45" from reading as flags.
                out.push(format!("{flag}={x}"));
            }
        }
    }
    positional.sort_by_key(|(i, _)| *i);
    if !positional.is_empty() {
        out.push("--".into());
        out.extend(positional.into_iter().flat_map(|(_, v)| v));
    }
    Ok(out)
}

/// Runs a tool call: `sheaf … --json` in a child process (a crash or a panic stays a tool error).
fn call(name: &str, args: &Map<String, Value>) -> Value {
    let fail = |msg: String| json!({ "content": [{ "type": "text", "text": json!({ "ok": false, "error": msg, "code": "usage" }).to_string() }], "isError": true });
    let argv = match argv(name, args) {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return fail(format!("cannot find the sheaf executable: {e}")),
    };
    let mut full = vec!["--json".to_string()];
    full.extend(argv);
    let out = match std::process::Command::new(exe).args(&full).stdin(std::process::Stdio::null()).output() {
        Ok(o) => o,
        Err(e) => return fail(format!("cannot run sheaf: {e}")),
    };
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let result: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        json!({ "ok": false, "error": format!("sheaf exited with {} and no JSON result: {}", out.status, String::from_utf8_lossy(&out.stderr).trim()), "code": "engine" })
    });
    let ok = result["ok"].as_bool() == Some(true);
    let mut content = vec![json!({ "type": "text", "text": result.to_string() })];
    if let Some(png) = result["outputs"].get(0).and_then(Value::as_str).filter(|_| ok && name == "render").and_then(|p| std::fs::read(p).ok()) {
        content.push(json!({ "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png" }));
    }
    json!({ "content": content, "structuredContent": result, "isError": !ok })
}

/// The common jobs, worded for an assistant, each naming the tools to use (MCP prompts).
fn prompts() -> Vec<Value> {
    let file = json!([{ "name": "file", "description": "Path of the PDF", "required": true }]);
    vec![
        json!({ "name": "summarize_pdf", "description": "Summarise a PDF from its text", "arguments": file }),
        json!({ "name": "redact_personal_data", "description": "Find and redact the personal data in a PDF", "arguments": file }),
        json!({ "name": "prepare_to_share", "description": "Make a PDF smaller, and protected if asked, before sending it", "arguments": file }),
    ]
}

fn prompt(name: &str, args: &Value) -> Result<Value, (i64, String)> {
    let file = args["file"].as_str().filter(|f| !f.trim().is_empty()).ok_or((-32602, "the prompt needs \"file\"".to_string()))?;
    let text = match name {
        "summarize_pdf" => format!("Read the text of {file} with the `text` tool, page by page if it is long, and look at its `info`. Then summarise it in a few short paragraphs: what it is for, its key points, and the figures, names and dates that matter."),
        "redact_personal_data" => format!("Find the personal data in {file} with the `text` tool: people's names, e-mail addresses, phone numbers, postal addresses, ID and account numbers. Remove them with the `redact` tool, which writes a new file. Then read the new file with `text` to check nothing is left, and report what was redacted and the path written."),
        "prepare_to_share" => format!("Make {file} ready to send: reduce its size with the `compress` tool, and if a password was asked for, protect the result with the `protect` tool. Report the size before and after, and the path of the file to send."),
        _ => return Err((-32602, format!("no prompt named {name}"))),
    };
    Ok(json!({ "messages": [{ "role": "user", "content": { "type": "text", "text": text } }] }))
}

/// A PDF's text or information as an MCP resource: `sheaf://text/<path>`, `sheaf://info/<path>`.
fn read_resource(uri: &str) -> Result<Value, (i64, String)> {
    let (tool, path) = uri
        .strip_prefix("sheaf://text/")
        .map(|p| ("text", p))
        .or_else(|| uri.strip_prefix("sheaf://info/").map(|p| ("info", p)))
        .ok_or((-32602, format!("unknown resource {uri} (sheaf://text/<path> or sheaf://info/<path>)")))?;
    let mut args = Map::new();
    args.insert("input".into(), json!(path));
    let r = call(tool, &args);
    if r["isError"] == true {
        return Err((-32602, r["content"][0]["text"].as_str().unwrap_or("cannot read it").to_string()));
    }
    Ok(json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": r["structuredContent"].to_string() }] }))
}

fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    let result = match msg["method"].as_str().unwrap_or("") {
        "initialize" => Ok(json!({
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
            "capabilities": { "tools": {}, "prompts": {}, "resources": {} },
            "serverInfo": { "name": "sheaf", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Sheaf edits PDF files on this computer. Every tool writes a new file and never overwrites one: read the path written from \"outputs\". Use info and text to inspect a PDF, and render to look at a page. A protected PDF needs \"password\".",
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "prompts/list" => Ok(json!({ "prompts": prompts() })),
        "prompts/get" => prompt(params["name"].as_str().unwrap_or(""), &params["arguments"]),
        "resources/list" => Ok(json!({ "resources": [] })),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": [
            { "uriTemplate": "sheaf://text/{path}", "name": "PDF text", "description": "The text of a PDF, page by page", "mimeType": "application/json" },
            { "uriTemplate": "sheaf://info/{path}", "name": "PDF information", "description": "Pages, size, metadata, protection", "mimeType": "application/json" },
        ] })),
        "resources/read" => read_resource(params["uri"].as_str().unwrap_or("")),
        "tools/call" => match params["name"].as_str() {
            Some(name) => Ok(call(name, params["arguments"].as_object().unwrap_or(&Map::new()))),
            None => Err((-32602, "tools/call needs a name".to_string())),
        },
        m => Err((-32601, format!("method not found: {m}"))),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
}

/// Reads requests from stdin until it closes, answering each on stdout.
pub fn serve() -> sheaf_core::Result<()> {
    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("parse error: {e}") } })),
        };
        if let Some(r) = reply {
            let mut out = stdout.lock();
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// A sample value for one property of a schema.
    fn sample(name: &str, s: &Value) -> Value {
        if let Some(e) = s["enum"].as_array() {
            return e[0].clone();
        }
        match (name, s["type"].as_str()) {
            ("at", _) => json!("72,72"),
            ("color", _) if s["type"] == "string" => json!("#000000"),
            ("degrees", _) => json!(90),
            ("width", _) => json!(100),
            ("font_size", _) => json!(24),
            ("order", _) => json!([1]),
            (_, Some("boolean")) => json!(true),
            (_, Some("integer")) => s.get("minimum").cloned().unwrap_or(json!(1)),
            (_, Some("number")) => json!(1.5),
            (_, Some("array")) => json!([sample(name, &s["items"])]),
            _ => json!("x.pdf"),
        }
    }

    /// Every tool's required arguments, and each optional one in turn, turn into a command line
    /// clap accepts: the schemas cannot drift from the CLI.
    #[test]
    fn every_schema_parses_back() {
        for t in tools() {
            let name = t["name"].as_str().unwrap();
            let s = &t["inputSchema"];
            let props = s["properties"].as_object().unwrap();
            let mut base = Map::new();
            for r in s["required"].as_array().unwrap() {
                let r = r.as_str().unwrap();
                base.insert(r.into(), if r == "items" { json!([{ "text": "a", "at": "1,1" }]) } else { sample(r, &props[r]) });
            }
            // A command whose group needs one of several arguments gets the first; each other
            // member is tried in its place.
            let mut group: Vec<String> = Vec::new();
            if let Some(g) = t["description"].as_str().unwrap().split("Give exactly one of: ").nth(1) {
                group = g.split('.').next().unwrap().split(',').map(|m| m.trim().to_string()).collect();
                let first = group[0].as_str();
                base.insert(first.into(), sample(first, &props[first]));
            }
            let parse = |args: &Map<String, Value>| {
                let mut v = vec!["sheaf".to_string()];
                v.extend(argv(name, args).unwrap_or_else(|e| panic!("{name}: {e}")));
                crate::Cli::try_parse_from(&v).unwrap_or_else(|e| panic!("{name} {v:?}: {e}"));
            };
            parse(&base);
            for (k, p) in props {
                if base.contains_key(k) || ["text", "image", "strokes", "ranges", "every", "remove", "custom"].contains(&k.as_str()) {
                    continue;
                }
                let mut a = base.clone();
                if group.iter().skip(1).any(|m| m == k) {
                    a.remove(&group[0]);
                }
                a.insert(k.clone(), sample(k, p));
                if k.starts_with("p12_") || k == "reason" || k == "location" {
                    a.insert("p12".into(), json!("id.p12"));
                }
                if k == "landscape" {
                    a.insert("a4".into(), json!(true));
                }
                parse(&a);
            }
        }
    }
}
