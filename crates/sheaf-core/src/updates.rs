//! Check for updates (iLovePDF's Help ▸ Check for updates, Acrobat's Check for Updates): the
//! latest release published on GitHub (MeNass89/sheaf, Nassim 28/10/2026), compared with the
//! running version, with its notes and the download made for this computer. Nothing is installed:
//! the app opens the download in the browser. One blocking request (`ureq`), so callers run it off
//! the UI thread. Debug builds read `SHEAF_TEST_UPDATES_URL` instead of GitHub's address (tests,
//! smoke runs against a mock server).

use serde_json::Value;
use std::cmp::Ordering;
use std::time::Duration;

/// GitHub's latest release of Sheaf: never a draft nor a prerelease.
pub const LATEST: &str = "https://api.github.com/repos/MeNass89/sheaf/releases/latest";
/// Where every release is listed, for a check that found none.
pub const RELEASES: &str = "https://github.com/MeNass89/sheaf/releases";

/// A version as tags write it: `v1.2.3`, `1.2`, `1.2.3-beta.1`. A prerelease part ranks below the
/// same numbers without one; build metadata (`+…`) is ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: String,
}

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let s = s.strip_prefix(['v', 'V']).unwrap_or(s);
        let s = s.split('+').next().unwrap_or_default();
        let (nums, pre) = s.split_once('-').unwrap_or((s, ""));
        let mut it = nums.split('.');
        let mut part = |required: bool| -> Option<u64> {
            match it.next() {
                Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => n.parse().ok(),
                None if !required => Some(0),
                _ => None,
            }
        };
        let (major, minor, patch) = (part(true)?, part(true)?, part(false)?);
        if it.next().is_some() {
            return None;
        }
        Some(Version { major, minor, patch, pre: pre.to_string() })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre)?;
        }
        Ok(())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)).then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            // Semver's rule: dot-separated parts, numbers by value and below words.
            (false, false) => {
                let (a, b): (Vec<&str>, Vec<&str>) = (self.pre.split('.').collect(), other.pre.split('.').collect());
                for (x, y) in a.iter().zip(&b) {
                    let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                        (Ok(x), Ok(y)) => x.cmp(&y),
                        (Ok(_), Err(_)) => Ordering::Less,
                        (Err(_), Ok(_)) => Ordering::Greater,
                        (Err(_), Err(_)) => x.cmp(y),
                    };
                    if o != Ordering::Equal {
                        return o;
                    }
                }
                a.len().cmp(&b.len())
            }
        })
    }
}

/// A file attached to a release.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub name: String,
    /// Its download address (`browser_download_url`).
    pub url: String,
    pub size: u64,
}

/// A published release.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub tag: String,
    pub version: Version,
    /// Its title ("" when it has none).
    pub name: String,
    /// Its notes as written (Markdown).
    pub notes: String,
    /// Its page on GitHub.
    pub page: String,
    pub assets: Vec<Asset>,
}

/// What a check found.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Nothing newer than the running version (the latest release, when there is one).
    UpToDate(Option<Release>),
    /// A newer release, and the file made for this computer when the release has one.
    Available(Release, Option<Asset>),
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UpdateError {
    #[error("GitHub could not be reached ({0}). Check the connection and try again.")]
    Offline(String),
    #[error("GitHub asks to wait before checking again (too many checks from this network).")]
    RateLimited(Option<u64>),
    #[error("GitHub answered with an error ({0}).")]
    Http(u16),
    #[error("GitHub's answer could not be read.")]
    BadResponse,
}

/// The release in GitHub's answer to `releases/latest` (`status`, `body`): `None` when none is
/// published yet (404), a draft, or a prerelease; an error for a refusal. `remaining` and `reset`
/// are the `x-ratelimit-*` headers (a 403 with nothing remaining is the rate limit; `reset` is the
/// Unix time it lifts).
pub fn parse_latest(status: u16, remaining: Option<&str>, reset: Option<&str>, body: &str) -> Result<Option<Release>, UpdateError> {
    if status == 404 {
        return Ok(None);
    }
    if status == 429 || (status == 403 && remaining.is_some_and(|r| r.trim() == "0")) {
        return Err(UpdateError::RateLimited(reset.and_then(|r| r.trim().parse().ok())));
    }
    if !(200..300).contains(&status) {
        return Err(UpdateError::Http(status));
    }
    let v: Value = serde_json::from_str(body).map_err(|_| UpdateError::BadResponse)?;
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) {
        return Ok(None);
    }
    let tag = v["tag_name"].as_str().ok_or(UpdateError::BadResponse)?.to_string();
    let version = Version::parse(&tag).ok_or(UpdateError::BadResponse)?;
    if !version.pre.is_empty() {
        return Ok(None);
    }
    let text = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    let assets = v["assets"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some(Asset {
                        name: x["name"].as_str()?.to_string(),
                        url: x["browser_download_url"].as_str()?.to_string(),
                        size: x["size"].as_u64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let page = Some(text("html_url")).filter(|p| !p.is_empty()).unwrap_or_else(|| RELEASES.to_string());
    Ok(Some(Release { tag, version, name: text("name"), notes: text("body"), page, assets }))
}

/// What `current` should do about `latest`.
pub fn outcome(current: &Version, latest: Option<Release>, os: &str, arch: &str) -> Outcome {
    match latest {
        Some(r) if r.version > *current => {
            let asset = pick_asset(&r.assets, os, arch).cloned();
            Outcome::Available(r, asset)
        }
        other => Outcome::UpToDate(other),
    }
}

/// The download made for `os` and `arch` (`std::env::consts` names): a disk image on macOS, an
/// installer (else the portable zip) on Windows, an AppImage, a .deb (else the tarball) on Linux.
/// A file naming another processor is never chosen; one naming none (or "universal") fits all.
pub fn pick_asset<'a>(assets: &'a [Asset], os: &str, arch: &str) -> Option<&'a Asset> {
    let kinds: &[(&str, &str)] = match os {
        "macos" => &[(".dmg", "")],
        "windows" => &[(".msi", ""), (".exe", ""), (".zip", "win")],
        "linux" => &[(".appimage", ""), (".deb", ""), (".tar.gz", "linux")],
        _ => &[],
    };
    let tokens = |a: &str| -> &'static [&'static str] {
        match a {
            "x86_64" => &["x86_64", "x64", "amd64", "intel"],
            "aarch64" => &["aarch64", "arm64"],
            _ => &[],
        }
    };
    let all = ["x86_64", "aarch64"];
    let words = |name: &str| -> Vec<String> { name.to_ascii_lowercase().split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map(str::to_string).collect() };
    let names = |w: &[String], a: &str| tokens(a).iter().any(|t| w.iter().any(|x| x == t));
    for (ext, needs) in kinds {
        let mut fits: Vec<(&Asset, bool)> = assets
            .iter()
            .filter(|x| {
                let lower = x.name.to_ascii_lowercase();
                lower.ends_with(ext) && (needs.is_empty() || lower.contains(needs))
            })
            .filter_map(|x| {
                let w = words(&x.name);
                let other = all.iter().filter(|a| **a != arch).any(|a| names(&w, a));
                let mine = names(&w, arch);
                // Ours named, or no processor named at all (a universal build).
                (mine || !other).then_some((x, mine))
            })
            .collect();
        // A file naming our processor beats one naming none.
        fits.sort_by_key(|(_, mine)| !mine);
        if let Some((a, _)) = fits.first() {
            return Some(a);
        }
    }
    None
}

/// The address checked: GitHub's, or `SHEAF_TEST_UPDATES_URL` in a debug build.
pub fn api_url() -> String {
    if cfg!(debug_assertions)
        && let Ok(u) = std::env::var("SHEAF_TEST_UPDATES_URL")
        && !u.is_empty()
    {
        return u;
    }
    LATEST.to_string()
}

/// Asks `url` for the latest release, for a running `current` version (`CARGO_PKG_VERSION`).
pub fn check_at(url: &str, current: &str) -> Result<Outcome, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .timeout_connect(Some(Duration::from_secs(8)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut resp = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", &format!("Sheaf/{current}"))
        .call()
        .map_err(offline)?;
    let status = resp.status().as_u16();
    let header = |k: &str| resp.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    let (remaining, reset) = (header("x-ratelimit-remaining"), header("x-ratelimit-reset"));
    let body = resp.body_mut().read_to_string().map_err(offline)?;
    let latest = parse_latest(status, remaining.as_deref(), reset.as_deref(), &body)?;
    let current = Version::parse(current).ok_or(UpdateError::BadResponse)?;
    Ok(outcome(&current, latest, std::env::consts::OS, std::env::consts::ARCH))
}

/// `check_at` GitHub (see [`api_url`]).
pub fn check(current: &str) -> Result<Outcome, UpdateError> {
    check_at(&api_url(), current)
}

fn offline(e: ureq::Error) -> UpdateError {
    UpdateError::Offline(match e {
        ureq::Error::Timeout(_) => "no answer in time".to_string(),
        ureq::Error::HostNotFound => "no connection to the internet?".to_string(),
        ureq::Error::ConnectionFailed => "the connection was refused".to_string(),
        e => e.to_string(),
    })
}

/// Release notes in Markdown as plain text for a dialog: headings on their own lines, list items
/// with a bullet, links as their text, the rest as written.
pub fn notes_text(markdown: &str) -> String {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut out = String::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let newline = |out: &mut String| {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
    };
    for e in Parser::new(markdown) {
        match e {
            // A block starts after a blank line, except inside a list item (a loose list's paragraphs).
            Event::Start(Tag::Heading { .. } | Tag::Paragraph | Tag::CodeBlock(_)) if lists.is_empty() => {
                newline(&mut out);
                if !out.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::List(first)) => {
                newline(&mut out);
                lists.push(first);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                newline(&mut out);
                out.push_str(&"  ".repeat(lists.len().saturating_sub(1)));
                match lists.last_mut() {
                    Some(Some(n)) => {
                        out.push_str(&format!("{n}. "));
                        *n += 1;
                    }
                    _ => out.push_str("• "),
                }
            }
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push('\n'),
            _ => {}
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn asset(name: &str) -> Asset {
        Asset { name: name.into(), url: format!("https://example.test/{name}"), size: 1 }
    }

    #[test]
    fn versions_are_read_as_tags_write_them() {
        assert_eq!(v("v1.2.3"), Version { major: 1, minor: 2, patch: 3, pre: String::new() });
        assert_eq!(v("0.3"), v("0.3.0"));
        assert_eq!(v("V2.0.1+build.7"), v("2.0.1"));
        assert_eq!(v("1.0.0-beta.2").pre, "beta.2");
        for bad in ["", "v", "1", "1.x.0", "1.2.3.4", "release-1", "1..2"] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
        assert_eq!(v("v0.10.0").to_string(), "0.10.0");
    }

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(v("0.10.0") > v("0.9.9"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("0.2.2") > v("0.2.1"));
        assert_eq!(v("0.2.1").cmp(&v("v0.2.1")), Ordering::Equal);
        assert!(v("1.0.0") > v("1.0.0-rc.1"));
        assert!(v("1.0.0-rc.2") > v("1.0.0-rc.1"));
        assert!(v("1.0.0-rc.10") > v("1.0.0-rc.9"));
        assert!(v("1.0.0-beta") < v("1.0.0-rc"));
        assert!(v("1.0.0-alpha") < v("1.0.0-alpha.1"));
    }

    const RELEASE: &str = r#"{"tag_name":"v0.3.0","name":"Sheaf 0.3","body":"Faster pages","html_url":"https://github.com/MeNass89/sheaf/releases/tag/v0.3.0","draft":false,"prerelease":false,
        "assets":[{"name":"Sheaf-0.3.0-arm64.dmg","browser_download_url":"https://github.com/MeNass89/sheaf/releases/download/v0.3.0/Sheaf-0.3.0-arm64.dmg","size":52000000},{"name":"broken"}]}"#;

    #[test]
    fn github_answers_are_read() {
        let r = parse_latest(200, Some("59"), None, RELEASE).unwrap().unwrap();
        assert_eq!((r.tag.as_str(), r.version.to_string().as_str(), r.name.as_str(), r.notes.as_str()), ("v0.3.0", "0.3.0", "Sheaf 0.3", "Faster pages"));
        assert_eq!(r.page, "https://github.com/MeNass89/sheaf/releases/tag/v0.3.0");
        // An asset without its address is left out.
        assert_eq!(r.assets.len(), 1);
        assert_eq!(r.assets[0].size, 52_000_000);
        // No release yet; drafts and prereleases are not offered.
        assert_eq!(parse_latest(404, None, None, r#"{"message":"Not Found"}"#), Ok(None));
        assert_eq!(parse_latest(200, None, None, &RELEASE.replace(r#""draft":false"#, r#""draft":true"#)), Ok(None));
        assert_eq!(parse_latest(200, None, None, &RELEASE.replace(r#""prerelease":false"#, r#""prerelease":true"#)), Ok(None));
        assert_eq!(parse_latest(200, None, None, &RELEASE.replace("v0.3.0\"", "v0.3.0-rc.1\"")), Ok(None));
        // Refusals, said plainly.
        assert_eq!(parse_latest(403, Some("0"), Some("1761649200"), "{}"), Err(UpdateError::RateLimited(Some(1_761_649_200))));
        assert_eq!(parse_latest(429, None, None, ""), Err(UpdateError::RateLimited(None)));
        assert_eq!(parse_latest(403, Some("12"), None, "{}"), Err(UpdateError::Http(403)));
        assert_eq!(parse_latest(502, None, None, "<html>"), Err(UpdateError::Http(502)));
        assert_eq!(parse_latest(200, None, None, "<html>"), Err(UpdateError::BadResponse));
        assert_eq!(parse_latest(200, None, None, r#"{"tag_name":"nightly"}"#), Err(UpdateError::BadResponse));
        assert!(UpdateError::Offline("no answer in time".into()).to_string().contains("could not be reached"));
    }

    #[test]
    fn only_a_newer_release_is_offered() {
        let r = parse_latest(200, None, None, RELEASE).unwrap();
        let Outcome::Available(found, Some(a)) = outcome(&v("0.2.1"), r.clone(), "macos", "aarch64") else { panic!() };
        assert_eq!((found.tag.as_str(), a.name.as_str()), ("v0.3.0", "Sheaf-0.3.0-arm64.dmg"));
        // Newer, but nothing built for this computer: the release page instead.
        assert!(matches!(outcome(&v("0.2.1"), r.clone(), "linux", "x86_64"), Outcome::Available(_, None)));
        assert_eq!(outcome(&v("0.3.0"), r.clone(), "macos", "aarch64"), Outcome::UpToDate(r.clone()));
        assert_eq!(outcome(&v("0.4.0"), r.clone(), "macos", "aarch64"), Outcome::UpToDate(r));
        assert_eq!(outcome(&v("0.2.1"), None, "macos", "aarch64"), Outcome::UpToDate(None));
    }

    #[test]
    fn the_download_is_the_one_made_for_this_computer() {
        let all: Vec<Asset> = [
            "Sheaf-0.3.0-arm64.dmg",
            "Sheaf-0.3.0-x86_64.dmg",
            "Sheaf-0.3.0-windows-x64.zip",
            "Sheaf-0.3.0-x64.msi",
            "Sheaf-0.3.0-linux-x64.tar.gz",
            "Sheaf-0.3.0-amd64.deb",
            "Sheaf-0.3.0-aarch64.AppImage",
            "SHA256SUMS.txt",
        ]
        .map(asset)
        .to_vec();
        let pick = |os, arch| pick_asset(&all, os, arch).map(|a| a.name.as_str());
        assert_eq!(pick("macos", "aarch64"), Some("Sheaf-0.3.0-arm64.dmg"));
        assert_eq!(pick("macos", "x86_64"), Some("Sheaf-0.3.0-x86_64.dmg"));
        assert_eq!(pick("windows", "x86_64"), Some("Sheaf-0.3.0-x64.msi"));
        // The AppImage is for ARM: an x86 Linux takes the .deb.
        assert_eq!(pick("linux", "x86_64"), Some("Sheaf-0.3.0-amd64.deb"));
        assert_eq!(pick("linux", "aarch64"), Some("Sheaf-0.3.0-aarch64.AppImage"));
        assert_eq!(pick("freebsd", "x86_64"), None);
        // Today's packages (scripts/make-dmg.sh, package-*.sh), and a universal image.
        let today: Vec<Asset> = ["Sheaf-0.3.0-arm64.dmg", "Sheaf-0.3.0-windows-x64.zip", "Sheaf-0.3.0-linux-x64.tar.gz"].map(asset).to_vec();
        assert_eq!(pick_asset(&today, "windows", "x86_64").map(|a| a.name.as_str()), Some("Sheaf-0.3.0-windows-x64.zip"));
        assert_eq!(pick_asset(&today, "linux", "x86_64").map(|a| a.name.as_str()), Some("Sheaf-0.3.0-linux-x64.tar.gz"));
        assert_eq!(pick_asset(&today, "macos", "x86_64"), None);
        assert_eq!(pick_asset(&today, "linux", "aarch64"), None);
        let universal = [asset("Sheaf-universal.dmg"), asset("Sheaf.dmg")];
        assert_eq!(pick_asset(&universal, "macos", "x86_64").map(|a| a.name.as_str()), Some("Sheaf-universal.dmg"));
        // One naming the processor beats one naming none.
        let both = [asset("Sheaf.dmg"), asset("Sheaf-arm64.dmg")];
        assert_eq!(pick_asset(&both, "macos", "aarch64").map(|a| a.name.as_str()), Some("Sheaf-arm64.dmg"));
    }

    #[test]
    fn notes_read_as_plain_text() {
        let md = "## What's new\n\n- Faster **pages**\n- [Updates](https://x.test) from GitHub\n  - nested\n\n1. one\n2. two\n\nThanks `all`.";
        assert_eq!(notes_text(md), "What's new\n• Faster pages\n• Updates from GitHub\n  • nested\n1. one\n2. two\n\nThanks all.");
        assert_eq!(notes_text(""), "");
    }
}
