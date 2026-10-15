//! The task history (`history.json`): one entry per successful job, newest first.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store_file;

/// Entries kept; older ones are dropped.
pub const MAX_ENTRIES: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `ToolId::as_str` key.
    pub tool: String,
    pub inputs: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub at_unix: u64,
}

pub struct Store {
    path: PathBuf,
    entries: Vec<Entry>,
}

impl Store {
    /// Loads `<dir>/history.json`. A missing or corrupt file is an empty history; the folder
    /// is created on the first save.
    pub fn open(dir: &Path) -> Store {
        let path = dir.join("history.json");
        let mut entries: Vec<Entry> = store_file::read(&path).unwrap_or_default();
        entries.truncate(MAX_ENTRIES);
        Store { path, entries }
    }

    /// Adds `e` at the top and saves. An entry JSON cannot hold (a non-UTF-8 path) is
    /// rejected and not kept, so it never blocks later saves. A disk error keeps it in
    /// memory; the next save retries.
    pub fn push(&mut self, e: Entry) -> io::Result<()> {
        self.entries.insert(0, e);
        let keep = self.entries.len().min(MAX_ENTRIES);
        let json = match serde_json::to_vec_pretty(&self.entries[..keep]) {
            Ok(j) => j,
            Err(err) => {
                self.entries.remove(0);
                return Err(io::Error::new(io::ErrorKind::InvalidData, err));
            }
        };
        self.entries.truncate(MAX_ENTRIES);
        store_file::replace(&self.path, &json)
    }

    /// Newest first.
    pub fn list(&self) -> &[Entry] {
        &self.entries
    }

    pub fn clear(&mut self) -> io::Result<()> {
        self.entries.clear();
        self.save()
    }

    fn save(&self) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(&self.entries).map_err(io::Error::other)?;
        store_file::replace(&self.path, &json)
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// "just now", "2 min ago", "3 h ago", "yesterday", else "4 Oct 2025" (UTC date).
pub fn relative_time(at: u64, now: u64) -> String {
    let d = now.saturating_sub(at);
    match d {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", d / 60),
        3600..86_400 => format!("{} h ago", d / 3600),
        86_400..172_800 => "yesterday".into(),
        _ => {
            let (y, m, day) = civil_from_days((at / 86_400) as i64);
            const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
            format!("{day} {} {y}", MONTHS[(m - 1) as usize])
        }
    }
}

/// Days since 1970-01-01 → (year, month 1-12, day), proleptic Gregorian (H. Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
