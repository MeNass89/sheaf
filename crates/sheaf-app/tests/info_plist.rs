//! The bundle's Info.plist declares Sheaf as a PDF viewer for Finder's "Open With" menu,
//! never as the default PDF app.

use std::path::Path;
use std::process::Command;

const PLIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/macos/Info.plist");

fn text() -> String {
    std::fs::read_to_string(PLIST).expect("Info.plist is readable")
}

/// The `<string>` (or `<array>` of strings) that follows `<key>name</key>` inside `block`.
fn value_after<'a>(block: &'a str, name: &str) -> &'a str {
    let key = format!("<key>{name}</key>");
    let at = block.find(&key).unwrap_or_else(|| panic!("{name} missing")) + key.len();
    let rest = block[at..].trim_start();
    let end = if rest.starts_with("<array>") { rest.find("</array>").unwrap() + 8 } else { rest.find("</string>").unwrap() + 9 };
    &rest[..end]
}

#[test]
fn info_plist_declares_pdf_as_an_alternate_viewer() {
    let s = text();
    let at = s.find("<key>CFBundleDocumentTypes</key>").expect("CFBundleDocumentTypes missing");
    // These keys only exist inside the document types, so the first match after it is theirs.
    let types = &s[at..];
    assert!(value_after(types, "LSItemContentTypes").contains("<string>com.adobe.pdf</string>"));
    assert_eq!(value_after(types, "CFBundleTypeRole"), "<string>Viewer</string>");
    // Alternate: listed in "Open With", never taking over the user's default PDF app.
    assert_eq!(value_after(types, "LSHandlerRank"), "<string>Alternate</string>");
    assert!(!s.contains("<string>Owner</string>") && !s.contains("<string>Default</string>"));
}

#[test]
fn info_plist_parses() {
    let s = text();
    assert!(s.starts_with("<?xml") && s.trim_end().ends_with("</plist>"));
    // plutil is the reference parser; it ships with macOS.
    if cfg!(target_os = "macos") && Path::new("/usr/bin/plutil").exists() {
        let out = Command::new("/usr/bin/plutil").args(["-lint", PLIST]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
        let json = Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleDocumentTypes.0.LSItemContentTypes.0", "raw", "-o", "-", PLIST])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&json.stdout).trim(), "com.adobe.pdf");
    }
}
