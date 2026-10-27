//! Local password-policy library, separate from credentials (Acrobat's Security Policies).
mod passwords;
pub use passwords::Passwords;

use sheaf_core::security_policy::SecurityPolicy;
use std::path::{Path, PathBuf};

type Entry = (PathBuf, SecurityPolicy);

fn entries(dir: &Path) -> Result<Vec<Entry>, String> {
    let folder = dir.join("security-policies");
    let files = match std::fs::read_dir(&folder) {
        Ok(files) => files,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut policies = Vec::new();
    for file in files {
        let path = file.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("json")) { continue; }
        let policy = SecurityPolicy::load(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        policies.push((path, policy));
    }
    policies.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    Ok(policies)
}

pub fn load(dir: &Path) -> Result<Vec<SecurityPolicy>, String> {
    Ok(entries(dir)?.into_iter().map(|(_, policy)| policy).collect())
}

/// A named policy is updated deliberately; credentials cannot be represented in this type.
pub fn save(dir: &Path, policy: &SecurityPolicy) -> Result<(), String> {
    policy.validate().map_err(|e| e.to_string())?;
    let checked = policy.clone();
    if let Some((path, _)) = entries(dir)?.into_iter().find(|(_, p)| p.name == checked.name) {
        let bytes = serde_json::to_vec_pretty(&checked).map_err(|e| e.to_string())?;
        return crate::store_file::replace(&path, &bytes).map_err(|e| e.to_string());
    }
    let folder = dir.join("security-policies");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    checked.save(&folder.join("policy.json")).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
    let (path, _) = entries(dir)?.into_iter().find(|(_, p)| p.name == name).ok_or("That saved policy no longer exists")?;
    passwords::forget(dir, name)?;
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

/// Local credentials bound to the exact saved settings. Policy JSON never contains them.
pub fn saved_passwords(dir: &Path, policy: &SecurityPolicy) -> Result<Option<Passwords>, String> {
    passwords::get(dir, policy)
}

pub fn remember_passwords(dir: &Path, policy: &SecurityPolicy, value: Passwords) -> Result<(), String> {
    if !load(dir)?.iter().any(|saved| saved == policy) { return Err("Save the policy settings before remembering passwords".into()); }
    passwords::save(dir, policy, value)
}

pub fn forget_passwords(dir: &Path, name: &str) -> Result<(), String> { passwords::forget(dir, name) }

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_core::security::ProtectPermissions;
    #[test]
    fn a_policy_library_survives_edits_import_export_and_deletion_without_passwords() {
        let t = tempfile::tempdir().unwrap();
        let mut policy = SecurityPolicy::new("External review".into(), ProtectPermissions::default(), true).unwrap();
        save(t.path(), &policy).unwrap();
        policy.options.print_quality = sheaf_core::security::PrintQuality::Low;
        policy.options.changes = sheaf_core::security::ChangeLevel::FillForms;
        policy.options.algorithm = sheaf_core::security::EncryptionAlgorithm::Aes128;
        policy.permissions.copying = false;
        policy.encrypt_metadata = false;
        save(t.path(), &policy).unwrap();
        assert_eq!(load(t.path()).unwrap(), [policy.clone()]);
        let exported = policy.save(&t.path().join("exchange.json")).unwrap();
        let other = tempfile::tempdir().unwrap();
        save(other.path(), &SecurityPolicy::load(&exported).unwrap()).unwrap();
        assert_eq!(load(other.path()).unwrap(), [policy]);
        for (path, _) in entries(t.path()).unwrap() {
            assert!(!std::fs::read_to_string(path).unwrap().contains("password"));
        }
        remove(t.path(), "External review").unwrap();
        assert!(load(t.path()).unwrap().is_empty());
    }
    #[test]
    fn a_corrupt_policy_is_reported_without_overwriting_the_library() {
        let t = tempfile::tempdir().unwrap();
        let policy = SecurityPolicy::new("Keep".into(), ProtectPermissions::default(), true).unwrap();
        save(t.path(), &policy).unwrap();
        let bad = t.path().join("security-policies/broken.json");
        std::fs::write(&bad, b"broken").unwrap();
        assert!(load(t.path()).is_err());
        assert!(save(t.path(), &policy).is_err());
        assert_eq!(std::fs::read(bad).unwrap(), b"broken");
    }
}
