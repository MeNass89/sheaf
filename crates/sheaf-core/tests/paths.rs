use sheaf_core::paths::{atomic_write, unique_path};
#[test]
fn unique_path_appends_counter() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("out.pdf");
    assert_eq!(unique_path(&p), p);
    std::fs::write(&p, b"x").unwrap();
    assert_eq!(unique_path(&p), t.path().join("out (1).pdf"));
    std::fs::write(t.path().join("out (1).pdf"), b"x").unwrap();
    assert_eq!(unique_path(&p), t.path().join("out (2).pdf"));
}
#[test]
fn atomic_write_never_leaves_part_on_error() {
    let t = tempfile::tempdir().unwrap();
    let target = t.path().join("r.pdf");
    let r = atomic_write(&target, |part| { std::fs::write(part, b"half")?; Err(sheaf_core::OpdfError::Cancelled) });
    assert!(r.is_err());
    assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), 0);
}
#[test]
fn atomic_write_keeps_existing_file() {
    let t = tempfile::tempdir().unwrap();
    let target = t.path().join("r.pdf");
    std::fs::write(&target, b"orig").unwrap();
    let out = atomic_write(&target, |part| { std::fs::write(part, b"new")?; Ok(()) }).unwrap();
    assert_eq!(out, t.path().join("r (1).pdf"));
    assert_eq!(std::fs::read(&target).unwrap(), b"orig");
    assert_eq!(std::fs::read(&out).unwrap(), b"new");
}
#[test]
fn atomic_write_part_name_next_to_target() {
    let t = tempfile::tempdir().unwrap();
    let target = t.path().join("r.pdf");
    let mut seen = None;
    let out = atomic_write(&target, |part| { seen = Some(part.to_path_buf()); std::fs::write(part, b"x")?; Ok(()) }).unwrap();
    assert_eq!(out, target);
    let part = seen.unwrap();
    assert_eq!(part.parent().unwrap(), t.path());
    let name = part.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("r.pdf.") && name.ends_with(".part"), "{name}");
}
#[test]
fn atomic_write_concurrent_same_target() {
    let t = tempfile::tempdir().unwrap();
    let target = t.path().join("r.pdf");
    let hs: Vec<_> = [b"one".to_vec(), b"two".to_vec()].into_iter().map(|bytes| {
        let target = target.clone();
        std::thread::spawn(move || {
            let out = atomic_write(&target, |part| {
                std::fs::write(part, &bytes)?;
                std::thread::sleep(std::time::Duration::from_millis(50));
                Ok(())
            }).unwrap();
            (out, bytes)
        })
    }).collect();
    let mut outs: Vec<_> = hs.into_iter().map(|h| h.join().unwrap()).collect();
    for (out, bytes) in &outs { assert_eq!(&std::fs::read(out).unwrap(), bytes); }
    outs.sort();
    let paths: Vec<_> = outs.into_iter().map(|(o, _)| o).collect();
    let mut want = vec![t.path().join("r (1).pdf"), t.path().join("r.pdf")];
    want.sort();
    assert_eq!(paths, want);
    assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), 2);
}
#[test]
fn unique_path_treats_dangling_symlink_as_taken() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("out.pdf");
    std::os::unix::fs::symlink(t.path().join("missing.pdf"), &p).unwrap();
    assert_eq!(unique_path(&p), t.path().join("out (1).pdf"));
}
