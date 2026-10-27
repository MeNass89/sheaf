//! Acrobat's Save passwords with the policy. Credentials stay local and authenticated;
//! exported settings have no field capable of containing them.
use chacha20poly1305::{aead::{Aead, Payload}, ChaCha20Poly1305, KeyInit};
use serde::{Deserialize, Serialize};
use sheaf_core::security_policy::SecurityPolicy;
use std::collections::BTreeMap;
use std::path::Path;

const HEADER: &[u8] = b"SheafPolicyPasswords\x01";
const FILE: &str = "policy-passwords.secure";
static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Serialize, Deserialize)]
pub struct Passwords {
    pub user: String,
    pub owner: String,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    policy: SecurityPolicy,
    passwords: Passwords,
}

type Secrets = BTreeMap<String, Entry>;

fn read(dir: &Path) -> Result<Secrets, String> {
    let bytes = match std::fs::read(dir.join(FILE)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Secrets::new()),
        Err(e) => return Err(e.to_string()),
    };
    let payload = bytes.strip_prefix(HEADER).filter(|p| p.len() >= 28).ok_or("Invalid saved policy passwords")?;
    let key = crate::signature_store::read_key(dir).map_err(|e| e.to_string())?;
    let cipher = ChaCha20Poly1305::new((&key).into());
    let data = cipher.decrypt((&payload[..12]).into(), Payload { msg: &payload[12..], aad: HEADER })
        .map_err(|_| "Cannot decrypt saved policy passwords; keep signatures.key with this profile".to_string())?;
    serde_json::from_slice(&data).map_err(|e| format!("Invalid saved policy passwords: {e}"))
}

fn write(dir: &Path, entries: &Secrets) -> Result<(), String> {
    let key = crate::signature_store::create_key(dir).map_err(|e| e.to_string())?;
    let cipher = ChaCha20Poly1305::new((&key).into());
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let data = serde_json::to_vec(entries).map_err(|e| e.to_string())?;
    let encrypted = cipher.encrypt((&nonce).into(), Payload { msg: &data, aad: HEADER })
        .map_err(|_| "Cannot encrypt saved policy passwords".to_string())?;
    let mut bytes = HEADER.to_vec();
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&encrypted);
    crate::signature_store::private_write(&dir.join(FILE), &bytes).map_err(|e| e.to_string())
}

pub(super) fn get(dir: &Path, policy: &SecurityPolicy) -> Result<Option<Passwords>, String> {
    Ok(read(dir)?.remove(&policy.name).filter(|entry| entry.policy == *policy).map(|entry| entry.passwords))
}

pub(super) fn save(dir: &Path, policy: &SecurityPolicy, passwords: Passwords) -> Result<(), String> {
    policy.validate().map_err(|e| e.to_string())?;
    if passwords.user.is_empty() && passwords.owner.is_empty() { return Err("Enter at least one password before saving it".into()); }
    // Serialize read-modify-write across policy panels in different document tabs.
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Refuse to overwrite a damaged or unreadable vault, including when its key is missing.
    let mut entries = read(dir)?;
    entries.insert(policy.name.clone(), Entry { policy: policy.clone(), passwords });
    write(dir, &entries)
}

pub(super) fn forget(dir: &Path, name: &str) -> Result<(), String> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut entries = read(dir)?;
    if entries.remove(name).is_none() { return Ok(()); }
    if entries.is_empty() {
        return std::fs::remove_file(dir.join(FILE)).map_err(|e| e.to_string());
    }
    write(dir, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_core::security::ProtectPermissions;

    #[test]
    fn missing_policy_password_key_refuses_new_saves_without_destroying_existing_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let policy = SecurityPolicy::new("Keep".into(), ProtectPermissions::default(), true).unwrap();
        save(dir.path(), &policy, Passwords { user: "open".into(), owner: "owner".into() }).unwrap();
        let original = std::fs::read(dir.path().join(FILE)).unwrap();
        std::fs::remove_file(dir.path().join("signatures.key")).unwrap();
        assert!(get(dir.path(), &policy).is_err());
        assert!(save(dir.path(), &policy, Passwords { user: "new".into(), owner: "new-owner".into() }).is_err());
        assert!(forget(dir.path(), &policy.name).is_err());
        assert_eq!(std::fs::read(dir.path().join(FILE)).unwrap(), original);
    }

    #[test]
    fn remembered_policy_passwords_survive_restart_but_never_export_or_follow_changed_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = SecurityPolicy::new("Review".into(), ProtectPermissions::default(), true).unwrap();
        super::super::save(dir.path(), &policy).unwrap();
        save(dir.path(), &policy, Passwords { user: "private-open".into(), owner: "private-owner".into() }).unwrap();
        let reopened = super::super::load(dir.path()).unwrap().remove(0);
        let stored = get(dir.path(), &reopened).unwrap().unwrap();
        assert_eq!(stored.user, "private-open");
        assert_eq!(stored.owner, "private-owner");
        let export = policy.save(&dir.path().join("export.json")).unwrap();
        assert!(!std::fs::read_to_string(export).unwrap().contains("private-"));
        let bytes = std::fs::read(dir.path().join(FILE)).unwrap();
        assert!(!bytes.windows(12).any(|w| w == b"private-open"));
        policy.permissions.copying = false;
        assert!(get(dir.path(), &policy).unwrap().is_none());
        policy.permissions.copying = true;
        assert!(get(dir.path(), &policy).unwrap().is_some());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.path().join(FILE)).unwrap().permissions().mode() & 0o077, 0);
        }
        super::super::remove(dir.path(), &policy.name).unwrap();
        assert!(get(dir.path(), &policy).unwrap().is_none());
    }

    #[test]
    fn corrupt_policy_passwords_are_never_replaced_and_forgetting_preserves_other_policies() {
        let dir = tempfile::tempdir().unwrap();
        let first = SecurityPolicy::new("First".into(), ProtectPermissions::default(), true).unwrap();
        let second = SecurityPolicy::new("Second".into(), ProtectPermissions::default(), true).unwrap();
        for policy in [&first, &second] {
            save(dir.path(), policy, Passwords { user: "open".into(), owner: "owner".into() }).unwrap();
        }
        forget(dir.path(), "First").unwrap();
        assert!(get(dir.path(), &first).unwrap().is_none());
        assert!(get(dir.path(), &second).unwrap().is_some());
        let file = dir.path().join(FILE);
        let mut corrupt = std::fs::read(&file).unwrap();
        *corrupt.last_mut().unwrap() ^= 1;
        std::fs::write(&file, &corrupt).unwrap();
        assert!(get(dir.path(), &second).is_err());
        assert!(save(dir.path(), &second, Passwords { user: "new".into(), owner: "".into() }).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), corrupt);
    }
}
