//! Reusable security settings without credentials (Acrobat's Security Policies).
//! Save/export and load/import use the same versioned JSON format. Passwords are supplied
//! only when applying a policy, so exchanging a policy cannot expose a document password.

use crate::security::{protect_with_settings, ProtectPermissions};
use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityPolicy {
    version: u32,
    pub name: String,
    pub permissions: ProtectPermissions,
    pub encrypt_metadata: bool,
}

impl SecurityPolicy {
    pub fn new(name: String, permissions: ProtectPermissions, encrypt_metadata: bool) -> Result<Self> {
        let policy = Self { version: 1, name: name.trim().into(), permissions, encrypt_metadata };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 { return Err(OpdfError::Unsupported("Unsupported security policy version".into())); }
        if self.name.trim().is_empty() || self.name.chars().count() > 200 || self.name.chars().any(char::is_control) {
            return Err(OpdfError::Unsupported("A security policy needs a name of 1 to 200 characters".into()));
        }
        Ok(())
    }

    /// Save or export, never replacing an existing file. Import by opening this file on any
    /// supported platform; a renamed policy can be saved as another file.
    pub fn save(&self, out: &Path) -> Result<PathBuf> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| OpdfError::Engine(e.to_string()))?;
        atomic_write(out, |part| { std::fs::write(part, &bytes)?; Ok(()) })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        let policy: Self = serde_json::from_slice(&bytes).map_err(|e| OpdfError::Unsupported(format!("Invalid security policy: {e}")))?;
        policy.validate()?;
        Ok(policy)
    }

    /// AES-256 encryption with this policy's permissions and metadata choice. A policy never
    /// weakens its settings implicitly when a required password is missing.
    pub fn apply(&self, input: &Input, user_password: &str, owner_password: Option<&str>, out: &Path) -> Result<PathBuf> {
        self.validate()?;
        protect_with_settings(input, user_password, owner_password, self.permissions, self.encrypt_metadata, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_exchange_permissions_and_metadata_but_never_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let permissions = ProtectPermissions { printing: false, copying: false, editing: false, commenting: true, filling_forms: true };
        let policy = SecurityPolicy::new("External review".into(), permissions, false).unwrap();
        let out = policy.save(&dir.path().join("policy.json")).unwrap();
        let imported = SecurityPolicy::load(&out).unwrap();
        assert_eq!(imported, policy);
        assert!(!std::fs::read_to_string(out).unwrap().contains("password"));
    }

    #[test]
    fn a_policy_with_unknown_security_fields_or_versions_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let policy = SecurityPolicy::new("Review".into(), ProtectPermissions::default(), true).unwrap();
        let mut value = serde_json::to_value(&policy).unwrap();
        value["user_password"] = "secret".into();
        let path = dir.path().join("policy.json");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(SecurityPolicy::load(&path).is_err());
        value.as_object_mut().unwrap().remove("user_password");
        value["version"] = 2.into();
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(SecurityPolicy::load(&path).is_err());
        assert!(SecurityPolicy::new(" ".into(), ProtectPermissions::default(), true).is_err());
    }
}
