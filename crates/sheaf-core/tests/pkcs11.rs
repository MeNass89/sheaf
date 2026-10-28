//! A real PKCS #11 token (SoftHSM, Ubuntu's `softhsm2`) signs a PDF: keys made on the token,
//! certificates issued by a software CA and stored next to them, as a smart card holds them.
//! Without SoftHSM the test says so and passes, except in CI, where it must run.
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, CertificateType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::types::AuthPin;
use sheaf_core::pages::Input;
use sheaf_core::pdfcraft::engine::sign::{der, Certificate, DigestAlg, Name, PrivateKey, PublicKey, Time};
use sheaf_core::pdfcraft::{engine::SignatureStatus, Doc};
use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
use std::path::{Path, PathBuf};

const PIN: &str = "4321";

fn softhsm() -> Option<PathBuf> {
    let found = std::env::var_os("SHEAF_PKCS11_MODULE").map(PathBuf::from).into_iter()
        .chain(["/usr/lib/softhsm/libsofthsm2.so", "/usr/lib/x86_64-linux-gnu/softhsm/libsofthsm2.so", "/opt/homebrew/lib/softhsm/libsofthsm2.so", "/usr/local/lib/softhsm/libsofthsm2.so"].map(PathBuf::from))
        .find(|p| p.exists());
    assert!(found.is_some() || std::env::var_os("CI").is_none(), "CI installs softhsm2: the token test must run there");
    found
}

/// A certificate for the token's public key `public`, issued by the software CA `ca`.
fn issue(ca: &PrivateKey, ca_name: &Name, holder: &str, public: &PublicKey, serial: u8) -> Certificate {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let algorithm = ca.signature_algorithm(DigestAlg::Sha256);
    // keyUsage digitalSignature and nonRepudiation, as a signing card's certificate.
    let usage = der::seq(&[&der::oid("2.5.29.15"), &der::boolean(true), &der::octets(&der::tlv(der::tag::BIT_STRING, &[6, 0xc0]))]);
    let tbs = der::seq(&[
        &der::explicit(0, &der::int(2)), &der::uint(&[serial]), &algorithm, &ca_name.raw,
        &der::seq(&[&Time::from_unix(now - 60).encode(), &Time::from_unix(now + 86400).encode()]),
        &Name::build(holder, "", "Sheaf token test", "", "").raw, &public.spki(),
        &der::explicit(3, &der::seq(&[&usage])),
    ]);
    let signature = ca.sign(DigestAlg::Sha256, &tbs).unwrap();
    Certificate::parse(&der::seq(&[&tbs, &algorithm, &der::bit_string(&signature)])).unwrap()
}

/// A key pair made on the token (private, sensitive, never extractable) under `id`, and its
/// public half as PdfCraft reads it.
fn generate(session: &Session, rsa: bool, id: &[u8]) -> PublicKey {
    let private = [Attribute::Token(true), Attribute::Private(true), Attribute::Sensitive(true), Attribute::Extractable(false), Attribute::Sign(true), Attribute::Id(id.to_vec())];
    if rsa {
        let public = [Attribute::Token(true), Attribute::Verify(true), Attribute::ModulusBits(2048.into()), Attribute::PublicExponent(vec![1, 0, 1]), Attribute::Id(id.to_vec())];
        let (handle, _) = session.generate_key_pair(&Mechanism::RsaPkcsKeyPairGen, &public, &private).unwrap();
        let (mut n, mut e) = (Vec::new(), Vec::new());
        for a in session.get_attributes(handle, &[AttributeType::Modulus, AttributeType::PublicExponent]).unwrap() {
            match a { Attribute::Modulus(v) => n = v, Attribute::PublicExponent(v) => e = v, _ => {} }
        }
        PublicKey::Rsa { n, e }
    } else {
        let public = [Attribute::Token(true), Attribute::Verify(true), Attribute::EcParams(der::oid("1.2.840.10045.3.1.7")), Attribute::Id(id.to_vec())];
        let (handle, _) = session.generate_key_pair(&Mechanism::EccKeyPairGen, &public, &private).unwrap();
        let point = session.get_attributes(handle, &[AttributeType::EcPoint]).unwrap().into_iter().find_map(|a| match a { Attribute::EcPoint(p) => Some(p), _ => None }).unwrap();
        // CKA_EC_POINT is the DER OCTET STRING around the SEC1 point.
        PublicKey::P256(der::Tlv::parse_all(&point).unwrap().expect(der::tag::OCTET_STRING, "EC point").unwrap().value.to_vec())
    }
}

fn store(session: &Session, cert: &Certificate, id: &[u8], label: &str) -> ObjectHandle {
    session.create_object(&[
        Attribute::Class(ObjectClass::CERTIFICATE), Attribute::CertificateType(CertificateType::X_509), Attribute::Token(true),
        Attribute::Value(cert.raw.clone()), Attribute::Id(id.to_vec()), Attribute::Label(label.as_bytes().to_vec()), Attribute::Subject(cert.subject.raw.clone()),
    ]).unwrap()
}

fn sign_with(reference: &str, pin: &str, out: &Path) -> sheaf_core::Result<sign::SignReport> {
    let invoice = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invoice.pdf");
    let digital = DigitalSpec { p12: reference.into(), password: pin.into(), reason: Some("Approved on the card".into()), location: None, certify: None };
    sign::sign(&Input::new(invoice), &SignSpec { page: 0, rect: [72.0, 72.0, 272.0, 132.0], graphic: Graphic::Typed("Ada Lovelace".into(), Default::default()), digital: Some(digital) }, out)
}

#[test]
fn a_digital_id_on_a_pkcs11_token_signs_a_pdf_with_its_pin() {
    let Some(module) = softhsm() else { return eprintln!("SoftHSM not installed (apt-get install softhsm2): token signing not tested") };
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join("tokens")).unwrap();
    let conf = t.path().join("softhsm2.conf");
    std::fs::write(&conf, format!("directories.tokendir = {}\nobjectstore.backend = file\nlog.level = ERROR\n", t.path().join("tokens").display())).unwrap();
    // SoftHSM reads its configuration when the module starts; this test binary starts it once.
    unsafe { std::env::set_var("SOFTHSM2_CONF", &conf) };

    // The card issuer's side: a blank token initialized, its PIN set, two keys made on it.
    let p11 = Pkcs11::new(&module).unwrap();
    p11.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK)).unwrap();
    let slot = p11.get_all_slots().unwrap()[0];
    p11.init_token(slot, &AuthPin::from("123456".to_string()), "Sheaf card").unwrap();
    let slot = p11.get_slots_with_initialized_token().unwrap()[0];
    let session = p11.open_rw_session(slot).unwrap();
    session.login(UserType::So, Some(&AuthPin::from("123456".to_string()))).unwrap();
    session.init_pin(&AuthPin::from(PIN.to_string())).unwrap();
    session.logout().unwrap();
    session.login(UserType::User, Some(&AuthPin::from(PIN.to_string()))).unwrap();
    let ca = PrivateKey::generate_p256().unwrap();
    let ca_name = Name::build("Sheaf Test Card CA", "", "Sheaf token test", "", "");
    for (rsa, id, holder, serial) in [(true, [1u8], "Ada Card RSA", 1u8), (false, [2u8], "Ada Card EC", 2u8)] {
        let public = generate(&session, rsa, &id);
        store(&session, &issue(&ca, &ca_name, holder, &public, serial), &id, holder);
    }
    // A key without its certificate is not an ID; a certificate is what Preferences lists.
    session.logout().unwrap();
    drop(session);

    // Preferences ▸ Signatures ▸ Add a smart card or token: the IDs listed without the PIN.
    let mut ids = sheaf_core::pkcs11::identities(&module).unwrap();
    ids.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(ids.iter().map(|i| (i.name.as_str(), i.token.as_str())).collect::<Vec<_>>(), [("Ada Card EC", "Sheaf card"), ("Ada Card RSA", "Sheaf card")]);
    assert_eq!(sheaf_core::digital_id::label(Path::new(&ids[1].reference)), "Ada Card RSA (Sheaf card)");

    for id in &ids {
        let out = t.path().join(format!("{}.pdf", id.name));
        let report = sign_with(&id.reference, PIN, &out).unwrap();
        assert_eq!(report.signer.as_deref(), Some(id.name.as_str()));
        let doc = Doc::open(&Input::new(&out)).unwrap();
        let signatures = &doc.document().signatures;
        // Intact, signed by the card's certificate; its CA is not trusted here, so Unknown.
        assert_eq!(signatures.len(), 1);
        assert!(signatures[0].signed && signatures[0].status == SignatureStatus::Unknown, "{}: {:?}", id.name, signatures[0].status);
        assert_eq!(signatures[0].reason.as_deref(), Some("Approved on the card"));
    }

    let wrong = sign_with(&ids[0].reference, "0000", &t.path().join("wrong.pdf")).unwrap_err();
    assert!(wrong.to_string().contains("Ada Card EC (Sheaf card): wrong PIN"), "{wrong}");
    assert!(!t.path().join("wrong.pdf").exists());
}
