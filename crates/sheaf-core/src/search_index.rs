//! A reusable full-text catalog alongside a collection of PDFs (Acrobat's Catalog index).
//! The catalog holds normalized page text and word postings. Source fingerprints prevent a
//! search from returning old text after a PDF is replaced, edited or deleted.

use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Serialize, Deserialize)]
struct Source {
    path: PathBuf,
    fingerprint: u64,
    pages: Vec<String>,
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
}

fn normalized(text: &str) -> String {
    text.replace("-\r\n", "").replace("-\n", "").nfkc().collect::<String>().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty())
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
        if inputs.is_empty() { return Err(OpdfError::Unsupported("Choose at least one PDF to index".into())); }
        let mut sources = Vec::new();
        let mut seen = BTreeSet::new();
        for (i, input) in inputs.iter().enumerate() {
            if !progress(i, inputs.len()) { return Err(OpdfError::Cancelled); }
            if input.is_encrypted() { return Err(OpdfError::Unsupported(format!("{}: protected text cannot be saved in a search catalog", input.path.display()))); }
            let path = std::fs::canonicalize(&input.path)?;
            if !seen.insert(path.clone()) { continue; }
            let before = fingerprint(&path)?;
            let pages = crate::text::page_texts(input, &[])?;
            if fingerprint(&path)? != before { return Err(OpdfError::Unsupported(format!("{} changed while indexing; build again", path.display()))); }
            sources.push(Source { path, fingerprint: before, pages });
        }
        progress(inputs.len(), inputs.len());
        Ok(Self::from_sources(sources))
    }

    fn from_sources(sources: Vec<Source>) -> Self {
        let mut postings: BTreeMap<String, BTreeSet<(usize, usize)>> = BTreeMap::new();
        for (file, source) in sources.iter().enumerate() {
            for (page, text) in source.pages.iter().enumerate() {
                for word in tokens(&normalized(text)) { postings.entry(word.into()).or_default().insert((file, page)); }
            }
        }
        Self { version: 1, sources, postings }
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
        Ok(Self::from_sources(catalog.sources))
    }

    /// All sources must still match before returning results; stale catalogs ask for a rebuild.
    pub fn validate(&self) -> Result<()> {
        for source in &self.sources {
            if fingerprint(&source.path)? != source.fingerprint { return Err(OpdfError::Unsupported(format!("{} changed; rebuild the search catalog", source.path.display()))); }
        }
        Ok(())
    }

    /// Cached text for a source that is still byte-for-byte identical. A folder search uses it
    /// when available and reads other or changed PDFs normally.
    pub fn texts(&self, path: &Path) -> Option<&[String]> {
        let path = std::fs::canonicalize(path).ok()?;
        let source = self.sources.iter().find(|s| s.path == path)?;
        (fingerprint(&path).ok()? == source.fingerprint).then_some(source.pages.as_slice())
    }

    /// Find a phrase across line ends, ignoring case and Unicode compatibility differences.
    pub fn search(&self, query: &str) -> Result<Vec<Hit>> {
        let query = normalized(query);
        if query.is_empty() { return Err(OpdfError::Unsupported("Type the words to find".into())); }
        self.validate()?;
        let words: Vec<&str> = tokens(&query).collect();
        let mut candidates: Option<BTreeSet<(usize, usize)>> = None;
        for word in words {
            // Partial CJK phrases may not have a whitespace boundary in the source.
            let pages: BTreeSet<_> = self.postings.iter().filter(|(token, _)| token.contains(word)).flat_map(|(_, pages)| pages.iter().copied()).collect();
            candidates = Some(match candidates { None => pages, Some(current) => current.intersection(&pages).copied().collect() });
        }
        let candidates = candidates.unwrap_or_else(|| self.sources.iter().enumerate().flat_map(|(file, source)| (0..source.pages.len()).map(move |page| (file, page))).collect());
        let mut out = Vec::new();
        for (file, page) in candidates {
            let source = &self.sources[file];
            let text = normalized(&source.pages[page]);
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
    fn a_saved_catalog_finds_unicode_phrases_and_refuses_changed_sources() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("source.pdf");
        std::fs::write(&pdf, b"source bytes").unwrap();
        let catalog = Catalog::from_sources(vec![Source { fingerprint: fingerprint(&pdf).unwrap(), path: pdf.clone(), pages: vec!["ÉTAT du\ncontrat 日本語の文書".into(), "Another page".into()] }]);
        let saved = catalog.save(&dir.path().join("catalog.json")).unwrap();
        let catalog = Catalog::load(&saved).unwrap();
        assert_eq!(catalog.search("état du contrat").unwrap()[0].page, 0);
        assert_eq!(catalog.search("日本語").unwrap().len(), 1);
        assert!(catalog.search("missing").unwrap().is_empty());
        std::fs::write(&pdf, b"edited bytes").unwrap();
        assert!(catalog.search("contrat").unwrap_err().to_string().contains("rebuild"));
        assert!(catalog.texts(&pdf).is_none());
    }
}
