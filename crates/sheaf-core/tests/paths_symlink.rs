//! Own test binary (own process), so the `.part` counter starts at 0 and the next names are predictable.
use sheaf_core::paths::atomic_write;

#[test]
#[cfg(unix)]
fn atomic_write_never_follows_a_planted_part_symlink() {
    let t = tempfile::tempdir().unwrap();
    let victim = t.path().join("victim.pdf");
    std::fs::write(&victim, b"precious").unwrap();
    let target = t.path().join("r.pdf");
    let pid = std::process::id();
    // Plant symlinks at the first predicted part names, plus a plain file.
    for n in 0..5 {
        std::os::unix::fs::symlink(&victim, t.path().join(format!("r.pdf.{pid}-{n}.part"))).unwrap();
    }
    std::fs::write(t.path().join(format!("r.pdf.{pid}-5.part")), b"other").unwrap();
    let out = atomic_write(&target, |part| { std::fs::write(part, b"new")?; Ok(()) }).unwrap();
    assert_eq!(out, target);
    assert_eq!(std::fs::read(&victim).unwrap(), b"precious");
    assert_eq!(std::fs::read(&out).unwrap(), b"new");
    assert_eq!(std::fs::read(t.path().join(format!("r.pdf.{pid}-5.part"))).unwrap(), b"other");
    for n in 0..5 {
        assert!(std::fs::symlink_metadata(t.path().join(format!("r.pdf.{pid}-{n}.part"))).unwrap().file_type().is_symlink());
    }
}
