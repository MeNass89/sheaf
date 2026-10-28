//! `sheaf ask`, `summarize` and `translate` against a mock provider (a local TcpListener speaking
//! OpenAI's chat completions): the flags and environment variables choose it, only the text goes.

use lopdf::{dictionary, Document, Object, Stream};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};

fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sheaf"));
    for k in ["SHEAF_AI_PROVIDER", "SHEAF_AI_MODEL", "SHEAF_AI_BASE_URL", "SHEAF_AI_KEY", "OPENAI_API_KEY", "MISTRAL_API_KEY"] {
        c.env_remove(k);
    }
    c
}

/// A PDF whose pages say `texts`.
fn pdf(path: &std::path::Path, texts: &[&str]) {
    let mut doc = Document::with_version("1.5");
    let pages = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let kids: Vec<Object> = texts
        .iter()
        .map(|t| {
            let c = doc.add_object(Stream::new(dictionary! {}, format!("BT /F1 12 Tf 72 700 Td ({t}) Tj ET").into_bytes()));
            doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Contents" => c, "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } } }).into()
        })
        .collect();
    let n = kids.len() as i64;
    doc.objects.insert(pages, dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => n, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()] }.into());
    let root = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", root);
    doc.save(path).unwrap();
}

/// (authorization header, body) of each request the mock got.
type Seen = Arc<Mutex<Vec<(String, serde_json::Value)>>>;

/// A mock OpenAI-compatible server: answers `answer`, keeps what each request carried.
fn mock(answer: &'static str) -> (String, Seen) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut r = BufReader::new(s.try_clone().unwrap());
            let (mut len, mut auth, mut line) = (0usize, String::new(), String::new());
            r.read_line(&mut line).unwrap();
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                let h = h.trim_end().to_string();
                if h.is_empty() {
                    break;
                }
                let (k, v) = h.split_once(':').unwrap();
                match k.to_ascii_lowercase().as_str() {
                    "content-length" => len = v.trim().parse().unwrap(),
                    "authorization" => auth = v.trim().to_string(),
                    _ => {}
                }
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap();
            log.lock().unwrap().push((auth, serde_json::from_slice(&body).unwrap()));
            let text = serde_json::json!({"choices": [{"message": {"content": answer}}]}).to_string();
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}", text.len()).as_bytes());
        }
    });
    (base, seen)
}

fn last_user(v: &serde_json::Value) -> String {
    v["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap().to_string()
}

#[test]
fn ask_sends_the_chosen_pages_text_to_the_provider_named_by_flags() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("doc.pdf");
    pdf(&f, &["Alpha page about budgets", "Beta page about holidays"]);
    let (base, seen) = mock("The budget is on page 1.");
    let out = bin().arg("ask").arg(&f).args(["--question", "Where is the budget?", "--pages", "1", "--provider", "openai", "--model", "gpt-test", "--api-key", "sk-flag", "--base-url", &base]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "The budget is on page 1.");
    let (auth, body) = seen.lock().unwrap()[0].clone();
    assert_eq!(auth, "Bearer sk-flag");
    assert_eq!(body["model"], "gpt-test");
    let q = last_user(&body);
    assert!(q.contains("[p. 1]") && q.contains("Alpha page") && q.contains("Where is the budget?") && !q.contains("Beta"), "{q}");
}

#[test]
fn summarize_and_translate_read_the_environment() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("doc.pdf");
    pdf(&f, &["Gamma report text"]);
    let (base, seen) = mock("- Point one (p. 1)");
    let env = |c: &mut Command| {
        c.env("SHEAF_AI_PROVIDER", "mistral").env("SHEAF_AI_BASE_URL", &base).env("MISTRAL_API_KEY", "m-env").env("SHEAF_AI_MODEL", "mistral-test");
    };
    let mut c = bin();
    env(&mut c);
    let out = c.arg("summarize").arg(&f).arg("--points").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "- Point one (p. 1)");
    let (auth, body) = seen.lock().unwrap()[0].clone();
    assert_eq!((auth.as_str(), body["model"].as_str()), ("Bearer m-env", Some("mistral-test")));
    assert!(last_user(&body).contains("main points"));
    let mut c = bin();
    env(&mut c);
    let txt = d.path().join("fr.txt");
    let out = c.arg("translate").arg(&f).args(["--to", "French", "-o"]).arg(&txt).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(std::fs::read_to_string(&txt).unwrap(), "- Point one (p. 1)");
    assert!(last_user(&seen.lock().unwrap()[1].1).contains("Translate into French"));
}

#[test]
fn a_missing_key_is_said_plainly_and_nothing_is_sent() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("doc.pdf");
    pdf(&f, &["Delta"]);
    let (base, seen) = mock("x");
    let out = bin().arg("--json").arg("ask").arg(&f).args(["-q", "Hi", "--provider", "anthropic", "--base-url", &base]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"], "Add your Anthropic (Claude) API key with --api-key or SHEAF_AI_KEY.");
    assert!(seen.lock().unwrap().is_empty());
}
