//! The search panel's In a folder… (Acrobat's Advanced Search ▸ All PDF documents in a folder): the
//! words searched in every PDF of a folder chosen (and the folders in it), the pages they are on
//! listed with a line around each; a result opens its file at its page.

use super::*;

/// How many files and results one search reads and lists at most.
const MAX_FILES: usize = 500;
const MAX_RESULTS: usize = 500;

/// A folder searched: its name, the words, the search still running, and the results (file, page
/// 0-based, the line around the words).
#[derive(Default)]
pub(super) struct FolderSearch {
    pub(super) folder: String,
    pub(super) query: String,
    pub(super) busy: bool,
    pub(super) results: Vec<(PathBuf, usize, String)>,
}

/// In a folder…: a folder chosen, the search panel's words searched in it off the UI thread.
pub fn folder_search(ui: &AppWindow, tab_id: i32, query: &str) {
    let query = query.trim().to_string();
    if query.is_empty() {
        return crate::flow::toast(ui, "Type the words to find first");
    }
    let Some(dir) = update(tab_id, |r, _| crate::dest::parent_dir(&r.path)) else { return };
    let d = crate::dialog::new(ui).set_title("Search the PDFs of a folder").set_directory(&dir);
    crate::dialog::pick_folder(ui, d, move |ui, folder| {
        update(tab_id, |r, _| {
            r.folder_search = Some(FolderSearch { folder: file_name(&folder), query: query.clone(), busy: true, results: Vec::new() });
        });
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let results = search_folder(&folder, &query);
            let _ = weak.upgrade_in_event_loop(move |_| {
                update(tab_id, |r, _| {
                    if let Some(f) = r.folder_search.as_mut() {
                        f.busy = false;
                        f.results = results;
                    }
                });
            });
        });
    });
}

/// The pages of the PDFs under `folder` holding `query` (any case), in file order.
pub(super) fn search_folder(folder: &std::path::Path, query: &str) -> Vec<(PathBuf, usize, String)> {
    let mut files = Vec::new();
    let mut dirs = vec![folder.to_path_buf()];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) && files.len() < MAX_FILES {
                files.push(p);
            }
        }
    }
    let q = query.to_lowercase();
    let mut out = Vec::new();
    for f in files {
        let Ok(pages) = sheaf_core::text::page_texts(&sheaf_core::pages::Input::new(&f), &[]) else { continue };
        for (page, text) in pages.iter().enumerate() {
            for line in text.lines().filter(|l| l.to_lowercase().contains(&q)) {
                out.push((f.clone(), page, line.trim().chars().take(120).collect()));
                if out.len() >= MAX_RESULTS {
                    return out;
                }
            }
        }
    }
    out
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
    search_folder(folder, query)
}
