//! Look up (PDF Expert's and Preview's, on a text selection): the selected words in the system's
//! dictionary: Dictionary on macOS, GoldenDict on Linux when installed; otherwise, and on
//! Windows, which has no dictionary app, Wiktionary in the browser.

use super::*;

/// The page a dictionary is asked for `words` (trimmed, one line).
pub(crate) fn dictionary_target(words: &str, local: bool) -> String {
    let w = words.split_whitespace().collect::<Vec<_>>().join(" ");
    let enc: String = w.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    if local { format!("dict://{enc}") } else { format!("https://en.wiktionary.org/wiki/{}", enc.replace("%20", "_")) }
}

pub fn look_up(ui: &AppWindow, words: slint::SharedString) {
    let words = words.trim();
    if words.is_empty() {
        return;
    }
    #[cfg(target_os = "macos")]
    crate::run::open_detached(dictionary_target(words, true));
    #[cfg(not(target_os = "macos"))]
    {
        let one_line = words.split_whitespace().collect::<Vec<_>>().join(" ");
        #[cfg(not(windows))]
        {
            if std::process::Command::new("goldendict").arg(&one_line).spawn().is_ok() {
                return;
            }
        }
        crate::run::open_detached(dictionary_target(&one_line, false));
        crate::flow::toast(ui, &sheaf_app::i18n::tr("Looked up in Wiktionary, in your browser"));
    }
    #[cfg(target_os = "macos")]
    let _ = ui;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn words_are_asked_of_the_dictionary_on_one_line() {
        assert_eq!(dictionary_target("  café\n au lait ", true), "dict://caf%C3%A9%20au%20lait");
        assert_eq!(dictionary_target("ad hoc", false), "https://en.wiktionary.org/wiki/ad_hoc");
    }
}
