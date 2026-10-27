//! Spell checking, offline (Acrobat's Check Spelling, Preview's Spelling and Grammar): a Hunspell
//! dictionary (`.aff` + `.dic`, the files LibreOffice, Firefox and most Linux systems ship) read by
//! `spellbook`, a pure-Rust Hunspell; else a plain word list (one word per line, as
//! `/usr/share/dict/words`); else nothing, and [`Speller::load`] says what is missing.
//!
//! The text is cut into words here (not by the dictionary): web and mail addresses, paths, words
//! with digits, all-capital acronyms and mixed-case names are left alone, as Word's and Acrobat's
//! defaults do, so a comment full of "PDF", "v2" and "iPhone" is not a sea of red.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The `.aff` and `.dic` files of one Hunspell dictionary, and its language ("en_US").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictFiles {
    pub lang: String,
    pub aff: PathBuf,
    pub dic: PathBuf,
}

/// A misspelt word: where it starts in the text (a byte offset) and the word as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub start: usize,
    pub word: String,
}

enum Kind {
    Hunspell(Box<spellbook::Dictionary>),
    Words(HashSet<String>),
}

/// A loaded dictionary, the words the user taught it (Learn Spelling) and those ignored for the
/// session (Ignore Spelling).
pub struct Speller {
    kind: Kind,
    /// The dictionary's language code ("en_US"), "" for a plain word list.
    pub lang: String,
    /// Where it was read from, to tell the user.
    pub source: PathBuf,
    learned: HashSet<String>,
    ignored: HashSet<String>,
}

/// Where Hunspell dictionaries usually are on this system, searched in this order after the
/// folders named by `SHEAF_DICTIONARIES` (a path list, as `PATH`).
pub fn system_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("SHEAF_DICTIONARIES").map(|v| std::env::split_paths(&v).collect()).unwrap_or_default();
    if cfg!(windows) {
        // LibreOffice's bundled dictionaries (dict-en, dict-fr…, one folder each).
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(p) = std::env::var_os(var) {
                dirs.push(PathBuf::from(p).join("LibreOffice").join("share").join("extensions"));
            }
        }
    } else {
        for d in ["/usr/share/hunspell", "/usr/share/myspell", "/usr/share/myspell/dicts", "/usr/local/share/hunspell", "/usr/lib/libreoffice/share/extensions", "/opt/homebrew/share/hunspell", "/Library/Spelling", "/Applications/LibreOffice.app/Contents/Resources/extensions"] {
            dirs.push(PathBuf::from(d));
        }
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join("Library").join("Spelling"));
        }
    }
    dirs
}

/// Plain word lists, used when no Hunspell dictionary is found.
pub fn word_list_paths() -> Vec<PathBuf> {
    if cfg!(windows) {
        return Vec::new();
    }
    ["/usr/share/dict/words", "/usr/share/dict/american-english", "/usr/share/dict/british-english", "/usr/share/dict/web2"].iter().map(PathBuf::from).collect()
}

/// "fr-FR.UTF-8" → "fr_FR"; "en" stays "en".
fn normal_lang(l: &str) -> String {
    l.split(['.', '@']).next().unwrap_or("").replace('-', "_")
}

/// The dictionaries in `dirs` and in their direct subfolders (LibreOffice keeps one per folder):
/// (language, aff, dic), sorted by language for a stable choice.
fn installed(dirs: &[PathBuf]) -> Vec<DictFiles> {
    let mut found = Vec::new();
    let scan = |dir: &Path, found: &mut Vec<DictFiles>| {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dic")) {
                let aff = p.with_extension("aff");
                if aff.is_file() {
                    let lang = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    found.push(DictFiles { lang: normal_lang(&lang), aff, dic: p });
                }
            }
        }
    };
    for d in dirs {
        scan(d, &mut found);
        if let Ok(rd) = std::fs::read_dir(d) {
            let mut subs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
            subs.sort();
            for s in subs {
                scan(&s, &mut found);
            }
        }
    }
    // The first folder wins for one language; then by language.
    let mut seen = HashSet::new();
    found.retain(|f| seen.insert(f.lang.clone()));
    found.sort_by(|a, b| a.lang.cmp(&b.lang));
    found
}

/// The dictionary to use for the languages wanted, best first ("fr_FR", "fr"): the exact one, else
/// one of the same language ("fr" → "fr_BE"), else American then British English, else any.
pub fn find_dictionary(langs: &[String], dirs: &[PathBuf]) -> Option<DictFiles> {
    let all = installed(dirs);
    let wanted: Vec<String> = langs.iter().map(|l| normal_lang(l)).filter(|l| !l.is_empty()).chain(["en_US".to_string(), "en_GB".to_string()]).collect();
    for w in &wanted {
        if let Some(d) = all.iter().find(|d| d.lang.eq_ignore_ascii_case(w)) {
            return Some(d.clone());
        }
        let base = w.split('_').next().unwrap_or(w);
        if let Some(d) = all.iter().find(|d| d.lang.split('_').next().is_some_and(|b| b.eq_ignore_ascii_case(base))) {
            return Some(d.clone());
        }
    }
    all.into_iter().next()
}

/// A language code as a reader would say it ("en_US" → "English (United States)").
pub fn language_name(lang: &str) -> String {
    let base = match lang.split('_').next().unwrap_or("") {
        "en" => "English",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "nl" => "Dutch",
        "sv" => "Swedish",
        "da" => "Danish",
        "nb" | "no" => "Norwegian",
        "fi" => "Finnish",
        "pl" => "Polish",
        "cs" => "Czech",
        "ru" => "Russian",
        "uk" => "Ukrainian",
        "tr" => "Turkish",
        "el" => "Greek",
        "he" => "Hebrew",
        "ar" => "Arabic",
        _ => return lang.to_string(),
    };
    let region = match lang.split('_').nth(1).unwrap_or("") {
        "US" => " (United States)",
        "GB" => " (United Kingdom)",
        "CA" => " (Canada)",
        "AU" => " (Australia)",
        "BR" => " (Brazil)",
        "PT" => " (Portugal)",
        "BE" => " (Belgium)",
        "CH" => " (Switzerland)",
        _ => "",
    };
    format!("{base}{region}")
}

/// The text of a dictionary file in its declared encoding (`SET` in the `.aff`): old French and
/// German dictionaries are Latin-1.
fn decode(bytes: &[u8], encoding: &str) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match encoding.to_ascii_uppercase().as_str() {
        "ISO8859-1" | "ISO-8859-1" | "ISO8859-15" | "ISO-8859-15" | "LATIN1" => bytes.iter().map(|&b| b as char).collect(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// The `SET` line of an `.aff` file ("UTF-8" when absent).
fn aff_encoding(aff: &[u8]) -> String {
    aff.split(|&b| b == b'\n')
        .take(200)
        .find_map(|l| l.strip_prefix(b"SET ").map(|e| String::from_utf8_lossy(e).trim().to_string()))
        .unwrap_or_else(|| "UTF-8".into())
}

impl Speller {
    /// Reads one Hunspell dictionary.
    pub fn from_files(files: &DictFiles) -> Result<Speller, String> {
        let aff = std::fs::read(&files.aff).map_err(|e| format!("{}: {e}", files.aff.display()))?;
        let dic = std::fs::read(&files.dic).map_err(|e| format!("{}: {e}", files.dic.display()))?;
        let enc = aff_encoding(&aff);
        let dict = spellbook::Dictionary::new(&decode(&aff, &enc), &decode(&dic, &enc)).map_err(|e| format!("{}: {e}", files.dic.display()))?;
        Ok(Speller { kind: Kind::Hunspell(Box::new(dict)), lang: files.lang.clone(), source: files.dic.clone(), learned: HashSet::new(), ignored: HashSet::new() })
    }

    /// Reads a plain word list, one word per line.
    pub fn from_word_list(path: &Path) -> Result<Speller, String> {
        let text = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let words: HashSet<String> = String::from_utf8_lossy(&text).lines().map(str::trim).filter(|w| !w.is_empty()).map(str::to_string).collect();
        if words.is_empty() {
            return Err(format!("{}: no words", path.display()));
        }
        Ok(Speller { kind: Kind::Words(words), lang: String::new(), source: path.to_path_buf(), learned: HashSet::new(), ignored: HashSet::new() })
    }

    /// The best dictionary for `langs` in `extra_dirs` then the system's folders, else a word
    /// list; the error says what to install when there is none.
    pub fn load(langs: &[String], extra_dirs: &[PathBuf]) -> Result<Speller, String> {
        let dirs: Vec<PathBuf> = extra_dirs.iter().cloned().chain(system_dirs()).collect();
        if let Some(files) = find_dictionary(langs, &dirs)
            && let Ok(s) = Speller::from_files(&files)
        {
            return Ok(s);
        }
        if let Some(s) = word_list_paths().iter().filter(|p| p.is_file()).find_map(|p| Speller::from_word_list(p).ok()) {
            return Ok(s);
        }
        let place = extra_dirs.first().map(|d| d.display().to_string()).unwrap_or_else(|| "the dictionaries folder".into());
        Err(if cfg!(target_os = "linux") {
            format!("No spelling dictionary found. Install hunspell-en-us (or your language's), or put en_US.aff and en_US.dic in {place}.")
        } else {
            format!("No spelling dictionary found. Put a Hunspell dictionary (en_US.aff and en_US.dic, as LibreOffice ships) in {place}.")
        })
    }

    /// Whether the dictionary is English (a plain word list is taken as English): the grammar's
    /// English rules apply.
    pub fn is_english(&self) -> bool {
        self.lang.is_empty() || self.lang.to_ascii_lowercase().starts_with("en")
    }

    /// The dictionary's language as a reader would say it ("English (United States)"), or the
    /// word list's file.
    pub fn describe(&self) -> String {
        if self.lang.is_empty() {
            format!("Word list {}", self.source.display())
        } else {
            format!("{} · {}", language_name(&self.lang), self.source.display())
        }
    }

    /// Learn Spelling: the word is right from now on (kept by the caller between sessions).
    pub fn learn(&mut self, word: &str) {
        self.learned.insert(word.to_lowercase());
    }

    /// Forgets every learned word.
    pub fn forget_learned(&mut self) {
        self.learned.clear();
    }

    /// Ignore Spelling: the word is not marked again until the app quits.
    pub fn ignore(&mut self, word: &str) {
        self.ignored.insert(word.to_lowercase());
    }

    /// Whether one word is spelt right (the dictionary, then the learned and ignored words).
    pub fn check(&self, word: &str) -> bool {
        let lower = word.to_lowercase();
        if self.learned.contains(&lower) || self.ignored.contains(&lower) {
            return true;
        }
        // A typographic apostrophe is checked as a straight one (the dictionaries list "don't").
        let word = word.replace('’', "'");
        match &self.kind {
            Kind::Hunspell(d) => d.check(&word),
            Kind::Words(w) => w.contains(&word) || w.contains(&word.to_lowercase()),
        }
    }

    /// Up to `max` corrections for a misspelt word, best first.
    pub fn suggest(&self, word: &str, max: usize) -> Vec<String> {
        let mut out = Vec::new();
        match &self.kind {
            Kind::Hunspell(d) => d.suggest(&word.replace('’', "'"), &mut out),
            Kind::Words(w) => out = edits_in(w, word),
        }
        let mut seen = HashSet::new();
        out.retain(|s| s.as_str() != word && seen.insert(s.clone()));
        out.truncate(max);
        out
    }

    /// The misspelt words of `text`, in order. `typing`: the word touching the end of the text is
    /// being typed and is left alone until a space or a punctuation mark ends it (Preview's).
    pub fn misspelt(&self, text: &str, typing: bool) -> Vec<Issue> {
        words(text)
            .into_iter()
            .filter(|&(start, w)| !(typing && start + w.len() == text.len()))
            .filter(|&(_, w)| !self.check(w))
            .map(|(start, w)| Issue { start, word: w.to_string() })
            .collect()
    }
}

/// A grammar finding (Preview's Check Grammar With Spelling, kept light and offline): where it
/// starts (a byte offset), the text it covers, what is wrong and the text to put instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrammarIssue {
    pub start: usize,
    pub text: String,
    pub note: &'static str,
    pub fix: String,
}

/// Every run of letters (with apostrophes inside), unfiltered: the grammar rules look at all words.
fn plain_words(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (k, &(i, c)) in chars.iter().enumerate() {
        let apostrophe = (c == '\'' || c == '’') && start.is_some() && chars.get(k + 1).is_some_and(|&(_, n)| n.is_alphabetic());
        match (c.is_alphanumeric() || apostrophe, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, &text[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out
}

/// Words a full stop ends without ending the sentence.
const ABBREVIATIONS: &[&str] = &["e.g", "i.e", "etc", "vs", "cf", "mr", "mrs", "ms", "dr", "st", "no", "p", "pp", "fig", "approx", "al", "jr", "sr", "inc", "ltd", "co", "eq", "vol", "ch", "sec", "dept", "est", "min", "max"];

/// Whether `word` starts with a vowel sound, as "an" wants ("an hour", "a unit", "a one-off").
fn vowel_sound(word: &str) -> bool {
    let w = word.to_lowercase();
    if ["hour", "honest", "honor", "honour", "heir"].iter().any(|p| w.starts_with(p)) {
        return true;
    }
    if ["uni", "use", "usu", "uti", "ure", "eu", "ewe", "one", "once", "ufo", "uk", "us"].iter().any(|p| w.starts_with(p)) {
        return false;
    }
    w.starts_with(['a', 'e', 'i', 'o', 'u'])
}

/// Preview's Check Grammar With Spelling, the part that needs no language model: a word written
/// twice ("the the"), a sentence that starts small after a full stop, question or exclamation mark
/// ("done. then"; not after "e.g." or "Dr."), two spaces or more between words, and "a" / "an"
/// against the next word's sound ("a apple", "an car"; "an hour", "a unit" are right). English
/// rules, applied only when `english` (the dictionary's language): other languages get only the
/// repeated words and spaces (French, say, has no "a"/"an" and other abbreviations).
pub fn grammar(text: &str, english: bool) -> Vec<GrammarIssue> {
    let mut out = Vec::new();
    // Two spaces or more between two non-spaces on one line.
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            let j = (i..bytes.len()).find(|&j| bytes[j] != b' ').unwrap_or(bytes.len());
            if j - i >= 2 && i > 0 && j < bytes.len() && bytes[i - 1] != b'\n' && bytes[j] != b'\n' {
                out.push(GrammarIssue { start: i, text: text[i..j].to_string(), note: "More than one space", fix: " ".into() });
            }
            i = j;
        } else {
            i += 1;
        }
    }
    let words = plain_words(text);
    for pair in words.windows(2) {
        let ((s1, w1), (s2, w2)) = (pair[0], pair[1]);
        let between = &text[s1 + w1.len()..s2];
        let only_space = !between.is_empty() && between.chars().all(|c| c == ' ' || c == '\t');
        // The same word twice in a row ("had had" and "that that" can be right).
        if only_space && w1.eq_ignore_ascii_case(w2) && !w1.chars().any(|c| c.is_numeric()) && !["had", "that"].contains(&w1.to_lowercase().as_str()) {
            out.push(GrammarIssue { start: s1, text: text[s1..s2 + w2.len()].to_string(), note: "Repeated word", fix: w1.to_string() });
        }
        // "a" / "an" against the sound of the next word (not before an acronym, a number or a letter).
        let lower = w1.to_lowercase();
        let next_plain = w2.chars().count() > 1 && w2.chars().all(char::is_alphabetic) && !w2.chars().all(char::is_uppercase);
        if english && only_space && next_plain && (lower == "a" || lower == "an") {
            let want_an = vowel_sound(w2);
            if want_an != (lower == "an") {
                let fix = match (want_an, w1.starts_with(char::is_uppercase)) {
                    (true, true) => "An",
                    (true, false) => "an",
                    (false, true) => "A",
                    (false, false) => "a",
                };
                out.push(GrammarIssue { start: s1, text: w1.to_string(), note: if want_an { "\"an\" before a vowel sound" } else { "\"a\" before a consonant sound" }, fix: fix.into() });
            }
        }
        // A capital after the end of a sentence.
        let tail = between.trim_end_matches([' ', '\t']);
        let ends = tail.ends_with(['.', '!', '?']) && !tail.ends_with("..") && between.len() > tail.len() && !tail.contains('\n');
        let abbreviation = tail.ends_with('.') && (w1.chars().count() == 1 || ABBREVIATIONS.contains(&lower.as_str()) || text[..s1].ends_with('.'));
        if english && ends && !abbreviation && w2.starts_with(char::is_lowercase) && !w2.chars().any(|c| c.is_numeric()) {
            let mut c = w2.chars();
            let fixed: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
            out.push(GrammarIssue { start: s2, text: w2.to_string(), note: "Capital at the start of a sentence", fix: fixed });
        }
    }
    out.sort_by_key(|g| g.start);
    out
}

/// One thing Check Spelling stops at: its text (an index into the texts checked), where it starts
/// there (a byte offset), the words as written, and for a grammar finding what is wrong and its fix
/// (a misspelt word has neither: its corrections come from the dictionary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub text: usize,
    pub start: usize,
    pub word: String,
    pub note: Option<&'static str>,
    pub fix: Option<String>,
}

/// Acrobat's Check Spelling dialog over many texts (comments, replies, form fields): the texts are
/// scanned once, when the pass starts (off the UI thread if the caller likes); Next, Ignore, Learn
/// and Change then only move through that list, never scanning the texts again.
#[derive(Debug, Clone, Default)]
pub struct CheckPass {
    found: Vec<Finding>,
    at: usize,
}

impl CheckPass {
    /// Every misspelt word of `texts`, in order (and, with `grammar`, the grammar findings among
    /// them, see [`grammar`]).
    pub fn new(speller: &Speller, texts: &[&str], grammar_too: bool) -> CheckPass {
        let mut found = Vec::new();
        for (k, text) in texts.iter().enumerate() {
            let mut here: Vec<Finding> = speller.misspelt(text, false).into_iter().map(|i| Finding { text: k, start: i.start, word: i.word, note: None, fix: None }).collect();
            if grammar_too {
                here.extend(grammar(text, speller.is_english()).into_iter().map(|g| Finding { text: k, start: g.start, word: g.text, note: Some(g.note), fix: Some(g.fix) }));
                here.sort_by_key(|f| f.start);
            }
            found.extend(here);
        }
        CheckPass { found, at: 0 }
    }

    /// The finding shown now (none: the pass is over).
    pub fn current(&self) -> Option<&Finding> {
        self.found.get(self.at)
    }

    /// Next: this one is left as it is.
    pub fn skip(&mut self) {
        self.at += 1;
    }

    /// Ignore or Learn: this word is passed now and wherever it comes later (any case); a grammar
    /// finding only here.
    pub fn pass_word(&mut self) {
        let Some(f) = self.found.get(self.at) else { return };
        if f.note.is_none() {
            let word = f.word.to_lowercase();
            let at = self.at;
            let mut k = 0;
            self.found.retain(|g| {
                k += 1;
                k - 1 <= at || g.note.is_some() || g.word.to_lowercase() != word
            });
        }
        self.at += 1;
    }

    /// Change: the finding shown was replaced by `new` in its text; what comes after it in the same
    /// text moves by the difference in length, and a finding it overlapped goes.
    pub fn changed(&mut self, new: &str) {
        let Some(f) = self.found.get(self.at).cloned() else { return };
        let end = f.start + f.word.len();
        let delta = new.len() as isize - f.word.len() as isize;
        let at = self.at;
        let mut k = 0;
        self.found.retain(|g| {
            k += 1;
            k - 1 <= at || g.text != f.text || g.start >= end
        });
        for g in self.found.iter_mut().skip(at + 1).filter(|g| g.text == f.text) {
            g.start = (g.start as isize + delta) as usize;
        }
        self.at += 1;
    }

    /// How many findings are left, the one shown included.
    pub fn left(&self) -> usize {
        self.found.len().saturating_sub(self.at)
    }
}

/// Words of a word list one edit away from `word` (a letter dropped, swapped, changed or added),
/// as a list without Hunspell's rules can suggest.
fn edits_in(list: &HashSet<String>, word: &str) -> Vec<String> {
    let chars: Vec<char> = word.to_lowercase().chars().collect();
    let capital = word.chars().next().is_some_and(char::is_uppercase);
    let mut out = Vec::new();
    let push = |c: Vec<char>, out: &mut Vec<String>| {
        let s: String = c.into_iter().collect();
        let shown = if capital { let mut it = s.chars(); it.next().map(|f| f.to_uppercase().chain(it).collect()).unwrap_or_default() } else { s.clone() };
        if list.contains(&s) || list.contains(&shown) {
            out.push(shown);
        }
    };
    for i in 0..chars.len() {
        let mut c = chars.clone();
        c.remove(i);
        push(c, &mut out);
    }
    for i in 0..chars.len().saturating_sub(1) {
        let mut c = chars.clone();
        c.swap(i, i + 1);
        push(c, &mut out);
    }
    for i in 0..chars.len() {
        for l in 'a'..='z' {
            if chars[i] != l {
                let mut c = chars.clone();
                c[i] = l;
                push(c, &mut out);
            }
        }
    }
    for i in 0..=chars.len() {
        for l in 'a'..='z' {
            let mut c = chars.clone();
            c.insert(i, l);
            push(c, &mut out);
        }
    }
    out
}

/// A run of text between spaces that is an address or a path, not words: left out whole.
fn is_address(chunk: &str) -> bool {
    chunk.contains("://") || chunk.contains('@') || chunk.starts_with("www.") || chunk.contains('/') || chunk.contains('\\')
}

/// The words of `text` worth checking, with their byte offsets: letters with apostrophes inside
/// ("don't", "l’été"); hyphens split ("well-known" is two words). Left out: one-letter words,
/// words with digits, all capitals ("PDF"), capitals inside ("iPhone", "McKay"), addresses.
pub fn words(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut chunk_start = 0;
    for chunk in text.split_inclusive(char::is_whitespace) {
        let offset = chunk_start;
        chunk_start += chunk.len();
        if is_address(chunk.trim_end()) {
            continue;
        }
        let mut start: Option<usize> = None;
        let bytes: Vec<(usize, char)> = chunk.char_indices().collect();
        for (k, &(i, c)) in bytes.iter().enumerate() {
            let apostrophe = (c == '\'' || c == '’') && start.is_some() && bytes.get(k + 1).is_some_and(|&(_, n)| n.is_alphabetic());
            let inside = c.is_alphanumeric() || apostrophe;
            match (inside, start) {
                (true, None) => start = Some(i),
                (false, Some(s)) => {
                    out.push((offset + s, &text[offset + s..offset + i]));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            out.push((offset + s, &text[offset + s..offset + chunk.len()]));
        }
    }
    out.retain(|&(_, w)| {
        let letters = w.chars().filter(|c| c.is_alphabetic()).count();
        letters >= 2 && !w.chars().any(|c| c.is_numeric()) && !w.chars().skip(1).any(char::is_uppercase)
    });
    out
}

/// `text` with the misspelt `old` at byte `start` replaced by `new`. When the text changed since
/// it was checked (the offset no longer holds the word), the first whole-word `old` is replaced;
/// when there is none, the text comes back as it was.
pub fn replace_word(text: &str, start: usize, old: &str, new: &str) -> String {
    let fits = |i: usize| text.get(i..i + old.len()) == Some(old);
    // A word's edges must be word boundaries; a run of spaces (a grammar finding) needs none.
    let edge_letter = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    let whole = |i: usize| {
        let before = !edge_letter(old.chars().next()) || text[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let after = !edge_letter(old.chars().next_back()) || text[i + old.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        before && after
    };
    let at = if old.is_empty() {
        None
    } else if text.is_char_boundary(start) && fits(start) && whole(start) {
        Some(start)
    } else {
        text.match_indices(old).map(|(i, _)| i).find(|&i| whole(i))
    };
    match at {
        Some(i) => format!("{}{}{}", &text[..i], new, &text[i + old.len()..]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word_list(words: &[&str]) -> Speller {
        Speller { kind: Kind::Words(words.iter().map(|w| w.to_string()).collect()), lang: String::new(), source: PathBuf::new(), learned: HashSet::new(), ignored: HashSet::new() }
    }

    #[test]
    fn words_skip_addresses_acronyms_and_numbers() {
        let w: Vec<&str> = words("Teh PDF v2 iPhone don't well-known a see https://x.org/teh mail@teh.com C:\\teh “quoted”").into_iter().map(|(_, w)| w).collect();
        assert_eq!(w, ["Teh", "don't", "well", "known", "see", "quoted"]);
        // Offsets are bytes into the text, accents counted.
        assert_eq!(words("été recieve"), [(0, "été"), (6, "recieve")]);
        assert_eq!(words("l’été"), [(0, "l’été")]);
    }

    #[test]
    fn misspelt_skips_the_word_being_typed_and_learned_words() {
        let mut s = word_list(&["the", "cat", "sat"]);
        assert_eq!(s.misspelt("teh cat sat", false), [Issue { start: 0, word: "teh".into() }]);
        // The last word is still being typed.
        assert!(s.misspelt("the ca", true).is_empty());
        assert_eq!(s.misspelt("the ca ", true), [Issue { start: 4, word: "ca".into() }]);
        // Capitalised at the start of a sentence: still right.
        assert!(s.misspelt("The cat.", false).is_empty());
        s.learn("Sheaf");
        s.ignore("teh");
        assert!(s.misspelt("teh sheaf sat", false).is_empty());
        s.forget_learned();
        assert_eq!(s.misspelt("sheaf", false).len(), 1);
    }

    #[test]
    fn the_check_pass_goes_word_by_word_through_the_texts() {
        let s = word_list(&["the", "cat", "sat"]);
        let texts = ["the cat", "teh cat sta teh", "", "cta"];
        let mut pass = CheckPass::new(&s, &texts, false);
        let at = |p: &CheckPass| p.current().map(|f| (f.text, f.start, f.word.clone()));
        assert_eq!(at(&pass), Some((1, 0, "teh".into())));
        // Change "teh" to "the": the words after it in that text keep their place in the new text.
        pass.changed("the");
        assert_eq!(at(&pass), Some((1, 8, "sta".into())));
        pass.skip();
        assert_eq!(at(&pass), Some((1, 12, "teh".into())));
        // Ignore: the same word later on is passed too.
        pass.pass_word();
        assert_eq!(at(&pass), Some((3, 0, "cta".into())));
        assert_eq!(pass.left(), 1);
        pass.skip();
        assert_eq!(pass.current(), None);
        // A longer correction moves what follows the other way.
        let mut pass = CheckPass::new(&s, &["ab cd ef"], false);
        pass.changed("abcd");
        assert_eq!(at(&pass), Some((0, 5, "cd".into())));
        let mut pass = CheckPass::new(&s, &["ab ab cd"], false);
        pass.pass_word();
        assert_eq!(at(&pass), Some((0, 6, "cd".into())));
    }

    #[test]
    fn the_light_grammar_pass() {
        let g = |t: &str| grammar(t, true).into_iter().map(|g| (g.start, g.text, g.fix)).collect::<Vec<_>>();
        assert_eq!(g("We saw the the cat."), [(7, "the the".into(), "the".into())]);
        assert_eq!(g("It is done. then we go."), [(12, "then".into(), "Then".into())]);
        // Not after an abbreviation, an initial or an ellipsis; nor across lines or inside numbers.
        assert!(g("See e.g. this one, Dr. who, J. smith… and so... on. Pi is 3.14 here.").is_empty());
        assert_eq!(g("Two  spaces"), [(3, "  ".into(), " ".into())]);
        // Indentation and line ends are not doubled spaces.
        assert!(g("  indented\nnext  \nline").is_empty());
        assert_eq!(g("a apple and an car"), [(0, "a".into(), "an".into()), (12, "an".into(), "a".into())]);
        assert!(g("an hour, a unit, a one-off, an FBI agent, a 3 times").is_empty());
        assert_eq!(g("A owl"), [(0, "A".into(), "An".into())]);
        // "had had" can be right.
        assert!(g("She had had enough.").is_empty());
        // Another language: repeated words and spaces only ("Il a une idée", "Fini. ensuite").
        assert!(grammar("Il a une idée. ensuite on part.", false).is_empty());
        assert_eq!(grammar("Il a a  une idée", false).into_iter().map(|g| g.note).collect::<Vec<_>>(), ["Repeated word", "More than one space"]);
        // Replacing a grammar finding at its offset.
        assert_eq!(replace_word("Two  spaces", 3, "  ", " "), "Two spaces");
        assert_eq!(replace_word("We saw the the cat.", 7, "the the", "the"), "We saw the cat.");
        // A pass with grammar: the findings in reading order, spelling ones without a note.
        let s = word_list(&["we", "saw", "the", "cat"]);
        let pass = CheckPass::new(&s, &["we saw teh the cat"], true);
        assert_eq!(pass.current().map(|f| (f.word.as_str(), f.note)), Some(("teh", None)));
    }

    #[test]
    fn word_list_suggestions_are_one_edit_away() {
        let s = word_list(&["receive", "the", "hello", "help"]);
        assert_eq!(s.suggest("recieve", 3), ["receive"]);
        assert_eq!(s.suggest("teh", 3), ["the"]);
        // A capital stays a capital.
        assert_eq!(s.suggest("Helo", 5), ["Help", "Hello"]);
    }

    #[test]
    fn replace_word_follows_the_text_when_it_moved() {
        assert_eq!(replace_word("teh cat", 0, "teh", "the"), "the cat");
        // Typed before the word since it was checked: the whole word is found again.
        assert_eq!(replace_word("so teh cat", 0, "teh", "the"), "so the cat");
        assert_eq!(replace_word("tehx teh", 0, "teh", "the"), "tehx the");
        // Gone: nothing changes; an offset inside a character is not used.
        assert_eq!(replace_word("the cat", 0, "teh", "the"), "the cat");
        assert_eq!(replace_word("été teh", 1, "teh", "the"), "été the");
    }

    #[test]
    fn dictionaries_are_found_by_language_then_english() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("dict-fr");
        std::fs::create_dir(&sub).unwrap();
        for (d, name) in [(dir.path(), "en_GB"), (dir.path(), "en_US"), (sub.as_path(), "fr_FR"), (dir.path(), "de_DE")] {
            std::fs::write(d.join(format!("{name}.aff")), "SET UTF-8\n").unwrap();
            std::fs::write(d.join(format!("{name}.dic")), "1\nword\n").unwrap();
        }
        // A .dic without its .aff is not a dictionary.
        std::fs::write(dir.path().join("it_IT.dic"), "1\nparola\n").unwrap();
        let dirs = [dir.path().to_path_buf()];
        let pick = |l: &str| find_dictionary(&[l.to_string()], &dirs).map(|d| d.lang);
        assert_eq!(pick("fr-FR.UTF-8").as_deref(), Some("fr_FR"));
        assert_eq!(pick("fr_CA").as_deref(), Some("fr_FR"));
        assert_eq!(pick("de").as_deref(), Some("de_DE"));
        assert_eq!(pick("it_IT").as_deref(), Some("en_US"));
        assert_eq!(pick("en_GB").as_deref(), Some("en_GB"));
        assert!(find_dictionary(&[], &[dir.path().join("none")]).is_none());
        assert_eq!(language_name("en_US"), "English (United States)");
        assert_eq!(language_name("xx_YY"), "xx_YY");
    }

    #[test]
    fn latin1_dictionaries_are_decoded() {
        assert_eq!(aff_encoding(b"# c\nSET ISO8859-1\nTRY a\n"), "ISO8859-1");
        assert_eq!(decode(b"caf\xe9", "ISO8859-1"), "café");
        assert_eq!(decode("\u{feff}café".as_bytes(), "UTF-8"), "café");
    }

    #[test]
    fn a_hunspell_dictionary_checks_and_suggests() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("xx_XX.aff"), "SET UTF-8\nTRY esianrtolcdugmfphbyvkw\n").unwrap();
        std::fs::write(dir.path().join("xx_XX.dic"), "3\nhello\nworld\nreceive\n").unwrap();
        let files = find_dictionary(&["xx_XX".into()], &[dir.path().to_path_buf()]).unwrap();
        let s = Speller::from_files(&files).unwrap();
        assert!(s.check("hello") && s.check("Hello") && !s.check("helo"));
        assert!(s.suggest("helo", 3).contains(&"hello".to_string()));
        assert_eq!(s.misspelt("hello wrold receive", false), [Issue { start: 6, word: "wrold".into() }]);
    }

    #[test]
    fn the_system_english_dictionary_when_installed() {
        let Some(files) = find_dictionary(&["en_US".into()], &system_dirs()) else { return };
        let s = Speller::from_files(&files).unwrap();
        assert!(s.check("receive") && s.check("don’t") && !s.check("recieve"));
        assert!(s.suggest("recieve", 5).contains(&"receive".to_string()));
    }
}
