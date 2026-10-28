//! Plugins on the window (the PluginBridge global): Preferences ▸ Plugins, their tools on the
//! Tools page (cards "plugin:<plugin>/<tool>") and the warning before a plugin's first run. The
//! rules and the runs are in `sheaf_app::plugins`.

use sheaf_app::i18n;
use sheaf_app::plugins::{self, Plugin, PluginTool};
use sheaf_app::{AppWindow, PluginBridge, PluginRow, ToolCard};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;

/// What waits for the warning's answer: the plugin's version key and what to do once allowed.
type Pending = (String, Box<dyn FnOnce(&AppWindow)>);
thread_local! {
    static ASKED: RefCell<Option<Pending>> = const { RefCell::new(None) };
}

/// The Tools page's card id of a plugin tool.
pub const CARD: &str = "plugin:";

pub fn push(ui: &AppWindow) {
    let dir = plugins::folder();
    let (ok, bad) = plugins::list(&dir);
    let rows: Vec<PluginRow> = ok
        .iter()
        .map(|p| PluginRow {
            id: p.manifest.id.clone().into(),
            name: p.manifest.name.clone().into(),
            about: if p.manifest.author.is_empty() { p.manifest.version.clone() } else { i18n::trf("{} · by {}", &[&p.manifest.version, &p.manifest.author]) }.into(),
            description: p.manifest.description.clone().into(),
            tools: p.manifest.tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ").into(),
            enabled: p.enabled,
            allowed: p.allowed,
        })
        .collect();
    let g = ui.global::<PluginBridge>();
    g.set_rows(ModelRc::new(VecModel::from(rows)));
    g.set_broken(ModelRc::new(VecModel::from(
        bad.iter().map(|(d, why)| SharedString::from(i18n::trf("{}: {}", &[&d.file_name().unwrap_or_default().to_string_lossy(), why]))).collect::<Vec<_>>(),
    )));
    g.set_folder(dir.display().to_string().into());
}

/// The enabled plugins' tools as Tools page cards.
pub fn cards() -> Vec<ToolCard> {
    let (ok, _) = plugins::list(&plugins::folder());
    ok.iter()
        .filter(|p| p.enabled)
        .flat_map(|p| {
            p.manifest.tools.iter().map(move |t| ToolCard {
                id: format!("{CARD}{}/{}", p.manifest.id, t.id).into(),
                name: t.name.clone().into(),
                category: 0,
                icon: sheaf_app::icons::icon_image("puzzle"),
                tile: slint::Color::from_rgb_u8(0x5B, 0x6C, 0x8F),
                enabled: true,
                favourite: false,
            })
        })
        .collect()
}

/// The enabled plugins' tools for the action editor: "plugin/tool" ids and their labels.
pub fn action_tools() -> Vec<(String, String)> {
    let (ok, _) = plugins::list(&plugins::folder());
    ok.iter().filter(|p| p.enabled).flat_map(|p| p.manifest.tools.iter().map(move |t| (format!("{}/{}", p.manifest.id, t.id), i18n::trf("Plugin ▸ {} ▸ {}", &[&p.manifest.name, &t.name])))).collect()
}

fn command_line(p: &Plugin, t: &PluginTool) -> String {
    t.run.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() }).collect::<Vec<_>>().join(" ") + &i18n::trf(" (in {})", &[&p.dir.display()])
}

/// Runs `then` once the plugin may run: at once when this version was allowed, else after the
/// warning is answered Allow.
pub fn when_allowed(ui: &AppWindow, plugin: &Plugin, tool: &PluginTool, then: impl FnOnce(&AppWindow) + 'static) {
    if plugin.allowed {
        return then(ui);
    }
    let g = ui.global::<PluginBridge>();
    g.set_ask_author(plugin.manifest.author.clone().into());
    g.set_ask_command(command_line(plugin, tool).into());
    g.set_ask_name(plugin.manifest.name.clone().into());
    ASKED.with(|a| *a.borrow_mut() = Some((plugin.key(), Box::new(then))));
}

/// A plugin card of the Tools page clicked: the PDFs (those dropped on Home, else picked) run
/// through the tool, each output written beside its file and opened.
pub fn open(ui: &AppWindow, id: &str) {
    let Some((plugin, tool)) = plugins::find(&plugins::folder(), id) else {
        return crate::flow::toast(ui, &i18n::tr("That plugin is missing or turned off (Preferences ▸ Plugins)"));
    };
    let preload: Vec<PathBuf> = crate::state::with(|a| std::mem::take(&mut a.home_files)).into_iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))).collect();
    let (p2, t2, tool_name) = (plugin.clone(), tool.clone(), tool.name.clone());
    when_allowed(ui, &plugin, &tool, move |ui| {
        let run_on = move |ui: &AppWindow, files: Vec<PathBuf>| start(ui, p2, t2, files);
        if !preload.is_empty() {
            return run_on(ui, preload);
        }
        let d = crate::dialog::new(ui).set_title(i18n::trf("PDFs for {}", &[&tool_name])).add_filter("PDF", &["pdf"]);
        crate::dialog::pick_files(ui, d, run_on);
    });
}

/// Where a tool's output goes: beside its file, "<name> - <tool>.pdf", numbered if taken.
pub fn output_path(input: &std::path::Path, tool: &str) -> PathBuf {
    let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Document".into());
    let safe: String = tool.chars().map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c }).collect();
    let dir = input.parent().map(PathBuf::from).unwrap_or_default();
    let mut out = dir.join(format!("{stem} - {safe}.pdf"));
    let mut n = 2;
    while out.exists() {
        out = dir.join(format!("{stem} - {safe} {n}.pdf"));
        n += 1;
    }
    out
}

fn start(ui: &AppWindow, plugin: Plugin, tool: PluginTool, files: Vec<PathBuf>) {
    let plugin = Plugin { allowed: true, ..plugin };
    crate::flow::toast(ui, &i18n::trf("{} is running…", &[&tool.name]));
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let mut done = Vec::new();
        let mut errors = Vec::new();
        for f in files {
            let result = std::fs::read(&f).map_err(|e| e.to_string()).and_then(|b| plugins::run(&plugin, &tool, &b, &|| true)).and_then(|out| {
                let to = output_path(&f, &tool.name);
                std::fs::write(&to, out).map(|_| to).map_err(|e| e.to_string())
            });
            match result {
                Ok(to) => done.push(to),
                Err(e) => errors.push(e),
            }
        }
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if let Some(e) = errors.first() {
                crate::flow::toast_for(&ui, e, 8);
            } else {
                crate::flow::toast(&ui, &i18n::trf("{}: {} written", &[&tool.name, &done.len()]));
            }
            if !done.is_empty() {
                crate::flow::home_files(&ui, done);
            }
        });
    });
}

pub fn install(ui: &AppWindow) {
    let g = ui.global::<PluginBridge>();
    let w = ui.as_weak();
    g.on_set_enabled(move |id, on| {
        let Some(ui) = w.upgrade() else { return };
        if let Err(e) = plugins::set_enabled(&plugins::folder(), &id, on) {
            crate::flow::toast(&ui, &e.to_string());
        }
        changed(&ui);
    });
    let w = ui.as_weak();
    g.on_remove(move |id| {
        let Some(ui) = w.upgrade() else { return };
        let dir = plugins::folder();
        if let Some(p) = plugins::list(&dir).0.into_iter().find(|p| p.manifest.id == id.as_str()) {
            match plugins::remove(&dir, &p) {
                Ok(()) => crate::flow::toast(&ui, &i18n::trf("{} removed", &[&p.manifest.name])),
                Err(e) => crate::flow::toast(&ui, &e.to_string()),
            }
        }
        changed(&ui);
    });
    let w = ui.as_weak();
    g.on_add(move || {
        let Some(ui) = w.upgrade() else { return };
        let d = crate::dialog::new(&ui).set_title(i18n::tr("A plugin folder (holding plugin.json)"));
        crate::dialog::pick_folder(&ui, d, |ui, from| {
            match plugins::install(&plugins::folder(), &from) {
                Ok(m) => crate::flow::toast(ui, &i18n::trf("{} {} added", &[&m.name, &m.version])),
                Err(e) => crate::flow::toast_for(ui, &e, 8),
            }
            changed(ui);
        });
    });
    g.on_open_folder(|| {
        let dir = plugins::folder();
        let _ = std::fs::create_dir_all(&dir);
        crate::run::open_detached(&dir);
    });
    let w = ui.as_weak();
    g.on_answer(move |allow| {
        let Some(ui) = w.upgrade() else { return };
        ui.global::<PluginBridge>().set_ask_name("".into());
        let Some((key, then)) = ASKED.with(|a| a.borrow_mut().take()) else { return };
        if allow {
            if let Err(e) = plugins::allow(&plugins::folder(), &key) {
                return crate::flow::toast(&ui, &e.to_string());
            }
            push(&ui);
            then(&ui);
        }
    });
    push(ui);
}

/// Plugins changed: Preferences, the Tools page and the action editor follow.
fn changed(ui: &AppWindow) {
    push(ui);
    crate::install_catalogue(ui);
    crate::actions_ui::plugins_changed(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outputs_go_beside_their_file_numbered_when_taken() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("report.pdf");
        let first = output_path(&input, "Mark: reviewed");
        assert_eq!(first, dir.path().join("report - Mark- reviewed.pdf"));
        std::fs::write(&first, b"x").unwrap();
        assert_eq!(output_path(&input, "Mark: reviewed"), dir.path().join("report - Mark- reviewed 2.pdf"));
    }
}
