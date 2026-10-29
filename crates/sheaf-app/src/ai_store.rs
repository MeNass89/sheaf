//! Preferences ▸ AI: which provider answers (Acrobat's and PDF Expert's AI settings, opened to
//! every major provider and to local models), each provider's model and address, and its API key.
//! Keys are never in `prefs.json`: they are kept in `ai-keys.secure`, encrypted with the profile's
//! key as saved signatures are (`signature_store.rs`), owner-only on Unix.

use chacha20poly1305::{aead::{Aead, Payload}, ChaCha20Poly1305, KeyInit};
use serde::{Deserialize, Serialize};
use sheaf_core::ai::{AiError, Config, Provider};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct AiPrefs {
    /// The provider chosen (`Provider::id`); None: a local server when one answers.
    pub provider: Option<String>,
    /// Each provider's model, by id ("" or missing: its usual one).
    pub models: BTreeMap<String, String>,
    /// Each provider's address, by id (missing: its usual one).
    pub base_urls: BTreeMap<String, String>,
    /// Sending text to AI turned off (the panel's note ▸ Turn off).
    pub off: bool,
    /// The providers whose "only the text is sent" note was read.
    pub notice_seen: Vec<String>,
    /// The AI panel opens with every document (PDF Expert's Copilot auto-open).
    pub auto_open: bool,
    /// The language last translated into.
    pub language: Option<String>,
    /// The voices last chosen for the podcast's two hosts (`Voice::id`), kept while they exist.
    pub podcast_voices: Vec<String>,
}

impl AiPrefs {
    /// The setup to use: the chosen provider with its model, address and key; else a local server
    /// found by `detect`; else [`AiError::NotConfigured`].
    pub fn config(&self, keys: &BTreeMap<String, String>, detect: impl FnOnce() -> Option<Config>) -> Result<Config, AiError> {
        if self.off {
            return Err(AiError::Off);
        }
        let Some(p) = self.provider.as_deref().and_then(Provider::from_id) else {
            return detect().ok_or(AiError::NotConfigured);
        };
        let mut c = Config::new(p);
        c.model = self.models.get(p.id()).cloned().unwrap_or_default();
        c.base_url = self.base_urls.get(p.id()).cloned().unwrap_or_default();
        c.api_key = keys.get(p.id()).cloned().filter(|k| !k.trim().is_empty());
        Ok(c)
    }
}

const HEADER: &[u8] = b"SheafAiKeys\x01";
const FILE: &str = "ai-keys.secure";

/// The API keys saved in `dir`, by provider id (none when there is no file).
pub fn load_keys(dir: &Path) -> io::Result<BTreeMap<String, String>> {
    let path = dir.join(FILE);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let bytes = std::fs::read(&path)?;
    let payload = bytes.strip_prefix(HEADER).filter(|p| p.len() >= 28).ok_or_else(|| io::Error::other("Invalid AI key file"))?;
    let key = crate::signature_store::read_key(dir)?;
    let data = ChaCha20Poly1305::new((&key).into())
        .decrypt((&payload[..12]).into(), Payload { msg: &payload[12..], aad: HEADER })
        .map_err(|_| io::Error::other("Saved AI keys cannot be decrypted; keep ai-keys.secure and signatures.key together"))?;
    serde_json::from_slice(&data).map_err(io::Error::other)
}

/// Saves `keys` in `dir`, encrypted (an empty key removes the provider's).
pub fn save_keys(dir: &Path, keys: &BTreeMap<String, String>) -> io::Result<()> {
    let keys: BTreeMap<&String, &String> = keys.iter().filter(|(_, v)| !v.trim().is_empty()).collect();
    let key = crate::signature_store::create_key(dir)?;
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(io::Error::other)?;
    let data = serde_json::to_vec(&keys).map_err(io::Error::other)?;
    let sealed = ChaCha20Poly1305::new((&key).into())
        .encrypt((&nonce).into(), Payload { msg: &data, aad: HEADER })
        .map_err(|_| io::Error::other("Cannot encrypt the AI keys"))?;
    let mut out = HEADER.to_vec();
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    crate::signature_store::private_write(&dir.join(FILE), &out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_encrypted_on_disk_and_read_back() {
        let d = tempfile::tempdir().unwrap();
        assert!(load_keys(d.path()).unwrap().is_empty());
        let keys = BTreeMap::from([("openai".to_string(), "sk-secret-123".to_string()), ("mistral".to_string(), " ".to_string())]);
        save_keys(d.path(), &keys).unwrap();
        let raw = std::fs::read(d.path().join(FILE)).unwrap();
        assert!(!raw.windows(13).any(|w| w == b"sk-secret-123"));
        assert_eq!(load_keys(d.path()).unwrap(), BTreeMap::from([("openai".to_string(), "sk-secret-123".to_string())]));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(d.path().join(FILE)).unwrap().permissions().mode() & 0o077, 0);
        }
        // Another profile's key cannot open it.
        std::fs::remove_file(d.path().join("signatures.key")).unwrap();
        crate::signature_store::create_key(d.path()).unwrap();
        assert!(load_keys(d.path()).is_err());
    }

    #[test]
    fn the_chosen_provider_wins_then_a_local_server() {
        let keys = BTreeMap::from([("anthropic".to_string(), "sk-ant".to_string())]);
        let mut p = AiPrefs { provider: Some("anthropic".into()), ..Default::default() };
        p.models.insert("anthropic".into(), "claude-x".into());
        let c = p.config(&keys, || panic!("not asked")).unwrap();
        assert_eq!((c.provider, c.model.as_str(), c.api_key.as_deref()), (Provider::Anthropic, "claude-x", Some("sk-ant")));
        p.provider = None;
        assert_eq!(p.config(&keys, || Some(Config::new(Provider::Ollama))).unwrap().provider, Provider::Ollama);
        assert_eq!(p.config(&keys, || None), Err(AiError::NotConfigured));
        p.off = true;
        assert_eq!(p.config(&keys, || None), Err(AiError::Off));
    }
}
