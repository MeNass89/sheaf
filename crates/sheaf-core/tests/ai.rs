//! The AI layer against a mock provider: a local `TcpListener` that speaks each provider's request
//! and response format. No test ever calls a real provider.

use serde_json::{json, Value};
use sheaf_core::ai::tasks::{self, Source, Summary, ToSort};
use sheaf_core::ai::{chat, deck, detect_local_at, spaces, AiError, Config, Message, Provider};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

type Reply = Box<dyn Fn(&Seen) -> (u16, Vec<(String, String)>, String) + Send + Sync>;

/// A mock server answering every request with `reply`; returns its base URL and what it saw.
fn mock(reply: Reply) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            if r.read_line(&mut line).is_err() || line.is_empty() {
                continue;
            }
            let mut parts = line.split_whitespace();
            let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
            let mut headers = HashMap::new();
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                let h = h.trim_end();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':') {
                    headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
                }
            }
            let n: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
            let mut body = vec![0; n];
            r.read_exact(&mut body).unwrap();
            let seen = Seen { method, path, headers, body: serde_json::from_slice(&body).unwrap_or(Value::Null) };
            let (status, extra, text) = reply(&seen);
            log.lock().unwrap().push(seen);
            let mut out = format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n", text.len());
            for (k, v) in extra {
                out.push_str(&format!("{k}: {v}\r\n"));
            }
            out.push_str("\r\n");
            out.push_str(&text);
            let _ = s.write_all(out.as_bytes());
        }
    });
    (base, seen)
}

fn ok(body: Value) -> Reply {
    let text = body.to_string();
    Box::new(move |_| (200, Vec::new(), text.clone()))
}

fn cfg(p: Provider, base: &str, key: Option<&str>, model: &str) -> Config {
    let mut c = Config::new(p);
    c.base_url = base.into();
    c.api_key = key.map(str::to_string);
    c.model = model.into();
    c
}

fn convo() -> Vec<Message> {
    vec![Message::system("Be brief."), Message::user("What is this?"), Message::assistant("A report."), Message::user("And?")]
}

#[test]
fn openai_mistral_xai_and_compatible_speak_chat_completions() {
    for (p, key, base_path) in [(Provider::OpenAi, Some("sk-1"), "/v1"), (Provider::Mistral, Some("m-1"), "/v1"), (Provider::Xai, Some("x-1"), "/v1"), (Provider::Compatible, None, "/api"), (Provider::LlamaCpp, None, "/v1")] {
        let (base, seen) = mock(ok(json!({"choices": [{"message": {"role": "assistant", "content": "A summary."}}]})));
        let answer = chat(&cfg(p, &format!("{base}{base_path}"), key, "model-x"), &convo()).unwrap();
        assert_eq!(answer, "A summary.");
        let s = seen.lock().unwrap()[0].clone();
        assert_eq!((s.method.as_str(), s.path.as_str()), ("POST", format!("{base_path}/chat/completions").as_str()), "{p:?}");
        assert_eq!(s.headers.get("authorization").cloned(), key.map(|k| format!("Bearer {k}")), "{p:?}");
        assert_eq!(s.body["model"], "model-x");
        assert_eq!(s.body["messages"], json!([
            {"role": "system", "content": "Be brief."}, {"role": "user", "content": "What is this?"},
            {"role": "assistant", "content": "A report."}, {"role": "user", "content": "And?"},
        ]));
    }
}

#[test]
fn anthropic_gets_its_headers_and_a_separate_system_prompt() {
    let (base, seen) = mock(ok(json!({"type": "message", "content": [{"type": "text", "text": "Claude "}, {"type": "text", "text": "says hi."}]})));
    assert_eq!(chat(&cfg(Provider::Anthropic, &base, Some("sk-ant"), "claude-x"), &convo()).unwrap(), "Claude says hi.");
    let s = seen.lock().unwrap()[0].clone();
    assert_eq!(s.path, "/v1/messages");
    assert_eq!(s.headers["x-api-key"], "sk-ant");
    assert_eq!(s.headers["anthropic-version"], "2023-06-01");
    assert!(!s.headers.contains_key("authorization"));
    assert_eq!(s.body["system"], "Be brief.");
    assert_eq!(s.body["model"], "claude-x");
    assert!(s.body["max_tokens"].as_u64().unwrap() > 0);
    assert_eq!(s.body["messages"].as_array().unwrap().len(), 3);
    assert_eq!(s.body["messages"][1], json!({"role": "assistant", "content": "A report."}));
}

#[test]
fn gemini_gets_contents_and_a_system_instruction() {
    let (base, seen) = mock(ok(json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "Gemini answer."}]}}]})));
    assert_eq!(chat(&cfg(Provider::Gemini, &base, Some("g-key"), "gemini-x"), &convo()).unwrap(), "Gemini answer.");
    let s = seen.lock().unwrap()[0].clone();
    assert_eq!(s.path, "/v1beta/models/gemini-x:generateContent");
    assert_eq!(s.headers["x-goog-api-key"], "g-key");
    assert_eq!(s.body["systemInstruction"]["parts"][0]["text"], "Be brief.");
    assert_eq!(s.body["contents"][1], json!({"role": "model", "parts": [{"text": "A report."}]}));
    assert_eq!(s.body["contents"][2]["role"], "user");
}

#[test]
fn ollama_is_asked_without_streaming_and_without_a_key() {
    let (base, seen) = mock(ok(json!({"model": "llama3.2", "message": {"role": "assistant", "content": "Local answer."}, "done": true})));
    assert_eq!(chat(&cfg(Provider::Ollama, &base, None, "llama3.2"), &convo()).unwrap(), "Local answer.");
    let s = seen.lock().unwrap()[0].clone();
    assert_eq!(s.path, "/api/chat");
    assert_eq!(s.body["stream"], false);
    assert_eq!(s.body["messages"][0]["role"], "system");
    assert!(!s.headers.contains_key("authorization"));
}

#[test]
fn errors_are_told_plainly() {
    // A cloud provider without its key is never called.
    assert_eq!(chat(&cfg(Provider::OpenAi, "http://127.0.0.1:9", None, ""), &convo()), Err(AiError::MissingKey("OpenAI")));
    let (base, _) = mock(Box::new(|_| (429, vec![("retry-after".into(), "7".into())], r#"{"error":{"message":"Rate limit reached"}}"#.into())));
    assert_eq!(chat(&cfg(Provider::OpenAi, &base, Some("k"), ""), &convo()), Err(AiError::RateLimited("OpenAI", Some(7))));
    let (base, _) = mock(Box::new(|_| (400, vec![], r#"{"type":"error","error":{"type":"invalid_request_error","message":"model: not found"}}"#.into())));
    let e = chat(&cfg(Provider::Anthropic, &base, Some("k"), "nope"), &convo()).unwrap_err();
    assert_eq!(e.to_string(), "Anthropic (Claude) answered with an error (400): model: not found");
    let (base, _) = mock(Box::new(|_| (401, vec![], "{}".into())));
    assert_eq!(chat(&cfg(Provider::Gemini, &base, Some("k"), ""), &convo()), Err(AiError::BadKey("Google Gemini", 401)));
    // Nothing listening: the network error, not a hang.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    match chat(&cfg(Provider::Ollama, &format!("http://{closed}"), None, "m"), &convo()) {
        Err(AiError::Network(name, _)) => assert_eq!(name, Provider::Ollama.name()),
        other => panic!("{other:?}"),
    }
    let (base, _) = mock(Box::new(|_| (200, vec![], "not json".into())));
    assert_eq!(chat(&cfg(Provider::Mistral, &base, Some("k"), ""), &convo()), Err(AiError::BadResponse("Mistral")));
}

/// The text a request sent as its last user message.
fn last_user(s: &Seen) -> String {
    s.body["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap().to_string()
}

#[test]
fn a_long_document_is_mapped_then_reduced() {
    let (base, seen) = mock(Box::new(|s| {
        let q = last_user(s);
        let a = if q.contains("Notes taken from every part") { "Final answer (p. 7).".to_string() } else if q.contains("[p. 7]") { "The budget is 12 k€ (p. 7).".to_string() } else { "NONE".to_string() };
        (200, vec![], json!({"choices": [{"message": {"content": a}}]}).to_string())
    }));
    let pages: Vec<String> = (1..=8).map(|i| if i == 7 { "The budget is 12 k€. ".repeat(20) } else { format!("Filler text of page {i}. ").repeat(20) }).collect();
    let mut c = cfg(Provider::Compatible, &base, None, "m");
    c.max_chars = 600;
    let mut steps = Vec::new();
    let a = tasks::ask(&c, &tasks::page_sources(&pages), None, "What is the budget?", &[], &mut |i, n| { steps.push((i, n)); true }).unwrap();
    assert_eq!(a, "Final answer (p. 7).");
    let seen = seen.lock().unwrap();
    let parts = seen.len() - 1;
    assert!(parts >= 4, "{parts}");
    assert_eq!(steps.last(), Some(&(parts + 1, parts + 1)));
    let reduce = last_user(seen.last().unwrap());
    assert!(reduce.contains("The budget is 12 k€ (p. 7).") && !reduce.contains("NONE"), "{reduce}");
}

#[test]
fn a_selection_is_asked_alone_and_earlier_turns_are_kept() {
    let (base, seen) = mock(ok(json!({"choices": [{"message": {"content": "It means X."}}]})));
    let c = cfg(Provider::OpenAi, &base, Some("k"), "m");
    let a = tasks::ask(&c, &tasks::page_sources(&["Whole doc".into()]), Some("the selected words"), "Explain this", &[("Q1".into(), "A1".into())], &mut |_, _| true).unwrap();
    assert_eq!(a, "It means X.");
    let s = seen.lock().unwrap()[0].clone();
    let msgs = s.body["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 4);
    assert_eq!(msgs[1]["content"], "Q1");
    let last = last_user(&s);
    assert!(last.contains("the selected words") && last.contains("Explain this") && !last.contains("Whole doc"));
}

#[test]
fn summaries_translations_podcasts_and_slides_are_asked_for() {
    let (base, seen) = mock(Box::new(|s| {
        let q = last_user(s);
        let a = if q.contains("Translate into French") { format!("FR({})", q.matches("[p. ").count()) } else if q.contains("slide deck") { "# Report\n- Point A\n# Costs\n- Rent\n- Staff".into() } else { "Done.".into() };
        (200, vec![], json!({"choices": [{"message": {"content": a}}]}).to_string())
    }));
    let mut c = cfg(Provider::LlamaCpp, &base, None, "");
    let pages: Vec<String> = (1..=3).map(|i| format!("Text of page {i}.")).collect();
    let src = tasks::page_sources(&pages);
    assert_eq!(tasks::summarize(&c, &src, Summary::MainPoints, &mut |_, _| true).unwrap(), "Done.");
    assert!(last_user(&seen.lock().unwrap()[0]).contains("main points"));
    assert_eq!(tasks::podcast(&c, &src, &mut |_, _| true).unwrap(), "Done.");
    assert!(last_user(&seen.lock().unwrap()[1]).contains("Alex"));
    let outline = tasks::slides(&c, &src, &mut |_, _| true).unwrap();
    assert_eq!(deck::parse(&outline).len(), 2);
    // Translation is never reduced: each part translated, in order.
    c.max_chars = 200;
    let long: Vec<String> = (1..=6).map(|i| format!("Sentence {i}. ").repeat(8)).collect();
    let t = tasks::translate(&c, &tasks::page_sources(&long), "French", &mut |_, _| true).unwrap();
    assert!(t.matches("FR(").count() >= 3, "{t}");
    // Cancelled before the first request.
    let before = seen.lock().unwrap().len();
    assert_eq!(tasks::summarize(&c, &src, Summary::Paragraphs, &mut |_, _| false), Err(AiError::Cancelled));
    assert_eq!(seen.lock().unwrap().len(), before);
    assert_eq!(tasks::summarize(&c, &tasks::page_sources(&["  ".into()]), Summary::Paragraphs, &mut |_, _| true), Err(AiError::NoText));
}

#[test]
fn documents_are_classified_into_the_categories_given() {
    let (base, seen) = mock(Box::new(|s| {
        let q = last_user(s);
        let a = if q.contains("Invoice no.") { "Invoice" } else if q.contains("agree") { "This is a contract." } else { "Poem" };
        (200, vec![], json!({"message": {"content": a}}).to_string())
    }));
    let docs = vec![
        ToSort { name: "a.pdf".into(), text: "Invoice no. 12, total 40 €".into() },
        ToSort { name: "b.pdf".into(), text: "The parties agree as follows".into() },
        ToSort { name: "c.pdf".into(), text: "Roses are red".into() },
        ToSort { name: "d.pdf".into(), text: "".into() },
    ];
    let cats = vec!["Invoice".to_string(), "Contract".to_string()];
    let got = tasks::classify(&cfg(Provider::Ollama, &base, None, "m"), &docs, &cats, &mut |_, _| true).unwrap();
    assert_eq!(got, ["Invoice", "Contract", "Other", "Other"]);
    // The empty document was not sent.
    assert_eq!(seen.lock().unwrap().len(), 3);
    assert!(last_user(&seen.lock().unwrap()[0]).contains("Invoice, Contract"));
}

#[test]
fn a_local_server_is_found() {
    let (ollama, _) = mock(Box::new(|s| if s.path == "/api/tags" { (200, vec![], json!({"models": [{"name": "qwen3:4b"}]}).to_string()) } else { (404, vec![], "{}".into()) }));
    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let c = detect_local_at(&ollama, &format!("http://{closed}/v1")).unwrap();
    assert_eq!((c.provider, c.model.as_str()), (Provider::Ollama, "qwen3:4b"));
    let (llama, _) = mock(Box::new(|s| if s.path == "/v1/models" { (200, vec![], json!({"data": [{"id": "gguf-model"}]}).to_string()) } else { (404, vec![], "{}".into()) }));
    let c = detect_local_at(&format!("http://{closed}"), &format!("{llama}/v1")).unwrap();
    assert_eq!((c.provider, c.model.as_str()), (Provider::LlamaCpp, "gguf-model"));
    assert!(detect_local_at(&format!("http://{closed}"), &format!("http://{closed}/v1")).is_none());
}

#[test]
fn a_deck_is_written_as_pdf_and_powerpoint() {
    let d = tempfile::tempdir().unwrap();
    let slides = deck::parse("# Annual report\n- 2026\n# Sales\n- Up 12%\n- Café openings\n# Costs\n- Rent");
    let pdf = deck::write("Annual report", &slides, &d.path().join("deck.pdf")).unwrap();
    let doc = lopdf::Document::load(&pdf).unwrap();
    assert_eq!(doc.get_pages().len(), 3);
    let texts = sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&pdf), &[]).unwrap();
    assert!(texts[1].contains("Sales") && texts[1].contains("Café openings"), "{texts:?}");
    let pptx = deck::write("Annual report", &slides, &d.path().join("deck.pptx")).unwrap();
    let bytes = std::fs::read(&pptx).unwrap();
    assert!(bytes.starts_with(b"PK"));
    let mut zip = String::new();
    for b in &bytes { zip.push(*b as char); }
    assert!(zip.contains("ppt/slides/slide3.xml"));
}

#[test]
fn a_space_reads_every_file_with_its_name() {
    let d = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for (n, t) in [("one.pdf", "First file text"), ("two.pdf", "Second file text")] {
        let bytes = sheaf_core::pdfcraft::engine::Session::new().create_from_text(n, t).unwrap();
        std::fs::write(d.path().join(n), bytes.as_slice()).unwrap();
        files.push(d.path().join(n));
    }
    files.push(d.path().join("missing.pdf"));
    let (src, failed): (Vec<Source>, Vec<String>) = spaces::sources(&files);
    assert_eq!(src.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["one.pdf, p. 1", "two.pdf, p. 1"]);
    assert!(src[1].text.contains("Second file text"));
    assert_eq!(failed.len(), 1);
}
