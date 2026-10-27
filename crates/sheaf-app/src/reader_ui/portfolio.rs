//! Acrobat's portfolio browser uses the reader's existing attachment open/save paths.
use super::*;
use sheaf_app::{PortfolioBridge, PortfolioRow};

pub fn browse(ui: &AppWindow, id: i32) {
    let Some(files) = update(id, |r, _| {
        r.snap.attachments.iter().map(|a| PortfolioRow {
            name: a.name.as_str().into(), detail: "Original embedded file".into(),
        }).collect::<Vec<_>>()
    }) else { return };
    let b = ui.global::<PortfolioBridge>();
    if b.get_busy() { return; }
    b.set_tab(id); b.set_browsing(true); b.set_visible(true);
    b.set_status(format!("{} embedded file(s)", files.len()).into());
    b.set_files(ModelRc::new(VecModel::from(files)));
}
