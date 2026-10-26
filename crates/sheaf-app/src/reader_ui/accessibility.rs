//! More ▸ More tools ▸ Accessibility check (Acrobat's Full Check, PdfCraft's 32 rules): the
//! document checked on its thread, the rules listed failed first with where and why; the ones
//! with a fix of their own fixed in one click (title, language, tab order, tags from the text,
//! bookmarks from the headings), the check run again after; Save report… writes Acrobat's HTML
//! report.

use super::*;
use sheaf_core::pdfcraft::engine::a11y::{self, Report, Rule, Status};

/// The rules as listed: failed, then to check by hand, then passed (skipped ones left out).
pub(super) fn rows(report: &Report) -> Vec<&a11y::RuleResult> {
    let rank = |s: Status| match s {
        Status::Failed => 0,
        Status::Manual => 1,
        Status::Passed => 2,
        Status::Skipped => 3,
    };
    let mut rows: Vec<_> = report.results.iter().filter(|r| r.status != Status::Skipped).collect();
    rows.sort_by_key(|r| rank(r.status));
    rows
}

/// What a row says under the rule's name: its first findings (page and sentence), else what the
/// rule checks.
pub(super) fn detail(r: &a11y::RuleResult) -> String {
    if r.findings.is_empty() {
        return r.rule.description().to_string();
    }
    let mut lines: Vec<String> = r.findings.iter().take(3).map(|f| match f.page {
        Some(p) => format!("p. {}: {}", p + 1, f.message),
        None => f.message.clone(),
    }).collect();
    if r.findings.len() > 3 {
        lines.push(format!("and {} more", r.findings.len() - 3));
    }
    lines.join(" · ")
}

/// Whether failed rule `rule` has a fix here.
pub(super) fn fixable(rule: Rule) -> bool {
    matches!(rule, Rule::Title | Rule::PrimaryLanguage | Rule::TabOrder | Rule::TaggedPdf | Rule::TaggedContent | Rule::Bookmarks)
}

/// The check run (again); the sheet opens on its answer.
pub fn accessibility_check(tab_id: i32) {
    update(tab_id, |r, _| {
        r.a11y_open = true;
        r.a11y_busy = true;
        r.in_flight += 1;
        r.send(Op::AccessibilityCheck);
    });
}

pub(super) fn checked(tab_id: i32, report: Option<Report>) {
    update(tab_id, |r, _| {
        r.a11y_busy = false;
        r.a11y = report;
    });
}

/// The sheet's row `k`: its fix applied, then the document checked again; −1 closes the sheet.
pub fn accessibility_fix(ui: &AppWindow, tab_id: i32, k: i32) {
    if k < 0 {
        update(tab_id, |r, _| r.a11y_open = false);
        return;
    }
    use sheaf_core::pdfcraft::engine::TabOrder;
    let edit = update(tab_id, |r, v| {
        let rule = rows(r.a11y.as_ref()?).get(k as usize)?.rule;
        let mut view = r.snap.opening.clone();
        Some(match rule {
            Rule::Title => {
                let title = Some(r.snap.props.title.trim()).filter(|t| !t.is_empty()).map_or_else(|| r.name.trim_end_matches(".pdf").trim_end_matches(".PDF").to_string(), str::to_string);
                view.display_title = true;
                Edit::Batch { label: "Set document title".into(), edits: vec![Edit::SetInfo { key: "Title".into(), value: title }, Edit::SetInitialView(Box::new(view))] }
            }
            Rule::PrimaryLanguage => {
                view.language = Some(sys_locale::get_locale().unwrap_or_else(|| "en-US".into()));
                Edit::SetInitialView(Box::new(view))
            }
            Rule::TabOrder => Edit::SetTabOrder { pages: (0..v.viewer.page_count()).collect(), order: TabOrder::Structure },
            Rule::TaggedPdf | Rule::TaggedContent => Edit::AutoTagText,
            Rule::Bookmarks => Edit::BookmarksFromStructure,
            _ => return None,
        })
    })
    .flatten();
    if let Some(e) = edit {
        apply(ui, tab_id, e, &[], None);
        // Checked again once the fix is in: the thread takes the operations in turn.
        accessibility_check(tab_id);
    }
}

/// Save report… (Acrobat's Accessibility Report): the last check as a web page.
pub fn accessibility_report(ui: &AppWindow, tab_id: i32) {
    let Some((dir, stem)) = update(tab_id, |r, _| (crate::dest::parent_dir(&r.path), r.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())) else { return };
    let d = crate::dialog::new(ui).set_directory(&dir).set_file_name(format!("{stem} - accessibility.html")).add_filter("Web page", &["html"]);
    crate::dialog::save_file(ui, d, move |ui, to| {
        let date = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let Some(html) = update(tab_id, |r, _| r.a11y.as_ref().map(|rep| a11y::report_html(rep, &r.name, &date, "en", &a11y::in_english))).flatten() else { return };
        match std::fs::write(&to, html) {
            Ok(()) => crate::flow::toast(ui, &format!("Report saved as {}", file_name(&to))),
            Err(e) => crate::flow::toast(ui, &format!("The report was not saved: {e}")),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_rules_come_first_with_where_and_why() {
        let r = |rule, status, findings| a11y::RuleResult { rule, status, findings };
        let missing = (0..5).map(|p| a11y::Finding::plain(Some(p), "a figure has no alternate text")).collect();
        let rep = Report { results: vec![r(Rule::Title, Status::Passed, vec![]), r(Rule::FiguresAltText, Status::Failed, missing), r(Rule::ColorContrast, Status::Manual, vec![]), r(Rule::Scripts, Status::Skipped, vec![])] };
        let rows = rows(&rep);
        assert_eq!(rows.iter().map(|x| x.rule).collect::<Vec<_>>(), [Rule::FiguresAltText, Rule::ColorContrast, Rule::Title]);
        assert_eq!(detail(rows[0]), "p. 1: a figure has no alternate text · p. 2: a figure has no alternate text · p. 3: a figure has no alternate text · and 2 more");
        assert_eq!(detail(rows[2]), Rule::Title.description());
        assert!(fixable(Rule::Title) && fixable(Rule::TaggedPdf) && !fixable(Rule::FiguresAltText));
    }
}
