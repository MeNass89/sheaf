//! A small fixed-size worker pool for file probes (page counts, thumbnails). Adding 30 photos
//! must not start 30 decoders at once: at most `size` tasks run concurrently, the rest queue.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

type Task = Box<dyn FnOnce() + Send>;

pub struct Pool {
    tx: Mutex<Sender<Task>>,
    peak: Arc<AtomicUsize>,
}

impl Pool {
    /// Starts `size` worker threads sharing one queue.
    pub fn new(size: usize) -> Self {
        let (tx, rx) = channel::<Task>();
        let rx = Arc::new(Mutex::new(rx));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        for i in 0..size.max(1) {
            let (rx, active, peak) = (rx.clone(), active.clone(), peak.clone());
            std::thread::Builder::new()
                .name(format!("sheaf-probe-{i}"))
                .spawn(move || worker(&rx, &active, &peak))
                .expect("spawn probe worker");
        }
        Self { tx: Mutex::new(tx), peak }
    }

    /// Queues `f`; it runs on one of the pool's threads.
    pub fn submit(&self, f: impl FnOnce() + Send + 'static) {
        let _ = self.tx.lock().unwrap().send(Box::new(f));
    }

    /// Highest number of tasks that ever ran at the same time (test hook).
    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
}

fn worker(rx: &Mutex<Receiver<Task>>, active: &AtomicUsize, peak: &AtomicUsize) {
    loop {
        // The lock is released before the task runs, so other workers can take the next one.
        let task = match rx.lock().unwrap().recv() {
            Ok(t) => t,
            Err(_) => return, // pool dropped
        };
        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(now, Ordering::SeqCst);
        // A panicking probe must not kill the worker.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task));
        active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Maximum concurrent probes.
pub const PROBE_THREADS: usize = 3;

/// The app-wide probe pool, created on first use.
pub fn probes() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();
    POOL.get_or_init(|| Pool::new(PROBE_THREADS))
}
