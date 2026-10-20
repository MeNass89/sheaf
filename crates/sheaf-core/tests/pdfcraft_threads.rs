//! Is PdfCraft safe to use from several threads at once, one session per thread?
//! 8 threads × 10 iterations of open → render page 1 → rotate → save, with a 120 s deadline.
//! `SHEAF_THREADS_PDF=<file>` runs it on another (richer) PDF instead of the generated fixture.
mod common;
use common::fixture_pdf;
use sheaf_core::{pages::Input, pdfcraft::{engine::Edit, Doc}};
use std::sync::{mpsc, Arc, Barrier};
use std::time::{Duration, Instant};

const THREADS: usize = 8;
const ITERATIONS: usize = 10;

#[test]
fn concurrent_sessions_open_render_edit_save() {
    let t = tempfile::tempdir().unwrap();
    let fixture = std::env::var_os("SHEAF_THREADS_PDF").map(Into::into).unwrap_or_else(|| fixture_pdf(t.path(), "f.pdf", 3));
    let (tx, rx) = mpsc::channel();
    let start = Arc::new(Barrier::new(THREADS));
    for n in 0..THREADS {
        let (tx, fixture, dir, start) = (tx.clone(), fixture.clone(), t.path().to_path_buf(), start.clone());
        std::thread::spawn(move || {
            start.wait();
            let r = std::panic::catch_unwind(|| {
                for i in 0..ITERATIONS {
                    let mut doc = Doc::open(&Input::new(&fixture)).unwrap();
                    let page = doc.render(0, 1.0).unwrap();
                    assert!(page.width > 100 && page.height > 100, "page 1 at 1 px/pt: {}×{}", page.width, page.height);
                    assert!(page.rgba.as_chunks::<4>().0.iter().any(|p| p[0] < 128), "the text left dark pixels");
                    doc.apply(Edit::RotatePages { pages: vec![0], degrees: 90 }).unwrap();
                    doc.save(&dir.join(format!("out-{n}-{i}.pdf"))).unwrap();
                }
            });
            tx.send((n, r.is_ok())).unwrap();
        });
    }
    drop(tx);
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut done = 0;
    while done < THREADS {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok((n, ok)) => {
                assert!(ok, "thread {n} panicked");
                done += 1;
            }
            Err(_) => panic!("deadlock: only {done}/{THREADS} threads finished within 120 s"),
        }
    }
    let outputs = std::fs::read_dir(t.path()).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("out-")).count();
    assert_eq!(outputs, THREADS * ITERATIONS);
}
