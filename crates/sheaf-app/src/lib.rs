//! Sheaf desktop app: the Slint UI, the job runner and file helpers.

slint::include_modules!();

pub mod actions;
pub mod library;
pub mod watch_folder;
pub mod files;
pub mod history;
pub mod icons;
pub mod jobs;
pub mod open_with;
pub mod pool;
pub mod prefs;
pub mod print;
pub mod reader;
pub mod saved_options;
pub mod store_file;
pub mod sign_preview;
pub mod viewer;
