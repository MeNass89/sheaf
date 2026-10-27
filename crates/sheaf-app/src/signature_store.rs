//! Reliable local signature storage (PDF Expert's saved signatures).
//! New payloads use authenticated encryption and a separate per-profile key. On Unix both
//! files are owner-only. This protects other local accounts, not a compromised user account;
//! unlike Keychain it does not require an unlock prompt or an external program to save.
//! Earlier credential-store profiles are read once and migrated; a locked legacy store is
//! never replaced with an empty library.

use crate::prefs::{KeptSignature, Prefs};
use chacha20poly1305::{aead::{Aead, Payload}, ChaCha20Poly1305, KeyInit};
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

const HEADER: &[u8] = b"SheafMarks\x02";

fn private_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| io::Error::other("Signature store has no directory"))?;
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}

fn read_key(dir: &Path) -> io::Result<[u8; 32]> {
    let path = dir.join("signatures.key");
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_file() { return Err(io::Error::other("Invalid signature key file")); }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::other("Signature key must be readable only by its owner (chmod 600 signatures.key)"));
        }
    }
    std::fs::read(path)?.try_into().map_err(|_| io::Error::other("Invalid signature key length"))
}

fn create_key(dir: &Path) -> io::Result<[u8; 32]> {
    if dir.join("signatures.key").exists() { return read_key(dir); }
    std::fs::create_dir_all(dir)?;
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(io::Error::other)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(&key)?;
    file.as_file().sync_all()?;
    match file.persist_noclobber(dir.join("signatures.key")) {
        Ok(_) => Ok(key),
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => read_key(dir),
        Err(e) => Err(e.error),
    }
}

fn write_local(dir: &Path, marks: &Marks) -> io::Result<()> {
    let key = create_key(dir)?;
    let cipher = ChaCha20Poly1305::new((&key).into());
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(io::Error::other)?;
    let data = serde_json::to_vec(marks).map_err(io::Error::other)?;
    let encrypted = cipher.encrypt((&nonce).into(), Payload { msg: &data, aad: HEADER })
        .map_err(|_| io::Error::other("Cannot encrypt saved signatures"))?;
    let mut output = HEADER.to_vec();
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&encrypted);
    private_write(&dir.join("signatures.secure"), &output)
}

fn read_local(dir: &Path, bytes: &[u8]) -> io::Result<Marks> {
    let payload = bytes.strip_prefix(HEADER).filter(|p| p.len() >= 28)
        .ok_or_else(|| io::Error::other("Invalid encrypted signature file"))?;
    let key = read_key(dir)?;
    let cipher = ChaCha20Poly1305::new((&key).into());
    let data = cipher.decrypt((&payload[..12]).into(), Payload { msg: &payload[12..], aad: HEADER })
        .map_err(|_| io::Error::other("Saved signatures cannot be decrypted; keep signatures.secure and signatures.key together"))?;
    serde_json::from_slice(&data).map_err(io::Error::other)
}

// Remember successful loads so a startup error cannot later erase an unread library.
static LOADED: Mutex<BTreeMap<PathBuf, Marks>> = Mutex::new(BTreeMap::new());

pub(crate) fn load(dir: &Path) -> io::Result<Option<Marks>> {
    let path = dir.join("signatures.secure");
    if !path.exists() { return Ok(None); }
    let bytes = std::fs::read(&path)?;
    let marks = if bytes.starts_with(HEADER) {
        read_local(dir, &bytes)?
    } else {
        let marks: Marks = serde_json::from_slice(&credential(dir, None)?).map_err(io::Error::other)?;
        // Complete the local write before changing the legacy marker. Failure leaves it intact.
        write_local(dir, &marks)?;
        marks
    };
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
    write_local(dir, marks)?;
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


    #[test]
    fn failed_migration_preserves_an_existing_legacy_signature_without_writing_new_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let original = br#"{"signature_name":"Legacy signature"}"#;
        std::fs::write(&path, original).unwrap();
        std::fs::create_dir(dir.path().join("signatures.secure")).unwrap();
        let changed = Prefs { signature_name: Some("New signature".into()), ..Default::default() };
        assert!(changed.save(dir.path()).is_err());
        assert_eq!(std::fs::read(path).unwrap(), original);
    }

    #[test]
    fn local_signatures_survive_restart_without_external_tools_or_size_caps() {
        let dir = tempfile::tempdir().unwrap();
        let p = Prefs { signature_name: Some("Private Name".into()), signature_strokes: vec![vec![[0.2, 0.3]; 20_000]], ..Default::default() };
        let marks = Marks::from_prefs(&p);
        save(dir.path(), &marks).unwrap();
        LOADED.lock().unwrap().remove(dir.path());
        assert_eq!(load(dir.path()).unwrap(), Some(marks));
        let bytes = std::fs::read(dir.path().join("signatures.secure")).unwrap();
        assert!(bytes.starts_with(HEADER));
        assert!(!bytes.windows(12).any(|s| s == b"Private Name"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for name in ["signatures.secure", "signatures.key"] {
                assert_eq!(std::fs::metadata(dir.path().join(name)).unwrap().permissions().mode() & 0o077, 0);
            }
        }
    }

    #[test]
    fn damaged_or_missing_signature_keys_never_clear_a_saved_library() {
        let dir = tempfile::tempdir().unwrap();
        let marks = Marks::from_prefs(&Prefs { signature_name: Some("Kept".into()), ..Default::default() });
        save(dir.path(), &marks).unwrap();
        let original = std::fs::read(dir.path().join("signatures.secure")).unwrap();
        LOADED.lock().unwrap().remove(dir.path());
        std::fs::remove_file(dir.path().join("signatures.key")).unwrap();
        assert!(load(dir.path()).is_err());
        assert!(save(dir.path(), &Marks::default()).is_err());
        assert_eq!(std::fs::read(dir.path().join("signatures.secure")).unwrap(), original);
        std::fs::write(dir.path().join("signatures.key"), [0u8; 32]).unwrap();
        assert!(load(dir.path()).is_err());
    }

}
