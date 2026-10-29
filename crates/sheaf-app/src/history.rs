//! Task history (`history.json`) and separate Recents exclusions (`hidden-recents.json`).

use std::collections::BTreeSet;
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
    /// The options it ran with, passwords left out (`saved_options`); empty in older files.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub options: serde_json::Map<String, serde_json::Value>,
}

pub struct Store {
    path: PathBuf,
    entries: Vec<Entry>,
    hidden_path: PathBuf,
    hidden_recents: BTreeSet<PathBuf>,
}

impl Store {
    /// Loads `<dir>/history.json`. A missing or corrupt file is an empty history; the folder
    /// is created on the first save.
    pub fn open(dir: &Path) -> Store {
        let path = dir.join("history.json");
        let mut entries: Vec<Entry> = store_file::read(&path).unwrap_or_default();
        entries.truncate(MAX_ENTRIES);
        let hidden_path = dir.join("hidden-recents.json");
        let hidden_recents = store_file::read(&hidden_path).unwrap_or_default();
        Store { path, entries, hidden_path, hidden_recents }
    }

    /// Adds `e` at the top and saves. An entry JSON cannot hold (a non-UTF-8 path) is
    /// rejected and not kept, so it never blocks later saves. A disk error keeps it in
    /// memory; the next save retries.
    pub fn push(&mut self, e: Entry) -> io::Result<()> {
        let outputs = e.outputs.clone();
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
        store_file::replace(&self.path, &json)?;
        let hidden = {
            let current: BTreeSet<_> = self.entries.iter().flat_map(|e| &e.outputs).collect();
            let reopened: BTreeSet<_> = outputs.iter().collect();
            let mut hidden = self.hidden_recents.clone();
            // Opening or producing a file again restores it; expired history needs no tombstone.
            hidden.retain(|p| current.contains(p) && !reopened.contains(p));
            hidden
        };
        self.set_hidden_recents(hidden)
    }

    /// Whether a history output may appear in Recents; the task entry itself is unchanged.
    pub fn visible_in_recents(&self, path: &Path) -> bool {
        !self.hidden_recents.contains(path)
    }

    pub fn remove_recent(&mut self, path: &Path) -> io::Result<()> {
        let mut hidden = self.hidden_recents.clone();
        hidden.insert(path.to_path_buf());
        self.set_hidden_recents(hidden)
    }

    /// Hide all recorded outputs, including those beyond the visible card limit.
    pub fn clear_recents(&mut self) -> io::Result<()> {
        let hidden = self.entries.iter().flat_map(|e| e.outputs.iter().cloned()).collect();
        self.set_hidden_recents(hidden)
    }

    fn set_hidden_recents(&mut self, hidden: BTreeSet<PathBuf>) -> io::Result<()> {
        if hidden == self.hidden_recents { return Ok(()); }
        let json = serde_json::to_vec_pretty(&hidden).map_err(io::Error::other)?;
        // Save independently so a Recents action never rewrites or deletes task history.
        store_file::replace(&self.hidden_path, &json)?;
        self.hidden_recents = hidden;
        Ok(())
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
