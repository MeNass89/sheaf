//! What the AI panel and `sheaf ask | summarize | translate` do with a document's text: long texts
//! are cut into chunks that each fit one request, read one by one (map), then combined (reduce),
//! as Acrobat's AI Assistant does with long PDFs. Pages keep their labels ("p. 3"), so answers can
//! cite where they come from.

use super::{chat, AiError, Config, Message};

/// A piece of text and where it comes from ("p. 3", "report.pdf, p. 3", "the selection").
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub label: String,
    pub text: String,
}

/// The pages of one document as sources ("p. 1"…); empty pages left out.
pub fn page_sources(pages: &[String]) -> Vec<Source> {
    pages.iter().enumerate().filter(|(_, t)| !t.trim().is_empty()).map(|(i, t)| Source { label: format!("p. {}", i + 1), text: t.trim().to_string() }).collect()
}

thread_local! {
    static SINK: std::cell::RefCell<Option<Box<dyn FnMut(&str) -> bool>>> = const { std::cell::RefCell::new(None) };
}

/// Runs `f` with the text of its final answers handed to `sink` as it is written (the AI panel
/// shows it growing); the notes of a long document's parts are not streamed, only what the user
/// reads. `sink` returning `false` stops the answer.
pub fn streaming<R>(sink: impl FnMut(&str) -> bool + 'static, f: impl FnOnce() -> R) -> R {
    SINK.with(|s| *s.borrow_mut() = Some(Box::new(sink)));
    let r = f();
    SINK.with(|s| *s.borrow_mut() = None);
    r
}

/// The text the user reads: streamed when a sink is set ([`streaming`]), else in one piece.
fn answer(cfg: &Config, messages: &[Message]) -> Result<String, AiError> {
    let sink = SINK.with(|s| s.borrow_mut().take());
    match sink {
        None => chat(cfg, messages),
        Some(mut sink) => {
            let r = super::chat_stream(cfg, messages, &mut |d| sink(d));
            SINK.with(|s| *s.borrow_mut() = Some(sink));
            r
        }
    }
}

/// Hands text to the sink, if any (between translated parts).
fn emit(text: &str) {
    SINK.with(|s| {
        if let Some(sink) = s.borrow_mut().as_mut() {
            sink(text);
        }
    });
}

/// Told before each request: (request about to be made, 1-based; requests in all). `false` stops.
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize) -> bool;

const SYSTEM: &str = "You are the AI assistant of Sheaf, a PDF app. You work only from the document text you are given, \
which is marked with page labels in square brackets such as [p. 3]. When you use a passage, cite its label in parentheses, \
for example (p. 3). If the text does not contain the answer, say so plainly. Answer in the language of the user's request.";

/// The sources packed into chunks of at most `max` characters, each part headed by its label; a
/// source longer than `max` is split at paragraph, then line, then word boundaries.
pub fn chunks(sources: &[Source], max: usize) -> Vec<String> {
    let max = max.max(200);
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for s in sources {
        for piece in split(&s.text, max.saturating_sub(s.label.len() + 4)) {
            let part = format!("[{}]\n{}\n", s.label, piece);
            if !cur.is_empty() && cur.len() + part.len() + 1 > max {
                out.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push('\n');
            }
            cur.push_str(&part);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn split(text: &str, max: usize) -> Vec<String> {
    let max = max.max(50);
    if text.len() <= max {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut rest = text;
    while rest.len() > max {
        let mut cut = max;
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        let head = &rest[..cut];
        let at = head.rfind("\n\n").or_else(|| head.rfind('\n')).or_else(|| head.rfind(' ')).filter(|&i| i > max / 3).map_or(cut, |i| i + 1);
        out.push(rest[..at].trim_end().to_string());
        rest = rest[at..].trim_start();
    }
    if !rest.is_empty() {
        out.push(rest.to_string());
    }
    out
}

fn step(progress: Progress, at: usize, total: usize) -> Result<(), AiError> {
    if progress(at, total) { Ok(()) } else { Err(AiError::Cancelled) }
}

fn need_text(sources: &[Source]) -> Result<(), AiError> {
    if sources.iter().all(|s| s.text.trim().is_empty()) { Err(AiError::NoText) } else { Ok(()) }
}

/// Map then reduce: each chunk read with `map(i, n, chunk)`, the notes combined by `reduce(notes)`.
/// A text that fits in one request goes straight to `single(chunk)`.
fn map_reduce(cfg: &Config, sources: &[Source], progress: Progress, single: impl Fn(&str) -> Vec<Message>, map: impl Fn(usize, usize, &str) -> Vec<Message>, reduce: impl Fn(&str) -> Vec<Message>) -> Result<String, AiError> {
    need_text(sources)?;
    let parts = chunks(sources, cfg.max_chars);
    if parts.len() == 1 {
        step(progress, 1, 1)?;
        return answer(cfg, &single(&parts[0]));
    }
    let total = parts.len() + 1;
    let mut notes = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        step(progress, i + 1, total)?;
        let n = chat(cfg, &map(i + 1, parts.len(), part))?;
        if !n.trim().eq_ignore_ascii_case("none") && !n.trim().is_empty() {
            notes.push(format!("Notes from part {} of {}:\n{}", i + 1, parts.len(), n.trim()));
        }
    }
    step(progress, total, total)?;
    // The notes themselves may be long: reduced in turn, never cut.
    let mut joined = notes.join("\n\n");
    while joined.len() > cfg.max_chars {
        let groups = chunks(&[Source { label: "notes".into(), text: joined.clone() }], cfg.max_chars);
        if groups.len() <= 1 {
            break;
        }
        let mut shorter = Vec::new();
        for g in groups {
            shorter.push(chat(cfg, &[Message::system(SYSTEM), Message::user(format!("Condense these notes, keeping every fact and page label:\n<<<\n{g}\n>>>"))])?);
        }
        joined = shorter.join("\n\n");
    }
    answer(cfg, &reduce(&joined))
}

/// A question about the document (`sources`), or about `selection` when there is one, with the
/// earlier turns of the conversation (`history`: question, answer).
pub fn ask(cfg: &Config, sources: &[Source], selection: Option<&str>, question: &str, history: &[(String, String)], progress: Progress) -> Result<String, AiError> {
    let earlier = |mut m: Vec<Message>| {
        let last = m.pop();
        for (q, a) in history.iter().rev().take(6).rev() {
            m.push(Message::user(q.clone()));
            m.push(Message::assistant(a.clone()));
        }
        m.extend(last);
        m
    };
    if let Some(sel) = selection.map(str::trim).filter(|s| !s.is_empty()) {
        step(progress, 1, 1)?;
        let sel: String = sel.chars().take(cfg.max_chars).collect();
        return answer(cfg, &earlier(vec![Message::system(SYSTEM), Message::user(format!("The user selected this passage of the document:\n<<<\n{sel}\n>>>\n\n{question}"))]));
    }
    map_reduce(
        cfg,
        sources,
        progress,
        |doc| earlier(vec![Message::system(SYSTEM), Message::user(format!("Document text:\n<<<\n{doc}\n>>>\n\nQuestion: {question}"))]),
        |i, n, part| vec![Message::system(SYSTEM), Message::user(format!("This is part {i} of {n} of a long document:\n<<<\n{part}\n>>>\n\nWrite down everything in this part that helps answer the question below, with its page labels. If nothing does, reply exactly NONE.\n\nQuestion: {question}"))],
        |notes| earlier(vec![Message::system(SYSTEM), Message::user(format!("Notes taken from every part of a long document:\n<<<\n{notes}\n>>>\n\nAnswer from these notes, citing pages. If they do not contain the answer, say so.\n\nQuestion: {question}"))]),
    )
}

/// A summary's form: paragraphs (PDF Expert's Summarize) or a list (Acrobat's key points).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Summary {
    Paragraphs,
    MainPoints,
}

/// A summary of `sources` (the whole document, some pages or a selection).
pub fn summarize(cfg: &Config, sources: &[Source], form: Summary, progress: Progress) -> Result<String, AiError> {
    let ask = match form {
        Summary::Paragraphs => "Summarize this text in a few short paragraphs: what it is, its key content and its conclusions. Cite pages.",
        Summary::MainPoints => "List the main points of this text, one per line starting with \"- \", most important first, each followed by its page in parentheses. Then stop.",
    };
    map_reduce(
        cfg,
        sources,
        progress,
        |doc| vec![Message::system(SYSTEM), Message::user(format!("{ask}\n<<<\n{doc}\n>>>"))],
        |i, n, part| vec![Message::system(SYSTEM), Message::user(format!("This is part {i} of {n} of a long document. Note its key facts and points as short lines, with page labels:\n<<<\n{part}\n>>>"))],
        |notes| vec![Message::system(SYSTEM), Message::user(format!("These are notes on every part of a long document, in order:\n<<<\n{notes}\n>>>\n\n{ask}"))],
    )
}

/// The languages offered for translation (any other can be typed on the command line).
pub const LANGUAGES: [&str; 14] = ["English", "French", "Spanish", "German", "Italian", "Portuguese", "Dutch", "Polish", "Russian", "Arabic", "Chinese (Simplified)", "Japanese", "Korean", "Turkish"];

/// `sources` translated into `language`, part by part, in order (nothing is reduced: every word
/// is translated). Each part keeps its page label as a heading.
pub fn translate(cfg: &Config, sources: &[Source], language: &str, progress: Progress) -> Result<String, AiError> {
    need_text(sources)?;
    let parts = chunks(sources, cfg.max_chars);
    let mut out = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        step(progress, i + 1, parts.len())?;
        if i > 0 {
            emit("\n\n");
        }
        out.push(answer(cfg, &[Message::system("You translate documents faithfully. Keep paragraphs, line breaks, lists and the labels in square brackets such as [p. 3] exactly where they are. Reply with the translation only, no comments."), Message::user(format!("Translate into {language}:\n<<<\n{part}\n>>>"))])?.trim().to_string());
    }
    Ok(out.join("\n\n"))
}

/// Acrobat's Generate podcast, as a script: two hosts talk the document through.
pub fn podcast(cfg: &Config, sources: &[Source], progress: Progress) -> Result<String, AiError> {
    let ask = "Write the script of a podcast episode of about five minutes in which two hosts, Alex and Sam, explain this document to a listener who has not read it: an opening, its main ideas with concrete details, and a closing. Write one line per turn, as \"Alex: …\" or \"Sam: …\", nothing else.";
    map_reduce(
        cfg,
        sources,
        progress,
        |doc| vec![Message::system(SYSTEM), Message::user(format!("{ask}\n<<<\n{doc}\n>>>"))],
        |i, n, part| vec![Message::system(SYSTEM), Message::user(format!("This is part {i} of {n} of a long document. Note its key ideas, facts and examples as short lines:\n<<<\n{part}\n>>>"))],
        |notes| vec![Message::system(SYSTEM), Message::user(format!("Notes on every part of a document:\n<<<\n{notes}\n>>>\n\n{ask}"))],
    )
}

/// Acrobat's Generate presentation: an outline of slides, as `# Title` lines each followed by
/// `- ` bullets (read by [`super::deck::parse`]).
pub fn slides(cfg: &Config, sources: &[Source], progress: Progress) -> Result<String, AiError> {
    let ask = "Make a slide deck of 6 to 10 slides presenting this document. Use exactly this format and nothing else: each slide starts with a line \"# \" and its title, followed by 3 to 5 lines starting with \"- \", each a short point (at most 15 words). The first slide is the title slide.";
    map_reduce(
        cfg,
        sources,
        progress,
        |doc| vec![Message::system(SYSTEM), Message::user(format!("{ask}\n<<<\n{doc}\n>>>"))],
        |i, n, part| vec![Message::system(SYSTEM), Message::user(format!("This is part {i} of {n} of a long document. Note its key points as short lines:\n<<<\n{part}\n>>>"))],
        |notes| vec![Message::system(SYSTEM), Message::user(format!("Notes on every part of a document:\n<<<\n{notes}\n>>>\n\n{ask}"))],
    )
}

/// A document to sort: its name and the start of its text.
#[derive(Debug, Clone, PartialEq)]
pub struct ToSort {
    pub name: String,
    pub text: String,
}

/// Stirling's Classify: each document given one of `categories` ("Other" when none fits), or,
/// with no categories, a short category the model names. One request per document.
pub fn classify(cfg: &Config, docs: &[ToSort], categories: &[String], progress: Progress) -> Result<Vec<String>, AiError> {
    let cats: Vec<&str> = categories.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
    let mut out = Vec::new();
    for (i, d) in docs.iter().enumerate() {
        step(progress, i + 1, docs.len())?;
        let excerpt: String = d.text.chars().take(cfg.max_chars.min(6_000)).collect();
        if excerpt.trim().is_empty() {
            out.push(if cats.is_empty() { "No text".to_string() } else { "Other".to_string() });
            continue;
        }
        let ask = if cats.is_empty() {
            "Reply with one short category (one to three words, for example Invoice, Contract, Research paper, Letter) for this document, and nothing else.".to_string()
        } else {
            format!("Choose the one category that best fits this document among: {}. If none fits, reply Other. Reply with the category's name only.", cats.join(", "))
        };
        let a = chat(cfg, &[Message::system("You sort documents into categories."), Message::user(format!("{ask}\n\nDocument \"{}\":\n<<<\n{excerpt}\n>>>", d.name))])?;
        out.push(category(&a, &cats));
    }
    Ok(out)
}

/// The category named in an answer: one of `cats` (whole name first, then contained), else
/// "Other"; with no categories, the answer's first line, cleaned.
pub fn category(answer: &str, cats: &[&str]) -> String {
    let line = answer.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let clean = line.trim_matches(|c: char| !c.is_alphanumeric() && c != ')' && c != '(').trim();
    let clean = clean.strip_prefix("Category:").map(str::trim).unwrap_or(clean);
    if cats.is_empty() {
        return if clean.is_empty() { "Other".into() } else { clean.chars().take(40).collect() };
    }
    let low = clean.to_lowercase();
    cats.iter()
        .find(|c| c.to_lowercase() == low)
        .or_else(|| {
            // The longest name contained wins ("Tax invoice" over "Invoice").
            let mut hits: Vec<&&str> = cats.iter().filter(|c| answer.to_lowercase().contains(&c.to_lowercase())).collect();
            hits.sort_by_key(|c| std::cmp::Reverse(c.len()));
            hits.first().copied()
        })
        .map_or_else(|| "Other".to_string(), |c| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_keep_labels_and_stay_under_the_limit() {
        let pages: Vec<String> = (0..10).map(|i| format!("Page {i} ").repeat(60)).collect();
        let parts = chunks(&page_sources(&pages), 1_000);
        assert!(parts.len() > 3);
        assert!(parts.iter().all(|p| p.len() <= 1_000), "{:?}", parts.iter().map(String::len).collect::<Vec<_>>());
        assert!(parts[0].starts_with("[p. 1]\n"));
        let joined = parts.join(" ");
        for i in 1..=10 {
            assert!(joined.contains(&format!("[p. {i}]")));
        }
        // Every word is kept.
        assert_eq!(joined.matches("Page 9").count(), 60);
    }

    #[test]
    fn a_short_text_is_one_chunk_and_an_empty_one_has_none() {
        assert_eq!(chunks(&page_sources(&["Hello".into(), " ".into(), "World".into()]), 24_000), vec!["[p. 1]\nHello\n\n[p. 3]\nWorld\n".to_string()]);
        assert!(chunks(&page_sources(&["  ".into()]), 1000).is_empty());
    }

    #[test]
    fn answers_are_matched_to_the_categories() {
        let cats = ["Invoice", "Tax invoice", "Contract"];
        assert_eq!(category("Contract.", &cats), "Contract");
        assert_eq!(category("\"invoice\"", &cats), "Invoice");
        assert_eq!(category("This is a tax invoice from ACME", &cats), "Tax invoice");
        assert_eq!(category("Recipe", &cats), "Other");
        assert_eq!(category("Category: Research paper\n", &[]), "Research paper");
    }
}
