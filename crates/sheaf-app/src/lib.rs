//! Sheaf desktop app: the Slint UI, the job runner and file helpers.

pub use sheaf_ui::*;

mod action_import;
pub mod actions;
pub mod ai_store;
pub mod action_settings;
pub mod library;
#[cfg(target_os = "macos")]
pub mod mac_menu;
pub mod javascript_console;
pub mod watch_folder;
pub mod display_color;
pub mod files;
pub mod file_preview;
pub mod history;
pub mod icons;
pub mod jobs;
pub mod open_with;
pub mod platform;
pub mod settings;
pub mod i18n;
pub mod feedback;
pub mod plugins;
pub mod pool;
pub mod quick_actions;
pub mod prefs;
pub mod print;
pub mod reader;
pub mod saved_options;
#[cfg(target_os = "macos")]
pub mod scan_mac;
pub mod security_policies;
pub mod store_file;
pub mod sign_preview;
mod signature_store;
pub mod viewer;
