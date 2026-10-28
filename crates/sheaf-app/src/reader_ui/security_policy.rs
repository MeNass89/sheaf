//! Acrobat's Security Policies: local named settings, exchange, and protected copies.
use super::*;
use sheaf_app::{security_policies, store_file};
use sheaf_core::security::{ProtectPermissions, ProtectionOptions, PrintQuality, ChangeLevel, EncryptionAlgorithm};
use sheaf_core::security_policy::SecurityPolicy;

pub(super) struct PolicyPanel {
    pub(super) policies: Vec<SecurityPolicy>,
    pub(super) draft: SecurityPolicy,
    pub(super) status: String,
    pub(super) busy: bool,
    pub(super) seq: i32,
    pub(super) remembered: bool,
}

fn blank() -> SecurityPolicy { SecurityPolicy::new("New policy".into(), ProtectPermissions::default(), true).expect("valid policy defaults") }

pub fn policy_open(id: i32) {
    let loaded = security_policies::load(&store_file::data_dir());
    let (policies, status) = match loaded {
        Ok(policies) => (policies, "Save settings here. You can remember passwords locally for later copies.".into()),
        Err(error) => (Vec::new(), format!("Cannot read saved policies: {error}")),
    };
    let mut panel = PolicyPanel { draft: policies.first().cloned().unwrap_or_else(blank), policies, status, busy: false, seq: 0, remembered: false };
    refresh_password_state(&mut panel);
    update(id, |r, _| r.policy_panel = Some(panel));
}

fn refresh_password_state(panel: &mut PolicyPanel) {
    match security_policies::saved_passwords(&store_file::data_dir(), &panel.draft) {
        Ok(saved) => panel.remembered = saved.is_some(),
        Err(error) => { panel.remembered = false; panel.status.push_str(&format!(" · Saved passwords unavailable: {error}")); }
    }
}

fn status(id: i32, text: String) {
    update(id, |r, _| { if let Some(panel) = r.policy_panel.as_mut() { panel.status = text; } });
}

pub fn policy_action(ui: &AppWindow, id: i32, action: i32, index: i32) {
    if action == 0 { update(id, |r, _| r.policy_panel = None); return; }
    if update(id, |r, _| r.policy_panel.as_ref().is_none_or(|p| p.busy)).unwrap_or(true) { return; }
    match action {
        1 => { update(id, |r, _| { if let Some(p) = r.policy_panel.as_mut() { p.draft = blank(); p.remembered = false; p.seq = p.seq.wrapping_add(1); p.status = "New policy. Choose settings, then Save policy.".into(); } }); }
        2 => {
            let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Import security policy")).add_filter("Sheaf security policy", &["json"]);
            crate::dialog::pick_file(ui, d, move |_, path| {
                match SecurityPolicy::load(&path) {
                    Ok(policy) => { update(id, |r, _| { if let Some(p) = r.policy_panel.as_mut() { p.draft = policy; p.remembered = false; p.seq = p.seq.wrapping_add(1); p.status = "Imported for review. Save policy to keep it in your library.".into(); } }); }
                    Err(error) => status(id, format!("Policy not imported: {error}")),
                }
            });
        }
        3 => {
            let Some(policy) = update(id, |r, _| r.policy_panel.as_ref().map(|p| p.draft.clone())).flatten() else { return };
            let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Export security policy")).set_file_name("security-policy.json").add_filter("Sheaf security policy", &["json"]);
            crate::dialog::save_file(ui, d, move |_, out| {
                status(id, match policy.save(&out) { Ok(path) => format!("Exported {}", path.display()), Err(e) => format!("Policy not exported: {e}") });
            });
        }
        4 => {
            let Some(name) = update(id, |r, _| Some(r.policy_panel.as_ref()?.policies.get(usize::try_from(index).ok()?)?.name.clone())).flatten() else { return };
            let result = security_policies::remove(&store_file::data_dir(), &name).and_then(|()| security_policies::load(&store_file::data_dir()));
            match result {
                Ok(policies) => { update(id, |r, _| { if let Some(p) = r.policy_panel.as_mut() { p.policies = policies; if p.draft.name == name { p.remembered = false; p.seq = p.seq.wrapping_add(1); } p.status = format!("Deleted {name}"); } }); }
                Err(error) => status(id, format!("Policy not deleted: {error}")),
            }
        }
        5 => { update(id, |r, _| { if let Some(p) = r.policy_panel.as_mut() && let Ok(k) = usize::try_from(index) && let Some(policy) = p.policies.get(k) { p.draft = policy.clone(); p.seq = p.seq.wrapping_add(1); p.status = "Saved policy selected.".into(); refresh_password_state(p); } }); }
        6 => {
            let Some(name) = update(id, |r, _| r.policy_panel.as_ref().map(|p| p.draft.name.clone())).flatten() else { return };
            match security_policies::forget_passwords(&store_file::data_dir(), &name) {
                Ok(()) => { update(id, |r, _| { if let Some(p) = r.policy_panel.as_mut() { p.remembered = false; p.seq = p.seq.wrapping_add(1); p.status = "Saved passwords forgotten. Policy settings are kept.".into(); } }); }
                Err(error) => status(id, format!("Passwords not forgotten: {error}")),
            }
        }
        _ => {}
    }
}

pub fn policy_save(id: i32, name: &str, flags: [bool; 6], choices: [i32; 3]) {
    if update(id, |r, _| r.policy_panel.as_ref().is_none_or(|p| p.busy)).unwrap_or(true) { return; }
    let result = SecurityPolicy::new(name.into(), ProtectPermissions { printing: flags[0], copying: flags[1], editing: flags[2], commenting: flags[3], filling_forms: flags[4] }, flags[5])
        .map_err(|e| e.to_string()).and_then(|mut policy| {
            policy.options = choices_to_options(choices)?;
            security_policies::save(&store_file::data_dir(), &policy)?;
            Ok((policy, security_policies::load(&store_file::data_dir())?))
        });
    match result {
        Ok((policy, policies)) => { update(id, |r, _| { if let Some(p) = r.policy_panel.as_mut() { p.status = format!("Saved {}", policy.name); p.draft = policy; p.policies = policies; p.seq = p.seq.wrapping_add(1); refresh_password_state(p); } }); }
        Err(error) => status(id, format!("Policy not saved: {error}")),
    }
}

fn choices_to_options(choices: [i32; 3]) -> Result<ProtectionOptions, String> {
    let [quality, changes, algorithm] = choices;
    Ok(ProtectionOptions {
        print_quality: match quality { 0 => PrintQuality::High, 1 => PrintQuality::Low, _ => return Err("Choose a print quality".into()) },
        changes: match changes { 0 => ChangeLevel::Any, 1 => ChangeLevel::None, 2 => ChangeLevel::Assemble, 3 => ChangeLevel::FillForms, 4 => ChangeLevel::CommentForms, _ => return Err("Choose the allowed changes".into()) },
        algorithm: match algorithm { 0 => EncryptionAlgorithm::Aes256, 1 => EncryptionAlgorithm::Aes128, _ => return Err("Choose AES-256 or AES-128 compatibility".into()) },
    })
}

fn checked_passwords(user: &str, confirmation: &str, owner: &str, owner_confirmation: &str, permissions: ProtectPermissions) -> Result<(), String> {
    if user != confirmation || owner != owner_confirmation { return Err("Passwords do not match their confirmation".into()); }
    if user.is_empty() && owner.is_empty() { return Err("Enter an open password or an owner password".into()); }
    if permissions != ProtectPermissions::default() && (owner.is_empty() || owner == user) { return Err("Restrictions need a separate owner password".into()); }
    Ok(())
}

fn passwords_for_apply(dir: &std::path::Path, policy: &SecurityPolicy, user: &str, confirmation: &str, owner: &str, owner_confirmation: &str) -> Result<security_policies::Passwords, String> {
    let passwords = if [user, confirmation, owner, owner_confirmation].iter().all(|s| s.is_empty()) {
        security_policies::saved_passwords(dir, policy)?.ok_or("Enter passwords or select a policy with remembered passwords")?
    } else {
        checked_passwords(user, confirmation, owner, owner_confirmation, policy.permissions)?;
        security_policies::Passwords { user: user.into(), owner: owner.into() }
    };
    checked_passwords(&passwords.user, &passwords.user, &passwords.owner, &passwords.owner, policy.permissions)?;
    if (policy.options.print_quality == PrintQuality::Low || policy.options.changes != ChangeLevel::Any)
        && (passwords.owner.is_empty() || passwords.owner == passwords.user) { return Err("Restrictions need a separate owner password".into()); }
    Ok(passwords)
}

pub fn policy_apply(ui: &AppWindow, id: i32, user: &str, confirmation: &str, owner: &str, owner_confirmation: &str, remember: bool) {
    let Some(policy) = update(id, |r, _| r.policy_panel.as_ref().map(|p| p.draft.clone())).flatten() else { return };
    let passwords = match passwords_for_apply(&store_file::data_dir(), &policy, user, confirmation, owner, owner_confirmation) {
        Ok(passwords) => passwords,
        Err(error) => return status(id, error),
    };
    if unsaved_name(id).is_some() { return status(id, "Save the PDF's edits first, then apply the policy.".into()); }
    let Some(input) = update(id, |r, _| (r.snap.read_only.is_empty()).then(|| Input { path: r.path.clone(), password: r.password() })).flatten() else {
        return status(id, "Open this PDF with permission to change its security first.".into());
    };

    let d = crate::dialog::new(ui).set_title(sheaf_app::i18n::tr("Save protected copy")).set_directory(crate::dest::parent_dir(&input.path)).set_file_name(format!("{} - protected.pdf", input.stem())).add_filter("PDF", &["pdf"]);
    crate::dialog::save_file(ui, d, move |ui, out| {
        if unsaved_name(id).is_some() { return status(id, "The PDF changed. Save it, then apply the policy again.".into()); }
        let Some(()) = update(id, |r, _| { let p = r.policy_panel.as_mut()?; if p.busy { return None; } p.busy = true; p.status = "Saving a protected copy…".into(); Some(()) }).flatten() else { return };
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let result = policy.apply(&input, &passwords.user, (!passwords.owner.is_empty()).then_some(passwords.owner.as_str()), &out);
            let mut password_notice = String::new();
            let mut remembered = false;
            if result.is_ok() {
                let saved = if remember {
                    security_policies::remember_passwords(&store_file::data_dir(), &policy, passwords)
                } else {
                    security_policies::forget_passwords(&store_file::data_dir(), &policy.name)
                };
                match saved {
                    Ok(()) => { remembered = remember; if remember { password_notice = " · Passwords remembered locally".into(); } }
                    Err(error) => password_notice = format!(" · Password storage unchanged: {error}"),
                }
            }
            let _ = weak.upgrade_in_event_loop(move |ui| {
                update(id, |r, _| { if let Some(p) = r.policy_panel.as_mut() { p.busy = false; if result.is_ok() { p.seq = p.seq.wrapping_add(1); p.remembered = remembered; } p.status = match &result { Ok(path) => format!("Protected copy saved: {}{password_notice}", path.display()), Err(error) => format!("Protected copy not saved: {error}") }; } });
                if let Ok(path) = result { crate::flow::toast(&ui, &sheaf_app::i18n::trf("Protected copy saved as {}", &[&file_name(&path)])); }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_saved_policy_applies_without_retyping_passwords_and_edits_require_new_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = SecurityPolicy::new("Review".into(), ProtectPermissions { copying: false, ..Default::default() }, true).unwrap();
        security_policies::save(dir.path(), &policy).unwrap();
        security_policies::remember_passwords(dir.path(), &policy, security_policies::Passwords { user: "open".into(), owner: "owner".into() }).unwrap();
        let reused = passwords_for_apply(dir.path(), &policy, "", "", "", "").unwrap();
        assert_eq!(reused.user, "open");
        assert_eq!(reused.owner, "owner");
        assert!(passwords_for_apply(dir.path(), &policy, "new", "wrong", "owner", "owner").is_err());
        policy.encrypt_metadata = false;
        assert!(passwords_for_apply(dir.path(), &policy, "", "", "", "").is_err());
        security_policies::forget_passwords(dir.path(), &policy.name).unwrap();
        assert!(passwords_for_apply(dir.path(), &policy, "", "", "", "").is_err());
    }
    #[test]
    fn applying_a_policy_requires_confirmed_passwords_and_a_distinct_owner_for_restrictions() {
        let all = ProtectPermissions::default();
        assert!(checked_passwords("open", "wrong", "", "", all).is_err());
        assert!(checked_passwords("", "", "", "", all).is_err());
        assert!(checked_passwords("open", "open", "", "", all).is_ok());
        let restricted = ProtectPermissions { copying: false, ..all };
        assert!(checked_passwords("open", "open", "open", "open", restricted).is_err());
        assert!(checked_passwords("open", "open", "owner", "owner", restricted).is_ok());
        assert!(checked_passwords("", "", "owner", "owner", restricted).is_ok());
    }
    #[test]
    fn policy_choice_indexes_map_to_exact_print_change_and_algorithm_settings() {
        assert_eq!(choices_to_options([0, 0, 0]).unwrap(), ProtectionOptions::default());
        let options = choices_to_options([1, 3, 1]).unwrap();
        assert_eq!(options.print_quality, PrintQuality::Low);
        assert_eq!(options.changes, ChangeLevel::FillForms);
        assert_eq!(options.algorithm, EncryptionAlgorithm::Aes128);
        assert!(choices_to_options([0, 5, 0]).is_err());
    }

}
