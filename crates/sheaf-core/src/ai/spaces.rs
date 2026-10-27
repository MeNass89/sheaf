//! Acrobat's AI spaces (PDF Expert's multi-file chat): named groups of PDFs asked and summarized
//! together. A space is a list of files kept in `ai-spaces.json` in the app's data folder; the
//! files stay where they are.

use super::tasks::Source;
use crate::pages::Input;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Space {
    pub name: String,
    pub files: Vec<PathBuf>,
}

const FILE: &str = "ai-spaces.json";

/// The spaces saved in `dir` (none when missing or unreadable).
pub fn load(dir: &Path) -> Vec<Space> {
    std::fs::read(dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Saves `spaces` in `dir`, through a temp file.
pub fn save(dir: &Path, spaces: &[Space]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(spaces).map_err(std::io::Error::other)?;
    let tmp = tempfile::NamedTempFile::new_in(dir)?;
    std::fs::write(tmp.path(), json)?;
    tmp.persist(dir.join(FILE)).map_err(|e| e.error)?;
    Ok(())
}

/// `files` added to `space` once each (absolute paths).
pub fn add(space: &mut Space, files: impl IntoIterator<Item = PathBuf>) {
    for f in files {
        let f = std::fs::canonicalize(&f).unwrap_or(f);
        if !space.files.contains(&f) {
            space.files.push(f);
        }
    }
}

/// A name not used yet: `base`, then "base 2"…
pub fn fresh_name(spaces: &[Space], base: &str) -> String {
    let base = if base.trim().is_empty() { "Space" } else { base.trim() };
    (1..).map(|i| if i == 1 { base.to_string() } else { format!("{base} {i}") }).find(|n| !spaces.iter().any(|s| &s.name == n)).unwrap_or_default()
}

/// Every page of every file of `files` as sources labelled "name.pdf, p. 3". A file that cannot
/// be read (moved, protected) is reported in the second list rather than stopping the others.
pub fn sources(files: &[PathBuf]) -> (Vec<Source>, Vec<String>) {
    let mut out = Vec::new();
    let mut failed = Vec::new();
    for f in files {
        let name = f.file_name().unwrap_or_default().to_string_lossy().to_string();
        match crate::text::page_texts(&Input::new(f), &[]) {
            Ok(pages) => out.extend(pages.iter().enumerate().filter(|(_, t)| !t.trim().is_empty()).map(|(i, t)| Source { label: format!("{name}, p. {}", i + 1), text: t.trim().to_string() })),
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    (out, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_are_saved_and_named_once() {
        let d = tempfile::tempdir().unwrap();
        let mut s = Space { name: fresh_name(&[], " "), files: Vec::new() };
        add(&mut s, [d.path().join("a.pdf"), d.path().join("a.pdf"), d.path().join("b.pdf")]);
        assert_eq!(s.files.len(), 2);
        save(d.path(), std::slice::from_ref(&s)).unwrap();
        assert_eq!(load(d.path()), vec![s.clone()]);
        assert_eq!(fresh_name(&[s], "Space"), "Space 2");
    }
}
