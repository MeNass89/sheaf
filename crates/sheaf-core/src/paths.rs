use crate::{OpdfError, Result};
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static PART_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Serialises "pick a free name + rename" on the no-hard-link fallback, so two jobs of this
/// process never pick the same final name. Another process can still race us there (see `claim_final`).
static RENAME_LOCK: Mutex<()> = Mutex::new(());

/// Returns `p` if nothing (not even a dangling symlink) is at it, otherwise `stem (n).ext` with the smallest free `n`.
pub fn unique_path(p: &Path) -> PathBuf {
    let taken = |c: &Path| std::fs::symlink_metadata(c).is_ok();
    if !taken(p) { return p.to_path_buf(); }
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let dir = p.parent().unwrap_or(Path::new("."));
    (1..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|c| !taken(c)).unwrap()
}

/// Claims a fresh `<file name>.<pid>-<n>.part` next to `target` with `create_new`, which fails
/// if anything (a file, or a symlink, which is not followed) is already there; tries the next `n`.
fn claim_part(target: &Path) -> Result<PathBuf> {
    for _ in 0..100 {
        let n = PART_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut name = target.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        name.push(format!(".{}-{n}.part", std::process::id()));
        let part = target.with_file_name(name);
        match OpenOptions::new().write(true).create_new(true).open(&part) {
            Ok(_) => return Ok(part),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(OpdfError::Conversion(format!("no free temporary name next to {}", target.display())))
}

/// Calls `f` with a per-call temp path `<file name>.<pid>-<n>.part` next to `target`, created
/// exclusively beforehand (never a pre-existing file or symlink), then moves it to
/// `unique_path(target)` without ever overwriting an existing file (the final name is claimed
/// with `hard_link`, which fails if the name exists). On any error the `.part` file is removed.
pub fn atomic_write(target: &Path, f: impl FnOnce(&Path) -> Result<()>) -> Result<PathBuf> {
    let part = claim_part(target)?;
    let r = f(&part).and_then(|()| claim_final(&part, target));
    let _ = std::fs::remove_file(&part);
    r
}

/// Runs `f` over `items` and collects the paths written. If one fails, the files already
/// written by this call are deleted and the error is returned: a single-input operation that
/// writes several files (Split, PDF to JPG) is all-or-nothing.
pub(crate) fn all_or_nothing<T>(items: impl IntoIterator<Item = T>, mut f: impl FnMut(T) -> Result<PathBuf>) -> Result<Vec<PathBuf>> {
    let mut outs = Vec::new();
    for it in items {
        match f(it) {
            Ok(p) => outs.push(p),
            Err(e) => {
                for p in &outs { let _ = std::fs::remove_file(p); }
                return Err(e);
            }
        }
    }
    Ok(outs)
}

#[cfg(test)]
static FORCE_RENAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn hard_link(part: &Path, fin: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if FORCE_RENAME.load(Ordering::Relaxed) {
        return Err(std::io::Error::new(ErrorKind::Unsupported, "forced: no hard links"));
    }
    std::fs::hard_link(part, fin)
}

fn claim_final(part: &Path, target: &Path) -> Result<PathBuf> {
    loop {
        let fin = unique_path(target);
        match hard_link(part, &fin) {
            Ok(()) => return Ok(fin),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            // Filesystem without hard links (e.g. FAT/exFAT): fall back to rename, re-checking the
            // name under a process-wide lock right before it. Residual risk: another *process*
            // creating that same name between the check and the rename would be replaced.
            Err(_) => {
                let _g = RENAME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                let fin = unique_path(target);
                std::fs::rename(part, &fin)?;
                return Ok(fin);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_or_nothing_deletes_earlier_outputs_on_failure() {
        let t = tempfile::tempdir().unwrap();
        let r = all_or_nothing(0..3, |i| {
            if i == 1 { return Err(OpdfError::Conversion("boom".into())); }
            atomic_write(&t.path().join(format!("{i}.pdf")), |p| Ok(std::fs::write(p, b"x")?))
        });
        assert!(matches!(r, Err(OpdfError::Conversion(_))));
        assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), 0);
        let ok = all_or_nothing(0..2, |i| atomic_write(&t.path().join(format!("{i}.pdf")), |p| Ok(std::fs::write(p, b"x")?))).unwrap();
        assert_eq!(ok.len(), 2);
    }

    #[test]
    fn rename_fallback_never_reuses_a_name() {
        FORCE_RENAME.store(true, Ordering::Relaxed);
        let t = tempfile::tempdir().unwrap();
        let target = t.path().join("r.pdf");
        std::fs::write(&target, b"orig").unwrap();
        let hs: Vec<_> = (0..8u8).map(|i| {
            let target = target.clone();
            std::thread::spawn(move || {
                let out = atomic_write(&target, |part| { std::fs::write(part, [i])?; Ok(()) }).unwrap();
                (out, i)
            })
        }).collect();
        let outs: Vec<_> = hs.into_iter().map(|h| h.join().unwrap()).collect();
        FORCE_RENAME.store(false, Ordering::Relaxed);
        assert_eq!(std::fs::read(&target).unwrap(), b"orig");
        for (out, i) in &outs { assert_eq!(std::fs::read(out).unwrap(), [*i], "{out:?}"); }
        let mut names: Vec<_> = outs.iter().map(|(o, _)| o.clone()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 8);
        assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), 9);
    }
}
