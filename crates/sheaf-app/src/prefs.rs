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

/// A kept signature: drawn strokes (y up, the pad 1 wide), or a typed name when there are none;
/// the pen it was made with (ink, width: see `Prefs::signature_pen`); the handwriting font of a
/// typed name (`sheaf_core::script::ScriptFont` index).
pub type KeptSignature = (Vec<Vec<[f64; 2]>>, Option<String>, (u8, u8), u8);

/// Kept signatures as saved: before handwriting fonts, without the font (it was the first).
fn kept_signatures<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<KeptSignature>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Saved {
        WithFont(Vec<Vec<[f64; 2]>>, Option<String>, (u8, u8), u8),
        Before(Vec<Vec<[f64; 2]>>, Option<String>, (u8, u8)),
    }
    let list: Vec<Saved> = Vec::deserialize(d)?;
    Ok(list
        .into_iter()
        .map(|s| match s {
            Saved::WithFont(a, b, c, f) => (a, b, c, f),
            Saved::Before(a, b, c) => (a, b, c, 0),
        })
        .collect())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
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
    /// Fillable fields are not tinted in the reader (More ▸ Highlight form fields off).
    pub plain_fields: bool,
    /// The sixth colour of the reader's colour row (Acrobat's "Other color"), RRGGBB: for strokes
    /// and text, and for highlights and notes.
    pub custom_ink: Option<String>,
    pub custom_light: Option<String>,
    /// How dated stamps write the time: 0 Acrobat's ("10:32 am, Oct 25, 2026"), 1 day first
    /// ("25/10/2026 10:32"), 2 ISO ("2026-10-25 10:32").
    pub stamp_date: i32,
    /// How a document never opened before is shown: 0 fit width, 1 fit page, 2 actual size.
    pub default_zoom: i32,
    /// And its pages: 0 one column, 1 two side by side, 2 two with the cover alone (PDF Expert's
    /// and Preview's default view).
    pub default_spread: u8,
    /// Documents reopen on their first page, not where they were left (Acrobat's "Restore last view
    /// settings when reopening documents" off).
    pub open_first_page: bool,
    /// The reader's side panel width in px, dragged by its edge (0: the default, 248).
    pub panel_width: u16,
    /// Accent colour (0 Sheaf red, 1 blue, 2 green, 3 purple, 4 orange, 5 graphite).
    pub accent: u8,
    /// The window background behind the pages (0 as the theme, 1 light grey, 2 mid grey, 3 dark,
    /// 4 black; a page theme's own background, Night or Sepia, comes first).
    pub backdrop: u8,
    /// The page each document was left on (newest first, at most 200): reopening goes back to it.
    pub last_pages: Vec<(PathBuf, u32)>,
    /// How each document was viewed when left: fit mode (0 width, 1 page, 2 a chosen zoom), the
    /// zoom, pages side by side (newest first, as `last_pages`).
    pub last_views: Vec<(PathBuf, i32, u32, u8)>,
    /// The reader's style per tool, kept between sessions as PDF Expert keeps it: tool, colour,
    /// width, opacity (indices).
    pub tool_styles: Vec<(i32, u8, u8, u8)>,
    /// Tools starred by the user (`ToolId::as_str`), in the order starred.
    pub favourite_tools: Vec<String>,
    /// Files starred on Home (PDF Expert's Favorites, Acrobat's Starred): listed first, kept
    /// when Recents are cleared.
    pub pinned_files: Vec<PathBuf>,
    /// The signature made in the reader, reused on every document (PDF Expert keeps it too):
    /// drawn strokes (pad units, y up) or a typed name.
    pub signature_strokes: Vec<Vec<[f64; 2]>>,
    pub signature_name: Option<String>,
    /// The signature's pen: colour (index in `reader::SIGNATURE_INKS`) and width (0 fine, 1 medium, 2 bold).
    pub signature_pen: (u8, u8),
    /// The handwriting font of the typed `signature_name` (`sheaf_core::script::ScriptFont` index).
    pub signature_font: u8,
    /// Signatures made before, newest first (at most four): strokes or a typed name each. The
    /// first is `signature_strokes` / `signature_name`.
    #[serde(deserialize_with = "kept_signatures")]
    pub signatures: Vec<KeptSignature>,
    /// The initials kept apart from the signatures (Acrobat's Fill & Sign ▸ Add Initials).
    pub initials: Option<KeptSignature>,
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
