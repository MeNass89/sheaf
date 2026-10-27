//! Kept signatures in the system credential store (Preview's Keychain), never in preferences.
//! Linux uses Secret Service's `secret-tool`; Windows uses the current user's DPAPI. A locked
//! or missing store is an error: it must never cause a plaintext fallback or erase a saved mark.

use crate::prefs::{KeptSignature, Prefs};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::process::{Command, Stdio};

#[derive(Default, Clone, Serialize, Deserialize, PartialEq, Debug)]
pub(crate) struct Marks {
    current: KeptSignature,
    signatures: Vec<KeptSignature>,
    initials: Option<KeptSignature>,
}

impl Marks {
    pub(crate) fn from_prefs(p: &Prefs) -> Self {
        Self { current: (p.signature_strokes.clone(), p.signature_name.clone(), p.signature_pen, p.signature_font), signatures: p.signatures.clone(), initials: p.initials.clone() }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.current.0.is_empty() && self.current.1.is_none() && self.signatures.is_empty() && self.initials.is_none()
    }

    pub(crate) fn apply(self, p: &mut Prefs) {
        (p.signature_strokes, p.signature_name, p.signature_pen, p.signature_font) = self.current;
        p.signatures = self.signatures;
        p.initials = self.initials;
    }
}

fn run(command: &mut Command, input: Option<&[u8]>) -> io::Result<Vec<u8>> {
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let written = match input {
        Some(bytes) => child.stdin.take().ok_or_else(|| io::Error::other("signature store has no input"))?.write_all(bytes),
        None => { drop(child.stdin.take()); Ok(()) }
    };
    let result = child.wait_with_output()?;
    written?;
    if !result.status.success() {
        return Err(io::Error::other("Cannot open the secure signature store. Unlock your system keyring; the signature remains available for this session."));
    }
    Ok(result.stdout)
}

#[cfg(target_os = "linux")]
fn credential(dir: &Path, data: Option<&[u8]>) -> io::Result<Vec<u8>> {
    let mut c = Command::new("secret-tool");
    if data.is_some() { c.args(["store", "--label=Sheaf signatures"]); } else { c.arg("lookup"); }
    c.args(["application", "Sheaf", "profile"]).arg(dir);
    run(&mut c, data)
}

#[cfg(target_os = "macos")]
fn credential(dir: &Path, data: Option<&[u8]>) -> io::Result<Vec<u8>> {
    // Interactive input keeps the secret out of process arguments. JSON is ASCII-escaped by
    // hex encoding, so neither a name nor a path can introduce a security(1) command.
    let account: String = dir.as_os_str().to_string_lossy().as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    let mut c = Command::new("/usr/bin/security");
    if let Some(data) = data {
        let secret: String = data.iter().map(|b| format!("{b:02x}")).collect();
        let line = format!("add-generic-password -U -s Sheaf.signatures -a {account} -w {secret}\n");
        run(c.arg("-i"), Some(line.as_bytes()))
    } else {
        let raw = run(c.args(["find-generic-password", "-s", "Sheaf.signatures", "-a", &account, "-w"]), None)?;
        let hex = std::str::from_utf8(&raw).map_err(io::Error::other)?.trim();
        if !hex.len().is_multiple_of(2) { return Err(io::Error::other("Invalid signature store")); }
        hex.as_bytes().as_chunks::<2>().0.iter().map(|p| {
            let s = std::str::from_utf8(p).map_err(io::Error::other)?;
            u8::from_str_radix(s, 16).map_err(io::Error::other)
        }).collect()
    }
}

#[cfg(target_os = "windows")]
fn credential(dir: &Path, data: Option<&[u8]>) -> io::Result<Vec<u8>> {
    // ConvertFrom-SecureString's default is DPAPI CurrentUser; the resulting file contains
    // ciphertext only and cannot be opened by another Windows account.
    let (script, input) = match data {
        Some(bytes) => ("$ErrorActionPreference='Stop'; $s=[Console]::In.ReadToEnd(); $v=ConvertTo-SecureString $s -AsPlainText -Force; [Console]::Out.Write((ConvertFrom-SecureString $v))", bytes.to_vec()),
        None => ("$ErrorActionPreference='Stop'; $s=[Console]::In.ReadToEnd(); $v=ConvertTo-SecureString $s; $p=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($v); try {[Console]::Out.Write([Runtime.InteropServices.Marshal]::PtrToStringBSTR($p))} finally {[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($p)}", std::fs::read(dir.join("signatures.secure"))?),
    };
    let script = format!("[Console]::InputEncoding=[Text.UTF8Encoding]::new($false); [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); {script}");
    let result = run(Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", &script]), Some(&input))?;
    if data.is_some() { crate::store_file::replace(&dir.join("signatures.secure"), &result)?; }
    Ok(result)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn credential(_dir: &Path, _data: Option<&[u8]>) -> io::Result<Vec<u8>> {
    Err(io::Error::other("No secure signature store on this platform"))
}

// Besides avoiding credential-store calls on every ordinary preference save, this records
// that the profile actually loaded its marks. A failed startup read must not turn into a
// deletion when the keyring becomes available later in the session.
static LOADED: Mutex<BTreeMap<PathBuf, Marks>> = Mutex::new(BTreeMap::new());

pub(crate) fn load(dir: &Path) -> io::Result<Option<Marks>> {
    if !dir.join("signatures.secure").exists() { return Ok(None); }
    if !dir.join("signatures.secure").is_file() { return Err(io::Error::other("Invalid signature store marker")); }
    let data = credential(dir, None)?;
    let marks: Marks = serde_json::from_slice(&data).map_err(io::Error::other)?;
    LOADED.lock().unwrap_or_else(|e| e.into_inner()).insert(dir.to_path_buf(), marks.clone());
    Ok(Some(marks))
}

pub(crate) fn save(dir: &Path, marks: &Marks) -> io::Result<()> {
    let old = LOADED.lock().unwrap_or_else(|e| e.into_inner()).get(dir).cloned();
    if old.as_ref() == Some(marks) { return Ok(()); }
    if old.is_none() {
        if dir.join("signatures.secure").exists() {
            return Err(io::Error::other("Reload Sheaf after unlocking the signature store; the saved marks have not been changed"));
        }
        if marks.is_empty() { return Ok(()); }
    }
    let data = serde_json::to_vec(marks).map_err(io::Error::other)?;
    credential(dir, Some(&data))?;
    let saved: Marks = serde_json::from_slice(&credential(dir, None)?).map_err(io::Error::other)?;
    if &saved != marks { return Err(io::Error::other("The secure signature store did not keep the signature")); }
    #[cfg(not(target_os = "windows"))]
    crate::store_file::replace(&dir.join("signatures.secure"), b"system-credential-store-v1")?;
    LOADED.lock().unwrap_or_else(|e| e.into_inner()).insert(dir.to_path_buf(), marks.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_and_initials_leave_preferences_and_round_trip_as_a_secret() {
        let p = Prefs { signature_name: Some("Private Name".into()), signature_strokes: vec![vec![[0.2, 0.3]]], signatures: vec![(vec![], Some("Other Name".into()), (1, 2), 3)], initials: Some((vec![], Some("PN".into()), (0, 1), 2)), ..Default::default() };
        let json = serde_json::to_string(&p).unwrap();
        for key in ["signature_name", "signature_strokes", "signatures", "initials", "Private Name", "Other Name"] { assert!(!json.contains(key), "{key} leaked to preferences"); }
        let secret = serde_json::to_vec(&Marks::from_prefs(&p)).unwrap();
        let decoded: Marks = serde_json::from_slice(&secret).unwrap();
        let mut restored = Prefs::default();
        decoded.apply(&mut restored);
        assert_eq!(Marks::from_prefs(&restored), Marks::from_prefs(&p));
    }

    #[test]
    fn an_unreadable_store_is_not_erased_when_preferences_are_saved() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("signatures.secure")).unwrap();
        assert!(save(dir.path(), &Marks::default()).is_err());
        assert!(dir.path().join("signatures.secure").is_dir());
    }

    #[test]
    fn an_unloaded_profile_cannot_clear_a_store_that_later_becomes_available() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("signatures.secure");
        std::fs::write(&marker, b"system-credential-store-v1").unwrap();
        assert!(save(dir.path(), &Marks::default()).unwrap_err().to_string().contains("Reload"));
        assert_eq!(std::fs::read(marker).unwrap(), b"system-credential-store-v1");
    }

}
