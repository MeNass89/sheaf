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

/// Encrypt to every listed recipient. The sender must include their own certificate to reopen.
/// All recipients have full permissions; per-recipient restrictions are not exposed yet.
pub fn encrypt(input: &Input, certificates: &[Certificate], scope: Scope, out: &Path) -> Result<PathBuf> {
    if certificates.is_empty() || certificates.len() > 128 { return Err(unsupported("Choose between 1 and 128 recipient certificates")); }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(crypto)?.as_secs() as i64;
    let mut seen = std::collections::BTreeSet::new();
    let mut seed = [0u8; 20];
    getrandom::fill(&mut seed).map_err(crypto)?;
    let mut unique = Vec::new();
    for cert in certificates {
        if !seen.insert(cert.raw.clone()) { continue; }
        if Certificate::parse(&cert.raw).map_err(crypto)? != *cert { return Err(unsupported("Recipient certificate fields do not match its encoded X.509 data")); }
        if !matches!(cert.public_key, PublicKey::Rsa { .. }) { return Err(unsupported("Choose an RSA recipient certificate; EC key agreement is not supported yet")); }
        if cert.key_usage.is_some_and(|bits| bits & (1 << 2) == 0) { return Err(unsupported("The recipient certificate does not allow key encipherment")); }
        if now < cert.not_before.unix() || now > cert.not_after.unix() { return Err(unsupported("A recipient certificate is expired or not yet valid")); }
        if !cert.unknown_critical.is_empty() { return Err(unsupported("A recipient certificate has unsupported critical extensions")); }
        unique.push(cert);
    }
    let envelopes = vec![envelope::seal(&unique, &seed)?];
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

/// Unlock an authorized copy with a local PKCS#12 digital ID. No password fallback and no
/// subprocess; the original encrypted PDF is retained. Restrictions in external envelopes
/// require the recipient's change-security permission before exporting an unprotected copy.
pub fn unlock(input: &Path, identity: &DigitalId, out: &Path) -> Result<PathBuf> {
    let bytes = std::fs::read(input)?;
    let parsed = lopdf::Document::load_mem(&bytes)?;
    let encryption = parsed.trailer.get(b"Encrypt")?.as_reference()?;
    let dict = parsed.get_dictionary(encryption)?;
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
    let mut seed = None;
    for recipient in &envelopes {
        if let Some((found, permissions)) = envelope::open(recipient, identity)? {
            if permissions & 2 == 0 { return Err(unsupported("This recipient certificate is not allowed to remove document security")); }
            seed = Some(found); break;
        }
    }
    let seed = seed.ok_or_else(|| unsupported("This digital ID is not an authorized recipient"))?;
    let mut doc = objects::read(&bytes, parsed)?;
    objects::transform(&mut doc, Some(encryption), &file_key(&seed, &envelopes, scope), scope, true)?;
    doc.objects.remove(&encryption);
    doc.trailer.remove(b"Encrypt");
    doc.trailer.remove(b"Prev");
    doc.trailer.remove(b"XRefStm");
    doc.encryption_state = None;
    atomic_write(out, |part| { doc.save(part)?; Ok(()) })
}
