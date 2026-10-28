//! AI on a document's text (Acrobat's AI Assistant, PDF Expert's AI Copilot): ask, summarize,
//! translate, podcast scripts, slide decks, AI spaces and classification, through the provider the
//! user chose: OpenAI, Anthropic, Google Gemini, Mistral, xAI, any OpenAI-compatible endpoint, or
//! a local Ollama or llama.cpp server. Only the text is sent, never the file. Each call is a
//! blocking HTTP request (`ureq`), so callers run it off the UI thread.

pub mod deck;
pub mod spaces;
pub mod tasks;
pub mod audio;
pub mod labels;
pub mod unicode_pdf;

use serde_json::{json, Value};
use std::time::Duration;

/// Who answers. `id` is what preferences and the CLI's `--provider` store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    OpenAi,
    Anthropic,
    Gemini,
    Mistral,
    Xai,
    /// Any server that speaks OpenAI's chat completions (LM Studio, vLLM, OpenRouter, Groq…).
    Compatible,
    Ollama,
    LlamaCpp,
}

impl Provider {
    pub const ALL: [Provider; 8] = [Provider::OpenAi, Provider::Anthropic, Provider::Gemini, Provider::Mistral, Provider::Xai, Provider::Compatible, Provider::Ollama, Provider::LlamaCpp];

    pub fn id(self) -> &'static str {
        match self {
            Provider::OpenAi => "openai",
            Provider::Anthropic => "anthropic",
            Provider::Gemini => "gemini",
            Provider::Mistral => "mistral",
            Provider::Xai => "xai",
            Provider::Compatible => "compatible",
            Provider::Ollama => "ollama",
            Provider::LlamaCpp => "llamacpp",
        }
    }

    pub fn from_id(id: &str) -> Option<Provider> {
        let id = id.trim().to_ascii_lowercase();
        Provider::ALL.into_iter().find(|p| p.id() == id).or(match id.as_str() {
            "claude" => Some(Provider::Anthropic),
            "google" => Some(Provider::Gemini),
            "grok" => Some(Provider::Xai),
            "openai-compatible" => Some(Provider::Compatible),
            "llama.cpp" | "llama-cpp" => Some(Provider::LlamaCpp),
            _ => None,
        })
    }

    /// The name the panel and errors use.
    pub fn name(self) -> &'static str {
        match self {
            Provider::OpenAi => "OpenAI",
            Provider::Anthropic => "Anthropic (Claude)",
            Provider::Gemini => "Google Gemini",
            Provider::Mistral => "Mistral",
            Provider::Xai => "xAI (Grok)",
            Provider::Compatible => "OpenAI-compatible server",
            Provider::Ollama => "Ollama (on this computer)",
            Provider::LlamaCpp => "llama.cpp (on this computer)",
        }
    }

    /// The address used when the user gave none ("" for a compatible server: it must be given).
    pub fn default_base(self) -> &'static str {
        match self {
            Provider::OpenAi => "https://api.openai.com/v1",
            Provider::Anthropic => "https://api.anthropic.com",
            Provider::Gemini => "https://generativelanguage.googleapis.com",
            Provider::Mistral => "https://api.mistral.ai/v1",
            Provider::Xai => "https://api.x.ai/v1",
            Provider::Compatible => "",
            Provider::Ollama => "http://127.0.0.1:11434",
            Provider::LlamaCpp => "http://127.0.0.1:8080/v1",
        }
    }

    /// A model to start from; the user can name any other in Preferences ▸ AI.
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::OpenAi => "gpt-5-mini",
            Provider::Anthropic => "claude-sonnet-4-5",
            Provider::Gemini => "gemini-2.5-flash",
            Provider::Mistral => "mistral-small-latest",
            Provider::Xai => "grok-4",
            Provider::Compatible => "",
            Provider::Ollama => "llama3.2",
            Provider::LlamaCpp => "local",
        }
    }

    /// Cloud providers refuse a request without a key; local servers and compatible ones may not.
    pub fn needs_key(self) -> bool {
        matches!(self, Provider::OpenAi | Provider::Anthropic | Provider::Gemini | Provider::Mistral | Provider::Xai)
    }

    pub fn is_local(self) -> bool {
        matches!(self, Provider::Ollama | Provider::LlamaCpp)
    }

    /// The environment variable the provider's own tools read the key from.
    pub fn key_env(self) -> Option<&'static str> {
        match self {
            Provider::OpenAi => Some("OPENAI_API_KEY"),
            Provider::Anthropic => Some("ANTHROPIC_API_KEY"),
            Provider::Gemini => Some("GEMINI_API_KEY"),
            Provider::Mistral => Some("MISTRAL_API_KEY"),
            Provider::Xai => Some("XAI_API_KEY"),
            _ => None,
        }
    }
}

/// One provider set up: where, with which key and model, and how much text one request carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub provider: Provider,
    /// "" means [`Provider::default_base`].
    pub base_url: String,
    pub api_key: Option<String>,
    /// "" means [`Provider::default_model`].
    pub model: String,
    /// Characters of document text per request; longer texts are mapped then reduced.
    pub max_chars: usize,
}

impl Config {
    pub fn new(provider: Provider) -> Config {
        // Local models usually run with a small context (Ollama's default is a few thousand tokens).
        Config { provider, base_url: String::new(), api_key: None, model: String::new(), max_chars: if provider.is_local() { 6_000 } else { 24_000 } }
    }

    pub fn base(&self) -> String {
        let b = if self.base_url.trim().is_empty() { self.provider.default_base() } else { self.base_url.trim() };
        b.trim_end_matches('/').to_string()
    }

    pub fn model(&self) -> String {
        if self.model.trim().is_empty() { self.provider.default_model().to_string() } else { self.model.trim().to_string() }
    }

    /// "OpenAI · gpt-5-mini", what the panel's header shows.
    pub fn label(&self) -> String {
        let m = self.model();
        if m.is_empty() { self.provider.name().to_string() } else { format!("{} · {m}", self.provider.name()) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(s: impl Into<String>) -> Message { Message { role: Role::System, content: s.into() } }
    pub fn user(s: impl Into<String>) -> Message { Message { role: Role::User, content: s.into() } }
    pub fn assistant(s: impl Into<String>) -> Message { Message { role: Role::Assistant, content: s.into() } }
}

/// What went wrong, said plainly in the panel and by the CLI.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AiError {
    #[error("Choose an AI provider in Preferences ▸ AI (or start Ollama or a llama.cpp server on this computer).")]
    NotConfigured,
    #[error("Add your {0} API key in Preferences ▸ AI.")]
    MissingKey(&'static str),
    #[error("Give the address of the OpenAI-compatible server in Preferences ▸ AI.")]
    MissingBase,
    #[error("Cannot reach {0}: {1}. Check the connection or the server's address.")]
    Network(&'static str, String),
    #[error("{name} is limiting requests right now. Try again {when}.", name = .0, when = .1.map(|s| format!("in {s} s")).unwrap_or_else(|| "in a moment".into()))]
    RateLimited(&'static str, Option<u64>),
    #[error("{0} refused the API key ({1}). Check it in Preferences ▸ AI.")]
    BadKey(&'static str, u16),
    #[error("{0} answered with an error ({1}): {2}")]
    Provider(&'static str, u16, String),
    #[error("{0} sent an answer Sheaf cannot read.")]
    BadResponse(&'static str),
    #[error("This document has no text to send. If it is a scan, recognize its text first (OCR).")]
    NoText,
    #[error("Sending text to AI is turned off. Turn it on in Preferences ▸ AI.")]
    Off,
    #[error("Stopped.")]
    Cancelled,
}

/// An HTTP request ready to send: the provider's URL, headers and JSON body.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

/// The request `cfg`'s provider expects for a chat of `messages` (system messages first).
pub fn request(cfg: &Config, messages: &[Message]) -> Result<Request, AiError> {
    let p = cfg.provider;
    let key = cfg.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty());
    if p.needs_key() && key.is_none() {
        return Err(AiError::MissingKey(p.name()));
    }
    let base = cfg.base();
    if base.is_empty() {
        return Err(AiError::MissingBase);
    }
    let model = cfg.model();
    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    let system: Vec<&str> = messages.iter().filter(|m| m.role == Role::System).map(|m| m.content.as_str()).collect();
    let turns = messages.iter().filter(|m| m.role != Role::System);
    let role = |r: Role| if r == Role::Assistant { "assistant" } else { "user" };
    let (url, body) = match p {
        Provider::Anthropic => {
            headers.push(("x-api-key".into(), key.unwrap_or_default().into()));
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
            let mut body = json!({
                "model": model,
                "max_tokens": 4096,
                "messages": turns.map(|m| json!({"role": role(m.role), "content": m.content})).collect::<Vec<_>>(),
            });
            if !system.is_empty() {
                body["system"] = Value::String(system.join("\n\n"));
            }
            (format!("{base}/v1/messages"), body)
        }
        Provider::Gemini => {
            headers.push(("x-goog-api-key".into(), key.unwrap_or_default().into()));
            let mut body = json!({
                "contents": turns.map(|m| json!({"role": if m.role == Role::Assistant { "model" } else { "user" }, "parts": [{"text": m.content}]})).collect::<Vec<_>>(),
            });
            if !system.is_empty() {
                body["systemInstruction"] = json!({"parts": [{"text": system.join("\n\n")}]});
            }
            (format!("{base}/v1beta/models/{model}:generateContent"), body)
        }
        Provider::Ollama => {
            let all = messages.iter().map(|m| json!({"role": match m.role { Role::System => "system", r => role(r) }, "content": m.content})).collect::<Vec<_>>();
            (format!("{base}/api/chat"), json!({"model": model, "messages": all, "stream": false}))
        }
        Provider::OpenAi | Provider::Mistral | Provider::Xai | Provider::Compatible | Provider::LlamaCpp => {
            if let Some(k) = key {
                headers.push(("authorization".into(), format!("Bearer {k}")));
            }
            let all = messages.iter().map(|m| json!({"role": match m.role { Role::System => "system", r => role(r) }, "content": m.content})).collect::<Vec<_>>();
            let mut body = json!({"messages": all});
            if !model.is_empty() {
                body["model"] = Value::String(model);
            }
            (format!("{base}/chat/completions"), body)
        }
    };
    Ok(Request { url, headers, body })
}

/// [`request`] asking the provider to stream its answer (server-sent events; Ollama: JSON lines).
pub fn stream_request(cfg: &Config, messages: &[Message]) -> Result<Request, AiError> {
    let mut r = request(cfg, messages)?;
    match cfg.provider {
        Provider::Gemini => r.url = r.url.replace(":generateContent", ":streamGenerateContent?alt=sse"),
        _ => r.body["stream"] = Value::Bool(true),
    }
    Ok(r)
}

/// The text one streamed line adds ("" none), or the provider's error inside the stream.
pub fn stream_delta(provider: Provider, line: &str) -> Result<String, AiError> {
    let line = line.trim();
    let data = if provider == Provider::Ollama { line } else { line.strip_prefix("data:").map(str::trim).unwrap_or("") };
    if data.is_empty() || data == "[DONE]" {
        return Ok(String::new());
    }
    let Ok(v) = serde_json::from_str::<Value>(data) else { return Ok(String::new()) };
    if v["type"] == "error" || (v.get("error").is_some() && !v["error"].is_null()) {
        let m = v["error"]["message"].as_str().or_else(|| v["error"].as_str()).unwrap_or("error in the answer");
        return Err(AiError::Provider(provider.name(), 200, m.to_string()));
    }
    Ok(match provider {
        Provider::Anthropic => if v["type"] == "content_block_delta" { v["delta"]["text"].as_str().unwrap_or("").to_string() } else { String::new() },
        Provider::Gemini => v["candidates"][0]["content"]["parts"].as_array().map(|p| p.iter().filter_map(|x| x["text"].as_str()).collect::<String>()).unwrap_or_default(),
        Provider::Ollama => v["message"]["content"].as_str().unwrap_or("").to_string(),
        _ => v["choices"][0]["delta"]["content"].as_str().unwrap_or("").to_string(),
    })
}

/// [`chat`] with the answer handed to `delta` piece by piece as the provider writes it (Acrobat's
/// and PDF Expert's answers appear as they come). `delta` returning `false` stops reading
/// ([`AiError::Cancelled`]). A server that ignores streaming and answers whole is read whole.
pub fn chat_stream(cfg: &Config, messages: &[Message], delta: &mut dyn FnMut(&str) -> bool) -> Result<String, AiError> {
    use std::io::BufRead;
    let req = stream_request(cfg, messages)?;
    let name = cfg.provider.name();
    let mut r = agent(Duration::from_secs(300)).post(&req.url);
    for (k, v) in &req.headers {
        r = r.header(k, v);
    }
    let mut resp = r.send(req.body.to_string().as_bytes()).map_err(|e| network(name, e))?;
    let status = resp.status().as_u16();
    let retry = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).map(str::to_string);
    let kind = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let streamed = kind.contains("event-stream") || kind.contains("ndjson") || (cfg.provider == Provider::Ollama && kind.is_empty());
    if !(200..300).contains(&status) || !streamed {
        let body = resp.body_mut().read_to_string().map_err(|e| network(name, e))?;
        let text = parse(cfg.provider, status, retry.as_deref(), &body)?;
        if !delta(&text) {
            return Err(AiError::Cancelled);
        }
        return Ok(text);
    }
    let reader = std::io::BufReader::new(resp.body_mut().as_reader());
    let mut all = String::new();
    for line in reader.lines() {
        let line = line.map_err(|e| AiError::Network(name, e.to_string()))?;
        let d = stream_delta(cfg.provider, &line)?;
        if !d.is_empty() {
            all.push_str(&d);
            if !delta(&d) {
                return Err(AiError::Cancelled);
            }
        }
    }
    Ok(all)
}

/// The answer's text from the provider's reply (`status`, `retry_after` header, body).
pub fn parse(provider: Provider, status: u16, retry_after: Option<&str>, body: &str) -> Result<String, AiError> {
    let name = provider.name();
    let v: Option<Value> = serde_json::from_str(body).ok();
    if status == 429 {
        return Err(AiError::RateLimited(name, retry_after.and_then(|s| s.trim().parse().ok())));
    }
    if status == 401 || status == 403 {
        return Err(AiError::BadKey(name, status));
    }
    if !(200..300).contains(&status) {
        let message = v
            .as_ref()
            .and_then(|v| v["error"]["message"].as_str().or_else(|| v["error"].as_str()).or_else(|| v["message"].as_str()).map(str::to_string))
            .unwrap_or_else(|| body.chars().take(300).collect::<String>().trim().to_string());
        return Err(AiError::Provider(name, status, if message.is_empty() { "no details".into() } else { message }));
    }
    let v = v.ok_or(AiError::BadResponse(name))?;
    let text = match provider {
        Provider::Anthropic => v["content"].as_array().map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("")),
        Provider::Gemini => v["candidates"][0]["content"]["parts"].as_array().map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("")),
        Provider::Ollama => v["message"]["content"].as_str().map(str::to_string),
        _ => v["choices"][0]["message"]["content"].as_str().map(str::to_string),
    };
    text.ok_or(AiError::BadResponse(name))
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(10).min(timeout)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn network(name: &'static str, e: ureq::Error) -> AiError {
    let detail = match e {
        ureq::Error::Timeout(_) => "no answer in time".to_string(),
        ureq::Error::HostNotFound => "the server's name is unknown (offline?)".to_string(),
        ureq::Error::ConnectionFailed => "the connection was refused".to_string(),
        ureq::Error::Io(e) => e.to_string(),
        e => e.to_string(),
    };
    AiError::Network(name, detail)
}

/// One chat call: `messages` sent to `cfg`'s provider, its answer back.
pub fn chat(cfg: &Config, messages: &[Message]) -> Result<String, AiError> {
    let req = request(cfg, messages)?;
    let name = cfg.provider.name();
    let mut r = agent(Duration::from_secs(300)).post(&req.url);
    for (k, v) in &req.headers {
        r = r.header(k, v);
    }
    let mut resp = r.send(req.body.to_string().as_bytes()).map_err(|e| network(name, e))?;
    let status = resp.status().as_u16();
    let retry = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).map(str::to_string);
    let body = resp.body_mut().read_to_string().map_err(|e| network(name, e))?;
    parse(cfg.provider, status, retry.as_deref(), &body)
}

/// A local server answering: Ollama at `ollama` (its first model), else llama.cpp at `llamacpp`
/// (its loaded model). Asked briefly, so a missing server costs well under a second.
pub fn detect_local_at(ollama: &str, llamacpp: &str) -> Option<Config> {
    let a = agent(Duration::from_millis(800));
    let get = |url: String| -> Option<Value> {
        let mut r = a.get(&url).call().ok()?;
        if r.status().as_u16() != 200 {
            return None;
        }
        serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()
    };
    if let Some(v) = get(format!("{}/api/tags", ollama.trim_end_matches('/'))) {
        let mut c = Config::new(Provider::Ollama);
        c.base_url = ollama.to_string();
        c.model = v["models"][0]["name"].as_str().unwrap_or_default().to_string();
        return Some(c);
    }
    if let Some(v) = get(format!("{}/models", llamacpp.trim_end_matches('/'))) {
        let mut c = Config::new(Provider::LlamaCpp);
        c.base_url = llamacpp.to_string();
        c.model = v["data"][0]["id"].as_str().unwrap_or_default().to_string();
        return Some(c);
    }
    None
}

/// [`detect_local_at`] at Ollama's and llama.cpp's usual addresses.
pub fn detect_local() -> Option<Config> {
    detect_local_at(Provider::Ollama.default_base(), Provider::LlamaCpp.default_base())
}

/// The setup from the environment, for the CLI: `SHEAF_AI_PROVIDER`, `SHEAF_AI_MODEL`,
/// `SHEAF_AI_BASE_URL`, `SHEAF_AI_KEY` (else the provider's own variable, `OPENAI_API_KEY`…),
/// each overridden by the matching flag. Without a provider, a local server if one answers.
pub fn config_from(provider: Option<&str>, model: Option<&str>, base_url: Option<&str>, key: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Result<Config, String> {
    let named = provider.map(str::to_string).or_else(|| env("SHEAF_AI_PROVIDER"));
    let base = base_url.map(str::to_string).or_else(|| env("SHEAF_AI_BASE_URL"));
    let mut cfg = match named {
        Some(n) => Config::new(Provider::from_id(&n).ok_or_else(|| format!("unknown provider \"{n}\" (use one of: {})", Provider::ALL.map(|p| p.id()).join(", ")))?),
        None => match detect_local() {
            Some(c) if base.is_none() => c,
            _ => return Err(AiError::NotConfigured.to_string().replace("Preferences ▸ AI", "--provider or SHEAF_AI_PROVIDER")),
        },
    };
    if let Some(b) = base {
        cfg.base_url = b;
    }
    if let Some(m) = model.map(str::to_string).or_else(|| env("SHEAF_AI_MODEL")) {
        cfg.model = m;
    }
    cfg.api_key = key.map(str::to_string).or_else(|| env("SHEAF_AI_KEY")).or_else(|| cfg.provider.key_env().and_then(env)).filter(|k| !k.trim().is_empty());
    Ok(cfg)
}

/// An answer or a translation saved at `out`: a PDF when it ends in `.pdf` (Helvetica for Western
/// languages; any other script set in a bundled or system font that draws it, see
/// [`unicode_pdf`]), plain UTF-8 text otherwise.
/// Never overwrites (the path written is returned).
pub fn write_text(title: &str, text: &str, out: &std::path::Path) -> crate::Result<std::path::PathBuf> {
    if out.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        let bytes = if pdfcraft_fonts::first_undrawable(text.replace(['\n', '\r', '\u{c}'], "").as_str()).is_none() {
            crate::pdfcraft::engine::Session::new().create_from_text(title, text).map_err(|e| crate::OpdfError::Engine(e.to_string()))?.to_vec()
        } else {
            unicode_pdf::text_pdf(title, text)?
        };
        return crate::paths::atomic_write(out, |part| Ok(std::fs::write(part, &bytes)?));
    }
    crate::paths::atomic_write(out, |part| Ok(std::fs::write(part, text)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_round_trips_its_id_and_has_a_name() {
        for p in Provider::ALL {
            assert_eq!(Provider::from_id(p.id()), Some(p));
            assert!(!p.name().is_empty());
        }
        assert_eq!(Provider::from_id("Claude"), Some(Provider::Anthropic));
    }

    #[test]
    fn errors_are_plain() {
        assert_eq!(AiError::RateLimited("OpenAI", Some(20)).to_string(), "OpenAI is limiting requests right now. Try again in 20 s.");
        assert_eq!(parse(Provider::OpenAi, 401, None, "{}"), Err(AiError::BadKey("OpenAI", 401)));
        assert_eq!(parse(Provider::Ollama, 404, None, r#"{"error":"model \"x\" not found"}"#), Err(AiError::Provider(Provider::Ollama.name(), 404, "model \"x\" not found".into())));
    }

    #[test]
    fn the_environment_names_the_provider_and_its_own_key() {
        let env = |k: &str| match k {
            "SHEAF_AI_PROVIDER" => Some("anthropic".to_string()),
            "ANTHROPIC_API_KEY" => Some("sk-ant".to_string()),
            _ => None,
        };
        let c = config_from(None, Some("claude-x"), None, None, &env).unwrap();
        assert_eq!((c.provider, c.model.as_str(), c.api_key.as_deref()), (Provider::Anthropic, "claude-x", Some("sk-ant")));
        assert!(config_from(Some("nope"), None, None, None, &env).is_err());
    }
}
