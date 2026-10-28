//! Acrobat's Catalog: manage a saved collection and search it without reparsing its PDFs.
use super::*;
use sheaf_core::search_index::{Catalog, Hit, SearchMode, SearchOptions};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub(super) struct CatalogPanel {
    pub(super) files: Vec<PathBuf>,
    catalog: Option<Arc<Catalog>>,
    previous: Option<Arc<Catalog>>,
    pub(super) results: Vec<Hit>,
    pub(super) status: String,
    pub(super) busy: bool,
    cancel: Arc<AtomicBool>,
}

impl Drop for CatalogPanel {
    fn drop(&mut self) { self.cancel.store(true, Ordering::Relaxed); }
}

pub fn catalog_open(tab_id: i32) {
    update(tab_id, |r, _| {
        let path = r.path.clone();
        r.catalog_panel.get_or_insert_with(|| {
            // The panel stops its search when dropped (Drop), so no struct update.
            #[allow(clippy::field_reassign_with_default)]
            let mut panel = CatalogPanel::default();
            panel.files = vec![path];
            panel.status = "Choose saved PDFs, then build a catalog.".into();
            panel
        });
    });
}

fn start(tab_id: i32, status: &str) -> Option<Arc<AtomicBool>> {
    update(tab_id, |r, _| {
        let panel = r.catalog_panel.as_mut()?;
        if panel.busy { return None; }
        panel.cancel.store(true, Ordering::Relaxed);
        panel.cancel = Arc::new(AtomicBool::new(false));
        panel.busy = true;
        panel.status = status.into();
        panel.results.clear();
        Some(panel.cancel.clone())
    }).flatten()
}

fn current(panel: &CatalogPanel, cancel: &Arc<AtomicBool>) -> bool {
    Arc::ptr_eq(&panel.cancel, cancel) && !cancel.load(Ordering::Relaxed)
}

fn add_files(files: &mut Vec<PathBuf>, added: Vec<PathBuf>) {
    for path in added {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if !files.contains(&path) { files.push(path); }
    }
}

pub fn catalog_action(ui: &AppWindow, id: i32, action: i32, index: i32, query: &str) {
    if action == 0 { update(id, |r, _| r.catalog_panel = None); return; }
    if action == 7 {
        update(id, |r, _| {
            if let Some(panel) = r.catalog_panel.as_mut() {
                panel.cancel.store(true, Ordering::Relaxed);
                panel.busy = false;
                panel.status = "Cancelled. The previous catalog is kept.".into();
            }
        });
        return;
    }
    if update(id, |r, _| r.catalog_panel.as_ref().is_none_or(|p| p.busy)).unwrap_or(true) { return; }
    match action {
        1 => {
            let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Add PDFs to the catalog")).add_filter("PDF", &["pdf"]);
            crate::dialog::pick_files(ui, d, move |_, paths| {
                update(id, |r, _| {
                    if let Some(panel) = r.catalog_panel.as_mut().filter(|p| !p.busy) {
                        add_files(&mut panel.files, paths);
                        panel.previous = panel.catalog.take().or_else(|| panel.previous.take());
                        panel.results.clear();
                        panel.status = "Sources changed. Build the catalog to search.".into();
                    }
                });
            });
        }
        2 => { update(id, |r, _| {
            if let Some(panel) = r.catalog_panel.as_mut() && let Ok(k) = usize::try_from(index) && k < panel.files.len() {
                panel.files.remove(k);
                panel.previous = panel.catalog.take().or_else(|| panel.previous.take());
                panel.results.clear();
                panel.status = "Sources changed. Build the catalog to search.".into();
            }
        }); }
        3 => build(ui, id),
        4 => load(ui, id),
        5 => match search_options(index) {
            Ok(options) => search(ui, id, query.trim(), options),
            Err(error) => crate::flow::toast(ui, error),
        },
        8 => {
            let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Add a folder of PDFs (including subfolders)"));
            crate::dialog::pick_folder(ui, d, move |_, folder| {
                update(id, |r, _| {
                    if let Some(panel) = r.catalog_panel.as_mut().filter(|p| !p.busy) {
                        add_files(&mut panel.files, vec![folder]);
                        panel.previous = panel.catalog.take().or_else(|| panel.previous.take());
                        panel.results.clear();
                        panel.status = "Folder added, including subfolders. Build to index its PDFs.".into();
                    }
                });
            });
        },
        6 => {
            let hit = update(id, |r, _| r.catalog_panel.as_ref()?.results.get(usize::try_from(index).ok()?).cloned()).flatten();
            if let Some(hit) = hit {
                crate::home::remember_page(&hit.path, hit.page);
                // A result's source may already be open; explicitly navigate that tab too.
                let canon = std::fs::canonicalize(&hit.path).unwrap_or_else(|_| hit.path.clone());
                let open_id = with(|a| a.tabs.iter().find(|t| t.reader_file.as_ref() == Some(&canon)).map(|t| t.id));
                crate::deliver(vec![hit.path]);
                if let Some(open_id) = open_id {
                    update(open_id, |_, v| v.show_page(hit.page, None));
                    crate::viewer_ui::request(ui, open_id);
                }
            }
        }
        _ => {}
    }
}

fn build(ui: &AppWindow, id: i32) {
    let Some(files) = update(id, |r, _| r.catalog_panel.as_ref().map(|p| p.files.clone())).flatten() else { return };
    if files.is_empty() { return crate::flow::toast(ui, &sheaf_app::i18n::tr("Add at least one PDF first")); }
    let previous = update(id, |r, _| r.catalog_panel.as_ref().and_then(|p| p.catalog.clone().or_else(|| p.previous.clone()))).flatten();
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Build search catalog")).set_file_name(".sheaf-index.json").add_filter("Sheaf search catalog", &["json"]);
    crate::dialog::save_file(ui, d, move |ui, out| {
        let Some(cancel) = start(id, "Building the catalog…") else { return };
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let inputs: Vec<_> = files.iter().map(Input::new).collect();
            let mut progress = |at: usize, total: usize| {
                let progress = sheaf_app::i18n::trf("Indexing PDF {} of {}…", &[&at, &total]);
                let flag = cancel.clone();
                let _ = weak.upgrade_in_event_loop(move |_| {
                    update(id, |r, _| {
                        if let Some(p) = r.catalog_panel.as_mut().filter(|p| current(p, &flag)) { p.status = progress; }
                    });
                });
                !cancel.load(Ordering::Relaxed)
            };
            let result = match previous {
                Some(catalog) => catalog.update(&inputs, &mut progress),
                None => Catalog::build(&inputs, &mut progress),
            }.and_then(|catalog| {
                if cancel.load(Ordering::Relaxed) { return Err(sheaf_core::OpdfError::Cancelled); }
                let saved = catalog.save(&out)?;
                Ok((catalog, saved))
            });
            let _ = weak.upgrade_in_event_loop(move |_| {
                update(id, |r, _| {
                    let Some(p) = r.catalog_panel.as_mut().filter(|p| current(p, &cancel)) else { return };
                    p.busy = false;
                    match result {
                        Ok((catalog, saved)) => {
                            p.files = catalog.sources().into_iter().map(|(path, _)| path).collect();
                            p.catalog = Some(Arc::new(catalog));
                            p.status = sheaf_app::i18n::trf("Saved {} · {} PDFs", &[&saved.display(), &p.files.len()]);
                        }
                        Err(error) => p.status = sheaf_app::i18n::trf("Catalog not built: {}", &[&error]),
                    }
                });
            });
        });
    });
}

fn load(ui: &AppWindow, id: i32) {
    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Open search catalog")).add_filter("Sheaf search catalog", &["json"]);
    crate::dialog::pick_file(ui, d, move |ui, path| {
        let Some(cancel) = start(id, "Opening the catalog…") else { return };
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let result = Catalog::load(&path);
            let _ = weak.upgrade_in_event_loop(move |_| {
                update(id, |r, _| {
                    let Some(p) = r.catalog_panel.as_mut().filter(|p| current(p, &cancel)) else { return };
                    p.busy = false;
                    match result {
                        Ok(catalog) => {
                            p.previous = None;
                            p.files = catalog.sources().into_iter().map(|(path, _)| path).collect();
                            p.catalog = Some(Arc::new(catalog));
                            p.status = sheaf_app::i18n::trf("Opened {} · {} PDFs", &[&path.display(), &p.files.len()]);
                        }
                        Err(error) => p.status = sheaf_app::i18n::trf("Cannot open catalog: {}", &[&error]),
                    }
                });
            });
        });
    });
}

fn search_options(flags: i32) -> Result<SearchOptions, &'static str> {
    Ok(SearchOptions {
        mode: match flags & 7 { 0 => SearchMode::Phrase, 1 => SearchMode::AllWords, 2 => SearchMode::AnyWords, 3 => SearchMode::Boolean, 4 => SearchMode::Proximity, _ => return Err("Choose a search mode") },
        whole_words: flags & 8 != 0, case_sensitive: flags & 16 != 0, english_stemming: flags & 32 != 0,
        ..Default::default()
    })
}

fn search(ui: &AppWindow, id: i32, query: &str, options: SearchOptions) {
    if query.is_empty() { return crate::flow::toast(ui, &sheaf_app::i18n::tr("Type the words to find first")); }
    let Some(catalog) = update(id, |r, _| r.catalog_panel.as_ref()?.catalog.clone()).flatten() else {
        return crate::flow::toast(ui, &sheaf_app::i18n::tr("Build or open a catalog first"));
    };
    let Some(cancel) = start(id, "Searching the catalog…") else { return };
    let query = query.to_owned();
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let result = catalog.search_with_options(&query, options);
        let _ = weak.upgrade_in_event_loop(move |_| {
            update(id, |r, _| {
                let Some(p) = r.catalog_panel.as_mut().filter(|p| current(p, &cancel)) else { return };
                p.busy = false;
                match result {
                    Ok(mut hits) => {
                        p.status = if hits.len() > 500 { sheaf_app::i18n::trf("{} pages found · first 500 shown", &[&hits.len()]) } else { sheaf_app::i18n::trf("{} pages found", &[&hits.len()]) };
                        let stale = catalog.stale_sources().len();
                        if stale > 0 { p.status.push_str(&sheaf_app::i18n::trf(" · {} changed/missing PDFs excluded; update the catalog", &[&stale])); }
                        hits.truncate(500);
                        p.results = hits;
                    }
                    Err(error) => p.status = sheaf_app::i18n::trf("Search unavailable: {}. Rebuild if a source changed or moved.", &[&error]),
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_search_controls_keep_mode_and_independent_options() {
        let options = search_options(4 + 8 + 32).unwrap();
        assert_eq!(options.mode, SearchMode::Proximity);
        assert!(options.whole_words && options.english_stemming && !options.case_sensitive);
        assert_eq!(options.proximity, 10);
        assert!(search_options(7).is_err());
    }
    #[test]
    fn closing_a_catalog_cancels_work_and_reopening_cannot_accept_its_reply() {
        let panel = CatalogPanel::default();
        let old = panel.cancel.clone();
        drop(panel);
        assert!(old.load(Ordering::Relaxed));
        assert!(!current(&CatalogPanel::default(), &old));
    }
    #[test]
    fn catalog_selection_deduplicates_sources_in_order() {
        let mut files = Vec::new();
        add_files(&mut files, vec![PathBuf::from("absent-a.pdf"), PathBuf::from("absent-b.pdf"), PathBuf::from("absent-a.pdf")]);
        assert_eq!(files, [PathBuf::from("absent-a.pdf"), PathBuf::from("absent-b.pdf")]);
    }
}
