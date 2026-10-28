//! Digital IDs on smart cards and USB tokens (Acrobat's Preferences ▸ Signatures ▸ PKCS#11
//! Modules and Tokens): the token's PKCS #11 module is loaded at run time, its certificates are
//! listed without a PIN, and a signature is made on the token after logging in with the PIN.
//! The private key never leaves the token.
//!
//! An ID is kept as an RFC 7512 URI, `pkcs11:token=…;serial=…;id=…;object=…?module-path=…`,
//! next to the .p12 paths in Preferences, so everything that remembers an ID keeps working.
use crate::pdfcraft::engine::sign::{der, keys::ExternalKey, Certificate, DigestAlg, DigitalId, PrivateKey, PublicKey, SignError};
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::error::{Error, RvError};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, CertificateType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::slot::Slot;
use cryptoki::types::AuthPin;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const SCHEME: &str = "pkcs11:";

/// A digital ID on a token, as Preferences lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenId {
    /// The `pkcs11:` URI that names it.
    pub reference: String,
    /// The certificate's holder.
    pub name: String,
    /// The token's label.
    pub token: String,
}

/// Whether `path` names a token ID rather than a .p12 file.
pub fn is_reference(path: &Path) -> bool {
    path.to_str().is_some_and(|s| s.starts_with(SCHEME))
}

/// "Name (token)" for a token ID, None for a file.
pub fn label(path: &Path) -> Option<String> {
    let r = Reference::parse(path.to_str()?)?;
    Some(format!("{} ({})", r.object, r.token))
}

fn fail(what: &str, e: impl std::fmt::Display) -> SignError {
    SignError::Crypto(format!("{what}: {e}"))
}

/// Each module is initialized once per process (a second C_Initialize is an error); later
/// listings and signatures share it.
fn module(path: &Path) -> Result<Pkcs11, SignError> {
    static MODULES: Mutex<Vec<(PathBuf, Pkcs11)>> = Mutex::new(Vec::new());
    let mut modules = MODULES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, m)) = modules.iter().find(|(p, _)| p == path) {
        return Ok(m.clone());
    }
    let m = Pkcs11::new(path).map_err(|e| fail(&format!("{} is not a PKCS #11 module", path.display()), e))?;
    match m.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK)) {
        Ok(()) | Err(Error::Pkcs11(RvError::CryptokiAlreadyInitialized, _)) => {}
        Err(e) => return Err(fail(&format!("{} did not start", path.display()), e)),
    }
    modules.push((path.to_path_buf(), m.clone()));
    Ok(m)
}

/// The certificates on a session's token that name a key (a non-empty CKA_ID), with that ID.
fn certificates(session: &Session) -> Result<Vec<(Certificate, Vec<u8>)>, SignError> {
    let found = session
        .find_objects(&[Attribute::Class(ObjectClass::CERTIFICATE), Attribute::CertificateType(CertificateType::X_509)])
        .map_err(|e| fail("the token's certificates", e))?;
    let mut out = Vec::new();
    for handle in found {
        let (mut value, mut id) = (None, None);
        for a in session.get_attributes(handle, &[AttributeType::Value, AttributeType::Id]).map_err(|e| fail("a token certificate", e))? {
            match a {
                Attribute::Value(v) => value = Some(v),
                Attribute::Id(i) => id = Some(i),
                _ => {}
            }
        }
        let (Some(value), Some(id)) = (value, id) else { continue };
        // Certificates of keys PdfCraft can't sign with (other curves, Ed25519) are left out.
        let Ok(cert) = Certificate::parse(&value) else { continue };
        if id.is_empty() || cert.public_key.store_signing_key().is_none() { continue; }
        out.push((cert, id));
    }
    Ok(out)
}

fn name(cert: &Certificate) -> String {
    cert.subject.common_name().map(str::to_string).unwrap_or_else(|| cert.display_name())
}

/// The signing IDs on every token the module at `module_path` serves. No PIN is asked:
/// certificates are public objects on a token.
pub fn identities(module_path: &Path) -> Result<Vec<TokenId>, SignError> {
    let m = module(module_path)?;
    let mut out = Vec::new();
    // A blank token, or a card the module can't read, is no reason to hide the others.
    for slot in m.get_slots_with_initialized_token().map_err(|e| fail("the module's tokens", e))? {
        let (Ok(info), Ok(session)) = (m.get_token_info(slot), m.open_ro_session(slot)) else { continue };
        for (cert, id) in certificates(&session)? {
            let r = Reference { module: module_path.to_path_buf(), token: info.label().trim().to_string(), serial: info.serial_number().trim().to_string(), id, object: name(&cert) };
            out.push(TokenId { reference: r.to_string(), name: r.object.clone(), token: r.token.clone() });
        }
    }
    Ok(out)
}

/// The key on the token: it signs in its own session, logged in with the PIN.
struct TokenKey {
    session: Mutex<Session>,
    key: ObjectHandle,
    public: PublicKey,
}

/// The DER prefix PKCS #1 v1.5 signs before the digest: DigestInfo with NULL parameters.
fn digest_info(alg: DigestAlg, digest: &[u8]) -> Vec<u8> {
    der::seq(&[&der::algorithm(alg.oid(), Some(&der::null())), &der::octets(digest)])
}

impl ExternalKey for TokenKey {
    fn sign(&self, alg: DigestAlg, msg: &[u8]) -> Result<Vec<u8>, SignError> {
        let digest = alg.digest(&[msg]);
        let session = self.session.lock().unwrap_or_else(|e| e.into_inner());
        // The raw mechanisms (CKM_RSA_PKCS over a DigestInfo, CKM_ECDSA over the digest) are the
        // ones every card offers; hashing on the card is often missing.
        if matches!(self.public, PublicKey::Rsa { .. }) {
            session.sign(&Mechanism::RsaPkcs, self.key, &digest_info(alg, &digest)).map_err(|e| fail("the token did not sign", e))
        } else {
            let raw = session.sign(&Mechanism::Ecdsa, self.key, &digest).map_err(|e| fail("the token did not sign", e))?;
            self.public.store_signature_der(&raw)
        }
    }
}

/// The digital ID `reference` names, logged in with `pin` (empty: a reader with its own
/// keypad, or a token without a PIN). A wrong PIN is [`SignError::WrongPassword`].
pub fn open(reference: &str, pin: &str) -> Result<DigitalId, SignError> {
    let r = Reference::parse(reference).ok_or_else(|| SignError::Malformed(format!("not a token digital ID: {reference}")))?;
    let m = module(&r.module)?;
    let slot = find_slot(&m, &r)?;
    let session = m.open_ro_session(slot).map_err(|e| fail("a token session", e))?;
    let (certificate, _) = certificates(&session)?
        .into_iter()
        .find(|(_, id)| *id == r.id)
        .ok_or_else(|| SignError::Crypto(format!("{} is no longer on the token {}", r.object, r.token)))?;
    let pin = (!pin.is_empty()).then(|| AuthPin::from(pin.to_string()));
    match session.login(UserType::User, pin.as_ref()) {
        Ok(()) | Err(Error::Pkcs11(RvError::UserAlreadyLoggedIn, _)) => {}
        Err(Error::Pkcs11(RvError::PinIncorrect | RvError::PinLenRange, _)) => return Err(SignError::WrongPassword),
        Err(Error::Pkcs11(RvError::PinLocked, _)) => return Err(SignError::Crypto(format!("the PIN of {} is locked", r.token))),
        Err(e) => return Err(fail("the token refused the PIN", e)),
    }
    let key = session
        .find_objects(&[Attribute::Class(ObjectClass::PRIVATE_KEY), Attribute::Id(r.id.clone())])
        .map_err(|e| fail("the token's keys", e))?
        .into_iter()
        .next()
        .ok_or_else(|| SignError::Crypto(format!("the token has no private key for {}", r.object)))?;
    let public = certificate.public_key.clone();
    let key = PrivateKey::external(public.clone(), Arc::new(TokenKey { session: Mutex::new(session), key, public }));
    Ok(DigitalId { key, certificate, chain: Vec::new(), friendly_name: Some(r.object) })
}

fn find_slot(m: &Pkcs11, r: &Reference) -> Result<Slot, SignError> {
    for slot in m.get_slots_with_initialized_token().map_err(|e| fail("the module's tokens", e))? {
        let Ok(info) = m.get_token_info(slot) else { continue };
        if info.label().trim() == r.token && (r.serial.is_empty() || info.serial_number().trim() == r.serial) {
            return Ok(slot);
        }
    }
    Err(SignError::Crypto(format!("insert the token {} (no reader shows it)", r.token)))
}

/// The RFC 7512 attributes Sheaf writes and reads.
#[derive(Debug, PartialEq)]
struct Reference {
    module: PathBuf,
    token: String,
    serial: String,
    id: Vec<u8>,
    object: String,
}

impl Reference {
    fn parse(s: &str) -> Option<Reference> {
        let (path, query) = s.strip_prefix(SCHEME)?.split_once('?').unwrap_or((s.strip_prefix(SCHEME)?, ""));
        let (mut token, mut serial, mut id, mut object, mut module) = (String::new(), String::new(), Vec::new(), String::new(), None);
        for (k, v) in path.split(';').chain(query.split('&')).filter_map(|kv| kv.split_once('=')) {
            let v = unescape(v)?;
            match k {
                "token" => token = String::from_utf8(v).ok()?,
                "serial" => serial = String::from_utf8(v).ok()?,
                "id" => id = v,
                "object" => object = String::from_utf8(v).ok()?,
                "module-path" => module = Some(PathBuf::from(String::from_utf8(v).ok()?)),
                _ => {}
            }
        }
        (!id.is_empty()).then_some(Reference { module: module?, token, serial, id, object })
    }
}

impl std::fmt::Display for Reference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let module = self.module.to_string_lossy();
        write!(f, "{SCHEME}token={};serial={};id={};object={}?module-path={}", escape(self.token.as_bytes()), escape(self.serial.as_bytes()), escape(&self.id), escape(self.object.as_bytes()), escape(module.as_bytes()))
    }
}

/// Percent-encoding (RFC 3986): unreserved characters and `/` stay, every other byte is %XX.
fn escape(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn unescape(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        out.push(if b == b'%' { u8::from_str_radix(std::str::from_utf8(&[bytes.next()?, bytes.next()?]).ok()?, 16).ok()? } else { b });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_reference_round_trips_and_reads_as_its_holder_and_token() {
        let r = Reference { module: "/usr/lib/softhsm/libsofthsm2.so".into(), token: "Ada's card".into(), serial: "9a3f".into(), id: vec![0, 1, 0xfe], object: "Ada Lovelace; Ltd".into() };
        let s = r.to_string();
        assert_eq!(s, "pkcs11:token=Ada%27s%20card;serial=9a3f;id=%00%01%FE;object=Ada%20Lovelace%3B%20Ltd?module-path=/usr/lib/softhsm/libsofthsm2.so");
        assert_eq!(Reference::parse(&s), Some(r));
        assert!(is_reference(Path::new(&s)) && !is_reference(Path::new("/ids/ada.p12")));
        assert_eq!(label(Path::new(&s)).as_deref(), Some("Ada Lovelace; Ltd (Ada's card)"));
        assert_eq!(Reference::parse("pkcs11:token=x;id=%01"), None, "no module");
    }
}
