//! Plugins (Acrobat's plug-ins, Stirling's custom scripts): a folder holding a `plugin.json`
//! manifest (its name, version, and the tools it adds, each a command run on a PDF). Sheaf never
//! loads plugin code: a tool runs as a separate process on a copy of the PDF, writes a new PDF,
//! and Sheaf checks what it wrote. Each plugin version is run only after the user allowed it once
//! (Acrobat warns before running an untrusted plug-in).
//!
//! ```json
//! { "id": "reviewed-stamp", "name": "Reviewed stamp", "version": "1.0", "author": "…",
//!   "description": "…",
//!   "tools": [{ "id": "stamp", "name": "Mark as reviewed", "description": "…",
//!               "run": ["python3", "stamp.py", "{input}", "{output}"] }] }
//! ```

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A plugin's manifest file in its folder.
pub const MANIFEST: &str = "plugin.json";
/// How long one run may take before it is stopped.
pub const TIMEOUT: Duration = Duration::from_secs(300);
/// An output larger than this is refused.
const MAX_OUTPUT: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub tools: Vec<PluginTool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginTool {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// The program and its arguments; `{input}` and `{output}` are the PDF read and the PDF to write.
    pub run: Vec<String>,
}

/// A plugin found in the plugins folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Plugin {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub enabled: bool,
    /// This version was allowed to run.
    pub allowed: bool,
}

impl Plugin {
    /// "id@version": what an approval covers (a new version asks again).
    pub fn key(&self) -> String {
        format!("{}@{}", self.manifest.id, self.manifest.version)
    }
}

/// What Sheaf keeps about plugins, beside them (`plugins.json`): those turned off, versions allowed.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginState {
    pub disabled: Vec<String>,
    pub allowed: Vec<String>,
}

fn ident(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let m: Manifest = serde_json::from_str(text).map_err(|e| format!("plugin.json: {e}"))?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !ident(&self.id) {
            return Err("A plugin id is 1–64 lower-case letters, digits, - or _".into());
        }
        if self.name.trim().is_empty() || self.name.len() > 128 || self.version.trim().is_empty() || self.version.len() > 32 {
            return Err("A plugin needs a name (up to 128 bytes) and a version".into());
        }
        if self.tools.is_empty() || self.tools.len() > 20 {
            return Err("A plugin adds 1–20 tools".into());
        }
        for t in &self.tools {
            if !ident(&t.id) || t.name.trim().is_empty() || t.name.len() > 128 {
                return Err(format!("Tool {:?}: an id (lower-case letters, digits, - or _) and a name are needed", t.id));
            }
            if self.tools.iter().filter(|o| o.id == t.id).count() > 1 {
                return Err(format!("Tool id {:?} is used twice", t.id));
            }
            let Some(program) = t.run.first() else { return Err(format!("Tool {:?} runs nothing", t.id)) };
            if !t.run.iter().any(|a| a.contains("{input}")) || !t.run.iter().any(|a| a.contains("{output}")) {
                return Err(format!("Tool {:?}: its command needs {{input}} and {{output}}", t.id));
            }
            if program.contains("{input}") || program.contains("{output}") || Path::new(program).is_absolute() || program.split(['/', '\\']).any(|c| c == "..") {
                return Err(format!("Tool {:?}: the program is a name on the system's path or a file inside the plugin", t.id));
            }
        }
        Ok(())
    }
}

/// The plugins folder, in Sheaf's data folder.
pub fn folder() -> PathBuf {
    #[cfg(test)]
    if let Some(d) = TEST_FOLDER.with(|f| f.borrow().clone()) {
        return d;
    }
    crate::store_file::data_dir().join("plugins")
}

#[cfg(test)]
thread_local! {
    /// A test's own plugins folder (this thread only).
    pub static TEST_FOLDER: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join("plugins.json")
}

pub fn load_state(dir: &Path) -> PluginState {
    crate::store_file::read::<PluginState>(&state_path(dir)).unwrap_or_default()
}

pub fn save_state(dir: &Path, s: &PluginState) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    crate::store_file::replace(&state_path(dir), &serde_json::to_vec_pretty(s).map_err(std::io::Error::other)?)
}

/// Every plugin of `dir`, by name; folders whose manifest is wrong are returned apart with why.
pub fn list(dir: &Path) -> (Vec<Plugin>, Vec<(PathBuf, String)>) {
    let state = load_state(dir);
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    let Ok(rd) = std::fs::read_dir(dir) else { return (ok, bad) };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        match std::fs::read_to_string(p.join(MANIFEST)).map_err(|e| e.to_string()).and_then(|t| Manifest::parse(&t)) {
            Ok(manifest) if ok.iter().any(|o: &Plugin| o.manifest.id == manifest.id) => bad.push((p, format!("Another plugin is {:?}", manifest.id))),
            Ok(manifest) => {
                let key = format!("{}@{}", manifest.id, manifest.version);
                ok.push(Plugin { enabled: !state.disabled.contains(&manifest.id), allowed: state.allowed.contains(&key), dir: p, manifest });
            }
            Err(why) => bad.push((p, why)),
        }
    }
    ok.sort_by_key(|p| p.manifest.name.to_lowercase());
    (ok, bad)
}

/// A plugin's tool by its "plugin/tool" id, among the enabled plugins.
pub fn find(dir: &Path, id: &str) -> Option<(Plugin, PluginTool)> {
    let (pid, tid) = id.split_once('/')?;
    let (plugins, _) = list(dir);
    let p = plugins.into_iter().find(|p| p.enabled && p.manifest.id == pid)?;
    let t = p.manifest.tools.iter().find(|t| t.id == tid)?.clone();
    Some((p, t))
}

/// Turns a plugin on or off.
pub fn set_enabled(dir: &Path, id: &str, on: bool) -> std::io::Result<()> {
    let mut s = load_state(dir);
    s.disabled.retain(|d| d != id);
    if !on {
        s.disabled.push(id.to_string());
    }
    save_state(dir, &s)
}

/// The user allowed this version to run.
pub fn allow(dir: &Path, key: &str) -> std::io::Result<()> {
    let mut s = load_state(dir);
    if !s.allowed.iter().any(|a| a == key) {
        s.allowed.push(key.to_string());
    }
    save_state(dir, &s)
}

/// Copies the plugin folder `from` into `dir` (named after its id), after checking its manifest.
/// A plugin of the same id is replaced; its approval is not carried over to another version.
pub fn install(dir: &Path, from: &Path) -> Result<Manifest, String> {
    let manifest = Manifest::parse(&std::fs::read_to_string(from.join(MANIFEST)).map_err(|e| format!("No plugin.json in that folder: {e}"))?)?;
    let to = dir.join(&manifest.id);
    if from.canonicalize().ok() == to.canonicalize().ok() {
        return Ok(manifest);
    }
    let staging = dir.join(format!(".installing-{}", manifest.id));
    let _ = std::fs::remove_dir_all(&staging);
    copy_dir(from, &staging, 0).map_err(|e| format!("Could not copy the plugin: {e}"))?;
    let _ = std::fs::remove_dir_all(&to);
    std::fs::rename(&staging, &to).map_err(|e| e.to_string())?;
    let mut s = load_state(dir);
    s.disabled.retain(|d| *d != manifest.id);
    save_state(dir, &s).map_err(|e| e.to_string())?;
    Ok(manifest)
}

fn copy_dir(from: &Path, to: &Path, depth: usize) -> std::io::Result<()> {
    if depth > 8 {
        return Err(std::io::Error::other("folders nested too deep"));
    }
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        let kind = e.file_type()?;
        if kind.is_dir() {
            copy_dir(&src, &dst, depth + 1)?;
        } else if kind.is_file() {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Deletes a plugin's folder and what Sheaf kept about it.
pub fn remove(dir: &Path, plugin: &Plugin) -> std::io::Result<()> {
    if plugin.dir.parent() != Some(dir) {
        return Err(std::io::Error::other("not in the plugins folder"));
    }
    std::fs::remove_dir_all(&plugin.dir)?;
    let mut s = load_state(dir);
    s.disabled.retain(|d| *d != plugin.manifest.id);
    s.allowed.retain(|a| !a.starts_with(&format!("{}@", plugin.manifest.id)));
    save_state(dir, &s)
}

/// The command a tool runs on `input`, writing `output`: its program found in the plugin's folder
/// when it names a file there, else on the system's path.
pub fn command(plugin: &Plugin, tool: &PluginTool, input: &Path, output: &Path) -> std::process::Command {
    let fill = |a: &str| a.replace("{input}", &input.to_string_lossy()).replace("{output}", &output.to_string_lossy());
    let program = &tool.run[0];
    let local = plugin.dir.join(program);
    let mut c = std::process::Command::new(if (program.contains('/') || program.contains('\\')) && local.is_file() { local.into_os_string() } else { program.into() });
    c.args(tool.run[1..].iter().map(|a| fill(a))).current_dir(&plugin.dir).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());
    c
}

/// Runs `tool` on the PDF `bytes` in its own process and returns the PDF it wrote. Refused when the
/// version was not allowed; stopped after [`TIMEOUT`] (or when `keep_going` says so).
pub fn run(plugin: &Plugin, tool: &PluginTool, bytes: &[u8], keep_going: &dyn Fn() -> bool) -> Result<Vec<u8>, String> {
    if !plugin.allowed {
        return Err(format!("{} has not been allowed to run yet: run one of its tools from the Tools page first", plugin.manifest.name));
    }
    let work = tempfile::tempdir().map_err(|e| e.to_string())?;
    let (input, output) = (work.path().join("input.pdf"), work.path().join("output.pdf"));
    std::fs::write(&input, bytes).map_err(|e| e.to_string())?;
    let mut child = command(plugin, tool, &input, &output).spawn().map_err(|e| format!("{} could not start {:?}: {e}", plugin.manifest.name, tool.run[0]))?;
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if start.elapsed() > TIMEOUT || !keep_going() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if start.elapsed() > TIMEOUT { format!("{} took longer than {} minutes and was stopped", tool.name, TIMEOUT.as_secs() / 60) } else { "Cancelled".into() });
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.by_ref().take(64 * 1024).read_to_string(&mut err);
    }
    let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    if !status.success() {
        return Err(format!("{} failed ({status}){}", tool.name, if last.is_empty() { String::new() } else { format!(": {last}") }));
    }
    let size = std::fs::metadata(&output).map(|m| m.len()).map_err(|_| format!("{} wrote no PDF", tool.name))?;
    if size > MAX_OUTPUT {
        return Err(format!("{} wrote a file over 2 GB", tool.name));
    }
    let out = std::fs::read(&output).map_err(|e| e.to_string())?;
    if !out.starts_with(b"%PDF-") || lopdf::Document::load_mem(&out).is_err() {
        return Err(format!("{} wrote a file that is not a readable PDF", tool.name));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(run: &[&str]) -> String {
        serde_json::json!({ "id": "copy", "name": "Copier", "version": "1.0", "tools": [{ "id": "same", "name": "Same PDF", "run": run }] }).to_string()
    }

    #[test]
    fn manifests_are_checked_before_anything_runs() {
        assert!(Manifest::parse(&manifest(&["cp", "{input}", "{output}"])).is_ok());
        for (bad, why) in [
            (manifest(&["cp", "{input}"]), "{output}"),
            (manifest(&[]), "runs nothing"),
            (manifest(&["/bin/cp", "{input}", "{output}"]), "program"),
            (manifest(&["../evil.sh", "{input}", "{output}"]), "program"),
            (manifest(&["{input}", "{output}"]), "program"),
            (r#"{"id":"Bad Id","name":"x","version":"1","tools":[]}"#.to_string(), "plugin id"),
            (r#"{"id":"x","name":"x","version":"1","tools":[]}"#.to_string(), "1–20 tools"),
            (r#"{"id":"x","name":"x","version":"1","tools":[{"id":"a","name":"A","run":["a","{input}","{output}"]},{"id":"a","name":"B","run":["a","{input}","{output}"]}]}"#.to_string(), "twice"),
            ("{".to_string(), "plugin.json"),
        ] {
            let e = Manifest::parse(&bad).unwrap_err();
            assert!(e.contains(why), "{bad}: {e}");
        }
    }

    #[test]
    fn plugins_are_installed_listed_turned_off_allowed_and_removed() {
        let root = tempfile::tempdir().unwrap();
        let (src, dir) = (root.path().join("src"), root.path().join("plugins"));
        std::fs::create_dir_all(src.join("lib")).unwrap();
        std::fs::write(src.join(MANIFEST), manifest(&["cp", "{input}", "{output}"])).unwrap();
        std::fs::write(src.join("lib/readme.txt"), "x").unwrap();
        std::fs::create_dir_all(dir.join("broken")).unwrap();
        std::fs::write(dir.join("broken").join(MANIFEST), "{}").unwrap();
        let m = install(&dir, &src).unwrap();
        assert_eq!(m.id, "copy");
        assert!(dir.join("copy/lib/readme.txt").is_file(), "the whole folder is copied");
        let (ok, bad) = list(&dir);
        assert_eq!(ok.len(), 1);
        assert_eq!(bad.len(), 1, "a wrong manifest is listed apart");
        let p = &ok[0];
        assert!(p.enabled && !p.allowed, "added on, not yet allowed to run");
        assert!(find(&dir, "copy/same").is_some());
        set_enabled(&dir, "copy", false).unwrap();
        assert!(!list(&dir).0[0].enabled);
        assert!(find(&dir, "copy/same").is_none(), "a turned-off plugin's tools are gone");
        set_enabled(&dir, "copy", true).unwrap();
        allow(&dir, &p.key()).unwrap();
        assert!(list(&dir).0[0].allowed);
        // A new version must be allowed again.
        std::fs::write(src.join(MANIFEST), manifest(&["cp", "{input}", "{output}"]).replace("1.0", "2.0")).unwrap();
        install(&dir, &src).unwrap();
        assert!(!list(&dir).0[0].allowed);
        let p = list(&dir).0.remove(0);
        remove(&dir, &p).unwrap();
        assert!(list(&dir).0.is_empty() && !dir.join("copy").exists());
        assert!(load_state(&dir).allowed.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_tool_runs_as_its_own_process_and_its_output_is_checked() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("p");
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../sheaf-core/tests/fixtures/invoice.pdf")).unwrap();
        let plugin = |run: &[&str], allowed: bool| Plugin { dir: dir.clone(), manifest: Manifest::parse(&manifest(run)).unwrap(), enabled: true, allowed };
        let p = plugin(&["cp", "{input}", "{output}"], true);
        assert_eq!(run(&p, &p.manifest.tools[0], &pdf, &|| true).unwrap(), pdf);
        // Not allowed yet: refused before starting anything.
        let p2 = plugin(&["cp", "{input}", "{output}"], false);
        assert!(run(&p2, &p2.manifest.tools[0], &pdf, &|| true).unwrap_err().contains("allowed"));
        // A script of the plugin's own folder; its error line is reported; a non-PDF is refused.
        std::fs::write(dir.join("fail.sh"), "echo 'no fonts found' >&2; exit 3\n").unwrap();
        let p3 = plugin(&["sh", "./fail.sh", "{input}", "{output}"], true);
        let e = run(&p3, &p3.manifest.tools[0], &pdf, &|| true).unwrap_err();
        assert!(e.contains("no fonts found"), "{e}");
        std::fs::write(dir.join("junk.sh"), "echo hello > \"$2\"\n").unwrap();
        let p4 = plugin(&["sh", "./junk.sh", "{input}", "{output}"], true);
        assert!(run(&p4, &p4.manifest.tools[0], &pdf, &|| true).unwrap_err().contains("not a readable PDF"));
        let p5 = plugin(&["true", "{input}", "{output}"], true);
        assert!(run(&p5, &p5.manifest.tools[0], &pdf, &|| true).unwrap_err().contains("wrote no PDF"));
        // Cancelled: the process is stopped.
        std::fs::write(dir.join("slow.sh"), "sleep 30\n").unwrap();
        let p6 = plugin(&["sh", "./slow.sh", "{input}", "{output}"], true);
        let t = Instant::now();
        assert_eq!(run(&p6, &p6.manifest.tools[0], &pdf, &|| t.elapsed() < Duration::from_millis(300)).unwrap_err(), "Cancelled");
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn the_sample_plugin_is_valid_and_marks_a_pdf_reviewed() {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dev/plugins/reviewed-stamp"));
        let manifest = Manifest::parse(&std::fs::read_to_string(dir.join(MANIFEST)).unwrap()).unwrap();
        // Run for real where Python is installed (the sample's language).
        if std::process::Command::new("python3").arg("--version").output().is_err() {
            return;
        }
        let plugin = Plugin { dir: dir.to_path_buf(), manifest, enabled: true, allowed: true };
        let pdf = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../sheaf-core/tests/fixtures/invoice.pdf")).unwrap();
        let out = run(&plugin, &plugin.manifest.tools[0], &pdf, &|| true).unwrap();
        assert!(out.starts_with(&pdf), "an incremental update keeps the original bytes");
        let doc = lopdf::Document::load_mem(&out).unwrap();
        let info = doc.trailer.get(b"Info").and_then(|i| i.as_reference()).and_then(|r| doc.get_dictionary(r)).unwrap();
        assert_eq!(info.get(b"Title").unwrap().as_str().unwrap(), b"Reviewed");
    }
}
