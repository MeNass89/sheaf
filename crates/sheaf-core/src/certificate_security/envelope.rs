//! CMS EnvelopedData: RSA key transport or EC key agreement, and AES-256-CBC seed encryption
//! (Acrobat PubSec). EC recipients (P-256, P-384) follow RFC 5753: ephemeral-static ECDH, the
//! ANSI X9.63 KDF and AES-256 key wrap, as Acrobat and OpenSSL write them.
use super::{unsupported, crypto, Certificate, Result};
use crate::pdfcraft::engine::sign::{der::{self, tag, Tlv}, DigestAlg, DigitalId, PublicKey};
use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use rsa::{pkcs8::{DecodePrivateKey, DecodePublicKey}, traits::PublicKeyParts, Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};

const DATA: &str = "1.2.840.113549.1.7.1";
const ENVELOPED: &str = "1.2.840.113549.1.7.3";
const RSA: &str = "1.2.840.113549.1.1.1";
const AES256: &str = "2.16.840.1.101.3.4.1.42";
const AES256_WRAP: &str = "2.16.840.1.101.3.4.1.45";
const EC: &str = "1.2.840.10045.2.1";
const P256: &str = "1.2.840.10045.3.1.7";
const P384: &str = "1.3.132.0.34";
/// dhSinglePass-stdDH-sha256kdf-scheme and its SHA-384 sibling (RFC 5753 section 7.1.4).
const ECDH_SHA256_KDF: &str = "1.3.132.1.11.1";
const ECDH_SHA384_KDF: &str = "1.3.132.1.11.2";

#[derive(Clone, Copy)]
enum Curve { P256, P384 }

impl Curve {
    fn of(key: &PublicKey) -> Option<(Curve, &[u8])> {
        match key {
            PublicKey::P256(point) => Some((Curve::P256, point)),
            PublicKey::P384(point) => Some((Curve::P384, point)),
            _ => None,
        }
    }
    fn oid(self) -> &'static str { match self { Curve::P256 => P256, Curve::P384 => P384 } }
    /// The KDF scheme a new envelope uses: the curve's own strength (RFC 5753 section 8).
    fn scheme(self) -> &'static str { match self { Curve::P256 => ECDH_SHA256_KDF, Curve::P384 => ECDH_SHA384_KDF } }
}

/// The ECDH shared secret of private scalar `d` and the peer's SEC1 `point`.
fn agree(curve: Curve, d: &[u8], point: &[u8]) -> Result<Vec<u8>> {
    let bad = |_| unsupported("Invalid elliptic-curve key in the recipient envelope");
    Ok(match curve {
        Curve::P256 => {
            let (d, q) = (p256::SecretKey::from_slice(d).map_err(bad)?, p256::PublicKey::from_sec1_bytes(point).map_err(bad)?);
            p256::ecdh::diffie_hellman(d.to_nonzero_scalar(), q.as_affine()).raw_secret_bytes().to_vec()
        }
        Curve::P384 => {
            let (d, q) = (p384::SecretKey::from_slice(d).map_err(bad)?, p384::PublicKey::from_sec1_bytes(point).map_err(bad)?);
            p384::ecdh::diffie_hellman(d.to_nonzero_scalar(), q.as_affine()).raw_secret_bytes().to_vec()
        }
    })
}

/// A fresh ephemeral key on `curve`: its scalar and uncompressed SEC1 point.
fn ephemeral(curve: Curve) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut d = vec![0u8; match curve { Curve::P256 => 32, Curve::P384 => 48 }];
    loop {
        getrandom::fill(&mut d).map_err(crypto)?;
        let point = match curve {
            Curve::P256 => p256::SecretKey::from_slice(&d).ok().map(|k| k.public_key().to_sec1_bytes()),
            Curve::P384 => p384::SecretKey::from_slice(&d).ok().map(|k| k.public_key().to_sec1_bytes()),
        };
        if let Some(point) = point { return Ok((d, point.into_vec())); }
    }
}

/// The AES-256 key-encryption key: ANSI X9.63 KDF over the shared secret and the
/// ECC-CMS-SharedInfo (RFC 5753 section 7.2), with the scheme's hash.
fn kek(scheme: &str, shared: &[u8], ukm: Option<&[u8]>) -> Result<[u8; 32]> {
    let hash = match scheme {
        ECDH_SHA256_KDF => DigestAlg::Sha256,
        ECDH_SHA384_KDF => DigestAlg::Sha384,
        _ => return Err(unsupported("Only ECDH with the SHA-256 or SHA-384 KDF is supported")),
    };
    let ukm = ukm.map(|u| der::explicit(0, &der::octets(u))).unwrap_or_default();
    let info = der::seq(&[&der::algorithm(AES256_WRAP, None), &ukm, &der::explicit(2, &der::octets(&256u32.to_be_bytes()))]);
    let digest = hash.digest(&[shared, &1u32.to_be_bytes(), &info]);
    digest.get(..32).and_then(|k| k.try_into().ok()).ok_or_else(|| unsupported("Invalid key derivation"))
}

fn wrapper(kek: &[u8; 32]) -> aes_kw::KwAes256 {
    use aes_kw::KeyInit;
    aes_kw::KwAes256::new(kek.into())
}

/// A KeyAgreeRecipientInfo ([1] in RecipientInfo) giving `cert`'s EC key `envelope_key`.
fn key_agreement(cert: &Certificate, curve: Curve, point: &[u8], envelope_key: &[u8; 32]) -> Result<Vec<u8>> {
    let (d, ephemeral_point) = ephemeral(curve)?;
    let kek = kek(curve.scheme(), &agree(curve, &d, point)?, None)?;
    let mut wrapped = [0u8; 40];
    let wrapped = wrapper(&kek).wrap_key(envelope_key, &mut wrapped).map_err(crypto)?;
    let originator = der::tlv(tag::ctx(1), &[der::algorithm(EC, Some(&der::oid(curve.oid()))), der::bit_string(&ephemeral_point)].concat());
    let rid = der::seq(&[&cert.issuer.raw, &der::uint(&cert.serial)]);
    let keys = der::seq(&[&der::seq(&[&rid, &der::octets(wrapped)])]);
    let algorithm = der::algorithm(curve.scheme(), Some(&der::algorithm(AES256_WRAP, None)));
    Ok(der::tlv(tag::ctx(1), &[der::int(3), der::explicit(0, &originator), algorithm, keys].concat()))
}

/// The ID's key as PKCS #8: a smart card's key can't leave the card, and decrypting on it isn't done.
fn software_key(id: &DigitalId) -> Result<&[u8]> {
    if id.key.is_external() { return Err(unsupported("Opening a certificate-encrypted PDF with a smart card or token is not supported yet: use the .p12 file")); }
    Ok(id.key.pkcs8())
}

/// The private scalar of a PKCS #8 EC key (an ECPrivateKey, RFC 5915), with its curve.
fn ec_private(pkcs8: &[u8]) -> Result<(Curve, Vec<u8>)> {
    let bad = || unsupported("The digital ID's elliptic-curve key cannot be read");
    let info = Tlv::parse_all(pkcs8).and_then(|t| t.children()).map_err(|_| bad())?;
    let [_, alg, key, ..] = info.as_slice() else { return Err(bad()) };
    let alg = alg.children().map_err(|_| bad())?;
    let key = key.expect(tag::OCTET_STRING, "EC private key").and_then(|k| Tlv::parse_all(k.value)).and_then(|t| t.children()).map_err(|_| bad())?;
    let curve = alg.get(1).and_then(|c| c.oid().ok())
        .or_else(|| key.iter().find(|t| t.tag == tag::ctx(0)).and_then(|p| p.inner().ok()).and_then(|p| p.oid().ok()));
    let curve = match curve.as_deref() {
        Some(P256) => Curve::P256,
        Some(P384) => Curve::P384,
        _ => return Err(unsupported("Only P-256 and P-384 digital IDs open EC recipient envelopes")),
    };
    let d = key.get(1).and_then(|d| d.expect(tag::OCTET_STRING, "EC private key").ok()).ok_or_else(bad)?;
    Ok((curve, d.value.to_vec()))
}

/// The envelope key in a KeyAgreeRecipientInfo for `id`, or None when it is for someone else.
fn open_key_agreement(recipient: Tlv<'_>, id: &DigitalId) -> Result<Option<Vec<u8>>> {
    let fields = recipient.children().map_err(crypto)?;
    if fields.first().map(|v| v.u64()).transpose().map_err(crypto)? != Some(3) { return Err(unsupported("Invalid key-agreement recipient")); }
    let (ukm, rest) = match &fields[1..] {
        [_, ukm, rest @ ..] if ukm.tag == tag::ctx(1) => (Some(ukm.inner().and_then(|u| u.expect(tag::OCTET_STRING, "UKM")).map_err(crypto)?.value), rest),
        [_, rest @ ..] => (None, rest),
        [] => return Err(unsupported("Invalid key-agreement recipient")),
    };
    let [algorithm, keys] = rest else { return Err(unsupported("Invalid key-agreement recipient")) };
    let mut wrapped = None;
    for key in children(*keys, tag::SEQUENCE)? {
        let key = children(key, tag::SEQUENCE)?;
        let [rid, encrypted] = key.as_slice() else { return Err(unsupported("Invalid key-agreement recipient key")) };
        // Only issuer-and-serial identifiers; a subject key identifier [0] is someone we can't name.
        let Ok(rid) = rid.expect(tag::SEQUENCE, "recipient").and_then(|r| r.children()) else { continue };
        let [issuer, serial] = rid.as_slice() else { continue };
        if issuer.raw == id.certificate.issuer.raw.as_slice() && serial.uint_bytes() == id.certificate.serial.as_slice() {
            wrapped = Some(encrypted.expect(tag::OCTET_STRING, "wrapped envelope key").map_err(crypto)?.value);
        }
    }
    let Some(wrapped) = wrapped else { return Ok(None) };
    let originator = fields[1].expect(tag::ctx(0), "originator").and_then(|o| o.inner()).map_err(crypto)?;
    if originator.tag != tag::ctx(1) { return Err(unsupported("Only ephemeral-key EC envelopes are supported")); }
    let originator = originator.children().map_err(crypto)?;
    let [_, point] = originator.as_slice() else { return Err(unsupported("Invalid originator key")) };
    let algorithm = children(*algorithm, tag::SEQUENCE)?;
    let scheme = algorithm.first().ok_or_else(|| unsupported("Invalid key agreement algorithm"))?.oid().map_err(crypto)?;
    let wrap = algorithm.get(1).and_then(|w| w.children().ok()).and_then(|w| w.first().and_then(|o| o.oid().ok()));
    if wrap.as_deref() != Some(AES256_WRAP) { return Err(unsupported("Only AES-256 key wrap is supported for EC recipients")); }
    let (curve, d) = ec_private(software_key(id)?)?;
    let kek = kek(&scheme, &agree(curve, &d, point.bits().map_err(crypto)?)?, ukm)?;
    let mut key = [0u8; 32];
    let key = wrapper(&kek).unwrap_key(wrapped, &mut key).map_err(|_| unsupported("The digital ID cannot decrypt this recipient envelope"))?;
    Ok(Some(key.to_vec()))
}

pub(super) fn seal(certificates: &[&Certificate], seed: &[u8; 20], permissions: u32) -> Result<Vec<u8>> {
    let mut envelope_key = [0u8; 32];
    getrandom::fill(&mut envelope_key).map_err(crypto)?;
    let mut rng = getrandom::rand_core::UnwrapErr(getrandom::SysRng);
    // PubSec stores a 20-byte seed and the big-endian permission word of this envelope's recipients.
    let payload = [seed.as_slice(), &permissions.to_be_bytes()].concat();
    let ciphertext = Aes256CryptFilter.encrypt(&envelope_key, &payload).map_err(crypto)?;
    let mut recipients = Vec::new();
    for cert in certificates {
        if let Some((curve, point)) = Curve::of(&cert.public_key) {
            recipients.push(key_agreement(cert, curve, point, &envelope_key)?);
            continue;
        }
        let key = RsaPublicKey::from_public_key_der(&cert.public_key.spki()).map_err(crypto)?;
        if key.size() < 256 { return Err(unsupported("Recipient certificates need an RSA key of at least 2048 bits")); }
        let encrypted_key = key.encrypt(&mut rng, Pkcs1v15Encrypt, &envelope_key).map_err(crypto)?;
        let rid = der::seq(&[&cert.issuer.raw, &der::uint(&cert.serial)]);
        recipients.push(der::seq(&[&der::int(0), &rid, &der::algorithm(RSA, Some(&der::null())), &der::octets(&encrypted_key)]));
    }
    let recipients: Vec<_> = recipients.iter().map(Vec::as_slice).collect();
    let encrypted_content = der::seq(&[
        &der::oid(DATA), &der::algorithm(AES256, Some(&der::octets(&ciphertext[..16]))),
        &der::tlv(tag::ctx_prim(0), &ciphertext[16..]),
    ]);
    // Version 2 once a key-agreement recipient (version 3) is present (RFC 5652 section 6.1).
    let version = if certificates.iter().any(|c| Curve::of(&c.public_key).is_some()) { 2 } else { 0 };
    let data = der::seq(&[&der::int(version), &der::set_of(&recipients), &encrypted_content]);
    Ok(der::seq(&[&der::oid(ENVELOPED), &der::explicit(0, &data)]))
}

fn children<'a>(item: Tlv<'a>, expected: u8) -> Result<Vec<Tlv<'a>>> {
    item.expect(expected, "CMS envelope").and_then(|t| t.children()).map_err(crypto)
}

/// Unknown algorithms and malformed CMS are errors; a different recipient is simply no match.
pub(super) fn open(bytes: &[u8], id: &DigitalId) -> Result<Option<([u8; 20], u32)>> {
    let ci = children(Tlv::parse_all(bytes).map_err(crypto)?, tag::SEQUENCE)?;
    if ci.len() != 2 || ci[0].oid().map_err(crypto)? != ENVELOPED { return Err(unsupported("Expected a CMS EnvelopedData recipient")); }
    let data = children(ci[1].expect(tag::ctx(0), "CMS content").and_then(|t| t.inner()).map_err(crypto)?, tag::SEQUENCE)?;
    if data.len() != 3 || !matches!(data[0].u64().map_err(crypto)?, 0 | 2) { return Err(unsupported("Only RSA key-transport and EC key-agreement CMS envelopes are supported")); }
    let mut key = None;
    for recipient in children(data[1], tag::SET)? {
        if recipient.tag == tag::ctx(1) {
            match open_key_agreement(recipient, id)? {
                Some(k) => { key = Some(k); break; }
                None => continue,
            }
        }
        let fields = children(recipient, tag::SEQUENCE)?;
        if fields.len() != 4 || fields[0].u64().map_err(crypto)? != 0 { return Err(unsupported("Only issuer-and-serial RSA recipients are supported")); }
        let rid = children(fields[1], tag::SEQUENCE)?;
        if rid.len() != 2 { return Err(unsupported("Invalid recipient certificate identifier")); }
        let serial = rid[1].expect(tag::INTEGER, "recipient serial number").map_err(crypto)?;
        if rid[0].raw != id.certificate.issuer.raw.as_slice() || serial.uint_bytes() != id.certificate.serial.as_slice() { continue; }
        let algorithm = children(fields[2], tag::SEQUENCE)?;
        if algorithm.is_empty() || algorithm[0].oid().map_err(crypto)? != RSA { return Err(unsupported("This envelope does not use RSA PKCS#1 key transport")); }
        let private = RsaPrivateKey::from_pkcs8_der(software_key(id)?).map_err(crypto)?;
        let mut rng = getrandom::rand_core::UnwrapErr(getrandom::SysRng);
        let encrypted = fields[3].expect(tag::OCTET_STRING, "encrypted envelope key").map_err(crypto)?;
        key = Some(private.decrypt_blinded(&mut rng, Pkcs1v15Encrypt, encrypted.value).map_err(|_| unsupported("The digital ID cannot decrypt this recipient envelope"))?);
        break;
    }
    let Some(key) = key else { return Ok(None); };
    let encrypted = children(data[2], tag::SEQUENCE)?;
    if encrypted.len() != 3 || encrypted[0].oid().map_err(crypto)? != DATA { return Err(unsupported("Invalid encrypted CMS content")); }
    let algorithm = children(encrypted[1], tag::SEQUENCE)?;
    if algorithm.len() != 2 || algorithm[0].oid().map_err(crypto)? != AES256 { return Err(unsupported("Only AES-256-CBC CMS content is supported")); }
    let iv = algorithm[1].expect(tag::OCTET_STRING, "CMS initialization vector").map_err(crypto)?.value;
    let ciphertext = encrypted[2].expect(tag::ctx_prim(0), "CMS encrypted bytes").map_err(crypto)?.value;
    if iv.len() != 16 || key.len() != 32 { return Err(unsupported("Invalid CMS key or initialization vector length")); }
    let payload = Aes256CryptFilter.decrypt(&key, &[iv, ciphertext].concat()).map_err(crypto)?;
    if payload.len() != 24 { return Err(unsupported("Invalid PDF recipient seed and permissions")); }
    let seed = payload[..20].try_into().map_err(|_| unsupported("Invalid PDF recipient seed"))?;
    let permissions = u32::from_be_bytes(payload[20..].try_into().map_err(|_| unsupported("Invalid PDF recipient permissions"))?);
    Ok(Some((seed, permissions)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfcraft::engine::sign::pkcs12;
    use std::process::Command;

    /// An EC digital ID made by OpenSSL on `curve` with keyAgreement, as `(p12 bytes, cert PEM path)`.
    fn openssl_id(dir: &std::path::Path, curve: &str) -> Option<(DigitalId, std::path::PathBuf)> {
        let (key, cert, p12) = (dir.join(format!("{curve}.key")), dir.join(format!("{curve}.pem")), dir.join(format!("{curve}.p12")));
        let o = Command::new("openssl").args(["req", "-x509", "-newkey", "ec", "-pkeyopt", &format!("ec_paramgen_curve:{curve}"), "-nodes", "-days", "2", "-subj", "/CN=Ec Interop", "-addext", "keyUsage=critical,keyAgreement"])
            .arg("-keyout").arg(&key).arg("-out").arg(&cert).output().ok()?;
        assert!(o.status.success(), "openssl req: {}", String::from_utf8_lossy(&o.stderr));
        let o = Command::new("openssl").args(["pkcs12", "-export", "-passout", "pass:pw", "-inkey"]).arg(&key).arg("-in").arg(&cert).arg("-out").arg(&p12).output().unwrap();
        assert!(o.status.success(), "openssl pkcs12: {}", String::from_utf8_lossy(&o.stderr));
        Some((pkcs12::open(&std::fs::read(&p12).unwrap(), "pw").unwrap(), cert))
    }

    /// Our key-agreement envelopes are standard CMS: OpenSSL opens them, and we open OpenSSL's.
    #[test]
    fn ec_envelopes_interoperate_with_openssl_both_ways() {
        let dir = tempfile::tempdir().unwrap();
        for curve in ["P-256", "P-384"] {
            let Some((id, cert)) = openssl_id(dir.path(), curve) else { return eprintln!("openssl not installed: EC interop not tested") };
            let key = dir.path().join(format!("{curve}.key"));
            let seed = [7u8; 20];
            let ours = dir.path().join("ours.der");
            std::fs::write(&ours, seal(&[&id.certificate], &seed, 0xffff_f0c4).unwrap()).unwrap();
            let o = Command::new("openssl").args(["cms", "-decrypt", "-inform", "DER", "-binary", "-in"]).arg(&ours).arg("-recip").arg(&cert).arg("-inkey").arg(&key).output().unwrap();
            assert!(o.status.success(), "{curve}: openssl could not open our envelope: {}", String::from_utf8_lossy(&o.stderr));
            assert_eq!(o.stdout, [seed.as_slice(), &0xffff_f0c4u32.to_be_bytes()].concat(), "{curve}");

            let payload = dir.path().join("payload");
            std::fs::write(&payload, [[9u8; 20].as_slice(), &0xffff_f0c0u32.to_be_bytes()].concat()).unwrap();
            let theirs = dir.path().join("theirs.der");
            let o = Command::new("openssl").args(["cms", "-encrypt", "-binary", "-aes256", "-outform", "DER", "-in"]).arg(&payload).arg("-out").arg(&theirs).arg("-recip").arg(&cert).args(["-keyopt", "ecdh_kdf_md:sha256"]).output().unwrap();
            assert!(o.status.success(), "openssl cms -encrypt: {}", String::from_utf8_lossy(&o.stderr));
            assert_eq!(open(&std::fs::read(&theirs).unwrap(), &id).unwrap(), Some(([9u8; 20], 0xffff_f0c0)), "{curve}: OpenSSL's envelope");
        }
    }
}
