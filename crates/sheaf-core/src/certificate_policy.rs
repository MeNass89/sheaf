//! Certificate security policies (Acrobat's Security Policies ▸ New ▸ Use public key
//! certificates): a named list of recipients with their permissions and what is encrypted,
//! saved, exchanged and applied again. Only public certificates are stored, never a private key.

use crate::certificate_security::{encrypt_for, Permissions, Recipient, Scope};
use crate::pdfcraft::engine::sign::Certificate;
use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    certificate_policy: u32,
    name: String,
    /// "all", "except-metadata" or "attachments".
    encrypt: String,
    recipients: Vec<StoredRecipient>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRecipient {
    /// The X.509 certificate, DER in hexadecimal.
    certificate: String,
    permissions: u32,
}

#[derive(Debug, Clone)]
pub struct CertificatePolicy {
    pub name: String,
    pub scope: Scope,
    pub recipients: Vec<Recipient>,
}

fn invalid(message: impl Into<String>) -> OpdfError { OpdfError::Unsupported(message.into()) }

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) { return None; }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

impl CertificatePolicy {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.name.chars().count() > 200 || self.name.chars().any(char::is_control) {
            return Err(invalid("A security policy needs a name of 1 to 200 characters"));
        }
        if self.recipients.is_empty() || self.recipients.len() > 128 { return Err(invalid("A certificate policy needs 1 to 128 recipients")); }
        Ok(())
    }

    /// Save or export, never replacing an existing file.
    pub fn save(&self, out: &Path) -> Result<PathBuf> {
        self.validate()?;
        let stored = Stored {
            certificate_policy: 1,
            name: self.name.trim().into(),
            encrypt: match self.scope { Scope::All => "all", Scope::ExceptMetadata => "except-metadata", Scope::Attachments => "attachments" }.into(),
            recipients: self.recipients.iter().map(|r| StoredRecipient { certificate: hex(&r.certificate.raw), permissions: r.permissions.0 }).collect(),
        };
        let bytes = serde_json::to_vec_pretty(&stored).map_err(|e| OpdfError::Engine(e.to_string()))?;
        atomic_write(out, |part| { std::fs::write(part, &bytes)?; Ok(()) })
    }

    /// Import: every certificate is parsed again, so a damaged or edited file is refused whole.
    pub fn load(path: &Path) -> Result<Self> {
        let stored: Stored = serde_json::from_slice(&std::fs::read(path)?).map_err(|e| invalid(format!("Invalid certificate policy: {e}")))?;
        if stored.certificate_policy != 1 { return Err(invalid("Unsupported certificate policy version")); }
        let scope = match stored.encrypt.as_str() { "all" => Scope::All, "except-metadata" => Scope::ExceptMetadata, "attachments" => Scope::Attachments, _ => return Err(invalid("Unknown encryption scope in the policy")) };
        let recipients = stored.recipients.iter().map(|r| {
            let der = unhex(&r.certificate).ok_or_else(|| invalid("A policy certificate is not hexadecimal"))?;
            let certificate = Certificate::parse(&der).map_err(|e| invalid(format!("A policy certificate cannot be read: {e}")))?;
            Ok(Recipient { certificate, permissions: Permissions(r.permissions) })
        }).collect::<Result<Vec<_>>>()?;
        let policy = Self { name: stored.name, scope, recipients };
        policy.validate()?;
        Ok(policy)
    }

    pub fn apply(&self, input: &Input, out: &Path) -> Result<PathBuf> {
        self.validate()?;
        encrypt_for(input, &self.recipients, self.scope, out)
    }
}

/// The certificate policies saved in `dir`, by name.
pub fn list(dir: &Path) -> Vec<(PathBuf, CertificatePolicy)> {
    let mut found: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten()
        .map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| CertificatePolicy::load(&p).ok().map(|c| (p, c))).collect();
    found.sort_by_key(|a| a.1.name.to_lowercase());
    found
}
