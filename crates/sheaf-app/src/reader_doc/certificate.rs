//! Certificate-encrypted PDFs opened in place. Acrobat opens an Encrypt with Certificate file
//! with the user's digital ID and saves it still encrypted; Sheaf reads the decrypted working
//! copy from an owner-only folder, holds it to the recipient's permissions, and every save
//! writes the original again, encrypted for all its recipients.

use sheaf_core::certificate_security::{self, Opened, Permissions};
use sheaf_core::pdfcraft::engine::sign::DigitalId;
use sheaf_core::pdfcraft::Doc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

struct Working {
    original: PathBuf,
    opened: Opened,
}

/// Working copy → the encrypted file it stands for. The document thread and the window both ask.
static WORKING: LazyLock<Mutex<HashMap<PathBuf, Working>>> = LazyLock::new(Default::default);

fn folder(data: &Path) -> std::io::Result<PathBuf> {
    let dir = data.join("certificate-working");
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

/// Decrypt `original` with `identity` into a fresh working copy under `data` and remember it.
pub fn open(data: &Path, original: &Path, identity: &DigitalId) -> Result<(PathBuf, Permissions), String> {
    let dir = folder(data).map_err(|e| e.to_string())?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let name = original.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "document.pdf".into());
    let working = dir.join(format!("{}-{nanos}-{name}", std::process::id()));
    let opened = certificate_security::open(original, identity, &working).map_err(|e| e.to_string())?;
    let permissions = opened.permissions;
    let original = std::fs::canonicalize(original).unwrap_or_else(|_| original.to_path_buf());
    WORKING.lock().unwrap_or_else(|e| e.into_inner()).insert(working.clone(), Working { original, opened });
    Ok((working, permissions))
}

/// The encrypted file a working copy stands for (what the tab shows and saves to).
pub fn original(working: &Path) -> Option<PathBuf> {
    WORKING.lock().unwrap_or_else(|e| e.into_inner()).get(working).map(|w| w.original.clone())
}

pub fn permissions(working: &Path) -> Option<Permissions> {
    WORKING.lock().unwrap_or_else(|e| e.into_inner()).get(working).map(|w| w.opened.permissions)
}

/// The recipient's permissions held on the document just opened from `working`; a recipient
/// allowed to change security has every right, as the owner of a password file does.
pub fn restrict(doc: &mut Doc, working: &Path) {
    if let Some(p) = permissions(working).filter(|p| p.0 & Permissions::CHANGE_SECURITY == 0) {
        let id = doc.id();
        doc.session_mut().restrict(id, p.0 as i32);
    }
}

/// Save: the edits go into the working copy (the session reopens it), and `target` gets the
/// same document encrypted again with the original's key, so every recipient still opens it.
pub fn save(doc: &mut Doc, working: &Path, target: &Path) -> Result<(), String> {
    let opened = WORKING.lock().unwrap_or_else(|e| e.into_inner()).get(working).map(|w| w.opened.clone()).ok_or("The certificate working copy is gone")?;
    let id = doc.id();
    let bytes = doc.session().save_bytes(id).map_err(|e| e.to_string())?;
    doc.session().check_save(id, &bytes).map_err(|e| e.to_string())?;
    let sealed = certificate_security::reseal(&bytes, &opened).map_err(|e| e.to_string())?;
    super::replace(target, &sealed).map_err(|e| e.to_string())?;
    super::replace(working, &bytes).map_err(|e| e.to_string())?;
    sheaf_app::library::Library::new(&sheaf_app::store_file::data_dir()).record_saved(target).map_err(|e| e.to_string())?;
    doc.session_mut().mark_saved(id, bytes, Some(working.to_string_lossy().into_owned())).map_err(|e| e.to_string())?;
    let target = std::fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
    if let Some(w) = WORKING.lock().unwrap_or_else(|e| e.into_inner()).get_mut(working) {
        w.original = target;
    }
    Ok(())
}

/// The tab closed: the decrypted copy leaves the disk.
pub fn forget(working: &Path) {
    if WORKING.lock().unwrap_or_else(|e| e.into_inner()).remove(working).is_some() {
        let _ = std::fs::remove_file(working);
    }
}

/// Decrypted copies a crashed session left (another process's copies are kept while it runs a
/// day; ours are always current).
pub fn sweep(data: &Path) {
    let Ok(entries) = std::fs::read_dir(data.join("certificate-working")) else { return };
    let mine = format!("{}-", std::process::id());
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 86_400);
        if !name.starts_with(&mine) && (old || !alive(&name)) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Whether the process that wrote `name` still runs (Linux knows; elsewhere the age decides).
fn alive(name: &str) -> bool {
    let pid = name.split('-').next().unwrap_or_default();
    if cfg!(target_os = "linux") { Path::new("/proc").join(pid).exists() } else { true }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_core::certificate_security::{Recipient, Scope};
    use sheaf_core::pages::Input;
    use sheaf_core::pdfcraft::engine::{sign::{self, der, Certificate, Name, PrivateKey, Time}, Edit};

    // A signed RSA encryption certificate made in Rust, as in sheaf-core's certificate tests.
    fn identity(name: &str, serial: u8) -> DigitalId {
        let key = PrivateKey::generate_rsa(2048).unwrap();
        let name = Name::build(name, "", "Sheaf test", "", "");
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let algorithm = key.signature_algorithm(sign::DigestAlg::Sha256);
        let usage = der::seq(&[&der::oid("2.5.29.15"), &der::boolean(true), &der::octets(&der::tlv(der::tag::BIT_STRING, &[5, 0x20]))]);
        let tbs = der::seq(&[
            &der::explicit(0, &der::int(2)), &der::uint(&[serial]), &algorithm, &name.raw,
            &der::seq(&[&Time::from_unix(now - 60).encode(), &Time::from_unix(now + 86400).encode()]), &name.raw, &key.public_key().spki(),
            &der::explicit(3, &der::seq(&[&usage])),
        ]);
        let signature = key.sign(sign::DigestAlg::Sha256, &tbs).unwrap();
        let certificate = Certificate::parse(&der::seq(&[&tbs, &algorithm, &der::bit_string(&signature)])).unwrap();
        DigitalId { key, certificate, chain: vec![], friendly_name: None }
    }

    /// Acrobat opens a certificate-encrypted file in place and saves it still encrypted: an editor
    /// recipient's rotation reaches the original, which every recipient still opens; a reader-only
    /// recipient is held to their permissions.
    #[test]
    fn a_certificate_file_opens_in_place_saves_encrypted_for_every_recipient_and_holds_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sheaf-core/tests/fixtures/invoice.pdf");
        let (owner, editor, reader) = (identity("Owner", 1), identity("Editor", 2), identity("Reader", 3));
        let recipients = [
            Recipient { certificate: owner.certificate.clone(), permissions: Permissions::FULL },
            Recipient { certificate: editor.certificate.clone(), permissions: Permissions::new(true, true, true, true, true, false) },
            Recipient { certificate: reader.certificate.clone(), permissions: Permissions::new(false, false, false, false, false, false) },
        ];
        let original = certificate_security::encrypt_for(&Input::new(&source), &recipients, Scope::All, &dir.path().join("shared.pdf")).unwrap();

        let (copy, permissions) = open(&data, &original, &editor).unwrap();
        assert!(!permissions.allows(Permissions::CHANGE_SECURITY));
        assert_eq!(super::original(&copy).unwrap(), std::fs::canonicalize(&original).unwrap());
        let mut doc = Doc::open(&Input::new(&copy)).unwrap();
        restrict(&mut doc, &copy);
        doc.apply(Edit::RotatePages { pages: vec![0], degrees: 90 }).unwrap();
        save(&mut doc, &copy, &original).unwrap();
        assert!(certificate_security::is_certificate_encrypted(&original));
        forget(&copy);
        assert!(!copy.exists() && super::original(&copy).is_none());

        for who in [&owner, &editor] {
            let (copy, _) = open(&data, &original, who).unwrap();
            let pdf = lopdf::Document::load(&copy).unwrap();
            let page = pdf.get_pages()[&1];
            assert_eq!(pdf.get_dictionary(page).unwrap().get(b"Rotate").unwrap().as_i64().unwrap(), 90);
            forget(&copy);
        }

        let (copy, _) = open(&data, &original, &reader).unwrap();
        let mut doc = Doc::open(&Input::new(&copy)).unwrap();
        restrict(&mut doc, &copy);
        let d = doc.session().get(doc.id()).unwrap();
        assert!(!d.allows_printing() && !d.allows_copying());
        assert!(doc.apply(Edit::RotatePages { pages: vec![0], degrees: 90 }).is_err());
        assert!(save(&mut doc, &copy, &original).unwrap_err().contains("do not allow changing"));
        forget(&copy);
    }

    #[test]
    fn stale_working_copies_of_dead_sessions_are_swept_and_live_ones_kept() {
        let data = tempfile::tempdir().unwrap();
        let dir = folder(data.path()).unwrap();
        let ours = dir.join(format!("{}-1-a.pdf", std::process::id()));
        let dead = dir.join("999999999-1-b.pdf");
        std::fs::write(&ours, b"x").unwrap();
        std::fs::write(&dead, b"x").unwrap();
        sweep(data.path());
        assert!(ours.exists());
        assert_eq!(dead.exists(), !cfg!(target_os = "linux"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        }
    }
}
