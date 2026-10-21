//! User preferences (`prefs.json`): appearance, where outputs go, what happens after a job.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store_file;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePref {
    /// The `Theme.mode` value: 0 = follow the OS, 1 = light, 2 = dark.
    pub fn mode(self) -> i32 {
        match self {
            ThemePref::System => 0,
            ThemePref::Light => 1,
            ThemePref::Dark => 2,
        }
    }
    pub fn from_mode(m: i32) -> Self {
        match m {
            1 => ThemePref::Light,
            2 => ThemePref::Dark,
            _ => ThemePref::System,
        }
    }
}

/// Where a job's outputs are written.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    /// Save dialog (one output) or folder picker (several).
    #[default]
    Ask,
    /// The first input's folder, default names.
    NextToOriginal,
    /// `Prefs::fixed_folder`, default names.
    Fixed,
}

impl OutputMode {
    pub fn index(self) -> i32 {
        match self {
            OutputMode::Ask => 0,
            OutputMode::NextToOriginal => 1,
            OutputMode::Fixed => 2,
        }
    }
    pub fn from_index(i: i32) -> Self {
        match i {
            1 => OutputMode::NextToOriginal,
            2 => OutputMode::Fixed,
            _ => OutputMode::Ask,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(default)]
pub struct Prefs {
    pub theme: ThemePref,
    pub output: OutputMode,
    pub fixed_folder: Option<PathBuf>,
    pub open_folder_after: bool,
    /// Name written as the author of annotations made in the reader.
    pub author: Option<String>,
    /// The reader's page theme (0 day, 1 night, 2 sepia), kept for the next document.
    pub reader_theme: i32,
    /// The page each document was left on (newest first, at most 200): reopening goes back to it.
    pub last_pages: Vec<(PathBuf, u32)>,
}

impl Prefs {
    /// Loads `<dir>/prefs.json`; missing or corrupt means the defaults.
    pub fn load(dir: &Path) -> Prefs {
        store_file::read(&dir.join("prefs.json")).unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        store_file::replace(&dir.join("prefs.json"), &json)
    }
}
