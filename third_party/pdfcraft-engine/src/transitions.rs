//! Sheaf: page transitions for full screen (Acrobat's Page Transitions): the effect a page comes
//! in with (`/Trans`) and how long it stays before the next (`/Dur`).

use pdfcraft_cos::{Dict, Document, Object};

use crate::EditError;

/// The effects, as PDF names (ISO 32000-1 Table 162), in the order Sheaf offers them.
pub const STYLES: [&str; 11] = ["Fade", "Dissolve", "Wipe", "Push", "Cover", "Uncover", "Split", "Blinds", "Box", "Glitter", "Fly"];

/// Pages `pages` (0-based) come in with `style` (None: no effect) over `seconds`, and stay
/// `advance` seconds in full screen before the next (None: until turned).
pub fn set_transition(doc: &mut Document, pages: &[usize], style: Option<&str>, seconds: f64, advance: Option<f64>) -> Result<(), EditError> {
    if let Some(s) = style.filter(|s| !STYLES.contains(s)) {
        return Err(EditError::Invalid(format!("\"{s}\" is not a transition")));
    }
    if !(0.0..=60.0).contains(&seconds) || advance.is_some_and(|a| !(0.0..=3600.0).contains(&a)) {
        return Err(EditError::Invalid("a transition lasts 0 to 60 s, a page 0 to 3600 s".into()));
    }
    let refs = pdfcraft_annot::page_refs(doc).map_err(|e| EditError::Invalid(e.to_string()))?;
    for &p in pages {
        let r = *refs.get(p).ok_or_else(|| EditError::Invalid(format!("page {} does not exist", p + 1)))?;
        doc.update_dict(r, |d| {
            match style {
                Some(s) => {
                    let mut t = Dict::new();
                    t.set(b"Type".to_vec(), Object::name("Trans"));
                    t.set(b"S".to_vec(), Object::name(s));
                    t.set(b"D".to_vec(), Object::Real(seconds));
                    // Wipe, Push, Cover, Uncover and Fly come from the left; Split and Blinds run
                    // across; Box and Split open outward.
                    match s {
                        "Wipe" | "Push" | "Cover" | "Uncover" | "Fly" => t.set(b"Di".to_vec(), Object::Int(0)),
                        "Split" | "Blinds" => t.set(b"Dm".to_vec(), Object::name("H")),
                        _ => {}
                    }
                    if matches!(s, "Split" | "Box") {
                        t.set(b"M".to_vec(), Object::name("O"));
                    }
                    d.set(b"Trans".to_vec(), t);
                }
                None => {
                    d.remove(b"Trans");
                }
            }
            match advance {
                Some(a) => d.set(b"Dur".to_vec(), Object::Real(a)),
                None => {
                    d.remove(b"Dur");
                }
            }
        })
        .map_err(|e| EditError::Write(e.to_string()))?;
    }
    Ok(())
}

/// Each page's time in full screen before the next (`/Dur`), in seconds.
pub fn page_durations(doc: &Document) -> Vec<Option<f64>> {
    pdfcraft_annot::page_refs(doc)
        .unwrap_or_default()
        .into_iter()
        .map(|r| doc.get(r).as_dict().and_then(|d| d.get(b"Dur").and_then(Object::as_f64)).filter(|s| *s >= 0.0))
        .collect()
}

impl crate::Document {
    /// Whether the document is a PDF Portfolio (its catalog has `/Collection`): its attached files
    /// are what it holds, its pages only a cover.
    pub fn is_portfolio(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.cos.root().is_some_and(|r| e.cos.get(r).as_dict().is_some_and(|d| d.get(b"Collection").is_some())))
    }

    /// Each page's time in full screen before the next (`/Dur`), in seconds.
    pub fn page_durations(&self) -> Vec<Option<f64>> {
        self.editor.as_ref().map(|e| page_durations(&e.cos)).unwrap_or_default()
    }

    /// Each page's transition: its effect (a name of [`STYLES`], `/S`) and length in seconds (`/D`).
    pub fn page_transitions(&self) -> Vec<Option<(String, f64)>> {
        let Some(e) = self.editor.as_ref() else { return Vec::new() };
        let doc = &e.cos;
        pdfcraft_annot::page_refs(doc)
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                let t = doc.get(r).as_dict()?.get(b"Trans").map(|t| doc.resolve(t))?;
                let t = t.as_dict()?;
                let s = String::from_utf8_lossy(t.name(b"S")?).into_owned();
                let d = t.get(b"D").and_then(Object::as_f64).unwrap_or(1.0).clamp(0.0, 60.0);
                (s != "R").then_some((s, d))
            })
            .collect()
    }
}
