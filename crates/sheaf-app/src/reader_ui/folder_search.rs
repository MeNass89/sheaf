//! The search panel's In a folder… (Acrobat's Advanced Search ▸ All PDF documents in a folder): the
//! words searched in every PDF of a folder chosen (and the folders in it), the pages they are on
//! listed with a line around each; a result opens its file at its page.

use super::*;

/// How many results one search lists at most. Every PDF of the folder is read, as Acrobat's Advanced
/// Search does (Sol, 27/10: only the first 500 files were, silently); a search stopped at this many
/// pages found says so, and how many files it had read.
const MAX_RESULTS: usize = 2000;

/// A folder searched: its name, the words, the search still running, and the results (file, page
/// 0-based, the line around the words).
#[derive(Default)]
pub(super) struct FolderSearch {
    pub(super) folder: String,
    pub(super) query: String,
    pub(super) busy: bool,
    pub(super) results: Vec<(PathBuf, usize, String)>,
    /// PDFs read, and whether the search stopped at `MAX_RESULTS` before the last one.
    pub(super) files: usize,
    pub(super) stopped: bool,
    /// PDFs that could not be read, with why (a password, a damaged file), listed as Acrobat lists
    /// the documents it skipped; they are not counted as searched (Sol, 28/10).
    pub(super) skipped: Vec<(PathBuf, String)>,
}

impl FolderSearch {
    /// The line under the title: searching, nothing, or how many pages in how many PDFs, and that it
    /// stopped early when it did.
    pub(super) fn status(&self) -> String {
        let pdfs = |n: usize| if n == 1 { "1 PDF".to_string() } else { sheaf_app::i18n::trf("{} PDFs", &[&n]) };
        let pages = |n: usize| if n == 1 { "1 page".to_string() } else { sheaf_app::i18n::trf("{} pages", &[&n]) };
        let line = if self.busy {
            return "Searching…".into();
        } else if self.results.is_empty() {
            sheaf_app::i18n::trf("Not found in the folder's {} read.", &[&pdfs(self.files)])
        } else if self.stopped {
            sheaf_app::i18n::trf("Stopped at the first {} found, after {} — type more words to narrow it.", &[&pages(self.results.len()), &pdfs(self.files)])
        } else {
            sheaf_app::i18n::trf("{} found in {}", &[&pages(self.results.len()), &pdfs(self.files)])
        };
        match self.skipped.len() {
            0 => line,
            n => sheaf_app::i18n::trf("{}{} {} could not be searched (listed below).", &[&line, &if line.ends_with('.') { "" } else { "." }, &pdfs(n)]),
        }
    }
}

/// What one search found: the pages, the PDFs read, whether it stopped at `MAX_RESULTS`, and the
/// PDFs it could not read, with why.
pub(super) struct Found {
    pub(super) results: Vec<(PathBuf, usize, String)>,
    pub(super) files: usize,
    pub(super) stopped: bool,
    pub(super) skipped: Vec<(PathBuf, String)>,
}

/// In a folder…: a folder chosen, the search panel's words searched in it off the UI thread.
pub fn folder_search(ui: &AppWindow, tab_id: i32, query: &str) {
    let query = query.trim().to_string();
    if query.is_empty() {
        return crate::flow::toast(ui, &sheaf_app::i18n::tr("Type the words to find first"));
    }
    let Some(dir) = update(tab_id, |r, _| crate::dest::parent_dir(&r.path)) else { return };
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Search the PDFs of a folder")).set_directory(&dir);
    crate::dialog::pick_folder(ui, d, move |ui, folder| {
        update(tab_id, |r, _| {
            r.folder_search = Some(FolderSearch { folder: file_name(&folder), query: query.clone(), busy: true, ..Default::default() });
        });
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let found = search_folder(&folder, &query, MAX_RESULTS);
            let _ = weak.upgrade_in_event_loop(move |_| {
                update(tab_id, |r, _| {
                    if let Some(f) = r.folder_search.as_mut() {
                        f.busy = false;
                        f.results = found.results;
                        f.files = found.files;
                        f.stopped = found.stopped;
                        f.skipped = found.skipped;
                    }
                });
            });
        });
    });
}

/// The pages of every PDF under `folder` holding `query` (any case), in file order, at most `max`.
pub(super) fn search_folder(folder: &std::path::Path, query: &str, max: usize) -> Found {
    let mut files = Vec::new();
    let mut dirs = vec![folder.to_path_buf()];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
                files.push(p);
            }
        }
    }
    // An optional catalog built with `sheaf index … -o .sheaf-index.json` skips repeated
    // PDF parsing. Changed or unindexed files are read normally, never omitted.
    let catalog = sheaf_core::search_index::Catalog::load(&folder.join(".sheaf-index.json")).ok();
    let mut found = Found { results: Vec::new(), files: 0, stopped: false, skipped: Vec::new() };
    for f in files {
        let cached = catalog.as_ref().and_then(|c| c.texts(&f));
        let pages = match cached {
            Some(pages) => pages.to_vec(),
            None => match sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&f), &[]) {
                Ok(pages) => pages,
                Err(e) => {
                    found.skipped.push((f, skip_reason(&e)));
                    continue;
                }
            },
        };
        found.files += 1;
        for (page, text) in pages.iter().enumerate() {
            for snippet in page_matches(text, query) {
                if found.results.len() >= max {
                    found.stopped = true;
                    return found;
                }
                found.results.push((f.clone(), page, snippet));
            }
        }
    }
    found
}

/// Why a PDF was not searched, in the words of Acrobat's list of skipped documents.
fn skip_reason(e: &sheaf_core::OpdfError) -> String {
    match e {
        sheaf_core::OpdfError::PasswordRequired(_) | sheaf_core::OpdfError::WrongPassword(_) => "Protected by a password".into(),
        _ => "Damaged or not a PDF: it could not be read".into(),
    }
}

/// A word as compared: lower case, its common English endings taken off ("forms", "formed",
/// "forming" → "form"), as Acrobat's search matches a word's forms (Sol, 26/10: exact only): a
/// consonant doubled before -ing/-ed made single ("running" → "run"), -ies/-ied made -y
/// ("studies" → "study"), and a final e dropped, so "create" and "creating" meet (Sol, 27/10).
fn stem(w: &str) -> String {
    let w = w.to_lowercase();
    let long = |s: &str| s.chars().count() >= 3;
    let mut base = None;
    for end in ["ies", "ied"] {
        if let Some(s) = w.strip_suffix(end).filter(|s| s.chars().count() >= 2) {
            base = Some(format!("{s}y"));
            break;
        }
    }
    if base.is_none() {
        for end in ["ing", "ed"] {
            if let Some(s) = w.strip_suffix(end).filter(|s| long(s)) {
                let c: Vec<char> = s.chars().collect();
                let n = c.len();
                let doubled = n >= 4 && c[n - 1] == c[n - 2] && !"aeioulsz".contains(c[n - 1]);
                base = Some(if doubled { c[..n - 1].iter().collect() } else { s.to_string() });
                break;
            }
        }
    }
    let base = base.unwrap_or_else(|| match w.strip_suffix("es").filter(|s| long(s) && !s.ends_with('e')) {
        Some(s) if s.ends_with(['s', 'x', 'z']) || s.ends_with("ch") || s.ends_with("sh") => s.to_string(),
        _ => w.strip_suffix('s').filter(|s| long(s) && !s.ends_with('s')).unwrap_or(&w).to_string(),
    });
    if let Some(s) = base.strip_suffix('e').filter(|s| s.chars().count() >= 3) {
        return s.to_string();
    }
    base
}

/// Where `query` is on a page's `text`: its words in a row (across line ends: Sol, 26/10), each in
/// any of its forms; failing that, all its words within 8 words of each other. The words around
/// each place found, at most 120 characters.
pub(crate) fn page_matches(text: &str, query: &str) -> Vec<String> {
    // The page as one run of words (a hyphen at a line's end joins the word).
    let joined = text.replace("-\n", "").replace('\n', " ");
    let words: Vec<&str> = joined.split_whitespace().collect();
    let clean = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
    let stems: Vec<String> = words.iter().map(|w| stem(&clean(w))).collect();
    let q: Vec<String> = query.split_whitespace().map(|w| stem(&clean(w))).filter(|w| !w.is_empty()).collect();
    if q.is_empty() {
        return Vec::new();
    }
    let around = |at: usize, len: usize| {
        let from = at.saturating_sub(6);
        let to = (at + len + 6).min(words.len());
        words[from..to].join(" ").chars().take(120).collect::<String>()
    };
    let phrase: Vec<usize> = (0..stems.len().saturating_sub(q.len() - 1)).filter(|&i| stems[i..i + q.len()] == q[..]).collect();
    if !phrase.is_empty() {
        return phrase.into_iter().map(|i| around(i, q.len())).collect();
    }
    // Near each other: a window of 8 words holding them all, once.
    if q.len() > 1 {
        for i in 0..stems.len() {
            let window = &stems[i..(i + 8).min(stems.len())];
            if window.first().is_some_and(|w| q.contains(w)) && q.iter().all(|w| window.contains(w)) {
                return vec![around(i, 8)];
            }
        }
    }
    Vec::new()
}

/// A result clicked (`k`), or the sheet closed (−1): its file opens at its page.
pub fn folder_result(ui: &AppWindow, tab_id: i32, k: i32) {
    let picked = update(tab_id, |r, _| {
        let f = r.folder_search.take()?;
        f.results.into_iter().nth(usize::try_from(k).ok()?)
    })
    .flatten();
    let Some((file, page, _)) = picked else { return };
    // Opened at that page (a file opened again goes back to its last page).
    crate::home::remember_page(&file, page);
    let canon = std::fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
    let open = with(|a| a.tabs.iter().find(|t| t.reader_file.as_ref() == Some(&canon)).map(|t| t.id));
    crate::deliver(vec![file]);
    if let Some(id) = open {
        update(id, |_, v| v.show_page(page, None));
        crate::viewer_ui::request(ui, id);
    }
}

#[cfg(test)]
pub fn folder_search_for_test(folder: &std::path::Path, query: &str) -> Vec<(PathBuf, usize, String)> {
    search_folder(folder, query, MAX_RESULTS).results
}

/// The search with its own limit, the status line it ends on, and the PDFs skipped (name, why).
#[cfg(test)]
pub fn folder_search_status_for_test(folder: &std::path::Path, query: &str, max: usize) -> (usize, String, Vec<(String, String)>) {
    let found = search_folder(folder, query, max);
    let skipped = found.skipped.iter().map(|(p, why)| (file_name(p), why.clone())).collect();
    let f = FolderSearch { folder: String::new(), query: query.into(), busy: false, results: found.results, files: found.files, stopped: found.stopped, skipped: found.skipped };
    (f.results.len(), f.status(), skipped)
}

#[cfg(test)]
mod match_tests {
    #[test]
    fn a_phrase_is_found_across_lines_in_any_of_its_forms_or_its_words_near_each_other() {
        let page = "The invoice was\nsigned by both parties.\nPayment terms are thirty days after delivery of the goods.";
        assert_eq!(super::page_matches(page, "was signed").len(), 1, "across the line end");
        assert_eq!(super::page_matches(page, "sign").len(), 1, "signed is a form of sign");
        assert_eq!(super::page_matches(page, "payment days").len(), 1, "near each other");
        assert!(super::page_matches(page, "payment goods parties").is_empty(), "too far apart");
        assert!(super::page_matches("A hyphen-\nated word", "hyphenated")[0].contains("hyphenated"));
    }

    /// A word's forms meet whatever their ending does to it (Sol, 27/10).
    #[test]
    fn a_words_forms_have_one_stem() {
        for forms in [
            &["run", "runs", "running"][..],
            &["create", "creates", "created", "creating"],
            &["study", "studies", "studied"],
            &["box", "boxes"],
            &["class", "classes"],
            &["form", "forms", "formed", "forming"],
            &["stop", "stopped", "stopping"],
            &["make", "makes", "making"],
        ] {
            let stems: Vec<String> = forms.iter().map(|w| super::stem(w)).collect();
            assert!(stems.iter().all(|s| *s == stems[0]), "{forms:?} → {stems:?}");
        }
        assert_ne!(super::stem("fill"), super::stem("file"), "fill keeps its two l");
        assert_eq!(super::page_matches("The script was running all night.", "run").len(), 1);
    }
}
