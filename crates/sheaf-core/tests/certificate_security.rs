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

// Acrobat's Encrypt with Certificate sets permissions per recipient and opens the file in place:
// each recipient reads with their own permissions, and a saved change keeps every recipient's access.
#[test]
fn certificate_recipients_keep_their_own_permissions_and_saved_changes_reopen_for_all() {
    use certificate_security::{Permissions, Recipient};
    let dir = tempfile::tempdir().unwrap();
    let source = source(dir.path());
    let owner = identity("Owner", 6);
    let reviewer = identity("Reviewer", 7);
    let reader = identity("Reader", 8);
    let review = Permissions::new(true, false, true, true, true, false);
    let read_only = Permissions::new(false, false, false, false, false, false);
    let recipients = [
        Recipient { certificate: owner.certificate.clone(), permissions: Permissions::FULL },
        Recipient { certificate: reviewer.certificate.clone(), permissions: review },
        Recipient { certificate: reader.certificate.clone(), permissions: read_only },
    ];
    let encrypted = certificate_security::encrypt_for(&Input::new(&source), &recipients, Scope::All, &dir.path().join("encrypted.pdf")).unwrap();
    assert!(certificate_security::is_certificate_encrypted(&encrypted));
    assert!(!certificate_security::is_certificate_encrypted(&source));
    let crypt = lopdf::Document::load(&encrypted).unwrap();
    let dict = crypt.get_dictionary(crypt.trailer.get(b"Encrypt").unwrap().as_reference().unwrap()).unwrap();
    // One envelope per permission set, as Acrobat writes them.
    assert_eq!(dict.get(b"CF").unwrap().as_dict().unwrap().get(b"DefaultCryptFilter").unwrap().as_dict().unwrap().get(b"Recipients").unwrap().as_array().unwrap().len(), 3);

    let working = dir.path().join("working.pdf");
    let opened = certificate_security::open(&encrypted, &reader, &working).unwrap();
    assert_eq!(opened.permissions.denied(), ["Printing", "Copying", "Editing", "Commenting", "Form filling", "Changing security"]);
    assert_eq!(lopdf::Document::load(&working).unwrap().extract_text(&[1]).unwrap(), lopdf::Document::load(&source).unwrap().extract_text(&[1]).unwrap());
    assert!(certificate_security::reseal(&std::fs::read(&working).unwrap(), &opened).unwrap_err().to_string().contains("do not allow changing"));
    assert!(certificate_security::unlock(&encrypted, &reader, &dir.path().join("plain.pdf")).unwrap_err().to_string().contains("not allowed to remove"));

    let opened = certificate_security::open(&encrypted, &reviewer, &working).unwrap();
    assert_eq!(opened.permissions.denied(), ["Copying", "Changing security"]);
    // The reviewer changes the title and saves: the file stays encrypted for all three.
    let mut plain = lopdf::Document::load(&working).unwrap();
    let info = plain.add_object(dictionary! { "Title" => Object::string_literal("Reviewed sentinel") });
    plain.trailer.set("Info", info);
    let mut bytes = Vec::new();
    plain.save_to(&mut bytes).unwrap();
    let saved = dir.path().join("saved.pdf");
    std::fs::write(&saved, certificate_security::reseal(&bytes, &opened).unwrap()).unwrap();
    let raw = std::fs::read(&saved).unwrap();
    assert!(!raw.windows(17).any(|w| w == b"Reviewed sentinel"));
    for (who, denied) in [(&owner, 0), (&reviewer, 2), (&reader, 6)] {
        let again = dir.path().join("again.pdf");
        let reopened = certificate_security::open(&saved, who, &again).unwrap();
        assert_eq!(reopened.permissions.denied().len(), denied);
        let doc = lopdf::Document::load(&again).unwrap();
        let info = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        assert_eq!(doc.get_dictionary(info).unwrap().get(b"Title").unwrap().as_str().unwrap(), b"Reviewed sentinel");
    }
    assert!(certificate_security::open(&saved, &identity("Other", 9), &dir.path().join("x.pdf")).unwrap_err().to_string().contains("not an authorized recipient"));
    // Attachments-only encryption has one envelope string, so one permission set.
    assert!(certificate_security::encrypt_for(&Input::new(&source), &recipients, Scope::Attachments, &dir.path().join("a.pdf")).unwrap_err().to_string().contains("same permissions"));
}

// Acrobat's certificate security policies: recipients, their permissions and the scope are saved,
// exchanged and applied again; only public certificates are stored, and a damaged file is refused.
#[test]
fn a_certificate_policy_round_trips_and_applies_with_each_recipients_permissions() {
    use certificate_security::{Permissions, Recipient};
    use sheaf_core::certificate_policy::{self, CertificatePolicy};
    let dir = tempfile::tempdir().unwrap();
    let source = source(dir.path());
    let (me, colleague) = (identity("Me", 10), identity("Colleague", 11));
    let read_only = Permissions::new(true, false, false, false, false, false);
    let policy = CertificatePolicy { name: "Team".into(), scope: Scope::ExceptMetadata, recipients: vec![
        Recipient { certificate: me.certificate.clone(), permissions: Permissions::FULL },
        Recipient { certificate: colleague.certificate.clone(), permissions: read_only },
    ] };
    let policies = dir.path().join("policies");
    std::fs::create_dir(&policies).unwrap();
    let saved = policy.save(&policies.join("team.json")).unwrap();
    let text = std::fs::read_to_string(&saved).unwrap();
    assert!(text.contains("except-metadata") && !text.to_lowercase().contains("private"));
    let listed = certificate_policy::list(&policies);
    assert_eq!(listed.len(), 1);
    let loaded = &listed[0].1;
    assert_eq!(loaded.name, "Team");
    assert_eq!(loaded.recipients[1].certificate, colleague.certificate);
    assert_eq!(loaded.recipients[1].permissions, read_only);
    let encrypted = loaded.apply(&Input::new(&source), &dir.path().join("team.pdf")).unwrap();
    let opened = certificate_security::open(&encrypted, &colleague, &dir.path().join("w.pdf")).unwrap();
    assert_eq!(opened.permissions, read_only);
    assert!(certificate_security::open(&encrypted, &me, &dir.path().join("w2.pdf")).unwrap().permissions.denied().is_empty());
    std::fs::write(dir.path().join("bad.json"), text.replacen("\"certificate\": \"30", "\"certificate\": \"31", 1)).unwrap();
    assert!(CertificatePolicy::load(&dir.path().join("bad.json")).is_err());
    assert!(CertificatePolicy { recipients: vec![], ..loaded.clone() }.save(&dir.path().join("empty.json")).is_err());
}
