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

/// A mock that answers with a raw `content_type` body (a stream), keeping the request.
fn mock_raw(content_type: &'static str, body: String) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            let mut n = 0usize;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h.trim().is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':')
                    && k.eq_ignore_ascii_case("content-length")
                {
                    n = v.trim().parse().unwrap();
                }
            }
            let mut b = vec![0; n];
            r.read_exact(&mut b).unwrap();
            log.lock().unwrap().push(Seen { method: "POST".into(), path, headers: HashMap::new(), body: serde_json::from_slice(&b).unwrap() });
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    });
    (base, seen)
}

#[test]
fn answers_are_streamed_in_each_providers_format() {
    let cases: [(Provider, &str, String); 4] = [
        (Provider::OpenAi, "text/event-stream", "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo.\"}}]}\n\ndata: [DONE]\n\n".into()),
        (Provider::Anthropic, "text/event-stream", "event: message_start\ndata: {\"type\":\"message_start\"}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"lo.\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n".into()),
        (Provider::Gemini, "text/event-stream", "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hel\"}]}}]}\n\ndata: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"lo.\"}]}}]}\n\n".into()),
        (Provider::Ollama, "application/x-ndjson", "{\"message\":{\"content\":\"Hel\"},\"done\":false}\n{\"message\":{\"content\":\"lo.\"},\"done\":true}\n".into()),
    ];
    for (p, kind, body) in cases {
        let (base, seen) = mock_raw(kind, body);
        let mut pieces = Vec::new();
        let all = sheaf_core::ai::chat_stream(&cfg(p, &base, Some("k"), "m"), &convo(), &mut |d| { pieces.push(d.to_string()); true }).unwrap();
        assert_eq!((all.as_str(), pieces.len()), ("Hello.", 2), "{p:?}");
        let s = seen.lock().unwrap()[0].clone();
        if p == Provider::Gemini {
            assert_eq!(s.path, "/v1beta/models/m:streamGenerateContent?alt=sse");
        } else {
            assert_eq!(s.body["stream"], true, "{p:?}");
        }
    }
    // A server that ignores streaming answers whole: read whole, handed over once.
    let (base, _) = mock(ok(json!({"choices": [{"message": {"content": "Whole."}}]})));
    let mut n = 0;
    assert_eq!(sheaf_core::ai::chat_stream(&cfg(Provider::Compatible, &base, None, "m"), &convo(), &mut |_| { n += 1; true }).unwrap(), "Whole.");
    assert_eq!(n, 1);
    // An error inside the stream is told, and the summary task streams its final answer.
    let (base, _) = mock_raw("text/event-stream", "data: {\"type\":\"error\",\"error\":{\"message\":\"Overloaded\"}}\n\n".into());
    assert_eq!(sheaf_core::ai::chat_stream(&cfg(Provider::Anthropic, &base, Some("k"), "m"), &convo(), &mut |_| true).unwrap_err().to_string(), "Anthropic (Claude) answered with an error (200): Overloaded");
    let (base, _) = mock_raw("text/event-stream", "data: {\"choices\":[{\"delta\":{\"content\":\"A \"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"summary.\"}}]}\n\n".into());
    let got = Arc::new(Mutex::new(String::new()));
    let g = got.clone();
    let c = cfg(Provider::Compatible, &base, None, "m");
    let r = tasks::streaming(move |d| { g.lock().unwrap().push_str(d); true }, || tasks::summarize(&c, &tasks::page_sources(&["Text.".into()]), Summary::Paragraphs, &mut |_, _| true));
    assert_eq!(r.unwrap(), "A summary.");
    assert_eq!(*got.lock().unwrap(), "A summary.");
}

/// A voice that writes `n` samples of the voice's number (1 or 2) per word, 16-bit mono 22050 Hz.
struct FakeVoice(Mutex<Vec<(String, Option<String>)>>);

/// A voice the fake speaks with, by id.
fn fake(id: &str) -> Option<sheaf_core::ai::audio::Voice> {
    Some(sheaf_core::ai::audio::Voice { engine: sheaf_core::ai::audio::Engine::Espeak, id: id.into(), label: id.into(), lang: "en".into(), female: None, quality: 1 })
}

impl sheaf_core::ai::audio::Synth for FakeVoice {
    fn to_wav(&self, text: &str, voice: Option<&sheaf_core::ai::audio::Voice>, out: &std::path::Path) -> sheaf_core::Result<()> {
        let voice = voice.map(|v| v.id.as_str());
        self.0.lock().unwrap().push((text.into(), voice.map(str::to_string)));
        let v: i16 = if voice == Some("m") { 1 } else { 2 };
        let data: Vec<u8> = (0..text.split_whitespace().count() * 10).flat_map(|_| v.to_le_bytes()).collect();
        let fmt: Vec<u8> = [1u16.to_le_bytes(), 1u16.to_le_bytes()].concat().into_iter().chain(22050u32.to_le_bytes()).chain(44100u32.to_le_bytes()).chain(2u16.to_le_bytes()).chain(16u16.to_le_bytes()).collect();
        let mut b = b"RIFF".to_vec();
        b.extend(((36 + data.len()) as u32).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(&fmt);
        b.extend(b"LIST\x04\x00\x00\x00INFO");
        b.extend(b"data");
        b.extend((data.len() as u32).to_le_bytes());
        b.extend(data);
        std::fs::write(out, b).unwrap();
        Ok(())
    }
}

#[test]
fn a_podcast_script_is_spoken_by_two_voices_into_one_wav() {
    use sheaf_core::ai::audio;
    let d = tempfile::tempdir().unwrap();
    let voice = FakeVoice(Mutex::new(Vec::new()));
    let out = audio::podcast_wav("Alex: one two\nSam: three\nAlex: four", &voice, &[fake("m"), fake("f")], &d.path().join("p.wav"), &mut |_, _| true).unwrap();
    let said = voice.0.lock().unwrap().clone();
    assert_eq!(said.iter().map(|(t, v)| (t.as_str(), v.as_deref().unwrap())).collect::<Vec<_>>(), [("one two", "m"), ("three", "f"), ("four", "m")]);
    let b = std::fs::read(out).unwrap();
    assert_eq!(&b[..4], b"RIFF");
    assert_eq!(&b[8..16], b"WAVEfmt ");
    let data_len = u32::from_le_bytes(b[40..44].try_into().unwrap()) as usize;
    // 20 + 10 + 10 samples of speech and two 0.4 s pauses (8820 samples each), 2 bytes a sample.
    assert_eq!(data_len, (40 + 2 * 8820) * 2);
    assert_eq!(b.len(), 44 + data_len);
    assert_eq!(i16::from_le_bytes([b[44], b[45]]), 1);
    assert_eq!(i16::from_le_bytes([b[44 + 2 * (20 + 8820)], b[45 + 2 * (20 + 8820)]]), 2);
    // Stopped: nothing written.
    assert!(audio::podcast_wav("Alex: hi", &voice, &[None, None], &d.path().join("q.wav"), &mut |_, _| false).is_err());
    assert!(!d.path().join("q.wav").exists());
}

/// eSpeak NG, when installed (it is on Sheaf's Linux test machines): real speech, two variants.
#[test]
fn espeak_speaks_a_podcast_when_installed() {
    if cfg!(any(target_os = "macos", windows)) || std::process::Command::new("espeak-ng").arg("--version").output().is_err() {
        eprintln!("skipped: no espeak-ng");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    use sheaf_core::ai::audio;
    // The best two voices of the script's language (eSpeak NG's variants, or Piper's when installed).
    let voices = audio::host_voices(&audio::rank_voices(&audio::system_voices(), audio::guess_language("Hello there. Hi Alex.")));
    assert!(voices.iter().all(|v| v.as_ref().is_some_and(|v| v.lang.starts_with("en"))), "{voices:?}");
    let out = sheaf_core::ai::audio::podcast_wav("Alex: Hello there.\nSam: Hi Alex.", &sheaf_core::ai::audio::SystemSynth, &voices, &d.path().join("p.wav"), &mut |_, _| true).unwrap();
    let b = std::fs::read(out).unwrap();
    assert!(b.len() > 22050, "{} bytes", b.len());
}

#[test]
fn a_translation_in_any_script_is_saved_as_a_searchable_pdf() {
    let d = tempfile::tempdir().unwrap();
    // Cyrillic, Greek and Japanese: Helvetica draws none of them.
    let text = "Привет, мир.\nΓειά σου κόσμε.\n東京は日本の首都です。";
    // A computer with no font for one of these scripts is told so (the test machines have them).
    let out = match sheaf_core::ai::write_text("Перевод", text, &d.path().join("t.pdf")) {
        Ok(p) => p,
        Err(e) if e.to_string().contains("no font on this computer") && std::env::var_os("CI").is_some() => return eprintln!("skipped: {e}"),
        Err(e) => panic!("{e}"),
    };
    let got = sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&out), &[]).unwrap().join("\n");
    for w in ["Привет", "κόσμε", "東京"] {
        assert!(got.contains(w), "{w} not in {got:?}");
    }
    // Western text still goes through Helvetica: no font embedded, so a small file.
    let p = sheaf_core::ai::write_text("T", "Café crème", &d.path().join("w.pdf")).unwrap();
    assert!(std::fs::metadata(&p).unwrap().len() < 20_000);
    assert!(sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&p), &[]).unwrap()[0].contains("Café crème"));
    // The Unicode one embeds its faces.
    assert!(std::fs::metadata(&out).unwrap().len() > 20_000);
}

/// Sol, round 31: a Japanese or Arabic deck had "?" for its text. Both the PDF and the PowerPoint
/// made from it keep the script.
#[test]
fn a_deck_in_any_script_keeps_its_text_in_pdf_and_powerpoint() {
    let d = tempfile::tempdir().unwrap();
    let slides = deck::parse("# 年次報告\n- 売上は12%増加\n# Продажи\n- Рост на 12%");
    let pdf = deck::write("年次報告", &slides, &d.path().join("deck.pdf")).unwrap();
    let texts = sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&pdf), &[]).unwrap();
    assert!(texts[0].contains("年次報告") && texts[1].contains("Продажи") && texts[1].contains("Рост"), "{texts:?}");
    assert!(!texts.concat().contains('?'));
    let pptx = deck::write("年次報告", &slides, &d.path().join("deck.pptx")).unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(pptx).unwrap()).unwrap();
    let mut xml = String::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        if f.name().starts_with("ppt/slides/slide") {
            std::io::Read::read_to_string(&mut f, &mut xml).unwrap();
        }
    }
    assert!(xml.contains("年次報告") && xml.contains("Продажи"), "slides: {}", xml.chars().take(400).collect::<String>());
}

/// The base font names of the faces a deck page draws its title with (the first text drawn on
/// the page) and its other text with.
fn deck_fonts(pdf: &std::path::Path) -> Vec<(String, Vec<String>)> {
    let doc = lopdf::Document::load(pdf).unwrap();
    doc.get_pages()
        .values()
        .map(|id| {
            let content = String::from_utf8_lossy(&doc.get_page_content(*id)).into_owned();
            let fonts = doc.get_page_fonts(*id).unwrap();
            let name = |key: &str| fonts.get(key.as_bytes()).and_then(|f| f.get(b"BaseFont").ok()).and_then(|n| n.as_name().ok()).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default();
            let used: Vec<String> = content.split_whitespace().filter(|t| t.starts_with("/F") || t.starts_with("/B")).map(|t| name(&t[1..])).collect();
            (used.first().cloned().unwrap_or_default(), used.into_iter().skip(1).collect())
        })
        .collect()
}

/// Author a8, round 34: a Japanese or Russian deck had its titles in the regular face. The title
/// of every slide is set in a bold face (BIZ UDPGothic Bold for Japanese, a Bold face for
/// Cyrillic), the points in a regular one, and the text still reads back.
#[test]
fn japanese_and_cyrillic_decks_have_bold_titles() {
    let d = tempfile::tempdir().unwrap();
    for (name, outline, words) in [
        ("ja", "# 年次報告\n- 売上は12%増加\n# 今後の計画\n- 新しい市場", ["年次報告", "今後の計画", "新しい市場"]),
        ("ru", "# Годовой отчёт\n- Рост на 12%\n# Планы\n- Новые рынки", ["Годовой отчёт", "Планы", "Новые рынки"]),
    ] {
        let slides = deck::parse(outline);
        let pdf = deck::write(&slides[0].title, &slides, &d.path().join(format!("{name}.pdf"))).unwrap();
        for (k, (title, rest)) in deck_fonts(&pdf).iter().enumerate() {
            assert!(title.contains("Bold"), "{name} slide {}: title in {title}", k + 1);
            assert!(rest.iter().any(|f| !f.contains("Bold")), "{name} slide {}: points in {rest:?}", k + 1);
        }
        if name == "ja" {
            assert!(deck_fonts(&pdf)[0].0.starts_with("BIZUDPGothic-Bold"), "{:?}", deck_fonts(&pdf)[0]);
        }
        let texts = sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&pdf), &[]).unwrap().join("\n");
        for w in words {
            assert!(texts.contains(w), "{w} not in {texts:?}");
        }
        // The PowerPoint made from it has its titles bold.
        let pptx = deck::write(&slides[0].title, &slides, &d.path().join(format!("{name}.pptx"))).unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(pptx).unwrap()).unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("ppt/slides/slide2.xml").unwrap(), &mut xml).unwrap();
        let run = xml.split("<a:r>").find(|r| r.contains(words[1])).unwrap_or_default();
        assert!(run.contains("b=\"1\""), "{name}: {run}");
    }
}

/// A script without a bold face (Thai: the bundled Noto Sans Thai is regular only) gets synthetic
/// bold: its regular face, named `Face,Bold` and stroked; the text reads back.
#[test]
fn a_title_without_a_bold_face_is_drawn_in_synthetic_bold() {
    let d = tempfile::tempdir().unwrap();
    let slides = deck::parse("# สวัสดีครับ\n- ประเทศไทย");
    let pdf = deck::write("t", &slides, &d.path().join("th.pdf")).unwrap();
    let (title, _) = &deck_fonts(&pdf)[0];
    assert!(title.ends_with(",Bold"), "{title}");
    let doc = lopdf::Document::load(&pdf).unwrap();
    let content = String::from_utf8_lossy(&doc.get_page_content(doc.page_iter().next().unwrap())).into_owned();
    assert!(content.contains("2 Tr"), "no stroke in {content}");
    let texts = sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&pdf), &[]).unwrap().concat();
    assert!(texts.contains("ประเทศไทย"), "{texts:?}");
}

/// Sol, round 31: an Arabic line's "12.50" came out as 50.12. Drawn right to left, the number's
/// glyphs stay in reading order, left to right (the text read back keeps 12.50).
#[test]
fn a_number_in_an_arabic_line_keeps_its_value() {
    let d = tempfile::tempdir().unwrap();
    let out = sheaf_core::ai::write_text("t", "السعر 12.50 دولار", &d.path().join("ar.pdf")).unwrap();
    let got = sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&out), &[]).unwrap().concat();
    assert!(got.contains("12.50"), "{got:?}");
    assert!(!got.contains("50.12"), "{got:?}");
}

/// Stirling keeps a document's class in the PDF: written in place as an incremental update (the
/// original bytes untouched), read back after reopening, replaced rather than stacked.
#[test]
fn a_category_is_kept_in_the_pdf_itself() {
    use sheaf_core::ai::labels;
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("bill.pdf");
    let original = sheaf_core::pdfcraft::engine::Session::new().create_from_text("Bill", "Invoice no. 12").unwrap();
    std::fs::write(&f, original.as_slice()).unwrap();
    assert_eq!(labels::read(&f), None);
    labels::write(&f, "Invoice").unwrap();
    let after = std::fs::read(&f).unwrap();
    assert!(after.starts_with(original.as_slice()) && after.len() > original.len(), "not an incremental update");
    assert_eq!(labels::read(&f).as_deref(), Some("Invoice"));
    labels::write(&f, "Reçu").unwrap();
    assert_eq!(labels::read(&f).as_deref(), Some("Reçu"));
    let kw = sheaf_core::pdfcraft::Doc::open(&sheaf_core::pages::Input::new(&f)).unwrap().document().info.keywords.clone().unwrap();
    assert_eq!(kw, "Category: Reçu");
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1, "no temporary file left");
}
