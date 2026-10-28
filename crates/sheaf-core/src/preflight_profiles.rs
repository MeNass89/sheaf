//! Acrobat's named Preflight profiles: several target rules in one read-only inspection.
//! These checks describe readiness for a target, never certify the complete PDF/X standard.

use super::{measured_report, validate, Profile};
use crate::{pdfcraft::Doc, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset { Custom, DigitalPress, PdfX1a, PdfX4 }

impl Preset {
    pub fn label(self) -> &'static str {
        match self { Self::Custom => "Custom print thresholds", Self::DigitalPress => "Digital press", Self::PdfX1a => "PDF/X-1a readiness", Self::PdfX4 => "PDF/X-4 readiness" }
    }
}

pub fn report_with_preset(doc: &Doc, name: &str, checked_at: &str, profile: Profile, preset: Preset) -> Result<String> {
    validate(profile)?;
    let readiness = doc.document().print_readiness_with(profile);
    let mut text = measured_report(name, checked_at, profile, &readiness);
    if preset == Preset::Custom { return Ok(text); }
    text.push_str(&format!("\nTarget profile: {}\nTarget checks (not full ISO validation):\n", preset.label()));
    let mut fails = Vec::new();
    for font in &readiness.fonts_not_embedded { fails.push(format!("Font {font} is not embedded")); }
    if !readiness.pages.iter().all(|p| p.trim_set) {
        fails.push("Every page needs an explicit TrimBox for this profile".into());
    }
    for finding in &readiness.findings {
        if finding.id == "resource-inventory-incomplete" || (preset == Preset::PdfX1a && finding.id == "transparency") {
            fails.push(finding.message.clone());
        }
    }
    if doc.was_encrypted() {
        fails.push("The document is encrypted; press standards forbid password protection".into());
        fails.push("The output-intent bytes could not be checked in the encrypted snapshot".into());
    } else {
        let bytes = doc.full_bytes()?;
        let pdf = lopdf::Document::load_mem(bytes.as_slice())?;
        if !press_intent(&pdf) { fails.push("An embedded CMYK or Gray press ICC output intent is required".into()); }
        if preset == Preset::PdfX1a && pdf.objects.values().any(crate::press::non_device_colour) {
            fails.push("RGB, calibrated, ICCBased or DeviceN colour declarations need conversion for this conservative X-1a profile".into());
        }
    }
    let target_fails = fails.len();
    for fail in fails { text.push_str(&format!("- [fail] {fail}\n")); }
    let measured = readiness.findings.iter().filter(|f| f.severity == "fail").count();
    let warnings = readiness.findings.iter().filter(|f| f.severity == "warn").count();
    text.push_str(&format!("Profile result: {} failing checks; {warnings} warnings.\n", target_fails + measured));
    text.push_str("Not checked by target profiles: painted ink coverage, separations, overprint simulation, complete PDF/X conformance.\n");
    Ok(text)
}

fn press_intent(pdf: &lopdf::Document) -> bool {
    let Ok(cat) = pdf.catalog() else { return false };
    let Some(intents) = cat.get(b"OutputIntents").ok().and_then(|v| pdf.dereference(v).ok()).and_then(|(_, v)| v.as_array().ok()) else { return false };
    intents.iter().any(|intent| {
        let Some(dict) = pdf.dereference(intent).ok().and_then(|(_, v)| v.as_dict().ok()) else { return false };
        if !dict.get(b"S").is_ok_and(|v| v.as_name().is_ok_and(|v| v == b"GTS_PDFX")) { return false; }
        let Some(stream) = dict.get(b"DestOutputProfile").ok().and_then(|v| pdf.dereference(v).ok()).and_then(|(_, v)| v.as_stream().ok()) else { return false };
        stream.get_plain_content().ok().is_some_and(|b| crate::press::profile_channels(&b).is_ok())
    })
}
