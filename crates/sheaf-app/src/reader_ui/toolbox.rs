//! The Tools popover (⌘K, the mode bar's wrench; DESIGN §3) and a tool run from the inspector
//! (§5.5). The popover lists every tool that takes the open document: the catalogue's PDF tools
//! by category, the plugins' tools and the custom tools, favourites first, searched as typed. A
//! tool whose run is a set of options (Compress, OCR, Protect, Crop, the exports…) shows them in
//! the inspector with one Run button and runs on the document; a tool that needs the page itself
//! (Organize, Edit, Sign, Redact) or more files (Merge, Compare) opens its tool tab with the
//! document in it. A plugin or custom tool runs at once, as it does from Home.
//!
//! The reader tab carries the tool's state (`TabState::tool`, `files`, `options`, `phase`): the
//! same job path as a tool tab (`run::run_tool`), so every tool's options panel is the one it has
//! there. When the job is done the result opens in its own tab and the reader's tool state is
//! cleared (`finished`).

use super::*;
use crate::state::Phase;
use sheaf_app::{ActionBridge, ToolRow};
use sheaf_core::tools::{self, Category, ToolId};

/// Custom tool rows: `custom:<row>` of `ActionBridge.commands`.
const CUSTOM: &str = "custom:";

/// Tools whose run is options only: they run from the inspector on the open document. The others
/// need the page (Organize, Edit, Sign, Redact) or more files (Merge, Compare): a tool tab.
pub fn in_inspector(t: ToolId) -> bool {
    !matches!(t, ToolId::Merge | ToolId::Compare | ToolId::Organize | ToolId::Edit | ToolId::Sign | ToolId::Redact)
}

fn row(id: &str, name: &str, detail: &str, icon: &str, tab: bool) -> ToolRow {
    ToolRow { id: id.into(), name: name.into(), detail: detail.into(), icon: sheaf_app::icons::icon_image(icon), header: false, tab }
}

fn header(name: &str) -> ToolRow {
    ToolRow { id: String::new().into(), name: name.into(), detail: String::new().into(), icon: Default::default(), header: true, tab: false }
}

fn catalogue_row(t: &tools::ToolInfo) -> ToolRow {
    let detail = if in_inspector(t.id) { String::new() } else { sheaf_app::i18n::tr("Opens in a tool tab") };
    row(t.id.as_str(), &sheaf_app::i18n::tr(t.name), &detail, t.icon, !in_inspector(t.id))
}

/// The plugins' tools and the custom tools that take the open document: (id, name, detail).
fn extra_rows(ui: &AppWindow) -> (Vec<ToolRow>, Vec<ToolRow>) {
    let plugins: Vec<ToolRow> = crate::plugins_ui::cards().iter().map(|c| row(&c.id, &c.name, &sheaf_app::i18n::tr("Plugin"), "puzzle", false)).collect();
    let commands = ui.global::<ActionBridge>().get_commands();
    let custom: Vec<ToolRow> = (0..commands.row_count())
        .filter_map(|i| commands.row_data(i).map(|c| (i, c)))
        .filter(|(_, c)| !c.separator && c.current && !c.interactive)
        .map(|(i, c)| ToolRow { id: format!("{CUSTOM}{i}").into(), name: c.label.clone(), detail: sheaf_app::i18n::tr("Custom tool").into(), icon: sheaf_app::icons::icon_image("zap"), header: false, tab: false })
        .collect();
    (plugins, custom)
}

/// The popover's rows for `query`. Nothing typed: Favourites, then each category, Plugins, Custom
/// tools. Typed: the catalogue's search order (sheaf_core::tools::search), then the plugins and
/// custom tools whose name holds the words. Only tools that take a PDF are listed; the converters
/// to PDF live on Home ▸ All Tools.
pub fn rows(ui: &AppWindow, query: &str) -> Vec<ToolRow> {
    let takes_pdf = |t: &tools::ToolInfo| t.accepts.contains(&"pdf");
    let (plugins, custom) = extra_rows(ui);
    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    if q.is_empty() {
        let favourites: Vec<ToolRow> = crate::home::favourites()
            .iter()
            .filter_map(|id| {
                ToolId::from_str(id).map(tools::find).filter(|t| takes_pdf(t)).map(catalogue_row).or_else(|| plugins.iter().find(|p| p.id.as_str() == id).cloned())
            })
            .collect();
        if !favourites.is_empty() {
            out.push(header(&sheaf_app::i18n::tr("Favourites")));
            out.extend(favourites);
        }
        for cat in Category::ALL {
            let list: Vec<ToolRow> = tools::catalogue().iter().filter(|t| t.category == cat && takes_pdf(t)).map(catalogue_row).collect();
            if !list.is_empty() {
                out.push(header(&sheaf_app::i18n::tr(cat.label())));
                out.extend(list);
            }
        }
        if !plugins.is_empty() {
            out.push(header(&sheaf_app::i18n::tr("Plugins")));
            out.extend(plugins);
        }
        if !custom.is_empty() {
            out.push(header(&sheaf_app::i18n::tr("Custom tools")));
            out.extend(custom);
        }
        return out;
    }
    let found: Vec<ToolRow> = tools::search(&q).into_iter().filter(|t| takes_pdf(t)).map(catalogue_row).collect();
    let extra: Vec<ToolRow> = plugins.into_iter().chain(custom).filter(|r| r.name.to_lowercase().contains(&q)).collect();
    if found.is_empty() && extra.is_empty() {
        return out;
    }
    out.push(header(&sheaf_app::i18n::tr("Tools")));
    out.extend(found);
    out.extend(extra);
    out
}

/// The popover's search: its rows for what is typed.
pub fn query(ui: &AppWindow, text: &str) {
    ui.global::<sheaf_app::ReaderBridge>().set_tool_rows(ModelRc::new(VecModel::from(rows(ui, text))));
}

/// A row of the popover chosen: the tool opens once the document is saved (a tool reads the
/// file; unsaved edits would be missed, so they are written first as the Tools button did).
pub fn pick(ui: &AppWindow, tab_id: i32, key: &str) {
    update(tab_id, |r, _| r.tool_pending = Some(key.to_string()));
    super::to_tools(ui, tab_id);
}

/// `key` opened on the saved document: a plugin or custom tool runs; a page or multi-file tool
/// opens its tab; any other catalogue tool shows its options in the inspector.
pub(super) fn show(ui: &AppWindow, tab_id: i32, key: &str) {
    let Some(path) = update(tab_id, |r, _| r.path.clone()) else { return };
    if key.starts_with(crate::plugins_ui::CARD) {
        return crate::plugins_ui::open_with(ui, key, vec![path]);
    }
    if let Some(i) = key.strip_prefix(CUSTOM).and_then(|i| i.parse::<i32>().ok()) {
        ui.global::<ActionBridge>().set_current_tab(tab_id);
        ui.global::<ActionBridge>().invoke_command("custom-current".into(), i);
        return;
    }
    let Some(tool) = ToolId::from_str(key) else { return };
    if !in_inspector(tool) {
        return crate::flow::open_tool(ui, tool, vec![path]);
    }
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            if matches!(t.phase, Phase::Running) {
                return;
            }
            t.tool = tool;
            t.files.clear();
            t.order.clear();
            t.page_thumbs.clear();
            t.preview = None;
            t.preview_note.clear();
            t.preview_page = 0;
            t.options = crate::home::last_options(tool);
            t.phase = Phase::Edit;
        }
    });
    update(tab_id, |r, _| r.tool_chosen = Some(tool));
    crate::flow::add_paths(ui, tab_id, vec![path]);
}

/// The inspector's × on a tool, or Escape: its options go; a run under way goes on and its result
/// still opens.
pub fn close(ui: &AppWindow, tab_id: i32) {
    update(tab_id, |r, _| r.tool_chosen = None);
    with(|a| {
        if let Some(t) = a.tab(tab_id).filter(|t| !matches!(t.phase, Phase::Running)) {
            reset(t);
        }
        refresh(a, tab_id);
    });
    crate::viewer_ui::request(ui, tab_id);
}

fn reset(t: &mut TabState) {
    t.tool = ToolId::Edit;
    t.files.clear();
    t.order.clear();
    t.page_thumbs.clear();
    t.preview = None;
    t.preview_note.clear();
    t.phase = Phase::Edit;
}

/// A tool's job on a document tab is done (run.rs): the result said in a toast and opened in its
/// own tab (a PDF; another kind of file is shown in its folder), the reader's tool state cleared.
pub fn finished(ui: &AppWindow, tab_id: i32) {
    let Some((tool, outputs, warn)) = with(|a| {
        let t = a.tab(tab_id)?;
        let Phase::Done(d) = &t.phase else { return None };
        Some((t.tool, d.outcome.outputs.clone(), d.outcome.was_encrypted))
    }) else {
        return;
    };
    let row = with(|a| a.tab_index(tab_id)).and_then(|i| tabs_model().row_data(i));
    let text = match (&row, tool) {
        (Some(r), ToolId::Compress) if !r.result_pct.is_empty() => sheaf_app::i18n::trf("Compressed to {} ({})", &[&r.size_after, &r.result_pct]),
        (Some(r), _) => r.result_title.to_string(),
        (None, _) => sheaf_app::i18n::trf("{} is done", &[&tools::find(tool).name]),
    };
    with(|a| {
        if let Some(t) = a.tab(tab_id) {
            reset(t);
        }
        refresh(a, tab_id);
    });
    update(tab_id, |r, _| r.tool_chosen = None);
    if warn {
        crate::flow::toast_for(ui, &format!("{text} {}", crate::state::NOT_PROTECTED), 6);
    } else {
        crate::flow::toast(ui, &text);
    }
    let (pdfs, others): (Vec<PathBuf>, Vec<PathBuf>) = outputs.into_iter().partition(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")));
    for p in pdfs {
        super::open(ui, p, None);
    }
    if let Some(p) = others.first() {
        crate::run::reveal(p);
    }
    crate::viewer_ui::request(ui, tab_id);
}
