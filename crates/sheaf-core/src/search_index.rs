//! A reusable full-text catalog alongside a collection of PDFs (Acrobat's Catalog index).
//! The catalog holds normalized page text and word postings. Source fingerprints prevent a
//! search from returning old text after a PDF is replaced, edited or deleted.

mod query;
pub use query::{SearchMode, SearchOptions};

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
    grams: query::Index,
    #[serde(skip)]
    words: query::Index,
    #[serde(skip)]
    stems: query::Index,
    #[serde(skip)]
    pages: query::Pages,
}

fn canonical(text: &str) -> String {
    text.replace("-\r\n", "").replace("-\n", "").nfkc().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalized(text: &str) -> String { canonical(text).to_lowercase() }

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
        let mut index: query::Index = BTreeMap::new();
        let mut words: query::Index = BTreeMap::new();
        let mut stems: query::Index = BTreeMap::new();
        let stemmer = rust_stemmers::Stemmer::create(rust_stemmers::Algorithm::English);
        for (file, source) in sources.iter_mut().enumerate() {
            source.normalized = source.pages.iter().map(|s| normalized(s)).collect();
            for (page, text) in source.normalized.iter().enumerate() {
                for gram in grams(text) { index.entry(gram).or_default().insert((file, page)); }
                for (_, word) in query::words(text) {
                    words.entry(word.into()).or_default().insert((file, page));
                }
            }
        }
        // Stem each distinct word once, never every page on every query.
        for (word, pages) in &words { stems.entry(stemmer.stem(word).into_owned()).or_default().extend(pages); }
        // The v1 postings field remains readable; only page text is authoritative.
        let pages = sources.iter().enumerate().flat_map(|(file, source)| (0..source.pages.len()).map(move |page| (file, page))).collect();
        Self { version: 1, sources, postings: BTreeMap::new(), grams: index, words, stems, pages }
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
        self.search_with_options(query, SearchOptions::default())
    }

    /// Acrobat's indexed phrase, word, Boolean, proximity and English stemming searches.
    /// Case and whole-word filtering are checked against authoritative page text after pruning.
    pub fn search_with_options(&self, query: &str, options: SearchOptions) -> Result<Vec<Hit>> {
        let query = query::Query::compile(query, options)?;
        let fresh: Vec<bool> = self.sources.iter().map(|s| s.stamp.is_some() && stamp(&s.path) == s.stamp).collect();
        let candidates = query.candidates(&self.grams, &self.words, &self.stems, &self.pages);
        let mut out = Vec::new();
        for (file, page) in candidates {
            if !fresh[file] { continue; }
            let source = &self.sources[file];
            let case_text;
            let text = if options.case_sensitive { case_text = canonical(&source.pages[page]); &case_text } else { &source.normalized[page] };
            if let Some(at) = query.matches(text) {
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
        // Catalogs keep canonical paths (macOS: /private/var for /var).
        let path = path.canonicalize().unwrap();
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

    fn advanced_catalog(dir: &Path) -> Catalog {
        let path = dir.join("source.pdf");
        std::fs::write(&path, b"unchanged source").unwrap();
        Catalog::from_sources(vec![Source {
            fingerprint: fingerprint(&path).unwrap(), stamp: stamp(&path), path,
            normalized: Vec::new(), pages: vec![
                "Contract signed. Running runners run. État du contrat.".into(),
                "contract unsigned; invoice pending.".into(),
                "Cat concatenate dog.".into(),
                "red one two blue. BLUE near red.".into(),
                "blue one two three four red.".into(),
                "go stop go".into(),
                "red unrelated green unrelated blue".into(),
                "हिन्दी भाषा".into(),
            ],
        }])
    }

    #[test]
    fn catalog_advanced_queries_cover_case_words_boolean_stems_and_proximity() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = advanced_catalog(dir.path());
        let pages = |query, options| catalog.search_with_options(query, options).unwrap().into_iter().map(|h| h.page).collect::<Vec<_>>();
        assert_eq!(pages("Contract", SearchOptions { case_sensitive: true, ..Default::default() }), [0]);
        assert_eq!(pages("cat", SearchOptions::default()), [2]);
        assert!(pages("concaten", SearchOptions { whole_words: true, ..Default::default() }).is_empty());
        assert_eq!(pages("cat", SearchOptions { whole_words: true, ..Default::default() }), [2]);
        assert_eq!(pages("हिन्दी", SearchOptions { whole_words: true, ..Default::default() }), [7]);
        assert!(pages("दी", SearchOptions { whole_words: true, ..Default::default() }).is_empty());
        assert_eq!(pages("(signed OR invoice) AND NOT unsigned", SearchOptions { mode: SearchMode::Boolean, ..Default::default() }), [0]);
        assert_eq!(pages("signed OR invoice AND NOT pending", SearchOptions { mode: SearchMode::Boolean, ..Default::default() }), [0]);
        assert_eq!(pages(r#"NOT "red blue" AND green"#, SearchOptions { mode: SearchMode::Boolean, ..Default::default() }), [6]);
        assert_eq!(pages(r#""état du contrat" AND signed"#, SearchOptions { mode: SearchMode::Boolean, ..Default::default() }), [0]);
        assert_eq!(pages("runs", SearchOptions { english_stemming: true, ..Default::default() }), [0]);
        assert_eq!(pages("red blue", SearchOptions { mode: SearchMode::Proximity, proximity: 1, ..Default::default() }), [3]);
        assert_eq!(pages("go go", SearchOptions { mode: SearchMode::Proximity, proximity: 1, ..Default::default() }), [5]);
        assert!(pages("go go", SearchOptions { mode: SearchMode::Proximity, proximity: 0, ..Default::default() }).is_empty());
        assert_eq!(pages("signed invoice", SearchOptions { mode: SearchMode::AnyWords, ..Default::default() }), [0, 1]);
        assert!(pages("signed invoice", SearchOptions { mode: SearchMode::AllWords, ..Default::default() }).is_empty());
        let saved = catalog.save(&dir.path().join("index.json")).unwrap();
        assert_eq!(Catalog::load(&saved).unwrap().search_with_options("runs", SearchOptions { english_stemming: true, ..Default::default() }).unwrap().len(), 1);
    }

    #[test]
    fn catalog_refuses_invalid_or_excessive_boolean_queries_and_conflicting_options() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = advanced_catalog(dir.path());
        let options = SearchOptions { mode: SearchMode::Boolean, ..Default::default() };
        for bad in ["AND signed", "signed OR", "(signed", "signed)", "\"signed", "signed AND ()", "\"\""] {
            assert!(catalog.search_with_options(bad, options).is_err(), "{bad}");
        }
        assert!(catalog.search_with_options(&format!("{}signed{}", "(".repeat(40), ")".repeat(40)), options).is_err());
        assert!(catalog.search_with_options(&"term ".repeat(200), options).is_err());
        assert!(catalog.search_with_options("run", SearchOptions { case_sensitive: true, english_stemming: true, ..Default::default() }).is_err());
        assert!(catalog.search_with_options("run", SearchOptions { mode: SearchMode::Proximity, ..Default::default() }).is_err());
        assert!(catalog.search("-\n").is_err());
    }

}
