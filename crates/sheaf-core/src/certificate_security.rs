//! Offline certificate encryption (Acrobat's Encrypt with Certificate).
//! Adobe.PubSec S5, CMS RSA key transport and AES-256; no outside program or network.
//! The initial CLI workflow writes ordinary objects and unlocks an authorized copy. External
//! object streams and explicit stream crypt filters are refused rather than losing content.
mod envelope;
mod objects;

use crate::{pages::Input, paths::atomic_write, OpdfError, Result};
use crate::pdfcraft::engine::sign::{Certificate, DigitalId, DigestAlg, PublicKey};
use lopdf::{dictionary, Object, StringFormat};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope { All, ExceptMetadata, Attachments }

fn unsupported(message: &str) -> OpdfError { OpdfError::Unsupported(message.into()) }
fn crypto(error: impl std::fmt::Display) -> OpdfError { OpdfError::Engine(format!("Certificate encryption: {error}")) }

fn file_key(seed: &[u8; 20], envelopes: &[Vec<u8>], scope: Scope) -> Vec<u8> {
    let mut parts = vec![seed.as_slice()];
    parts.extend(envelopes.iter().map(Vec::as_slice));
    if scope == Scope::ExceptMetadata { parts.push(&[0xff; 4]); }
    DigestAlg::Sha256.digest(&parts)
}

/// What one recipient may do, stored in their own CMS envelope (Acrobat's Encrypt with
/// Certificate dialog sets these per recipient). The word is ISO 32000's permission bits, plus
/// PubSec bit 2: "change security", which also grants everything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions(pub u32);

impl Permissions {
    pub const CHANGE_SECURITY: u32 = 1 << 1;
    pub const PRINT: u32 = 1 << 2;
    pub const MODIFY: u32 = 1 << 3;
    pub const COPY: u32 = 1 << 4;
    pub const COMMENT: u32 = 1 << 5;
    pub const FILL_FORMS: u32 = 1 << 8;
    pub const ACCESSIBILITY: u32 = 1 << 9;
    pub const ASSEMBLE: u32 = 1 << 10;
    pub const PRINT_HIGH: u32 = 1 << 11;
    /// Everything, the sender's own envelope.
    pub const FULL: Permissions = Permissions(u32::MAX);

    /// Acrobat's per-recipient choices. Screen readers keep their access (Acrobat always does);
    /// the reserved high bits stay set as the specification asks.
    pub fn new(printing: bool, copying: bool, editing: bool, commenting: bool, filling_forms: bool, change_security: bool) -> Self {
        if change_security { return Self::FULL; }
        let mut bits = 0xffff_f000 | 0b1100_0000 | Self::ACCESSIBILITY;
        for (on, bit) in [(printing, Self::PRINT | Self::PRINT_HIGH), (copying, Self::COPY), (editing, Self::MODIFY | Self::ASSEMBLE), (commenting, Self::COMMENT), (filling_forms, Self::FILL_FORMS)] {
            if on { bits |= bit; }
        }
        Permissions(bits)
    }
    pub fn allows(self, bit: u32) -> bool { self.0 & Self::CHANGE_SECURITY != 0 || self.0 & bit == bit }
    /// The actions a recipient may not take, for the reader's Properties and its refusals.
    pub fn denied(self) -> Vec<&'static str> {
        [(Self::PRINT, "Printing"), (Self::COPY, "Copying"), (Self::MODIFY, "Editing"), (Self::COMMENT, "Commenting"), (Self::FILL_FORMS, "Form filling"), (Self::CHANGE_SECURITY, "Changing security")]
            .into_iter().filter(|(bit, _)| !self.allows(*bit)).map(|(_, name)| name).collect()
    }
}

/// One recipient of Acrobat's Encrypt with Certificate: their certificate and what they may do.
#[derive(Debug, Clone)]
pub struct Recipient { pub certificate: Certificate, pub permissions: Permissions }

/// Encrypt to every listed recipient with full permissions (the CLI's workflow). The sender must
/// include their own certificate to reopen.
pub fn encrypt(input: &Input, certificates: &[Certificate], scope: Scope, out: &Path) -> Result<PathBuf> {
    let recipients: Vec<_> = certificates.iter().map(|c| Recipient { certificate: c.clone(), permissions: Permissions::FULL }).collect();
    encrypt_for(input, &recipients, scope, out)
}

/// Encrypt with per-recipient permissions. Recipients sharing permissions share one envelope,
/// as Acrobat writes them; a certificate listed twice keeps its first permissions.
pub fn encrypt_for(input: &Input, recipients: &[Recipient], scope: Scope, out: &Path) -> Result<PathBuf> {
    if recipients.is_empty() || recipients.len() > 128 { return Err(unsupported("Choose between 1 and 128 recipient certificates")); }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(crypto)?.as_secs() as i64;
    let mut seen = std::collections::BTreeSet::new();
    let mut seed = [0u8; 20];
    getrandom::fill(&mut seed).map_err(crypto)?;
    let mut groups: Vec<(u32, Vec<&Certificate>)> = Vec::new();
    for Recipient { certificate: cert, permissions } in recipients {
        if !seen.insert(cert.raw.clone()) { continue; }
        if Certificate::parse(&cert.raw).map_err(crypto)? != *cert { return Err(unsupported("Recipient certificate fields do not match its encoded X.509 data")); }
        if !matches!(cert.public_key, PublicKey::Rsa { .. }) { return Err(unsupported("Choose an RSA recipient certificate; EC key agreement is not supported yet")); }
        if cert.key_usage.is_some_and(|bits| bits & (1 << 2) == 0) { return Err(unsupported("The recipient certificate does not allow key encipherment")); }
        if now < cert.not_before.unix() || now > cert.not_after.unix() { return Err(unsupported("A recipient certificate is expired or not yet valid")); }
        if !cert.unknown_critical.is_empty() { return Err(unsupported("A recipient certificate has unsupported critical extensions")); }
        match groups.iter_mut().find(|(bits, _)| *bits == permissions.0) {
            Some((_, certs)) => certs.push(cert),
            None => groups.push((permissions.0, vec![cert])),
        }
    }
    // Several permission groups need one envelope each; an attachments-only filter has room
    // for one CMS string, so it takes one permission set.
    if scope == Scope::Attachments && groups.len() > 1 { return Err(unsupported("Encrypting only attachments gives every recipient the same permissions")); }
    let envelopes = groups.iter().map(|(bits, certs)| envelope::seal(certs, &seed, *bits)).collect::<Result<Vec<_>>>()?;
    let mut doc = input.load_lopdf()?;
    if doc.objects.values().any(|o| o.as_dict().is_ok_and(|d| d.has(b"ByteRange"))) { return Err(unsupported("Certificate encryption requires an unsigned copy of this PDF")); }
    // Full saves write every expanded object once. Obsolete object/xref streams must not
    // carry stale copies of text or encrypted members alongside their live replacements.
    doc.objects.retain(|_, o| !o.as_stream().is_ok_and(|s| matches!(s.dict.get(b"Type").and_then(Object::as_name).ok(), Some(b"ObjStm" | b"XRef"))));
    if scope == Scope::Attachments && !doc.objects.values().any(|o| o.as_stream().is_ok_and(|s| s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"EmbeddedFile".as_slice()))) {
        return Err(unsupported("This PDF has no embedded files to encrypt"));
    }
    // AES-256 is an Adobe PDF 1.7 extension. Preserve other vendors' extension entries.
    let root = doc.trailer.get(b"Root")?.as_reference()?;
    let mut extensions = match doc.get_dictionary(root)?.get(b"Extensions") {
        Ok(Object::Reference(id)) => doc.get_dictionary(*id)?.clone(),
        Ok(Object::Dictionary(dict)) => dict.clone(),
        Err(_) => lopdf::Dictionary::new(),
        _ => return Err(unsupported("Invalid PDF extension dictionary")),
    };
    extensions.set("ADBE", dictionary! { "BaseVersion" => "1.7", "ExtensionLevel" => 8 });
    doc.get_dictionary_mut(root)?.set("Extensions", extensions);
    objects::transform(&mut doc, None, &file_key(&seed, &envelopes, scope), scope, false)?;
    let mut recipients: Vec<_> = envelopes.into_iter().map(|v| Object::String(v, StringFormat::Hexadecimal)).collect();
    // A non-default EFF filter uses one CMS string containing all recipient infos.
    let recipients = if scope == Scope::Attachments { recipients.remove(0) } else { Object::Array(recipients) };
    let crypt_filter = dictionary! {
        "CFM" => "AESV3", "Length" => 256, "AuthEvent" => if scope == Scope::Attachments { "EFOpen" } else { "DocOpen" },
        "EncryptMetadata" => scope != Scope::ExceptMetadata, "Recipients" => recipients,
    };
    let default = if scope == Scope::Attachments { "Identity" } else { "DefaultCryptFilter" };
    let encryption = doc.add_object(dictionary! {
        "Filter" => "Adobe.PubSec", "SubFilter" => "adbe.pkcs7.s5", "V" => 5, "Length" => 256,
        "EncryptMetadata" => scope != Scope::ExceptMetadata,
        "CF" => dictionary! { "DefaultCryptFilter" => crypt_filter },
        "StmF" => default, "StrF" => default, "EFF" => "DefaultCryptFilter",
    });
    doc.trailer.set("Encrypt", encryption);
    doc.trailer.remove(b"Prev");
    doc.trailer.remove(b"XRefStm");
    doc.version = "1.7".into();
    doc.encryption_state = None;
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}

/// A certificate-encrypted PDF opened with a recipient's digital ID: its file key, what this
/// recipient may do, and the encryption dictionary so saved changes keep every recipient's access
/// (Acrobat opens such files in place and saves them still encrypted).
#[derive(Clone)]
pub struct Opened {
    pub permissions: Permissions,
    key: Vec<u8>,
    scope: Scope,
    encryption: lopdf::Dictionary,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Opened").field("permissions", &self.permissions).field("scope", &self.scope).finish_non_exhaustive() }
}

/// Whether `path` is encrypted for certificates (Adobe.PubSec): the reader asks for a digital
/// ID instead of a password. Only the trailer is read.
pub fn is_certificate_encrypted(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else { return false };
    let Ok(doc) = lopdf::Document::load_mem(&bytes) else { return bytes.windows(12).any(|w| w == b"Adobe.PubSec") };
    doc.trailer.get(b"Encrypt").ok().and_then(|o| match o {
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        Object::Dictionary(d) => Some(d),
        _ => None,
    }).is_some_and(|d| d.get(b"Filter").and_then(Object::as_name).ok() == Some(b"Adobe.PubSec".as_slice()))
}

fn decrypt(bytes: &[u8], identity: &DigitalId) -> Result<(lopdf::Document, Opened)> {
    let parsed = lopdf::Document::load_mem(bytes)?;
    let encryption = parsed.trailer.get(b"Encrypt")?.as_reference()?;
    let dict = parsed.get_dictionary(encryption)?.clone();
    if dict.get(b"Filter")?.as_name()? != b"Adobe.PubSec" || dict.get(b"SubFilter")?.as_name()? != b"adbe.pkcs7.s5" || dict.get(b"V")?.as_i64()? != 5 {
        return Err(unsupported("Only Adobe.PubSec S5 AES-256 certificate encryption is supported"));
    }
    let cf = dict.get(b"CF")?.as_dict()?.get(b"DefaultCryptFilter")?.as_dict()?;
    if cf.get(b"CFM")?.as_name()? != b"AESV3" { return Err(unsupported("This certificate PDF does not use the supported AES-256 crypt filter")); }
    let stream = dict.get(b"StmF")?.as_name()?;
    let strings = dict.get(b"StrF")?.as_name()?;
    let scope = match (stream, strings) {
        (b"Identity", b"Identity") => Scope::Attachments,
        (b"DefaultCryptFilter", b"DefaultCryptFilter") if !cf.get(b"EncryptMetadata").and_then(Object::as_bool).unwrap_or(true) => Scope::ExceptMetadata,
        (b"DefaultCryptFilter", b"DefaultCryptFilter") => Scope::All,
        _ => return Err(unsupported("This PDF uses multiple or unsupported certificate crypt filters")),
    };
    if dict.get(b"EFF")?.as_name()? != b"DefaultCryptFilter" { return Err(unsupported("Unsupported attachment crypt filter")); }
    let envelopes = match cf.get(b"Recipients")? {
        Object::Array(array) => array.iter().map(|o| o.as_str().map(<[u8]>::to_vec)).collect::<std::result::Result<Vec<_>, _>>()?,
        Object::String(bytes, _) if scope == Scope::Attachments => vec![bytes.clone()],
        _ => return Err(unsupported("Invalid certificate recipient envelopes")),
    };
    if envelopes.is_empty() || envelopes.len() > 128 { return Err(unsupported("Invalid number of recipient envelopes")); }
    let mut found = None;
    for recipient in &envelopes {
        if let Some(opened) = envelope::open(recipient, identity)? { found = Some(opened); break; }
    }
    let (seed, permissions) = found.ok_or_else(|| unsupported("This digital ID is not an authorized recipient"))?;
    let key = file_key(&seed, &envelopes, scope);
    let mut doc = objects::read(bytes, parsed)?;
    objects::transform(&mut doc, Some(encryption), &key, scope, true)?;
    doc.objects.remove(&encryption);
    doc.trailer.remove(b"Encrypt");
    doc.trailer.remove(b"Prev");
    doc.trailer.remove(b"XRefStm");
    doc.encryption_state = None;
    Ok((doc, Opened { permissions: Permissions(permissions), key, scope, encryption: dict }))
}

/// Open a certificate-encrypted PDF for reading with the user's digital ID: the decrypted
/// working copy is written to `out` (the reader keeps it in a private folder) and the returned
/// [`Opened`] reseals it on save. Any recipient may read; their permissions travel with it.
pub fn open(input: &Path, identity: &DigitalId, out: &Path) -> Result<Opened> {
    let (mut doc, opened) = decrypt(&std::fs::read(input)?, identity)?;
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes)?;
    std::fs::write(out, bytes)?;
    Ok(opened)
}

/// The working copy `plain` encrypted again with the same file key and recipients, as the bytes
/// of the file to write: every recipient opens the saved file as before. Refused for a recipient
/// without permission to change the document, and for signed copies (a full rewrite would break
/// the signature).
pub fn reseal(plain: &[u8], opened: &Opened) -> Result<Vec<u8>> {
    if !opened.permissions.allows(Permissions::MODIFY) { return Err(unsupported("Your certificate permissions do not allow changing this document")); }
    let mut doc = lopdf::Document::load_mem(plain)?;
    if doc.is_encrypted() { return Err(unsupported("The working copy is already encrypted")); }
    if doc.objects.values().any(|o| o.as_dict().is_ok_and(|d| d.has(b"ByteRange"))) { return Err(unsupported("A signed certificate-encrypted PDF cannot be saved again without breaking its signature")); }
    doc.objects.retain(|_, o| !o.as_stream().is_ok_and(|s| matches!(s.dict.get(b"Type").and_then(Object::as_name).ok(), Some(b"ObjStm" | b"XRef"))));
    objects::transform(&mut doc, None, &opened.key, opened.scope, false)?;
    let encryption = doc.add_object(opened.encryption.clone());
    doc.trailer.set("Encrypt", encryption);
    doc.trailer.remove(b"Prev");
    doc.trailer.remove(b"XRefStm");
    doc.version = "1.7".into();
    doc.encryption_state = None;
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes)?;
    Ok(bytes)
}

/// Unlock an authorized copy with a local PKCS#12 digital ID. No password fallback and no
/// subprocess; the original encrypted PDF is retained. Exporting an unprotected copy needs the
/// recipient's change-security permission.
pub fn unlock(input: &Path, identity: &DigitalId, out: &Path) -> Result<PathBuf> {
    let (mut doc, opened) = decrypt(&std::fs::read(input)?, identity)?;
    if opened.permissions.0 & Permissions::CHANGE_SECURITY == 0 { return Err(unsupported("This recipient certificate is not allowed to remove document security")); }
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}
