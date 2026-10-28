//! Acrobat's "Create a new digital ID ▸ save to a file": a self-signed RSA 2048 identity in a
//! password-protected PKCS#12 file, for signing and, when asked, for certificate encryption
//! (Acrobat's "Use digital ID for: digital signatures and data encryption"). Made in Rust, offline.
use crate::pdfcraft::engine::sign::{self, der, Certificate, DigitalId, Name, PrivateKey, Time};
use crate::{paths::atomic_write, OpdfError, Result};
use std::path::{Path, PathBuf};

/// What the user types in Acrobat's Add Digital ID form.
#[derive(Debug, Clone, Default)]
pub struct NewId {
    pub name: String,
    pub unit: String,
    pub organization: String,
    pub email: String,
    /// Two-letter ISO country, or empty.
    pub country: String,
    /// Also usable as a recipient of certificate-encrypted PDFs.
    pub encryption: bool,
}

fn bad(message: &str) -> OpdfError { OpdfError::Unsupported(message.into()) }

/// The digital ID `path` names, unlocked: a .p12/.pfx file opened with its password, or a
/// smart card or token ID (a `pkcs11:` reference, see [`crate::pkcs11`]) logged in with its PIN.
/// A wrong password or PIN is [`sign::SignError::WrongPassword`].
pub fn open(path: &Path, password: &str) -> std::result::Result<DigitalId, sign::SignError> {
    if crate::pkcs11::is_reference(path) {
        return crate::pkcs11::open(&path.to_string_lossy(), password);
    }
    let bytes = std::fs::read(path).map_err(|e| sign::SignError::Malformed(format!("{}: {e}", path.display())))?;
    sign::pkcs12::open(&bytes, password)
}

/// How Preferences and the sign panel name a digital ID: its file name, or its holder and token.
pub fn label(path: &Path) -> String {
    crate::pkcs11::label(path).unwrap_or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// Make the identity. Valid from a minute ago for five years, as Acrobat's.
pub fn create(spec: &NewId) -> Result<DigitalId> {
    if spec.name.trim().is_empty() { return Err(bad("Type the name the digital ID is issued to")); }
    let country = spec.country.trim();
    if !country.is_empty() && (country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic())) { return Err(bad("The country is a two-letter code, such as BE or FR")); }
    let key = PrivateKey::generate_rsa(2048).map_err(|e| OpdfError::Engine(e.to_string()))?;
    let name = Name::build(&spec.name, &spec.unit, &spec.organization, &spec.email, country);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| OpdfError::Engine(e.to_string()))?.as_secs() as i64;
    let mut serial = [0u8; 16];
    getrandom::fill(&mut serial).map_err(|e| OpdfError::Engine(e.to_string()))?;
    serial[0] &= 0x7f;
    let alg = sign::DigestAlg::Sha256;
    let sig_alg = key.signature_algorithm(alg);
    let spki = key.public_key().spki();
    // keyUsage: digitalSignature, nonRepudiation, and keyEncipherment for encryption.
    let usage = if spec.encryption { 0b1110_0000 } else { 0b1100_0000 };
    let unused = if spec.encryption { 5 } else { 6 };
    let ext = |oid: &str, critical: bool, value: &[u8]| if critical {
        der::seq(&[&der::oid(oid), &der::boolean(true), &der::octets(value)])
    } else {
        der::seq(&[&der::oid(oid), &der::octets(value)])
    };
    let ski = sign::DigestAlg::Sha1.digest(&[&spki]);
    let extensions = der::seq(&[
        &ext("2.5.29.15", true, &der::tlv(der::tag::BIT_STRING, &[unused, usage])),
        &ext("2.5.29.14", false, &der::octets(&ski)),
        // emailProtection and Adobe's document signing, as Acrobat's self-signed IDs carry.
        &ext("2.5.29.37", false, &der::seq(&[&der::oid("1.3.6.1.5.5.7.3.4"), &der::oid("1.2.840.113583.1.1.5")])),
    ]);
    let tbs = der::seq(&[
        &der::explicit(0, &der::int(2)), &der::uint(&serial), &sig_alg, &name.raw,
        &der::seq(&[&Time::from_unix(now - 60).encode(), &Time::from_unix(now + 5 * 365 * 86_400 + 86_400).encode()]),
        &name.raw, &spki, &der::explicit(3, &extensions),
    ]);
    let signature = key.sign(alg, &tbs).map_err(|e| OpdfError::Engine(e.to_string()))?;
    let certificate = Certificate::parse(&der::seq(&[&tbs, &sig_alg, &der::bit_string(&signature)])).map_err(|e| OpdfError::Engine(e.to_string()))?;
    Ok(DigitalId { key, certificate, chain: vec![], friendly_name: Some(spec.name.trim().to_string()) })
}

/// Create and save it as a `.p12` under `password` (6 characters at least, as Acrobat asks).
/// Never overwrites: an existing file gets a numbered name.
pub fn create_file(spec: &NewId, password: &str, out: &Path) -> Result<PathBuf> {
    if password.chars().count() < 6 { return Err(bad("Choose a password of at least 6 characters")); }
    let id = create(spec)?;
    let bytes = sign::pkcs12::write(&id, password).map_err(|e| OpdfError::Engine(e.to_string()))?;
    atomic_write(out, |part| { std::fs::write(part, &bytes)?; Ok(()) })
}

/// The certificate alone (`.cer`), to send to people who encrypt for you.
pub fn export_certificate(id: &DigitalId, out: &Path) -> Result<PathBuf> {
    atomic_write(out, |part| { std::fs::write(part, &id.certificate.raw)?; Ok(()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_created_digital_id_reopens_signs_and_encrypts_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let spec = NewId { name: "Ada Lovelace".into(), organization: "Sheaf".into(), email: "ada@example.org".into(), country: "be".into(), encryption: true, ..Default::default() };
        assert!(create_file(&spec, "short", &dir.path().join("x.p12")).is_err());
        assert!(create(&NewId { country: "Belgium".into(), ..spec.clone() }).is_err());
        let path = create_file(&spec, "secret-pw", &dir.path().join("ada.p12")).unwrap();
        let id = sign::pkcs12::open(&std::fs::read(&path).unwrap(), "secret-pw").unwrap();
        assert!(matches!(sign::pkcs12::open(&std::fs::read(&path).unwrap(), "wrong"), Err(sign::SignError::WrongPassword)));
        assert_eq!(id.certificate.subject.common_name(), Some("Ada Lovelace"));
        assert_eq!(id.certificate.subject.email(), Some("ada@example.org"));
        assert!(id.certificate.is_self_signed());
        assert!(id.certificate.key_usage.is_some_and(|u| u & (1 << 2) != 0), "key encipherment for encryption");
        // A copy encrypted for it opens with it.
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf");
        let encrypted = crate::certificate_security::encrypt(&crate::pages::Input::new(&source), std::slice::from_ref(&id.certificate), crate::certificate_security::Scope::All, &dir.path().join("e.pdf")).unwrap();
        assert!(crate::certificate_security::unlock(&encrypted, &id, &dir.path().join("u.pdf")).is_ok());
        let cer = export_certificate(&id, &dir.path().join("ada.cer")).unwrap();
        assert_eq!(Certificate::parse(&std::fs::read(cer).unwrap()).unwrap(), id.certificate);
        // Signing only: not a recipient of encryption.
        let signer = create(&NewId { encryption: false, ..spec }).unwrap();
        assert!(signer.certificate.key_usage.is_some_and(|u| u & (1 << 2) == 0));
        assert!(crate::certificate_security::encrypt(&crate::pages::Input::new(&source), &[signer.certificate], crate::certificate_security::Scope::All, &dir.path().join("f.pdf")).is_err());
    }
}
