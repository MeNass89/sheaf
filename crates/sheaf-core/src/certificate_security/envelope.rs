//! CMS EnvelopedData: RSA key transport and AES-256-CBC seed encryption (Acrobat PubSec).
use super::{unsupported, crypto, Certificate, Result};
use crate::pdfcraft::engine::sign::{der::{self, tag, Tlv}, DigitalId};
use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use rsa::{pkcs8::{DecodePrivateKey, DecodePublicKey}, traits::PublicKeyParts, Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};

const DATA: &str = "1.2.840.113549.1.7.1";
const ENVELOPED: &str = "1.2.840.113549.1.7.3";
const RSA: &str = "1.2.840.113549.1.1.1";
const AES256: &str = "2.16.840.1.101.3.4.1.42";

pub(super) fn seal(certificates: &[&Certificate], seed: &[u8; 20], permissions: u32) -> Result<Vec<u8>> {
    let mut envelope_key = [0u8; 32];
    getrandom::fill(&mut envelope_key).map_err(crypto)?;
    let mut rng = getrandom::rand_core::UnwrapErr(getrandom::SysRng);
    // PubSec stores a 20-byte seed and the big-endian permission word of this envelope's recipients.
    let payload = [seed.as_slice(), &permissions.to_be_bytes()].concat();
    let ciphertext = Aes256CryptFilter.encrypt(&envelope_key, &payload).map_err(crypto)?;
    let mut recipients = Vec::new();
    for cert in certificates {
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
    let data = der::seq(&[&der::int(0), &der::set_of(&recipients), &encrypted_content]);
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
    if data.len() != 3 || data[0].u64().map_err(crypto)? != 0 { return Err(unsupported("Only RSA key-transport CMS envelopes are supported")); }
    let mut key = None;
    for recipient in children(data[1], tag::SET)? {
        let fields = children(recipient, tag::SEQUENCE)?;
        if fields.len() != 4 || fields[0].u64().map_err(crypto)? != 0 { return Err(unsupported("Only issuer-and-serial RSA recipients are supported")); }
        let rid = children(fields[1], tag::SEQUENCE)?;
        if rid.len() != 2 { return Err(unsupported("Invalid recipient certificate identifier")); }
        let serial = rid[1].expect(tag::INTEGER, "recipient serial number").map_err(crypto)?;
        if rid[0].raw != id.certificate.issuer.raw.as_slice() || serial.uint_bytes() != id.certificate.serial.as_slice() { continue; }
        let algorithm = children(fields[2], tag::SEQUENCE)?;
        if algorithm.is_empty() || algorithm[0].oid().map_err(crypto)? != RSA { return Err(unsupported("This envelope does not use RSA PKCS#1 key transport")); }
        let private = RsaPrivateKey::from_pkcs8_der(id.key.pkcs8()).map_err(crypto)?;
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
