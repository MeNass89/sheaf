//! Acrobat's Preflight report: print checks on the current document, without changing it.

use crate::pdfcraft::Doc;
use crate::{paths::atomic_write, OpdfError, Result};

pub use crate::pdfcraft::engine::printcheck::Profile;
#[path = "preflight_profiles.rs"]
mod profiles;
pub use profiles::{Preset, report_with_preset};
use std::path::{Path, PathBuf};

/// A report with its profile and source identified, suitable for keeping beside a print job.
/// The structural checks deliberately do not certify PDF/X or simulate ink separations.
pub fn report(doc: &Doc, name: &str, checked_at: &str) -> String {
    report_with_profile(doc, name, checked_at, Profile::default()).expect("the default profile is valid")
}

/// Reject a profile whose comparisons could silently pass a job with bad measurements.
pub fn validate(profile: Profile) -> Result<()> {
    if !profile.fail_ppi.is_finite() || !profile.warn_ppi.is_finite() || !profile.min_bleed_mm.is_finite()
        || !(1.0..=9600.0).contains(&profile.fail_ppi) || !(profile.fail_ppi..=9600.0).contains(&profile.warn_ppi)
        || !(0.0..=50.0).contains(&profile.min_bleed_mm)
    {
        return Err(OpdfError::Unsupported("Use image thresholds from 1 to 9600 ppi (fail ≤ target), and bleed from 0 to 50 mm".into()));
    }
    Ok(())
}

/// A measured report with the exact thresholds kept beside the findings, as Acrobat's audit trail.
pub fn report_with_profile(doc: &Doc, name: &str, checked_at: &str, profile: Profile) -> Result<String> {
    validate(profile)?;
    let readiness = doc.document().print_readiness_with(profile);
    Ok(measured_report(name, checked_at, profile, &readiness))
}

fn measured_report(name: &str, checked_at: &str, profile: Profile, readiness: &crate::pdfcraft::engine::printcheck::PrintReadiness) -> String {
    let profile_name = if profile == Profile::default() { "Close-up print" }
        else if profile == (Profile { fail_ppi: 72.0, warn_ppi: 150.0, min_bleed_mm: 3.0 }) { "Large format" }
        else { "Custom print thresholds" };
    format!(
        "Sheaf print preflight\nDocument: {name}\nChecked: {checked_at}\nProfile: {profile_name}\nImage thresholds: fail below {} ppi; warn below {} ppi\nBleed: warn below {} mm on any trim edge\n\n{}",
        profile.fail_ppi, profile.warn_ppi, profile.min_bleed_mm, readiness.text()
    )
}

/// Write a complete report or leave no file. An existing report gets a free neighbouring name.
pub fn save_report(doc: &Doc, name: &str, checked_at: &str, out: &Path) -> Result<PathBuf> {
    let text = report(doc, name, checked_at);
    atomic_write(out, |part| {
        std::fs::write(part, text.as_bytes())?;
        Ok(())
    })
}
