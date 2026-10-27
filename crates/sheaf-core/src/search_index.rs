//! A reusable full-text catalog alongside a collection of PDFs (Acrobat's Catalog index).
//! The catalog holds normalized page text and word postings. Source fingerprints prevent a
//! search from returning old text after a PDF is replaced, edited or deleted.

use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Source {
    path: PathBuf,
    fingerprint: u64,
    pages: Vec<String>,
    #[serde(default)]
    stamp: Option<(u64, u128)>,
    #[serde(skip)]
    normalized: Vec<String>,
}

/// A page found by a catalog search. Page numbers are 0-based, as in the PDF engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hit {
    pub path: PathBuf,
    pub page: usize,
    pub snippet: String,
}

/// Versioned, portable JSON data; never contains PDF passwords. Protected inputs are refused
/// rather than leaving their extracted text in an unencrypted catalog.
#[derive(Debug, Serialize, Deserialize)]
pub struct Catalog {
    version: u32,
    sources: Vec<Source>,
    postings: BTreeMap<String, BTreeSet<(usize, usize)>>,
    #[serde(skip)]
    grams: BTreeMap<String, BTreeSet<(usize, usize)>>,
}

fn normalized(text: &str) -> String {
    text.replace("-\r\n", "").replace("-\n", "").nfkc().collect::<String>().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn stamp(path: &Path) -> Option<(u64, u128)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos()))
}

fn grams(text: &str) -> BTreeSet<String> {
    text.chars().collect::<Vec<_>>().windows(3).map(|g| g.iter().collect()).collect()
}

/// Recursively collect local PDFs. Symlinked directories are skipped to avoid cycles.
/// Missing inputs are errors; no source is silently dropped during a rebuild.
pub fn collect_inputs(inputs: &[Input]) -> Result<Vec<Input>> {
    fn visit(input: &Input, out: &mut Vec<Input>) -> Result<()> {
        if !input.path.is_dir() { out.push(input.clone()); return Ok(()); }
        let mut entries = std::fs::read_dir(&input.path)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let kind = entry.file_type()?;
            if kind.is_symlink() { continue; }
            let path = entry.path();
            if kind.is_dir() || path.extension().is_some_and(|s| s.eq_ignore_ascii_case("pdf")) {
                visit(&Input::new(path), out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    for input in inputs { visit(input, &mut out)?; }
    Ok(out)
}

fn fingerprint(path: &Path) -> Result<u64> {
    let mut file = std::fs::File::open(path)?;
    let mut bytes = [0u8; 16_384];
    let mut hash = 0xcbf29ce484222325u64;
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 { break; }
        for b in &bytes[..n] { hash = (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3); }
    }
    Ok(hash)
}

impl Catalog {
    /// Build or rebuild the catalog from these files. No partial catalog is returned on a
    /// failed source; the caller sees which PDF needs a password or could not be read.
    pub fn build(inputs: &[Input], progress: &mut dyn FnMut(usize, usize) -> bool) -> Result<Self> {
        Self::rebuild(None, inputs, progress)
    }

    /// Acrobat's incremental Catalog update: reuse unchanged pages, reindex changed PDFs,
    /// add new files and purge sources omitted from the selection. Failure keeps `self` intact.
    pub fn update(&self, inputs: &[Input], progress: &mut dyn FnMut(usize, usize) -> bool) -> Result<Self> {
        Self::rebuild(Some(self), inputs, progress)
    }

    fn rebuild(previous: Option<&Self>, inputs: &[Input], progress: &mut dyn FnMut(usize, usize) -> bool) -> Result<Self> {
        let inputs = collect_inputs(inputs)?;
        if inputs.is_empty() { return Err(OpdfError::Unsupported("Choose at least one PDF to index".into())); }
        let mut sources = Vec::new();
        let mut seen = BTreeSet::new();
        for (i, input) in inputs.iter().enumerate() {
            if !progress(i, inputs.len()) { return Err(OpdfError::Cancelled); }
            let path = std::fs::canonicalize(&input.path)?;
            if !seen.insert(path.clone()) { continue; }
            if let Some(old) = previous.and_then(|c| c.sources.iter().find(|s| s.path == path))
                && old.stamp.is_some() && stamp(&path) == old.stamp {
                sources.push(old.clone());
                continue;
            }
            if input.is_encrypted() { return Err(OpdfError::Unsupported(format!("{}: protected text cannot be saved in a search catalog", input.path.display()))); }
            let before_stamp = stamp(&path);
            let before = fingerprint(&path)?;
            let pages = crate::text::page_texts(input, &[])?;
            if fingerprint(&path)? != before || stamp(&path) != before_stamp { return Err(OpdfError::Unsupported(format!("{} changed while indexing; build again", path.display()))); }
            sources.push(Source { path, fingerprint: before, pages, stamp: before_stamp, normalized: Vec::new() });
        }
        if !progress(inputs.len(), inputs.len()) { return Err(OpdfError::Cancelled); }
        Ok(Self::from_sources(sources))
    }

    fn from_sources(mut sources: Vec<Source>) -> Self {
        let mut index: BTreeMap<String, BTreeSet<(usize, usize)>> = BTreeMap::new();
        for (file, source) in sources.iter_mut().enumerate() {
            source.normalized = source.pages.iter().map(|s| normalized(s)).collect();
            for (page, text) in source.normalized.iter().enumerate() {
                for gram in grams(text) { index.entry(gram).or_default().insert((file, page)); }
            }
        }
        // The v1 postings field remains readable; only page text is authoritative.
        Self { version: 1, sources, postings: BTreeMap::new(), grams: index }
    }

    /// Saved sources and their page counts for Acrobat's Catalog manager.
    pub fn sources(&self) -> Vec<(PathBuf, usize)> {
        self.sources.iter().map(|s| (s.path.clone(), s.pages.len())).collect()
    }

    /// Write a catalog, never overwriting an existing one. Rebuild to a new name, then replace
    /// the old catalog explicitly if desired; a failed build cannot damage the previous one.
    pub fn save(&self, out: &Path) -> Result<PathBuf> {
        let bytes = serde_json::to_vec(self).map_err(|e| OpdfError::Engine(e.to_string()))?;
        atomic_write(out, |part| { std::fs::write(part, &bytes)?; Ok(()) })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let catalog: Self = serde_json::from_slice(&std::fs::read(path)?).map_err(|e| OpdfError::Unsupported(format!("Invalid search catalog: {e}")))?;
        if catalog.version != 1 { return Err(OpdfError::Unsupported("Unsupported search catalog version".into())); }
        // Recompute postings from the stored pages so corrupt or edited posting offsets cannot
        // panic the reader or introduce false matches.
        let mut sources = catalog.sources;
        // Old v1 catalogs have no metadata stamp. Verify their bytes once on load, never
        // on every query; a changed legacy source remains stale until updated.
        for source in &mut sources {
            if source.stamp.is_none() && fingerprint(&source.path).ok() == Some(source.fingerprint) {
                source.stamp = stamp(&source.path);
            }
        }
        Ok(Self::from_sources(sources))
    }

    /// Explicit full byte audit, separate from ordinary metadata-checked searches.
    pub fn validate(&self) -> Result<()> {
        for source in &self.sources {
            if fingerprint(&source.path)? != source.fingerprint { return Err(OpdfError::Unsupported(format!("{} changed; rebuild the search catalog", source.path.display()))); }
        }
        Ok(())
    }

    /// Changed/missing sources are excluded individually, without blocking other PDFs.
    /// Normal searches check file metadata only; `validate` explicitly audits every byte.
    /// Files changed while preserving both size and modification time need that full audit.
    pub fn stale_sources(&self) -> Vec<PathBuf> {
        self.sources.iter().filter(|s| s.stamp.is_none() || stamp(&s.path) != s.stamp).map(|s| s.path.clone()).collect()
    }

    /// Cached text for a source whose size and modification time still match. A folder search uses it
    /// when available and reads other or changed PDFs normally.
    pub fn texts(&self, path: &Path) -> Option<&[String]> {
        let path = std::fs::canonicalize(path).ok()?;
        let source = self.sources.iter().find(|s| s.path == path)?;
        (source.stamp.is_some() && stamp(&path) == source.stamp).then_some(source.pages.as_slice())
    }

    /// Find a phrase across line ends, ignoring case and Unicode compatibility differences.
    pub fn search(&self, query: &str) -> Result<Vec<Hit>> {
        let query = normalized(query);
        if query.is_empty() { return Err(OpdfError::Unsupported("Type the words to find".into())); }
        let fresh: Vec<bool> = self.sources.iter().map(|s| s.stamp.is_some() && stamp(&s.path) == s.stamp).collect();
        let query_grams = grams(&query);
        let mut postings = Vec::new();
        for gram in &query_grams {
            let Some(pages) = self.grams.get(gram) else { return Ok(Vec::new()); };
            postings.push(pages);
        }
        postings.sort_by_key(|pages| pages.len());
        let candidates: BTreeSet<_> = if let Some(first) = postings.first() {
            first.iter().copied().filter(|p| postings.iter().skip(1).all(|pages| pages.contains(p))).collect()
        } else {
            self.sources.iter().enumerate().flat_map(|(file, source)| (0..source.pages.len()).map(move |page| (file, page))).collect()
        };
        let mut out = Vec::new();
        for (file, page) in candidates {
            if !fresh[file] { continue; }
            let source = &self.sources[file];
            let text = &source.normalized[page];
            if let Some(at) = text.find(&query) {
                let before = text[..at].chars().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<String>();
                let after = text[at..].chars().take(100).collect::<String>();
                out.push(Hit { path: source.path.clone(), page, snippet: before + &after });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_catalog_finds_unicode_phrases_and_excludes_changed_sources() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("source.pdf");
        std::fs::write(&pdf, b"source bytes").unwrap();
        let catalog = Catalog::from_sources(vec![Source { fingerprint: fingerprint(&pdf).unwrap(), path: pdf.clone(), stamp: stamp(&pdf), normalized: Vec::new(), pages: vec!["ÉTAT du\ncontrat 日本語の文書".into(), "Another page".into()] }]);
        let saved = catalog.save(&dir.path().join("catalog.json")).unwrap();
        let catalog = Catalog::load(&saved).unwrap();
        assert_eq!(catalog.search("état du contrat").unwrap()[0].page, 0);
        assert_eq!(catalog.search("日本語").unwrap().len(), 1);
        assert!(catalog.search("missing").unwrap().is_empty());
        std::fs::write(&pdf, b"edited source bytes").unwrap();
        assert!(catalog.search("contrat").unwrap().is_empty());
        assert_eq!(catalog.stale_sources(), std::slice::from_ref(&pdf));
        assert!(catalog.validate().is_err());
        assert!(catalog.texts(&pdf).is_none());
    }
    #[test]
    fn indexed_substrings_and_incremental_reuse_do_not_require_a_readable_pdf_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-real-pdf.pdf");
        std::fs::write(&path, b"not parsed during search or unchanged updates").unwrap();
        let catalog = Catalog::from_sources(vec![Source {
            fingerprint: fingerprint(&path).unwrap(), stamp: stamp(&path), path: path.clone(),
            pages: vec!["Contract 日本語の文書 punctuation: (foo-bar)".into()], normalized: Vec::new(),
        }]);
        for query in ["tract", "日本語", "語の", "(foo-bar)", "CONTRACT"] {
            assert_eq!(catalog.search(query).unwrap().len(), 1, "{query}");
        }
        let updated = catalog.update(&[Input::new(&path)], &mut |_, _| true).unwrap();
        assert_eq!(updated.search("contract").unwrap().len(), 1);
        assert!(updated.search("unrelated").unwrap().is_empty());
    }

}
