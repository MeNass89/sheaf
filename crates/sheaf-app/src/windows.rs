//! Sheaf's windows. Each window is a process of its own (Move to New Window starts one); Merge All
//! Windows (macOS's Window menu, PDF Expert's) brings every other window's documents into this one
//! as tabs and closes those windows. Each process listens on a local port, written with a secret
//! to a file of its own in the profile's `windows` folder; a window asked for its documents (the
//! secret given) hands over the paths of those without unsaved edits, closes their tabs, and
//! quits when none is left.

use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use sheaf_app::AppWindow;
use slint::ComponentHandle;

use crate::state::with;

fn folder() -> PathBuf {
    sheaf_app::store_file::data_dir().join("windows")
}

/// Starts listening for Merge All Windows asked from another window; this window's file is
/// removed when the process ends normally ([`forget`]).
pub fn listen(ui: &AppWindow) {
    let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else { return };
    let Ok(port) = listener.local_addr().map(|a| a.port()) else { return };
    let secret = format!("{:016x}", std::collections::hash_map::RandomState::new().build_hasher().finish());
    let dir = folder();
    if std::fs::create_dir_all(&dir).is_err() || write_private(&dir.join(std::process::id().to_string()), &format!("{port} {secret}")).is_err() {
        return;
    }
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = answer(stream, &secret, &weak);
        }
    });
}

/// This window's file removed (it quits).
pub fn forget() {
    let _ = std::fs::remove_file(folder().join(std::process::id().to_string()));
}

/// One request: the secret and `give`; the reply, the paths given, one per line.
fn answer(stream: TcpStream, secret: &str, weak: &slint::Weak<AppWindow>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    if line.trim() != format!("{secret} give") {
        return Ok(());
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let weak = weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        let _ = tx.send(weak.upgrade().map(|ui| give(&ui)).unwrap_or_default());
    });
    let paths = rx.recv_timeout(Duration::from_secs(10)).unwrap_or_default();
    let mut out = &stream;
    for p in paths {
        writeln!(out, "{}", p.to_string_lossy())?;
    }
    Ok(())
}

/// On the UI thread: the documents without unsaved edits handed over, their tabs closed; the
/// window quits when no tab is left.
fn give(ui: &AppWindow) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for id in with(|a| a.tabs.iter().filter(|t| t.reader.is_some()).map(|t| t.id).collect::<Vec<_>>()) {
        if crate::reader_ui::unsaved_name(id).is_some() || crate::viewer_ui::redact_pending(id).is_some() {
            continue;
        }
        let Some(path) = with(|a| a.tab(id).and_then(|t| t.reader.as_ref()).map(|r| r.path.clone())) else { continue };
        crate::reader_ui::remember_position(id);
        crate::flow::remove_tab(ui, id);
        paths.push(path);
    }
    if with(|a| a.tabs.is_empty()) {
        forget();
        let _ = slint::quit_event_loop();
    }
    paths
}

/// Merge All Windows: every other window's documents opened here as tabs. Asked off the UI
/// thread; says how many came, or that none could.
pub fn merge_all(ui: &AppWindow) {
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let mine = std::process::id().to_string();
        let mut paths = Vec::new();
        let mut windows = 0;
        for entry in std::fs::read_dir(folder()).into_iter().flatten().flatten() {
            if entry.file_name().to_string_lossy() == mine {
                continue;
            }
            match ask(&entry.path()) {
                Some(given) => {
                    windows += 1;
                    paths.extend(given);
                }
                // A window that quit without removing its file (it crashed).
                None => {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let n = paths.len();
            if !paths.is_empty() {
                crate::deliver(paths);
            }
            let msg = match (windows, n) {
                (0, _) => "No other Sheaf window is open".to_string(),
                (_, 0) => "The other windows' documents have unsaved edits: save them first".to_string(),
                (_, 1) => "1 document brought in from the other windows".to_string(),
                _ => format!("{n} documents brought in from the other windows"),
            };
            crate::flow::toast(&ui, &msg);
        });
    });
}

/// The documents window file `f` hands over; None when that window is gone.
fn ask(f: &std::path::Path) -> Option<Vec<PathBuf>> {
    let text = std::fs::read_to_string(f).ok()?;
    let (port, secret) = text.trim().split_once(' ')?;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port.parse().ok()?));
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(1)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(15))).ok()?;
    writeln!(s, "{secret} give").ok()?;
    Some(BufReader::new(s).lines().map_while(Result::ok).filter(|l| !l.is_empty()).map(PathBuf::from).collect())
}

/// `text` written to `f`, readable by this user only (the secret in it).
fn write_private(f: &std::path::Path, text: &str) -> std::io::Result<()> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(f)?.write_all(text.as_bytes())
}
