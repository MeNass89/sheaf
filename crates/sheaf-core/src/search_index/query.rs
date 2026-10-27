//! Acrobat's advanced catalog queries. Parsing is bounded before any pages are visited.
use crate::{OpdfError, Result};
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Pages = BTreeSet<(usize, usize)>;
pub(super) type Index = BTreeMap<String, Pages>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchMode {
    #[default]
    Phrase,
    AllWords,
    AnyWords,
    Boolean,
    Proximity,
}

#[derive(Debug, Clone, Copy)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub whole_words: bool,
    pub case_sensitive: bool,
    /// English inflections, using the Snowball algorithm; never silently applied to other languages.
    pub english_stemming: bool,
    /// Maximum intervening words in an unordered proximity match.
    pub proximity: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self { mode: SearchMode::Phrase, whole_words: false, case_sensitive: false, english_stemming: false, proximity: 10 }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token { Text(String), And, Or, Not, Open, Close }

enum Expr { Text(String), And(Box<Self>, Box<Self>), Or(Box<Self>, Box<Self>), Not(Box<Self>) }

// Keep byte offsets for snippets while treating punctuation as word boundaries.
pub(super) fn words(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (at, ch) in text.char_indices() {
        if word_char(ch) { start.get_or_insert(at); }
        else if let Some(start) = start.take() { out.push((start, &text[start..at])); }
    }
    if let Some(start) = start { out.push((start, &text[start..])); }
    out.into_iter()
}
fn word_char(ch: char) -> bool { ch.is_alphanumeric() || unicode_normalization::char::is_combining_mark(ch) || ch == '_' }
fn invalid(message: &str) -> OpdfError { OpdfError::Unsupported(message.into()) }

fn tokens(query: &str) -> Result<Vec<Token>> {
    let mut chars = query.chars().peekable();
    let mut out = Vec::new();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() { continue; }
        out.push(match ch {
            '(' => Token::Open,
            ')' => Token::Close,
            '"' => {
                let mut text = String::new();
                let mut closed = false;
                while let Some(ch) = chars.next() {
                    if ch == '"' { closed = true; break; }
                    if ch == '\\' { text.push(chars.next().ok_or_else(|| invalid("Finish the quoted phrase"))?); }
                    else { text.push(ch); }
                }
                if !closed || text.trim().is_empty() { return Err(invalid("Use a complete, nonempty quoted phrase")); }
                Token::Text(text)
            }
            _ => {
                let mut text = ch.to_string();
                while chars.peek().is_some_and(|c| !c.is_whitespace() && !matches!(c, '(' | ')' | '"')) {
                    text.push(chars.next().expect("peeked character"));
                }
                match text.as_str() { "AND" => Token::And, "OR" => Token::Or, "NOT" => Token::Not, _ => Token::Text(text) }
            }
        });
        if out.len() > 128 { return Err(invalid("Use at most 128 Boolean query terms and operators")); }
    }
    Ok(out)
}

struct Parser { tokens: Vec<Token>, at: usize }
impl Parser {
    fn or(&mut self, depth: usize) -> Result<Expr> {
        let mut left = self.and(depth)?;
        while self.tokens.get(self.at) == Some(&Token::Or) {
            self.at += 1;
            left = Expr::Or(Box::new(left), Box::new(self.and(depth)?));
        }
        Ok(left)
    }
    fn and(&mut self, depth: usize) -> Result<Expr> {
        let mut left = self.atom(depth)?;
        loop {
            match self.tokens.get(self.at) {
                Some(Token::And) => self.at += 1,
                Some(Token::Text(_) | Token::Not | Token::Open) => {},
                _ => break,
            }
            left = Expr::And(Box::new(left), Box::new(self.atom(depth)?));
        }
        Ok(left)
    }
    fn atom(&mut self, depth: usize) -> Result<Expr> {
        if depth > 32 { return Err(invalid("Use at most 32 nested Boolean groups")); }
        let token = self.tokens.get(self.at).cloned();
        self.at += 1;
        match token {
            Some(Token::Text(text)) => Ok(Expr::Text(text)),
            Some(Token::Not) => Ok(Expr::Not(Box::new(self.atom(depth + 1)?))),
            Some(Token::Open) => {
                let expr = self.or(depth + 1)?;
                if self.tokens.get(self.at) != Some(&Token::Close) { return Err(invalid("Close the Boolean parenthesis")); }
                self.at += 1;
                Ok(expr)
            }
            _ => Err(invalid("Put a word or quoted phrase after each Boolean operator")),
        }
    }
}

fn intersect<'a>(sets: impl Iterator<Item = Option<&'a Pages>>, all: &Pages) -> Pages {
    let Some(mut sets) = sets.collect::<Option<Vec<_>>>() else { return Pages::new(); };
    sets.sort_by_key(|set| set.len());
    match sets.first() {
        Some(first) => first.iter().copied().filter(|page| sets.iter().skip(1).all(|set| set.contains(page))).collect(),
        None => all.clone(),
    }
}

pub(super) struct Query {
    options: SearchOptions,
    expr: Expr,
    stemmer: Option<Stemmer>,
    nearby: Vec<String>,
}

impl Query {
    pub(super) fn compile(query: &str, options: SearchOptions) -> Result<Self> {
        if query.trim().is_empty() { return Err(invalid("Type the words to find")); }
        if query.len() > 4096 { return Err(invalid("Use a search query of at most 4096 bytes")); }
        if options.english_stemming && options.case_sensitive { return Err(invalid("Turn off Match case to use English word stems")); }
        let mut prepared = Self {
            options, expr: Expr::Text(String::new()), nearby: Vec::new(),
            stemmer: options.english_stemming.then(|| Stemmer::create(Algorithm::English)),
        };
        let text = if options.case_sensitive { super::canonical(query) } else { super::normalized(query) };
        if text.is_empty() { return Err(invalid("Type the words to find")); }
        prepared.expr = match options.mode {
            SearchMode::Boolean => {
                let mut parser = Parser { tokens: tokens(&super::canonical(query))?, at: 0 };
                let expr = parser.or(0)?;
                if parser.at != parser.tokens.len() { return Err(invalid("Unexpected Boolean operator or parenthesis")); }
                expr
            }
            SearchMode::AllWords | SearchMode::AnyWords | SearchMode::Proximity => {
                let terms: Vec<_> = words(&text).map(|(_, w)| w.to_string()).collect();
                if terms.is_empty() { return Err(invalid("Type at least one word")); }
                if terms.len() > 128 { return Err(invalid("Use at most 128 search words")); }
                if options.mode == SearchMode::Proximity && terms.len() < 2 { return Err(invalid("Proximity needs at least two words")); }
                prepared.nearby = terms.clone();
                let mut iter = terms.into_iter().map(Expr::Text);
                let first = iter.next().expect("nonempty terms");
                iter.fold(first, |left, right| if options.mode == SearchMode::AnyWords { Expr::Or(Box::new(left), Box::new(right)) } else { Expr::And(Box::new(left), Box::new(right)) })
            }
            SearchMode::Phrase => Expr::Text(text),
        };
        Ok(prepared)
    }

    fn term(&self, text: &str) -> String {
        let text = if self.options.case_sensitive { super::canonical(text) } else { super::normalized(text) };
        match &self.stemmer { Some(stemmer) => stemmer.stem(&text).into_owned(), None => text }
    }

    pub(super) fn candidates(&self, grams: &Index, word_index: &Index, stems: &Index, all: &Pages) -> Pages {
        fn visit(query: &Query, expr: &Expr, grams: &Index, word_index: &Index, stems: &Index, all: &Pages) -> Pages {
            match expr {
                Expr::Text(text) => {
                    let text = super::normalized(text);
                    let terms: Vec<_> = words(&text).map(|(_, w)| w).collect();
                    if query.stemmer.is_some() || query.options.mode != SearchMode::Phrase || query.options.whole_words {
                        if terms.is_empty() { return all.clone(); }
                        let index = if query.stemmer.is_some() { stems } else { word_index };
                        let keys: Vec<_> = terms.into_iter().map(|word| query.stemmer.as_ref().map_or_else(|| word.to_string(), |s| s.stem(word).into_owned())).collect();
                        intersect(keys.iter().map(|key| index.get(key)), all)
                    } else {
                        let keys = super::grams(&text);
                        intersect(keys.iter().map(|key| grams.get(key)), all)
                    }
                }
                Expr::And(a, b) => {
                    let mut pages = visit(query, a, grams, word_index, stems, all);
                    let other = visit(query, b, grams, word_index, stems, all);
                    pages.retain(|p| other.contains(p)); pages
                }
                Expr::Or(a, b) => {
                    let mut pages = visit(query, a, grams, word_index, stems, all);
                    pages.extend(visit(query, b, grams, word_index, stems, all)); pages
                }
                // Candidate pruning may have false positives. Negation must use the universe,
                // rather than exclude pages whose grams match but whose phrase does not.
                Expr::Not(_) => all.clone(),
            }
        }
        visit(self, &self.expr, grams, word_index, stems, all)
    }

    fn find(&self, text: &str, needle: &str) -> Option<usize> {
        let needle = if self.options.case_sensitive { super::canonical(needle) } else { super::normalized(needle) };
        if self.stemmer.is_some() {
            let terms: Vec<_> = words(needle.as_str()).map(|(_, w)| self.term(w)).collect();
            let page: Vec<_> = words(text).collect();
            if terms.is_empty() { return None; }
            return page.windows(terms.len()).find(|window| window.iter().zip(&terms).all(|((_, w), term)| self.term(w) == *term)).map(|window| window[0].0);
        }
        text.match_indices(&needle).find(|(at, _)| {
            let whole = self.options.whole_words || self.options.mode != SearchMode::Phrase;
            !whole || (text[..*at].chars().next_back().is_none_or(|c| !word_char(c)) && text[at + needle.len()..].chars().next().is_none_or(|c| !word_char(c)))
        }).map(|(at, _)| at)
    }

    pub(super) fn matches(&self, text: &str) -> Option<usize> {
        fn visit(query: &Query, expr: &Expr, text: &str) -> Option<usize> {
            match expr {
                Expr::Text(needle) => query.find(text, needle),
                Expr::And(a, b) => { let a = visit(query, a, text)?; let b = visit(query, b, text)?; Some(a.min(b)) }
                Expr::Or(a, b) => visit(query, a, text).or_else(|| visit(query, b, text)),
                Expr::Not(inner) => visit(query, inner, text).is_none().then_some(0),
            }
        }
        let at = visit(self, &self.expr, text)?;
        if self.options.mode != SearchMode::Proximity { return Some(at); }
        let needed: BTreeMap<String, usize> = self.nearby.iter().fold(BTreeMap::new(), |mut counts, word| { *counts.entry(self.term(word)).or_default() += 1; counts });
        let page: Vec<_> = words(text).collect();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut left = 0;
        for right in 0..page.len() {
            *counts.entry(self.term(page[right].1)).or_default() += 1;
            while needed.iter().all(|(word, n)| counts.get(word).copied().unwrap_or(0) >= *n) {
                if right - left + 1 - self.nearby.len() <= self.options.proximity { return Some(page[left].0); }
                if let Some(n) = counts.get_mut(&self.term(page[left].1)) { *n -= 1; }
                left += 1;
            }
        }
        None
    }
}
