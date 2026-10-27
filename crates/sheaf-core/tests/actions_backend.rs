//! Acrobat's action model and Stirling's pipeline/watch backend have no Slint dependency.
//! Compile their production sources directly so logic regressions do not require the large UI
//! build. This supplements the real-app smoke checks; it does not replace them.

// UI entry points are intentionally unused in this isolated backend harness. The normal app's
// lint still checks the same modules without this allowance.
#[allow(dead_code)]
#[path = "../../sheaf-app/src/store_file.rs"]
mod store_file;
#[allow(dead_code)]
#[path = "../../sheaf-app/src/action_import.rs"]
mod action_import;
#[allow(dead_code)]
#[path = "../../sheaf-app/src/actions.rs"]
mod actions;
#[allow(dead_code)]
#[path = "../../sheaf-app/src/watch_folder.rs"]
mod watch_folder;
