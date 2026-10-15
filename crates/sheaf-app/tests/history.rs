use sheaf_app::history::{relative_time, Entry, Store, MAX_ENTRIES};
use sheaf_app::prefs::{OutputMode, Prefs, ThemePref};
use std::path::PathBuf;

fn entry(n: u64) -> Entry {
    Entry {
        tool: "compress".into(),
        inputs: vec![PathBuf::from(format!("/in/{n}.pdf"))],
        outputs: vec![PathBuf::from(format!("/out/{n}-compressed.pdf"))],
        at_unix: n,
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
