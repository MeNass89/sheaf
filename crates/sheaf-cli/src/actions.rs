//! `sheaf actions`: the PDF actions a script or the system's automation can run, named as
//! Preview's Shortcuts actions name them (Rotate Pages, Insert Pages, Delete Pages, Get Pages,
//! Add Bookmark, Search, Save…), each with the command that does it. It is the discoverable list
//! on Linux and Windows, where there is no Shortcuts app; the MCP server offers the same commands
//! as tools.

use serde_json::{json, Value};

/// An action: its name, what it does, and the command line that runs it (the subcommand first).
pub(crate) struct Action {
    pub name: &'static str,
    pub summary: &'static str,
    pub command: &'static str,
}

const fn a(name: &'static str, summary: &'static str, command: &'static str) -> Action {
    Action { name, summary, command }
}

/// Every action, in Preview's order first, then Sheaf's others.
pub(crate) const ACTIONS: &[Action] = &[
    a("Rotate Pages", "Turn chosen pages by 90, 180 or 270 degrees.", "rotate IN.pdf --degrees 90 --pages 1-3 -o OUT.pdf"),
    a("Insert Pages", "Put a blank page or another PDF's pages at a place (0 is a blank page).", "organize IN.pdf --order 1,0,2 -o OUT.pdf"),
    a("Delete Pages", "Remove chosen pages.", "remove IN.pdf --pages 2,4-6 -o OUT.pdf"),
    a("Get Pages", "Take chosen pages out into a new PDF.", "extract IN.pdf --ranges 1-3 -o OUT.pdf"),
    a("Get Details", "Pages and their sizes, metadata, forms, bookmarks, as JSON.", "info IN.pdf"),
    a("Get Text", "The text of chosen pages.", "text IN.pdf --pages 1-3"),
    a("Search", "The pages where a word or phrase is, how often, and where on the page's text.", "search IN.pdf --query WORDS"),
    a("Add Bookmark", "A bookmark opening a page, after the last one.", "bookmark IN.pdf --title TITLE --page 3 -o OUT.pdf"),
    a("Combine", "Several PDFs into one, in order.", "merge A.pdf B.pdf -o OUT.pdf"),
    a("Split", "One PDF into several, by ranges or every N pages.", "split IN.pdf --every 1 -o FOLDER"),
    a("Save as Images", "Every page as a JPG or PNG file.", "to-jpg IN.pdf -o FOLDER"),
    a("Make Smaller", "Compress images and the file.", "compress IN.pdf -o OUT.pdf"),
    a("Encrypt", "A password to open the PDF.", "protect IN.pdf --user-password PASSWORD -o OUT.pdf"),
    a("Recognize Text", "Make a scan searchable (OCR).", "ocr IN.pdf -o OUT.pdf"),
    a("Render Page", "One page as a PNG, to look at it.", "render IN.pdf --page 1 -o PAGE.png"),
];

/// The list as JSON: `{"actions": [{"name", "summary", "command"}]}`, the command with `sheaf`.
pub(crate) fn json() -> Value {
    json!({ "actions": ACTIONS.iter().map(|a| json!({ "name": a.name, "summary": a.summary, "command": format!("sheaf {}", a.command) })).collect::<Vec<_>>() })
}

/// The pages of `texts` where `query` is (case ignored): 1-based page, how many times, and the
/// text around the first one.
pub(crate) fn search(texts: &[String], query: &str) -> Value {
    let q = query.trim().to_lowercase();
    let mut pages = Vec::new();
    if !q.is_empty() {
        for (k, text) in texts.iter().enumerate() {
            let lower = text.to_lowercase();
            let count = lower.matches(&q).count();
            if count == 0 {
                continue;
            }
            // The context in the original text: the same characters as the lower-cased match.
            let at = lower.find(&q).unwrap_or(0);
            let chars: Vec<char> = text.chars().collect();
            let char_at = lower[..at].chars().count();
            let from = char_at.saturating_sub(30);
            let to = (char_at + q.chars().count() + 30).min(chars.len());
            let context: String = chars[from..to].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
            pages.push(json!({ "page": k + 1, "count": count, "context": context }));
        }
    }
    json!({ "query": query, "pages": pages })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn every_action_runs_a_command_that_exists() {
        let cli = crate::args::Cli::command();
        let names: Vec<&str> = cli.get_subcommands().map(|c| c.get_name()).collect();
        for a in ACTIONS {
            let cmd = a.command.split_whitespace().next().unwrap();
            assert!(names.contains(&cmd), "{}: no command {cmd}", a.name);
        }
        assert_eq!(json()["actions"][0]["command"], "sheaf rotate IN.pdf --degrees 90 --pages 1-3 -o OUT.pdf");
    }

    #[test]
    fn search_finds_pages_counts_and_context() {
        let texts = vec!["Nothing here".to_string(), "The Quick fox, the quick dog".to_string(), "quick".to_string()];
        let v = search(&texts, "quick");
        let pages = v["pages"].as_array().unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!((pages[0]["page"].as_u64(), pages[0]["count"].as_u64()), (Some(2), Some(2)));
        assert!(pages[0]["context"].as_str().unwrap().contains("Quick fox"));
        assert_eq!(pages[1]["page"], 3);
        assert!(search(&texts, "  ")["pages"].as_array().unwrap().is_empty());
    }
}
