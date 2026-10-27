//! Preferences that are not about one screen, tested without a window: the print sheet's defaults
//! (iLovePDF's Printer preferences: paper size, grayscale), what a document may open outside
//! Sheaf (Acrobat's Enhanced Security, Trust Manager and Privileged Locations), and opening Sheaf
//! when the user logs in (iLovePDF's "Open iLovePDF automatically after you log into the computer").

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What Preferences keeps for these (one field of `Prefs`, `settings`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct SettingsPrefs {
    pub print: PrintDefaults,
    pub trust: TrustPrefs,
    /// Sheaf opens when the user logs in (the system's own login item is written and removed).
    pub open_at_login: bool,
    /// Quitting while a task still runs asks first (iLovePDF's "Show a warning message when
    /// exiting a task that is still processing"); true turns the question off.
    pub no_quit_warning: bool,
}

/// How the print sheet opens (iLovePDF's Printer preferences, Acrobat's Print defaults): each
/// choice as the sheet's own buttons number it, so a default of 0 is the sheet as it always was.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
#[serde(default)]
pub struct PrintDefaults {
    pub two_sided: bool,
    /// 0 the printer's own, else `print::PAPERS[paper - 1]`.
    pub paper: u8,
    pub gray: bool,
    /// Comments left out (they print by default, as Acrobat's Document and Markups).
    pub no_comments: bool,
    /// Actual size rather than Fit to paper.
    pub actual_size: bool,
    /// 0 auto, 1 portrait, 2 landscape.
    pub orientation: u8,
    /// Pages per sheet as the sheet's Layout list: 0–5 1, 2, 4, 6, 9, 16; 6 a booklet.
    pub layout: u8,
}

impl PrintDefaults {
    /// Out-of-range numbers (an edited prefs.json) fall back to the sheet's own default.
    pub fn clamped(self) -> PrintDefaults {
        PrintDefaults {
            paper: if (self.paper as usize) <= crate::print::PAPERS.len() { self.paper } else { 0 },
            orientation: if self.orientation <= 2 { self.orientation } else { 0 },
            layout: if self.layout <= 6 { self.layout } else { 0 },
            ..self
        }
    }
}

/// Acrobat's Enhanced Security and Trust Manager: what a document may open outside Sheaf.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TrustPrefs {
    /// Acrobat's Enable Enhanced Security (on by default): documents outside the trusted
    /// locations ask before a web link opens, and are refused what the user blocked.
    pub enhanced: bool,
    /// Web links of untrusted documents (Acrobat's Internet Access from PDF files): 0 ask each
    /// site, 1 allow every site, 2 block every site.
    pub web: u8,
    /// Acrobat's "Allow opening of non-PDF file attachments with external applications": linked
    /// and attached files may open in other apps (after asking).
    pub other_apps: bool,
    /// Acrobat's Privileged Locations: files and folders whose documents are trusted.
    pub locations: Vec<PathBuf>,
    /// Sites remembered from the question (Acrobat's "Remember this action for this site"),
    /// allowed and blocked.
    pub allowed_sites: Vec<String>,
    pub blocked_sites: Vec<String>,
}

impl Default for TrustPrefs {
    fn default() -> Self {
        TrustPrefs { enhanced: true, web: 0, other_apps: true, locations: Vec::new(), allowed_sites: Vec::new(), blocked_sites: Vec::new() }
    }
}

/// What to do with a document's request to open something outside Sheaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Ask,
    Block,
}

impl TrustPrefs {
    /// `doc` lies in (or is) a trusted location. Paths are compared as the system resolves them,
    /// so a link or `..` cannot step into or out of one.
    pub fn trusted(&self, doc: &Path) -> bool {
        let doc = doc.canonicalize().unwrap_or_else(|_| doc.to_path_buf());
        self.locations.iter().any(|l| doc.starts_with(l.canonicalize().unwrap_or_else(|_| l.clone())))
    }

    /// A web or mail link of `doc` clicked.
    pub fn web_link(&self, doc: &Path, url: &str) -> Verdict {
        let site = site(url);
        if site.as_ref().is_some_and(|s| self.blocked_sites.contains(s)) {
            return Verdict::Block;
        }
        if !self.enhanced || self.trusted(doc) || site.as_ref().is_some_and(|s| self.allowed_sites.contains(s)) {
            return Verdict::Allow;
        }
        match self.web {
            1 => Verdict::Allow,
            2 => Verdict::Block,
            _ => Verdict::Ask,
        }
    }

    /// A linked or attached file of `doc` that is not a PDF, to open in its own app: always asked
    /// (Acrobat's launch warning), except from a trusted document under enhanced security.
    pub fn other_app(&self, doc: &Path) -> Verdict {
        if !self.other_apps {
            Verdict::Block
        } else if self.enhanced && self.trusted(doc) {
            Verdict::Allow
        } else {
            Verdict::Ask
        }
    }

    /// The answer to the question, remembered for the site when asked (`allow` false blocks).
    pub fn remember(&mut self, url: &str, allow: bool) {
        let Some(s) = site(url) else { return };
        self.allowed_sites.retain(|x| *x != s);
        self.blocked_sites.retain(|x| *x != s);
        if allow { self.allowed_sites.push(s) } else { self.blocked_sites.push(s) }
    }

    /// A trusted location added once (Acrobat's Add Folder Path / Add File).
    pub fn add_location(&mut self, p: PathBuf) {
        if !self.locations.contains(&p) {
            self.locations.push(p);
        }
    }
}

/// The site a link goes to: the host of a web address (lower case, without port or user), the
/// domain of a mail address.
pub fn site(url: &str) -> Option<String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let host = if let Some(rest) = lower.strip_prefix("mailto:") {
        let addr = rest.split(['?', ',']).next().unwrap_or("");
        addr.rsplit_once('@').map(|(_, d)| d)?
    } else {
        let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"))?;
        let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
        let auth = auth.rsplit_once('@').map(|(_, h)| h).unwrap_or(auth);
        if auth.starts_with('[') { auth.split(']').next().map(|h| &h[1..]).unwrap_or("") } else { auth.split(':').next().unwrap_or("") }
    };
    (!host.is_empty()).then(|| host.to_string())
}

/// Where the system keeps a login item for Sheaf, and what it holds to open `exe`:
/// Linux's XDG autostart entry, macOS's LaunchAgent. Windows keeps it in the registry
/// ([`login_registry_args`]), so None there.
pub fn login_item(home: &Path, exe: &Path) -> Option<(PathBuf, String)> {
    let exe = exe.display().to_string();
    if cfg!(target_os = "macos") {
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>Label</key>\n\t<string>io.github.sheaf</string>\n\t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{}</string>\n\t</array>\n\t<key>RunAtLoad</key>\n\t<true/>\n</dict>\n</plist>\n",
            xml_escape(&exe)
        );
        Some((home.join("Library/LaunchAgents/io.github.sheaf.plist"), plist))
    } else if cfg!(windows) {
        None
    } else {
        let quoted = if exe.contains([' ', '"', '\\']) { format!("\"{}\"", exe.replace('\\', "\\\\").replace('"', "\\\"")) } else { exe };
        let entry = format!("[Desktop Entry]\nType=Application\nName=Sheaf\nComment=Private PDF toolbox\nExec={quoted}\nIcon=sheaf\nX-GNOME-Autostart-enabled=true\n");
        let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".config"));
        Some((config.join("autostart/sheaf.desktop"), entry))
    }
}

/// Windows: `reg` arguments adding (`on`) or deleting Sheaf's value under the user's Run key.
pub fn login_registry_args(exe: &Path, on: bool) -> Vec<String> {
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run".to_string();
    let mut a = vec![if on { "add" } else { "delete" }.to_string(), key, "/v".into(), "Sheaf".into()];
    if on {
        a.extend(["/t".into(), "REG_SZ".into(), "/d".into(), format!("\"{}\"", exe.display())]);
    }
    a.push("/f".into());
    a
}

/// Writes or removes the login item for `exe` (Windows through `reg`).
pub fn set_open_at_login(home: &Path, exe: &Path, on: bool) -> std::io::Result<()> {
    match login_item(home, exe) {
        Some((path, text)) if on => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, text)
        }
        Some((path, _)) => match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
        None => {
            let ok = std::process::Command::new("reg").args(login_registry_args(exe, on)).status()?.success();
            if ok || !on { Ok(()) } else { Err(std::io::Error::other("the registry refused the login item")) }
        }
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_are_hosts_without_port_user_or_case_and_mail_domains() {
        assert_eq!(site("https://User:pw@Example.COM:8080/a?b#c").as_deref(), Some("example.com"));
        assert_eq!(site("http://example.org").as_deref(), Some("example.org"));
        assert_eq!(site("http://[::1]:80/x").as_deref(), Some("::1"));
        assert_eq!(site("mailto:Ann@Mail.Example.net?subject=Hi").as_deref(), Some("mail.example.net"));
        assert_eq!(site("file:///tmp/a.pdf"), None);
        assert_eq!(site("https:///nohost"), None);
    }

    #[test]
    fn enhanced_security_asks_for_untrusted_documents_and_trusted_locations_skip_it() {
        let dir = std::env::temp_dir().join(format!("sheaf-trust-{}", std::process::id()));
        let inside = dir.join("trusted");
        std::fs::create_dir_all(inside.join("sub")).unwrap();
        let doc_in = inside.join("sub/a.pdf");
        let doc_out = dir.join("b.pdf");
        std::fs::write(&doc_in, b"%PDF").unwrap();
        std::fs::write(&doc_out, b"%PDF").unwrap();
        let mut t = TrustPrefs::default();
        assert!(t.enhanced && t.other_apps && t.web == 0, "Acrobat's defaults: enhanced security on, asking");
        assert_eq!(t.web_link(&doc_out, "https://example.com/x"), Verdict::Ask);
        assert_eq!(t.other_app(&doc_out), Verdict::Ask);
        t.add_location(inside.clone());
        t.add_location(inside.clone());
        assert_eq!(t.locations.len(), 1);
        assert_eq!(t.web_link(&doc_in, "https://example.com/x"), Verdict::Allow);
        assert_eq!(t.other_app(&doc_in), Verdict::Allow);
        // `..` cannot climb out of a trusted folder.
        assert_eq!(t.web_link(&inside.join("../b.pdf"), "https://example.com/x"), Verdict::Ask);
        // Remembered sites: allowed opens, blocked never opens, even from a trusted document.
        t.remember("https://Example.com/other", true);
        assert_eq!(t.web_link(&doc_out, "https://example.com/y"), Verdict::Allow);
        t.remember("https://example.com", false);
        assert!(t.allowed_sites.is_empty());
        assert_eq!(t.web_link(&doc_in, "https://example.com/y"), Verdict::Block);
        // Every site allowed or blocked; enhanced security off allows (blocked sites aside).
        t.web = 2;
        assert_eq!(t.web_link(&doc_out, "https://other.test"), Verdict::Block);
        t.web = 1;
        assert_eq!(t.web_link(&doc_out, "https://other.test"), Verdict::Allow);
        t.web = 2;
        t.enhanced = false;
        assert_eq!(t.web_link(&doc_out, "https://other.test"), Verdict::Allow);
        assert_eq!(t.other_app(&doc_in), Verdict::Ask, "only enhanced security trusts locations");
        // Other apps refused outright, even for trusted documents.
        t.other_apps = false;
        t.enhanced = true;
        assert_eq!(t.other_app(&doc_in), Verdict::Block);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn print_defaults_start_as_the_sheet_did_and_bad_numbers_fall_back() {
        let d = PrintDefaults::default();
        assert!(!d.two_sided && d.paper == 0 && !d.gray && !d.no_comments && !d.actual_size && d.orientation == 0 && d.layout == 0);
        let bad = PrintDefaults { paper: 99, orientation: 7, layout: 9, gray: true, ..d }.clamped();
        assert_eq!(bad, PrintDefaults { gray: true, ..d });
        let good = PrintDefaults { paper: crate::print::PAPERS.len() as u8, orientation: 2, layout: 6, ..d };
        assert_eq!(good.clamped(), good);
        // Older prefs.json files without the field read as the defaults.
        let s: SettingsPrefs = serde_json::from_str("{}").unwrap();
        assert_eq!(s, SettingsPrefs::default());
        assert!(s.trust.enhanced);
    }

    #[test]
    fn login_items_open_sheaf_from_the_systems_own_place() {
        let home = Path::new("/home/ann");
        let exe = Path::new("/opt/Sheaf App/sheaf");
        match login_item(home, exe) {
            Some((path, text)) if cfg!(target_os = "macos") => {
                assert!(path.ends_with("Library/LaunchAgents/io.github.sheaf.plist"));
                assert!(text.contains("<string>/opt/Sheaf App/sheaf</string>") && text.contains("<key>RunAtLoad</key>"));
            }
            Some((path, text)) => {
                assert!(path.ends_with("autostart/sheaf.desktop"), "{path:?}");
                assert!(text.contains("Exec=\"/opt/Sheaf App/sheaf\"\n") && text.starts_with("[Desktop Entry]\n"), "{text}");
            }
            None => assert_eq!(std::env::consts::OS, "windows", "only Windows has no login item file"),
        }
        let on = login_registry_args(Path::new(r"C:\Sheaf\sheaf.exe"), true);
        assert_eq!(on, ["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "Sheaf", "/t", "REG_SZ", "/d", "\"C:\\Sheaf\\sheaf.exe\"", "/f"]);
        assert_eq!(login_registry_args(Path::new("x"), false), ["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "Sheaf", "/f"]);
        // Written and removed for real (Linux and macOS), removing twice is fine.
        if !cfg!(windows) {
            let tmp = std::env::temp_dir().join(format!("sheaf-login-{}", std::process::id()));
            if cfg!(target_os = "macos") || std::env::var_os("XDG_CONFIG_HOME").is_none() {
                set_open_at_login(&tmp, exe, true).unwrap();
                let (path, _) = login_item(&tmp, exe).unwrap();
                assert!(path.is_file());
                set_open_at_login(&tmp, exe, false).unwrap();
                set_open_at_login(&tmp, exe, false).unwrap();
                assert!(!path.exists());
                std::fs::remove_dir_all(&tmp).unwrap();
            }
        }
    }
}
