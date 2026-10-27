//! PDF Expert's Edit Signature and Manage Signatures, backed by the system credential store.
use super::*;
use sheaf_app::prefs::KeptSignature;

pub(super) struct Manager {
    pub(super) selected: Option<(usize, KeptSignature)>,
    pub(super) seq: i32,
    pub(super) status: String,
}

pub fn signature_manager(id: i32) {
    let selected = crate::home::signatures().first().cloned().map(|s| (0, s));
    update(id, |r, _| r.signature_manager = Some(Manager { selected, seq: 0, status: "Saved in your system credential store. The first signature is active.".into() }));
}

fn move_mark(list: &mut Vec<KeptSignature>, index: usize, delta: i32) -> bool {
    let Some(target) = index.checked_add_signed(delta as isize).filter(|&target| target < list.len()) else { return false };
    if index >= list.len() || target == index { return false; }
    let mark = list.remove(index);
    list.insert(target, mark);
    true
}

fn replace_mark(list: &mut Vec<KeptSignature>, selected: Option<&(usize, KeptSignature)>, replacement: KeptSignature) -> Result<(), String> {
    if let Some((index, expected)) = selected {
        if list.get(*index) != Some(expected) { return Err("The signature list changed. Select the signature again before saving.".into()); }
        list[*index] = replacement;
    } else {
        if list.len() >= 4 { return Err("Four signatures are already saved. Delete one before adding another.".into()); }
        list.push(replacement);
    }
    Ok(())
}

fn message(id: i32, text: String) {
    update(id, |r, _| { if let Some(m) = r.signature_manager.as_mut() { m.status = text; } });
}

pub fn signature_manager_action(id: i32, action: i32, index: i32) {
    if action == 0 { update(id, |r, _| r.signature_manager = None); return; }
    let mut list = crate::home::signatures();
    if action == 6 {
        update(id, |r, _| { if let Some(m) = r.signature_manager.as_mut() { m.selected = None; m.seq = m.seq.wrapping_add(1); m.status = "Draw or type a new signature, then Save signature.".into(); } });
        return;
    }
    let Ok(index) = usize::try_from(index) else { return };
    if index >= list.len() { return; }
    if action == 1 {
        let mark = list[index].clone();
        update(id, |r, _| { if let Some(m) = r.signature_manager.as_mut() { m.selected = Some((index, mark)); m.seq = m.seq.wrapping_add(1); } });
        return;
    }
    let changed = match action {
        2 => move_mark(&mut list, index, -1),
        3 => move_mark(&mut list, index, 1),
        4 => { list.remove(index); true }
        5 => { let mark = list.remove(index); list.insert(0, mark); true }
        _ => false,
    };
    if !changed { return; }
    match crate::home::save_signature_list(list.clone()) {
        Ok(()) => { update(id, |r, _| { if let Some(m) = r.signature_manager.as_mut() { m.selected = list.first().cloned().map(|s| (0, s)); m.seq = m.seq.wrapping_add(1); m.status = "Signature list saved securely.".into(); } }); }
        Err(error) => message(id, format!("The signature list was not saved: {error}")),
    }
}

pub fn signature_manager_save(id: i32, commands: &str, name: &str, font: i32, ink: i32, pen: i32) {
    let Some(selected) = update(id, |r, _| r.signature_manager.as_ref().map(|m| m.selected.clone())).flatten() else { return };
    let strokes = crate::sign_ui::pad_strokes(commands, 320.0, 120.0);
    let name = name.trim();
    if strokes.is_empty() && name.is_empty() { return message(id, "Draw or type a signature first.".into()); }
    if name.chars().count() > sheaf_core::sign::MAX_TYPED_CHARS { return message(id, format!("Use at most {} characters in a typed signature.", sheaf_core::sign::MAX_TYPED_CHARS)); }
    let replacement = (strokes, (!name.is_empty()).then(|| name.to_string()), (ink.clamp(0, 2) as u8, pen.clamp(0, 2) as u8), font.clamp(0, 3) as u8);
    let mut list = crate::home::signatures();
    let index = selected.as_ref().map_or(list.len(), |(index, _)| *index);
    let result = replace_mark(&mut list, selected.as_ref(), replacement).and_then(|()| crate::home::save_signature_list(list.clone()));
    match result {
        Ok(()) => { update(id, |r, _| { if let Some(m) = r.signature_manager.as_mut() { m.selected = Some((index, list[index].clone())); m.seq = m.seq.wrapping_add(1); m.status = "Signature saved securely.".into(); } }); }
        Err(error) => message(id, error),
    }
}

pub(super) fn commands(mark: &KeptSignature) -> String {
    let mut commands = String::new();
    for stroke in &mark.0 {
        for (index, [x, y]) in stroke.iter().enumerate() {
            commands.push_str(&format!("{} {:.6} {:.6} ", if index == 0 { "M" } else { "L" }, x * 320.0, 120.0 - y * 320.0));
        }
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mark(name: &str) -> KeptSignature { (Vec::new(), Some(name.into()), (0, 1), 0) }
    #[test]
    fn signature_edits_preserve_position_and_reject_a_stale_selection() {
        let mut list = vec![mark("One"), mark("Two")];
        let selection = (1, list[1].clone());
        replace_mark(&mut list, Some(&selection), mark("Edited")).unwrap();
        assert_eq!(list, [mark("One"), mark("Edited")]);
        assert!(replace_mark(&mut list, Some(&selection), mark("Stale")).is_err());
        assert!(move_mark(&mut list, 1, -1));
        assert_eq!(list[0], mark("Edited"));
        assert!(!move_mark(&mut list, 0, -1));
        assert!(!move_mark(&mut list, 1, 1));
    }
    #[test]
    fn saved_drawing_can_be_edited_without_changing_its_normalized_points() {
        let mark = (vec![vec![[0.125, 0.25], [0.75, 0.125]], vec![[0.25, 0.375], [0.875, 0.25]]], None, (1, 2), 0);
        assert_eq!(crate::sign_ui::pad_strokes(&commands(&mark), 320.0, 120.0), mark.0);
    }
}
