use crate::{pages::Input, paths::atomic_write, Result};
use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use lopdf::{EncryptionState, EncryptionVersion, Permissions};
use std::{collections::BTreeMap, path::{Path, PathBuf}, sync::Arc};

/// A length-and-character-class hint, not an estimate of cracking time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordStrength { Weak, Fair, Strong }

/// Strong: at least 16 characters with a non-space class, or 12 with three classes. Fair: at least eight
/// with two classes. Classes are lowercase, uppercase, numeric and non-space symbols.
pub fn password_strength(password: &str) -> PasswordStrength {
    let length = password.chars().count();
    let classes = [
        password.chars().any(char::is_lowercase),
        password.chars().any(char::is_uppercase),
        password.chars().any(char::is_numeric),
        password.chars().any(|c| !c.is_alphanumeric() && !c.is_whitespace()),
    ].into_iter().filter(|v| *v).count();
    if (length >= 16 && classes > 0) || (length >= 12 && classes >= 3) {
        PasswordStrength::Strong
    } else if length >= 8 && classes >= 2 {
        PasswordStrength::Fair
    } else {
        PasswordStrength::Weak
    }
}

/// Operations a reader may perform with the open password. All are allowed by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectPermissions {
    pub printing: bool,
    pub copying: bool,
    pub editing: bool,
    pub commenting: bool,
    pub filling_forms: bool,
}

impl Default for ProtectPermissions {
    fn default() -> Self {
        Self { printing: true, copying: true, editing: true, commenting: true, filling_forms: true }
    }
}

impl ProtectPermissions {
    fn flags(self) -> Permissions {
        let mut flags = Permissions::all();
        flags.set(Permissions::PRINTABLE | Permissions::PRINTABLE_IN_HIGH_QUALITY, self.printing);
        flags.set(Permissions::COPYABLE, self.copying);
        // Page assembly is a separate PDF bit, but belongs to the Editing control.
        flags.set(Permissions::MODIFIABLE | Permissions::ASSEMBLABLE, self.editing);
        flags.set(Permissions::ANNOTABLE, self.commenting);
        flags.set(Permissions::FILLABLE, self.filling_forms);
        flags
    }
}

pub fn protect(input: &Input, user_pw: &str, owner_pw: Option<&str>, out: &Path) -> Result<PathBuf> {
    protect_with_permissions(input, user_pw, owner_pw, ProtectPermissions::default(), out)
}

pub fn protect_with_permissions(input: &Input, user_pw: &str, owner_pw: Option<&str>, permissions: ProtectPermissions, out: &Path) -> Result<PathBuf> {
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
        owner_password: owner_pw.filter(|pw| !pw.is_empty()).unwrap_or(user_pw),
        user_password: user_pw,
        permissions: permissions.flags(),
    };
    let state = EncryptionState::try_from(version)?;
    doc.encrypt(&state)?;
    set_aes256_key_length(&mut doc)?;
    for obj in doc.objects.values_mut() { hex_strings(obj); }
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}

pub fn unlock(input: &Input, out: &Path) -> Result<PathBuf> {
    let mut doc = input.load_lopdf()?; // already decrypted in memory when a password is given
    doc.trailer.remove(b"Encrypt");
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}

/// Encryption fills strings with random bytes (the Encrypt dictionary's /O /U /OE /UE /Perms, and
/// every encrypted string), which lopdf writes as literal strings with line feeds unescaped. That
/// is valid (ISO 32000-2 §7.3.4.2: each end-of-line marker reads as one LF), but PdfCraft's reader
/// folds a run of end-of-line bytes into a single LF, so a key string holding two line feeds in a
/// row came back one byte short and the file was refused ("InvalidEncryption"). Hex strings read
/// the same in every reader.
fn hex_strings(obj: &mut lopdf::Object) {
    match obj {
        lopdf::Object::String(_, format) => *format = lopdf::StringFormat::Hexadecimal,
        lopdf::Object::Array(items) => items.iter_mut().for_each(hex_strings),
        lopdf::Object::Dictionary(dict) => dict.iter_mut().for_each(|(_, v)| hex_strings(v)),
        lopdf::Object::Stream(stream) => stream.dict.iter_mut().for_each(|(_, v)| hex_strings(v)),
        _ => {}
    }
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

#[cfg(test)]
mod tests {
    use super::{password_strength, PasswordStrength::*};

    #[test]
    fn strength_uses_length_and_character_classes() {
        for (password, expected) in [
            ("", Weak), ("                ", Weak), ("aB1!", Weak), ("Abcdef1", Weak),
            ("abcdefgh", Weak), ("Abcdefgh", Fair), ("abcdefg!", Fair),
            ("Abcdefghij1", Fair), ("Abcdefghijk1", Strong),
            ("abcdefghijkL", Fair), ("abcdefghijklmno", Weak),
            ("abcdefghijklmnop", Strong), ("abcd efgh", Weak),
            ("éééé", Weak), ("Éééééééé", Fair),
        ] {
            assert_eq!(password_strength(password), expected, "{password:?}");
        }
    }
}
