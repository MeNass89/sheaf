use sheaf_core::{certificate_security::{self, Scope}, pages::Input, pdfcraft::{Doc, engine::sign::{self, der, Certificate, DigitalId, Name, PrivateKey, Time}}};
use lopdf::{dictionary, Object, Stream};
use std::path::{Path, PathBuf};

// A real, signed RSA encryption certificate, generated in Rust without an outside program.
fn identity(name: &str, serial: u8) -> DigitalId { identity_with_usage(name, serial, 0x20) }

fn identity_with_usage(name: &str, serial: u8, usage: u8) -> DigitalId {
    let key = PrivateKey::generate_rsa(2048).unwrap();
    let name = Name::build(name, "", "Sheaf test", "", "");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let from = Time::from_unix(now - 60);
    let until = Time::from_unix(now + 86400);
    let algorithm = key.signature_algorithm(sign::DigestAlg::Sha256);
    let usage = der::seq(&[&der::oid("2.5.29.15"), &der::boolean(true), &der::octets(&der::tlv(der::tag::BIT_STRING, &[usage.trailing_zeros() as u8, usage]))]);
    let tbs = der::seq(&[
        &der::explicit(0, &der::int(2)), &der::uint(&[serial]), &algorithm, &name.raw,
        &der::seq(&[&from.encode(), &until.encode()]), &name.raw, &key.public_key().spki(),
        &der::explicit(3, &der::seq(&[&usage])),
    ]);
    let signature = key.sign(sign::DigestAlg::Sha256, &tbs).unwrap();
    let certificate = Certificate::parse(&der::seq(&[&tbs, &algorithm, &der::bit_string(&signature)])).unwrap();
    DigitalId { key, certificate, chain: vec![], friendly_name: None }
}

fn source(dir: &Path) -> PathBuf {
    let invoice = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf");
    let mut pdf = lopdf::Document::load(invoice).unwrap();
    let metadata = pdf.add_object(Stream::new(dictionary! { "Type" => "Metadata", "Subtype" => "XML" }, b"<x:xmpmeta>private metadata sentinel</x:xmpmeta>".to_vec()));
    let attachment = pdf.add_object(Stream::new(dictionary! { "Type" => "EmbeddedFile" }, b"private attachment sentinel".to_vec()));
    let specification = pdf.add_object(dictionary! { "Type" => "Filespec", "F" => Object::string_literal("private.txt"), "EF" => dictionary! { "F" => attachment } });
    let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let catalog = pdf.get_dictionary_mut(root).unwrap();
    catalog.set("Metadata", metadata);
    catalog.set("Names", dictionary! { "EmbeddedFiles" => dictionary! { "Names" => vec![Object::string_literal("private.txt"), specification.into()] } });
    let path = dir.join("source.pdf");
    pdf.save(&path).unwrap();
    path
}

#[test]
fn certificate_encryption_reopens_for_each_recipient_and_preserves_every_content_scope() {
    let dir = tempfile::tempdir().unwrap();
    let source = source(dir.path());
    let alice = identity("Alice", 1);
    let bob = identity("Bob", 2);
    let stranger = identity("Other", 3);
    let original = lopdf::Document::load(&source).unwrap();
    let original_text = original.extract_text(&[1]).unwrap();
    for scope in [Scope::All, Scope::ExceptMetadata, Scope::Attachments] {
        let encrypted = certificate_security::encrypt(&Input::new(&source), &[alice.certificate.clone(), bob.certificate.clone()], scope, &dir.path().join("encrypted.pdf")).unwrap();
        let bytes = std::fs::read(&encrypted).unwrap();
        assert!(!bytes.windows(b"private attachment sentinel".len()).any(|s| s == b"private attachment sentinel"));
        assert_eq!(bytes.windows(b"private metadata sentinel".len()).any(|s| s == b"private metadata sentinel"), scope != Scope::All);
        let denied = dir.path().join("denied.pdf");
        assert!(certificate_security::unlock(&encrypted, &stranger, &denied).unwrap_err().to_string().contains("not an authorized recipient"));
        assert!(!denied.exists());
        for recipient in [&alice, &bob] {
            let output = certificate_security::unlock(&encrypted, recipient, &dir.path().join("unlocked.pdf")).unwrap();
            let reopened = lopdf::Document::load(&output).unwrap();
            assert!(!reopened.is_encrypted());
            assert_eq!(reopened.extract_text(&[1]).unwrap(), original_text);
            let root = reopened.trailer.get(b"Root").unwrap().as_reference().unwrap();
            let catalog = reopened.get_dictionary(root).unwrap();
            let metadata = catalog.get(b"Metadata").unwrap().as_reference().unwrap();
            assert_eq!(reopened.get_object(metadata).unwrap().as_stream().unwrap().content, b"<x:xmpmeta>private metadata sentinel</x:xmpmeta>");
            assert!(reopened.objects.values().any(|o| o.as_stream().is_ok_and(|s| s.content == b"private attachment sentinel")));
            assert_eq!(Doc::open(&Input::new(&output)).unwrap().document().info.pages.len(), 1);
        }
    }
}

#[test]
fn certificate_encryption_refuses_wrong_usage_invalid_inputs_and_does_not_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let source = source(dir.path());
    let id = identity("Alice", 4);
    let signing_only = identity_with_usage("Signing only", 5, 0x80);
    let out = dir.path().join("encrypted.pdf");
    assert!(certificate_security::encrypt(&Input::new(&source), &[], Scope::All, &out).is_err());
    assert!(certificate_security::encrypt(&Input::new(&source), &[signing_only.certificate], Scope::All, &out).unwrap_err().to_string().contains("key encipherment"));
    assert!(!out.exists());
    std::fs::write(&out, b"keep existing output").unwrap();
    let written = certificate_security::encrypt(&Input::new(&source), &[id.certificate.clone(), id.certificate.clone()], Scope::All, &out).unwrap();
    assert_ne!(written, out);
    assert_eq!(std::fs::read(&out).unwrap(), b"keep existing output");
    assert!(certificate_security::unlock(&written, &id, &dir.path().join("unlocked.pdf")).is_ok());
}
