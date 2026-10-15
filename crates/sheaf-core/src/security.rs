use crate::{pages::Input, paths::atomic_write, Result};
use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use lopdf::{EncryptionState, EncryptionVersion, Permissions};
use std::{collections::BTreeMap, path::{Path, PathBuf}, sync::Arc};

pub fn protect(input: &Input, user_pw: &str, owner_pw: Option<&str>, out: &Path) -> Result<PathBuf> {
    if user_pw.is_empty() { return Err(crate::OpdfError::Unsupported("password must not be empty".into())); }
    let mut doc = input.load_lopdf()?;
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|e| crate::OpdfError::Conversion(format!("rng: {e}")))?;
    let cf: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
    let version = EncryptionVersion::V5 {
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), cf)]),
        file_encryption_key: &key,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: owner_pw.unwrap_or(user_pw),
        user_password: user_pw,
        permissions: Permissions::all(),
    };
    let state = EncryptionState::try_from(version)?;
    doc.encrypt(&state)?;
    set_aes256_key_length(&mut doc)?;
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}

pub fn unlock(input: &Input, out: &Path) -> Result<PathBuf> {
    let mut doc = input.load_lopdf()?; // already decrypted in memory when a password is given
    doc.trailer.remove(b"Encrypt");
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}

/// lopdf 0.45 omits `/Length` from a V5 (AES-256) Encrypt dictionary; qpdf then warns on every
/// check. The spec gives it as the key length in bits, 256 for AES-256.
fn set_aes256_key_length(doc: &mut lopdf::Document) -> Result<()> {
    let enc = doc.trailer.get(b"Encrypt")?.clone();
    let dict = match enc {
        lopdf::Object::Reference(id) => doc.get_dictionary_mut(id)?,
        _ => doc.trailer.get_mut(b"Encrypt")?.as_dict_mut()?,
    };
    dict.set("Length", 256);
    Ok(())
}
