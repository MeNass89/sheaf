//! When Sheaf checks for updates by itself and what it says, tested without a window. The check
//! itself is `sheaf_core::updates`; the dialog and the notice are `src/updates_ui.rs`.
//!
//! Sheaf goes online only when the user asks (C-platform-013): the daily check is off until the
//! user ticks it (DECISIONS 28/10/2026), in the Check for Updates dialog or Preferences ▸ Startup.

use serde::{Deserialize, Serialize};
use sheaf_core::updates::{Outcome, Release};

/// Seconds between two automatic checks.
pub const DAY: i64 = 24 * 60 * 60;

/// What Preferences keeps (`SettingsPrefs::updates`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct UpdatePrefs {
    /// Check once a day, at launch.
    pub auto: bool,
    /// When the last automatic check started (Unix seconds).
    pub last_check: Option<i64>,
    /// The version the user skipped ("0.3.0"): the daily check stays quiet about it, a check the
    /// user asks for still shows it.
    pub skipped: Option<String>,
}

impl UpdatePrefs {
    /// An automatic check is due at `now`: turned on, and none for a day (or the clock went back).
    pub fn due(&self, now: i64) -> bool {
        self.auto && self.last_check.is_none_or(|t| now < t || now - t >= DAY)
    }

    /// The daily check's result worth a notice: a newer version not skipped.
    pub fn tell<'a>(&self, outcome: &'a Outcome) -> Option<&'a Release> {
        match outcome {
            Outcome::Available(r, _) if self.skipped.as_deref() != Some(r.version.to_string().as_str()) => Some(r),
            _ => None,
        }
    }
}

/// A download's size as the dialog says it ("52 MB").
pub fn size_text(bytes: u64) -> String {
    match bytes {
        0 => String::new(),
        b if b < 1 << 20 => format!("{} KB", b.div_ceil(1 << 10)),
        b => format!("{:.0} MB", b as f64 / (1u64 << 20) as f64),
    }
}

/// Seconds since the Unix epoch.
pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_core::updates::{Asset, Version};

    fn release(v: &str) -> Release {
        Release { tag: format!("v{v}"), version: Version::parse(v).unwrap(), name: String::new(), notes: String::new(), page: String::new(), assets: vec![] }
    }

    #[test]
    fn the_daily_check_is_off_until_turned_on_then_once_a_day() {
        let mut p = UpdatePrefs::default();
        assert!(!p.due(1_000_000), "off by default: Sheaf goes online only when asked");
        p.auto = true;
        assert!(p.due(1_000_000));
        p.last_check = Some(1_000_000);
        assert!(!p.due(1_000_000 + DAY - 1));
        assert!(p.due(1_000_000 + DAY));
        // A clock set back does not silence it for ever.
        assert!(p.due(999_000));
    }

    #[test]
    fn a_skipped_version_gets_no_notice_but_a_newer_one_does() {
        let mut p = UpdatePrefs { auto: true, ..Default::default() };
        let found = Outcome::Available(release("0.3.0"), None::<Asset>);
        assert_eq!(p.tell(&found).map(|r| r.tag.as_str()), Some("v0.3.0"));
        p.skipped = Some("0.3.0".into());
        assert!(p.tell(&found).is_none());
        assert!(p.tell(&Outcome::Available(release("0.3.1"), None)).is_some());
        assert!(p.tell(&Outcome::UpToDate(Some(release("0.3.0")))).is_none());
    }

    #[test]
    fn sizes_read_plainly() {
        assert_eq!(size_text(0), "");
        assert_eq!(size_text(1500), "2 KB");
        assert_eq!(size_text(52_000_000), "50 MB");
    }

    #[test]
    fn old_preferences_load_with_the_check_off() {
        let p: crate::settings::SettingsPrefs = serde_json::from_str(r#"{"open_at_login":true}"#).unwrap();
        assert_eq!(p.updates, UpdatePrefs::default());
    }
}
