use sheaf_app::history::{relative_time, Entry, Store, MAX_ENTRIES};
use sheaf_app::prefs::{OutputMode, Prefs, ThemePref};
use std::path::PathBuf;

fn entry(n: u64) -> Entry {
    Entry {
        tool: "compress".into(),
        inputs: vec![PathBuf::from(format!("/in/{n}.pdf"))],
        outputs: vec![PathBuf::from(format!("/out/{n}-compressed.pdf"))],
        at_unix: n,
        options: Default::default(),
    }
}

fn ats(s: &Store) -> Vec<u64> {
    s.list().iter().map(|e| e.at_unix).collect()
}

#[test]
fn push_lists_newest_first_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path());
    for n in 1..=3 {
        s.push(entry(n)).unwrap();
    }
    assert_eq!(ats(&s), vec![3, 2, 1]);
    let again = Store::open(dir.path());
    assert_eq!(ats(&again), vec![3, 2, 1]);
    assert_eq!(again.list()[0].outputs, vec![PathBuf::from("/out/3-compressed.pdf")]);
    assert_eq!(again.list()[0].tool, "compress");
}

#[test]
fn corrupt_file_is_an_empty_history_and_is_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("history.json"), b"{not json at all").unwrap();
    let mut s = Store::open(dir.path());
    assert!(s.list().is_empty());
    s.push(entry(7)).unwrap();
    assert_eq!(ats(&Store::open(dir.path())), vec![7]);
}

#[test]
fn history_is_capped_at_200() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path());
    for n in 1..=205 {
        s.push(entry(n)).unwrap();
    }
    assert_eq!(MAX_ENTRIES, 200);
    assert_eq!(s.list().len(), 200);
    assert_eq!(s.list()[0].at_unix, 205);
    assert_eq!(s.list()[199].at_unix, 6);
    assert_eq!(Store::open(dir.path()).list().len(), 200);
}

#[test]
fn clear_empties_memory_and_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path());
    s.push(entry(1)).unwrap();
    s.clear().unwrap();
    assert!(s.list().is_empty());
    assert!(Store::open(dir.path()).list().is_empty());
    // No temp file is left behind by the replace-write.
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from("history.json")]);
}

#[test]
fn open_creates_a_missing_data_dir() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("a/Sheaf");
    let mut s = Store::open(&nested);
    s.push(entry(1)).unwrap();
    assert!(nested.join("history.json").is_file());
}

#[test]
fn prefs_round_trip_and_corrupt_means_default() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(Prefs::load(dir.path()), Prefs::default());
    let d = Prefs::default();
    assert_eq!((d.theme, d.output, d.open_folder_after), (ThemePref::System, OutputMode::Ask, false));
    let p = Prefs {
        theme: ThemePref::Dark,
        output: OutputMode::Fixed,
        fixed_folder: Some(PathBuf::from("/tmp/out")),
        open_folder_after: true,
        author: Some("Ada".into()),
        reader_theme: 2,
        plain_fields: true,
        custom_ink: Some("123456".into()),
        custom_light: None,
        stamp_date: 2,
        reading_speed: 2,
        reading_voice: Some("Alex".into()),
        metric: true,
        guide_colour: 2,
        no_gpu: true,
        display_profile: Some(PathBuf::from("/profiles/monitor.icc")),
        display_system: true,
        show_boxes: true,
        physical_numbers: true,
        transparency_grid: true,
        digital_ids: vec![PathBuf::from("/ids/ada.p12")],
        form_entries: vec!["Ada Lovelace".into()],
        rough: 5,
        default_zoom: 1,
        default_spread: 2,
        open_first_page: true,
        panel_width: 320,
        accent: 2,
        backdrop: 3,
        last_pages: vec![(PathBuf::from("/tmp/a.pdf"), 12)],
        last_views: vec![(PathBuf::from("/tmp/a.pdf"), 2, 150, 1)],
        tool_styles: vec![(5, 2, 1, 0)],
        favourite_tools: vec!["compress".into()],
        signature_strokes: Vec::new(),
        signature_name: None,
        signature_pen: (1, 2),
        signature_font: 0,
        signatures: Vec::new(),
        initials: None,
        pinned_files: vec![PathBuf::from("/docs/starred.pdf")],
        platform: sheaf_app::platform::PlatformPrefs {
            no_spell_check: true,
            no_grammar_check: true,
            learned_words: vec!["Sheaf".into()],
            shortcuts: vec![("find".into(), "Control+Alt+F".into())],
            hide_unavailable_tools: true,
            direction: 2,
            tool_keys: vec![("note".into(), "q".into())],
            launch_keys: vec![("merge".into(), "Control+Alt+M".into())],
            dictionaries: vec!["fr_FR".into(), "en_US".into()],
            language: 2,
        },
        ai: sheaf_app::ai_store::AiPrefs {
            provider: Some("mistral".into()),
            models: [("mistral".to_string(), "mistral-large-latest".to_string())].into(),
            base_urls: [("compatible".to_string(), "http://localhost:1234/v1".to_string())].into(),
            off: false,
            notice_seen: vec!["mistral".into()],
            auto_open: true,
            language: Some("French".into()),
            podcast_voices: vec!["Ava (Premium)".into(), "Evan (Enhanced)".into()],
        },
        settings: sheaf_app::settings::SettingsPrefs {
            print: sheaf_app::settings::PrintDefaults { two_sided: true, paper: 1, gray: true, quality: 3, ..Default::default() },
            trust: sheaf_app::settings::TrustPrefs { web: 2, locations: vec![PathBuf::from("/docs/trusted")], ..Default::default() },
            open_at_login: true,
            no_quit_warning: true,
        },
        trap_presets: vec![sheaf_core::trap::TrapPreset { name: "Coated sheetfed".into(), width_pt: 0.2, step: 0.15, ..Default::default() }],
    };
    p.save(dir.path()).unwrap();
    assert_eq!(Prefs::load(dir.path()), p);
    std::fs::write(dir.path().join("prefs.json"), b"\x00\xffgarbage").unwrap();
    assert_eq!(Prefs::load(dir.path()), Prefs::default());
    // A file from an older version with missing fields keeps the ones it has.
    std::fs::write(dir.path().join("prefs.json"), br#"{"theme":"light"}"#).unwrap();
    assert_eq!(Prefs::load(dir.path()).theme, ThemePref::Light);
}

#[test]
fn relative_times() {
    let now = 1_760_000_000; // 2025-10-09 08:53:20 UTC
    assert_eq!(relative_time(now - 5, now), "just now");
    assert_eq!(relative_time(now - 120, now), "2 min ago");
    assert_eq!(relative_time(now - 3 * 3600, now), "3 h ago");
    assert_eq!(relative_time(now - 30 * 3600, now), "yesterday");
    assert_eq!(relative_time(now - 5 * 86400, now), "4 Oct 2025");
    assert_eq!(relative_time(now + 50, now), "just now");
}

#[test]
#[cfg(unix)]
fn an_unsavable_entry_is_rejected_and_later_pushes_still_save() {
    use std::os::unix::ffi::OsStrExt;
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path());
    s.push(entry(1)).unwrap();
    let mut bad = entry(2);
    bad.outputs = vec![PathBuf::from(std::ffi::OsStr::from_bytes(b"/out/\xff.pdf"))];
    assert!(s.push(bad).is_err(), "a non-UTF-8 path cannot be stored in JSON");
    assert_eq!(ats(&s), vec![1], "the rejected entry is not kept in memory");
    s.push(entry(3)).unwrap();
    assert_eq!(ats(&Store::open(dir.path())), vec![3, 1]);
}

#[test]
fn removing_a_recent_survives_reopen_without_changing_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path());
    let e = entry(1);
    let path = e.outputs[0].clone();
    s.push(e.clone()).unwrap();
    s.push(Entry { tool: "read".into(), ..e.clone() }).unwrap();
    let history = std::fs::read(dir.path().join("history.json")).unwrap();
    s.remove_recent(&path).unwrap();
    s.remove_recent(&path).unwrap();
    assert!(!s.visible_in_recents(&path));
    let again = Store::open(dir.path());
    assert!(!again.visible_in_recents(&path));
    assert_eq!(again.list().len(), 2);
    assert_eq!(std::fs::read(dir.path().join("history.json")).unwrap(), history);
    assert!(again.visible_in_recents(&PathBuf::from("/out/other.pdf")));
    s.push(e).unwrap();
    assert!(s.visible_in_recents(&path));
    assert!(Store::open(dir.path()).visible_in_recents(&path));
}

#[test]
fn clearing_recents_hides_the_whole_history_but_new_activity_returns() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path());
    for n in 0..20 { s.push(entry(n)).unwrap(); }
    let history = std::fs::read(dir.path().join("history.json")).unwrap();
    s.clear_recents().unwrap();
    let mut again = Store::open(dir.path());
    assert_eq!(again.list().len(), 20);
    for e in again.list() {
        for path in &e.outputs { assert!(!again.visible_in_recents(path)); }
    }
    assert_eq!(std::fs::read(dir.path().join("history.json")).unwrap(), history);
    let reopened = entry(3);
    again.push(reopened.clone()).unwrap();
    assert!(again.visible_in_recents(&reopened.outputs[0]));
    assert!(!again.visible_in_recents(&entry(4).outputs[0]));
    let new = entry(99);
    again.push(new.clone()).unwrap();
    assert!(again.visible_in_recents(&new.outputs[0]));
    let disk = Store::open(dir.path());
    assert!(disk.visible_in_recents(&new.outputs[0]));
    assert!(disk.visible_in_recents(&reopened.outputs[0]));
    assert!(!disk.visible_in_recents(&entry(4).outputs[0]));
}

#[test]
fn recent_store_write_failure_does_not_hide_a_file_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path());
    let e = entry(1);
    s.push(e.clone()).unwrap();
    std::fs::create_dir(dir.path().join("hidden-recents.json")).unwrap();
    assert!(s.remove_recent(&e.outputs[0]).is_err());
    assert!(s.visible_in_recents(&e.outputs[0]));
    assert!(s.clear_recents().is_err());
    assert!(s.visible_in_recents(&e.outputs[0]));
    assert_eq!(s.list(), &[e]);
}

#[test]
fn expired_recent_exclusions_are_pruned_and_corrupt_state_is_recoverable() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path());
    let e = entry(0);
    s.push(e.clone()).unwrap();
    s.remove_recent(&e.outputs[0]).unwrap();
    for n in 1..=MAX_ENTRIES { s.push(entry(n as u64)).unwrap(); }
    let hidden: Vec<PathBuf> = serde_json::from_slice(&std::fs::read(dir.path().join("hidden-recents.json")).unwrap()).unwrap();
    assert!(hidden.is_empty());
    std::fs::write(dir.path().join("hidden-recents.json"), b"{broken").unwrap();
    let mut again = Store::open(dir.path());
    assert_eq!(again.list().len(), MAX_ENTRIES);
    assert!(again.visible_in_recents(&entry(1).outputs[0]));
    again.clear_recents().unwrap();
    assert!(!Store::open(dir.path()).visible_in_recents(&entry(1).outputs[0]));
}

#[test]
fn recent_tools_are_the_newest_distinct_runs() {
    use sheaf_app::history::{recent_tools, Entry};
    let e = |tool: &str, at| Entry { tool: tool.into(), inputs: vec![], outputs: vec![], at_unix: at, options: Default::default() };
    let list = vec![e("compress", 9), e("read", 8), e("merge", 7), e("compress", 6), e("split", 5), e("rotate", 4)];
    assert_eq!(recent_tools(&list, "read", 3), ["compress", "merge", "split"]);
    assert!(recent_tools(&[], "read", 3).is_empty());
}

#[test]
fn signatures_kept_before_handwriting_fonts_still_load() {
    // Schema migration is independent of access to the developer's real credential store.
    let p: Prefs = serde_json::from_str(r#"{"signatures": [[[], "Ada", [0, 1]], [[], "Bo", [1, 0], 3]]}"#).unwrap();
    assert_eq!(p.signatures, vec![(Vec::new(), Some("Ada".into()), (0, 1), 0), (Vec::new(), Some("Bo".into()), (1, 0), 3)]);
}

#[test]
fn a_tasks_options_are_kept_with_it_and_older_files_still_load() {
    let dir = tempfile::tempdir().unwrap();
    let o = sheaf_app::ToolOptions { level: 2, pages: "3-5".into(), password: "secret".into(), ..Default::default() };
    let mut s = Store::open(dir.path());
    s.push(Entry { options: sheaf_app::saved_options::to_json(&o), ..entry(1) }).unwrap();
    let text = std::fs::read_to_string(dir.path().join("history.json")).unwrap();
    assert!(text.contains("\"pages\": \"3-5\"") && !text.contains("secret"), "{text}");
    let back = sheaf_app::saved_options::from_json(&Store::open(dir.path()).list()[0].options);
    assert!(back.level == 2 && back.pages == "3-5" && back.password.is_empty());
    // A history written before options were kept: its entries load, with none.
    std::fs::write(dir.path().join("history.json"), r#"[{"tool":"compress","inputs":["/a.pdf"],"outputs":["/b.pdf"],"at_unix":5}]"#).unwrap();
    assert!(Store::open(dir.path()).list()[0].options.is_empty());
}
