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

/// A space as a file to share (Acrobat shares a space with others): its name and its PDFs' paths
/// relative to the file's folder, so the folder can move, be zipped or sit on a shared drive.
#[derive(Serialize, Deserialize)]
struct Shared {
    sheaf_space: u32,
    name: String,
    files: Vec<String>,
}

/// The extension of a shared space file.
pub const EXTENSION: &str = "sheafspace";

/// `to` relative to the folder `from` ("../docs/a.pdf"); `to` itself on another drive.
pub fn relative(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to_c: Vec<_> = to.components().collect();
    let same = from.iter().zip(&to_c).take_while(|(a, b)| a == b).count();
    if same == 0 {
        return to.to_path_buf();
    }
    let mut out = PathBuf::new();
    for _ in same..from.len() {
        out.push("..");
    }
    for c in &to_c[same..] {
        out.push(c);
    }
    out
}

/// Writes `space` at `out` (never overwrites; the path written is returned).
pub fn export(space: &Space, out: &Path) -> crate::Result<PathBuf> {
    let dir = out.parent().map(|d| std::fs::canonicalize(d).unwrap_or_else(|_| d.to_path_buf())).unwrap_or_default();
    let files = space.files.iter().map(|f| relative(&dir, f).to_string_lossy().replace('\\', "/")).collect();
    let json = serde_json::to_vec_pretty(&Shared { sheaf_space: 1, name: space.name.clone(), files }).map_err(|e| crate::OpdfError::Engine(e.to_string()))?;
    crate::paths::atomic_write(out, |p| Ok(std::fs::write(p, &json)?))
}

/// The space a shared file describes, its paths resolved from the file's folder; the files that
/// are not there are named in the second list (they stay in the space, to be found again).
pub fn import(path: &Path) -> crate::Result<(Space, Vec<String>)> {
    let bad = |e: String| crate::OpdfError::Unsupported(format!("{}: not a Sheaf space ({e})", path.display()));
    let shared: Shared = serde_json::from_slice(&std::fs::read(path)?).map_err(|e| bad(e.to_string()))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut space = Space { name: shared.name, files: Vec::new() };
    let mut missing = Vec::new();
    for f in shared.files {
        let p = dir.join(&f);
        if !p.exists() {
            missing.push(f);
        }
        let p = std::fs::canonicalize(&p).unwrap_or(p);
        if !space.files.contains(&p) {
            space.files.push(p);
        }
    }
    Ok((space, missing))
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

    /// A space exported next to its PDFs keeps working once the whole folder is moved.
    #[test]
    fn a_shared_space_moves_with_its_folder() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("project");
        std::fs::create_dir_all(a.join("docs")).unwrap();
        std::fs::create_dir_all(d.path().join("other")).unwrap();
        for f in [a.join("docs/one.pdf"), d.path().join("other/two.pdf")] {
            std::fs::write(&f, b"%PDF").unwrap();
        }
        let mut s = Space { name: "Contracts".into(), files: Vec::new() };
        add(&mut s, [a.join("docs/one.pdf"), d.path().join("other/two.pdf")]);
        let file = export(&s, &a.join("contracts.sheafspace")).unwrap();
        let json = std::fs::read_to_string(&file).unwrap();
        assert!(json.contains("\"docs/one.pdf\"") && json.contains("\"../other/two.pdf\""), "{json}");
        // The folder moved elsewhere.
        let moved = d.path().join("moved");
        std::fs::create_dir_all(&moved).unwrap();
        std::fs::rename(&a, moved.join("project")).unwrap();
        std::fs::rename(d.path().join("other"), moved.join("other")).unwrap();
        let (back, missing) = import(&moved.join("project/contracts.sheafspace")).unwrap();
        assert_eq!(back.name, "Contracts");
        assert!(missing.is_empty(), "{missing:?}");
        assert!(back.files.iter().all(|f| f.starts_with(std::fs::canonicalize(&moved).unwrap())), "{:?}", back.files);
        assert!(import(&moved.join("project/docs/one.pdf")).is_err());
    }
}
