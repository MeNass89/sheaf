//! Acrobat's certificate security in the reader: Encrypt with Certificate (recipients chosen from
//! their certificates, permissions per recipient, what is encrypted), and opening such a file
//! directly with the user's digital ID, held to that recipient's permissions.
use super::*;
use crate::reader_doc::certificate as working;
use sheaf_app::store_file;
use sheaf_core::certificate_policy::{self, CertificatePolicy};
use sheaf_core::certificate_security::{self, Permissions, Recipient, Scope};
use sheaf_core::pdfcraft::engine::sign::{pkcs12, Certificate, SignError};
use std::cell::RefCell;
use std::path::Path;

pub(super) struct CertPanel {
    pub(super) recipients: Vec<Recipient>,
    /// Saved certificate security policies (Acrobat's Security Policies, public key kind).
    pub(super) policies: Vec<(PathBuf, CertificatePolicy)>,
    pub(super) status: String,
    pub(super) busy: bool,
    /// The scope a policy set, and a count bumped so the sheet's controls take it.
    pub(super) scope: i32,
    pub(super) seq: i32,
    pub(super) name: String,
}

/// The checkboxes of a recipient: Print, Copy, Edit, Comment, Fill forms, Change security.
pub(super) fn flags(p: Permissions) -> [bool; 6] {
    let change = p.0 & Permissions::CHANGE_SECURITY != 0;
    [p.allows(Permissions::PRINT), p.allows(Permissions::COPY), p.allows(Permissions::MODIFY), p.allows(Permissions::COMMENT), p.allows(Permissions::FILL_FORMS), change]
}

fn with_flag(p: Permissions, flag: usize, on: bool) -> Permissions {
    let mut f = flags(p);
    if let Some(slot) = f.get_mut(flag) { *slot = on; }
    // Unticking Change security keeps what the boxes showed (all of them).
    Permissions::new(f[0], f[1], f[2], f[3], f[4], f[5])
}

pub(super) fn name(c: &Certificate) -> String {
    c.subject.common_name().or(c.subject.email()).or(c.subject.organization()).unwrap_or("Unnamed certificate").to_string()
}

pub(super) fn detail(c: &Certificate) -> String {
    let issuer = c.issuer.common_name().or(c.issuer.organization()).unwrap_or("unknown issuer");
    match c.subject.email() {
        Some(mail) => format!("{mail} · issued by {issuer}"),
        None => format!("Issued by {issuer}"),
    }
}

/// A recipient added to the list: a new one reads only (Acrobat's default lets recipients
/// print and copy too; a reader-only start is the safer one to widen). Your own ID gets full
/// rights so you can reopen and change the copy.
fn add(recipients: &mut Vec<Recipient>, certificate: Certificate, permissions: Permissions) -> Result<String, String> {
    if recipients.iter().any(|r| r.certificate.raw == certificate.raw) { return Err(format!("{} is already a recipient", name(&certificate))); }
    if let Some(problem) = sheaf_core::certificate_security::recipient_problem(&certificate) { return Err(format!("{}: {problem}", name(&certificate))); }
    let added = format!("Added {}", name(&certificate));
    recipients.push(Recipient { certificate, permissions });
    Ok(added)
}

fn reader_default() -> Permissions { Permissions::new(true, true, false, false, true, false) }

fn status(id: i32, text: String) {
    update(id, |r, _| { if let Some(p) = r.cert_panel.as_mut() { p.status = text; } });
}

fn policy_dir() -> PathBuf { store_file::data_dir().join("certificate-policies") }

fn scope_of(index: i32) -> Scope { match index { 1 => Scope::ExceptMetadata, 2 => Scope::Attachments, _ => Scope::All } }

pub(super) fn scope_index(scope: Scope) -> i32 { match scope { Scope::All => 0, Scope::ExceptMetadata => 1, Scope::Attachments => 2 } }

/// A policy's recipients and scope fill the sheet, ready to apply or adjust.
fn use_policy(id: i32, policy: CertificatePolicy, how: &str) {
    update(id, |r, _| { if let Some(p) = r.cert_panel.as_mut() { p.status = format!("{how} {}: {} recipients", policy.name, policy.recipients.len()); p.recipients = policy.recipients; p.scope = scope_index(policy.scope); p.seq = p.seq.wrapping_add(1); p.name = policy.name; } });
}

pub fn cert_open(id: i32) {
    update(id, |r, _| r.cert_panel = Some(CertPanel { policies: certificate_policy::list(&policy_dir()), scope: 0, seq: 0, name: String::new(), recipients: Vec::new(), status: "Add each recipient's certificate, and your own digital ID to open the copy yourself.".into(), busy: false }));
}

pub fn cert_flag(id: i32, k: i32, flag: i32, on: bool) {
    update(id, |r, _| {
        if let Some(p) = r.cert_panel.as_mut() && let Some(rec) = usize::try_from(k).ok().and_then(|k| p.recipients.get_mut(k)) {
            rec.permissions = with_flag(rec.permissions, flag as usize, on);
        }
    });
}

pub fn cert_action(ui: &AppWindow, id: i32, action: i32, index: i32, password: &str) {
    if action == 0 { update(id, |r, _| r.cert_panel = None); return; }
    if update(id, |r, _| r.cert_panel.as_ref().is_none_or(|p| p.busy)).unwrap_or(true) { return; }
    match action {
        1 => {
            let d = crate::dialog::new(ui).set_title("Recipient certificate").add_filter("Certificate", &["cer", "crt", "pem", "der"]);
            crate::dialog::pick_file(ui, d, move |_, path| {
                let parsed = std::fs::read(&path).ok().and_then(|b| sheaf_core::pdfcraft::certificate_der(&b)).ok_or_else(|| format!("{} is not an X.509 certificate", file_name(&path)))
                    .and_then(|der| Certificate::parse(&der).map_err(|e| e.to_string()));
                let text = update(id, |r, _| {
                    let p = r.cert_panel.as_mut()?;
                    Some(match parsed.and_then(|c| add(&mut p.recipients, c, reader_default())) { Ok(t) | Err(t) => t })
                }).flatten();
                if let Some(text) = text { status(id, text); }
            });
        }
        2 => {
            let password = password.to_string();
            let d = crate::dialog::new(ui).set_title("Your digital ID").add_filter("Digital ID", &["p12", "pfx"]);
            crate::dialog::pick_file(ui, d, move |ui, path| {
                let opened = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| pkcs12::open(&b, &password).map_err(|e| match e {
                    SignError::WrongPassword => "Wrong password for this digital ID".to_string(),
                    e => format!("Not a usable digital ID: {e}"),
                }));
                if opened.is_ok() { crate::home::remember_digital_id(ui, &path); }
                let text = update(id, |r, _| {
                    let p = r.cert_panel.as_mut()?;
                    Some(match opened.and_then(|idn| add(&mut p.recipients, idn.certificate, Permissions::FULL)) { Ok(t) | Err(t) => t })
                }).flatten();
                if let Some(text) = text { status(id, text); }
            });
        }
        3 => { update(id, |r, _| { if let Some(p) = r.cert_panel.as_mut() && let Ok(k) = usize::try_from(index) && k < p.recipients.len() { let gone = p.recipients.remove(k); p.status = format!("Removed {}", name(&gone.certificate)); } }); }
        4 => save(ui, id, scope_of(index)),
        5 | 9 => {
            let Some(policy) = update(id, |r, _| r.cert_panel.as_ref().map(|p| CertificatePolicy { name: password.trim().to_string(), scope: scope_of(index), recipients: p.recipients.clone() })).flatten() else { return };
            if let Err(e) = policy.validate() { return status(id, e.to_string()); }
            if action == 9 {
                let d = crate::dialog::new(ui).set_title("Export certificate policy").set_file_name(format!("{}.json", policy.name)).add_filter("Sheaf certificate policy", &["json"]);
                return crate::dialog::save_file(ui, d, move |_, out| status(id, match policy.save(&out) { Ok(p) => format!("Exported {}", p.display()), Err(e) => format!("Policy not exported: {e}") }));
            }
            let saved = std::fs::create_dir_all(policy_dir()).map_err(sheaf_core::OpdfError::from).and_then(|()| {
                // Saving under a name already used replaces that policy, as Acrobat's Edit does.
                if let Some((old, _)) = certificate_policy::list(&policy_dir()).into_iter().find(|(_, p)| p.name.eq_ignore_ascii_case(&policy.name)) { std::fs::remove_file(old)?; }
                let file: String = policy.name.chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
                policy.save(&policy_dir().join(format!("{file}.json")))
            });
            let text = match saved { Ok(_) => format!("Saved policy {}", policy.name), Err(e) => format!("Policy not saved: {e}") };
            update(id, |r, _| { if let Some(p) = r.cert_panel.as_mut() { p.policies = certificate_policy::list(&policy_dir()); p.status = text; } });
        }
        6 => {
            if let Some(policy) = update(id, |r, _| r.cert_panel.as_ref().and_then(|p| p.policies.get(usize::try_from(index).ok()?).map(|x| x.1.clone()))).flatten() { use_policy(id, policy, "Using"); }
        }
        7 => {
            let Some((path, name)) = update(id, |r, _| r.cert_panel.as_ref().and_then(|p| p.policies.get(usize::try_from(index).ok()?).map(|x| (x.0.clone(), x.1.name.clone())))).flatten() else { return };
            let text = match std::fs::remove_file(&path) { Ok(()) => format!("Deleted policy {name}"), Err(e) => format!("Policy not deleted: {e}") };
            update(id, |r, _| { if let Some(p) = r.cert_panel.as_mut() { p.policies = certificate_policy::list(&policy_dir()); p.status = text; } });
        }
        8 => {
            let d = crate::dialog::new(ui).set_title("Import certificate policy").add_filter("Sheaf certificate policy", &["json"]);
            crate::dialog::pick_file(ui, d, move |_, path| match CertificatePolicy::load(&path) {
                Ok(policy) => use_policy(id, policy, "Imported (Save as policy to keep it)"),
                Err(e) => status(id, format!("Policy not imported: {e}")),
            });
        }
        _ => {}
    }
}

fn save(ui: &AppWindow, id: i32, scope: Scope) {
    if unsaved_name(id).is_some() { return status(id, "Save the PDF's edits first, then encrypt it.".into()); }
    let Some((input, recipients)) = update(id, |r, _| Some((Input { path: r.path.clone(), password: r.password() }, r.cert_panel.as_ref()?.recipients.clone()))).flatten() else { return };
    let d = crate::dialog::new(ui).set_title("Save encrypted copy").set_directory(crate::dest::parent_dir(&input.path)).set_file_name(format!("{} - encrypted.pdf", input.stem())).add_filter("PDF", &["pdf"]);
    crate::dialog::save_file(ui, d, move |ui, out| {
        let Some(()) = update(id, |r, _| { let p = r.cert_panel.as_mut()?; if p.busy { return None; } p.busy = true; p.status = "Encrypting…".into(); Some(()) }).flatten() else { return };
        // A certificate-encrypted file is encrypted from its decrypted working copy.
        let source = with(|a| a.tab(id).and_then(|t| t.viewer.as_ref().map(|v| v.input.clone()))).filter(|i| working::original(&i.path).is_some()).unwrap_or(input);
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let result = certificate_security::encrypt_for(&source, &recipients, scope, &out);
            let _ = weak.upgrade_in_event_loop(move |ui| {
                update(id, |r, _| { if let Some(p) = r.cert_panel.as_mut() { p.busy = false; p.status = match &result { Ok(path) => format!("Encrypted copy saved: {}", path.display()), Err(e) => format!("Encrypted copy not saved: {e}") }; } });
                if let Ok(path) = result { crate::flow::toast(&ui, &format!("Encrypted copy saved as {}", file_name(&path))); }
            });
        });
    });
}

thread_local! {
    /// The certificate-encrypted file the prompt is about, and the digital ID chosen for it.
    static ASK: RefCell<Option<(PathBuf, Option<PathBuf>)>> = const { RefCell::new(None) };
}

/// Ask for the digital ID that opens `path` (`error`: why the last try failed).
pub fn ask(ui: &AppWindow, path: PathBuf, error: &str) {
    let id = ASK.with(|a| a.borrow().as_ref().filter(|(p, _)| *p == path).and_then(|(_, id)| id.clone()))
        // A smart card's ID signs; opening with it is not done yet, so the first file is offered.
        .or_else(|| crate::home::prefs().digital_ids.into_iter().find(|p| !sheaf_core::pkcs11::is_reference(p)));
    ui.set_pw_id(id.as_deref().map(file_name).unwrap_or_default().into());
    ui.set_pw_file(file_name(&path).into());
    ASK.with(|a| *a.borrow_mut() = Some((path, id)));
    ui.set_pw_certificate(true);
    ui.set_pw_error(error.into());
    ui.set_pw_wrong(!error.is_empty());
    if !error.is_empty() { ui.set_pw_attempt(ui.get_pw_attempt() + 1); }
    ui.set_pw_visible(true);
}

/// The prompt's Choose…: another digital ID file, kept in Preferences ▸ Signatures.
pub fn choose_id(ui: &AppWindow) {
    let d = crate::dialog::new(ui).set_title("Your digital ID").add_filter("Digital ID", &["p12", "pfx"]);
    crate::dialog::pick_file(ui, d, |ui, p| {
        crate::home::remember_digital_id(ui, &p);
        ui.set_pw_id(file_name(&p).into());
        ASK.with(|a| if let Some((_, id)) = a.borrow_mut().as_mut() { *id = Some(p); });
    });
}

fn done(ui: &AppWindow) {
    ui.set_pw_visible(false);
    ui.set_pw_certificate(false);
    ui.set_pw_error("".into());
    ui.set_pw_wrong(false);
}

/// Unlock typed in the prompt: true when the prompt was about a certificate-encrypted file.
pub fn entered(ui: &AppWindow, password: &str) -> bool {
    let Some((path, id)) = ASK.with(|a| a.borrow().clone()) else { return false };
    let Some(id) = id else { ask(ui, path, "Choose your digital ID"); return true };
    let password = password.to_string();
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let result = std::fs::read(&id).map_err(|e| e.to_string()).and_then(|b| pkcs12::open(&b, &password).map_err(|e| match e {
            SignError::WrongPassword => "Wrong password for this digital ID".to_string(),
            e => format!("Not a usable digital ID: {e}"),
        })).and_then(|identity| working::open(&store_file::data_dir(), &path, &identity));
        let _ = weak.upgrade_in_event_loop(move |ui| match result {
            Ok((copy, permissions)) => {
                ASK.with(|a| *a.borrow_mut() = None);
                done(&ui);
                super::open_as(&ui, copy, None, &path);
                let denied = permissions.denied();
                if !denied.is_empty() { crate::flow::toast(&ui, &format!("Opened with your digital ID. Not allowed: {}", denied.join(", ").to_lowercase())); }
            }
            Err(e) => ask(&ui, path, &e),
        });
    });
    true
}

/// Skip: true when the prompt was about a certificate-encrypted file.
pub fn skipped(ui: &AppWindow) -> bool {
    if ASK.with(|a| a.borrow_mut().take()).is_none() { return false; }
    done(ui);
    true
}

/// The security row of Properties for a certificate-encrypted file opened here.
pub fn security_note(path: &Path) -> Option<String> {
    let p = working::permissions(path)?;
    let denied = p.denied();
    Some(if denied.is_empty() { "Certificate encryption (AES-256), all permissions".into() } else { format!("Certificate encryption (AES-256): no {}", denied.join(", no ").to_lowercase()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipient_checkboxes_map_to_their_envelope_permissions() {
        let reader = reader_default();
        assert_eq!(flags(reader), [true, true, false, false, true, false]);
        let editor = with_flag(reader, 2, true);
        assert!(editor.allows(Permissions::MODIFY) && editor.allows(Permissions::ASSEMBLE) && !editor.allows(Permissions::COMMENT));
        assert_eq!(with_flag(editor, 5, true), Permissions::FULL);
        // Unticking Change security keeps the other boxes as they showed.
        assert_eq!(flags(with_flag(Permissions::FULL, 5, false)), [true, true, true, true, true, false]);
        assert_eq!(flags(with_flag(reader, 0, false)), [false, true, false, false, true, false]);
    }
}
