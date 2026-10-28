//! Acrobat's working display profile: convert rendered sRGB into a chosen monitor ICC.
//! The transform is built once per preference change and shared by page-render workers.

use moxcms::{ColorProfile, DataColorSpace, Layout, ProfileClass, Transform8BitExecutor, TransformOptions};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct DisplayTransform(Arc<Transform8BitExecutor>);

impl std::fmt::Debug for DisplayTransform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("DisplayTransform") }
}

impl DisplayTransform {
    pub fn load(path: &Path) -> Result<Self, String> {
        let length = std::fs::metadata(path).map_err(|e| format!("Display ICC: {e}"))?.len();
        if !(128..=16 * 1024 * 1024).contains(&length) { return Err("Choose an RGB display ICC profile of at most 16 MB".into()); }
        Self::from_bytes(&std::fs::read(path).map_err(|e| e.to_string())?)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 16 * 1024 * 1024 { return Err("The display ICC exceeds 16 MB".into()); }
        // The parser can reject a printer profile's tables before exposing its class.
        // Report the actionable monitor requirement from the ICC header first.
        if bytes.len() >= 128 && (&bytes[12..16] != b"mntr" || &bytes[16..20] != b"RGB ") {
            return Err("Choose an RGB monitor profile, rather than a printer or device-link profile".into());
        }
        let profile = ColorProfile::new_from_slice(bytes).map_err(|e| format!("Invalid display ICC: {e}"))?;
        if profile.color_space != DataColorSpace::Rgb || profile.profile_class != ProfileClass::DisplayDevice {
            return Err("Choose an RGB monitor profile, rather than a printer or device-link profile".into());
        }
        Self::for_profile(&profile)
    }

    pub(crate) fn for_profile(profile: &ColorProfile) -> Result<Self, String> {
        ColorProfile::new_srgb().create_transform_8bit(Layout::Rgba, profile, Layout::Rgba, TransformOptions::default())
            .map(Self).map_err(|e| format!("This display ICC cannot convert RGB: {e}"))
    }

    /// Preserve alpha: colour management changes the monitor values, never page opacity.
    pub fn apply(&self, rgba: &mut [u8]) -> Result<(), String> {
        if !rgba.len().is_multiple_of(4) { return Err("The display pixels are incomplete".into()); }
        let original = rgba.to_vec();
        self.0.transform(&original, rgba).map_err(|e| e.to_string())?;
        for (src, dst) in original.as_chunks::<4>().0.iter().zip(rgba.as_chunks_mut::<4>().0.iter_mut()) { dst[3] = src[3]; }
        Ok(())
    }
}

/// The profile the system gives the display (Acrobat's "Use display profile"): on Linux the X
/// root window's `_ICC_PROFILE` (set by colord, GNOME, KDE and xcalib, XWayland included), read
/// with `xprop`. None where none is set; macOS and Windows not read yet (sRGB, or a chosen ICC).
// sheaf-core/tests/display_color.rs includes this file without calling it.
#[cfg_attr(test, allow(dead_code))]
pub fn system_profile() -> Option<Vec<u8>> {
    if !cfg!(all(unix, not(target_os = "macos"))) {
        return None;
    }
    let out = std::process::Command::new("xprop").args(["-root", "-notype", "_ICC_PROFILE"]).stderr(std::process::Stdio::null()).output().ok()?;
    out.status.success().then(|| parse_xprop_icc(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// The bytes of `xprop -root -notype _ICC_PROFILE`'s answer ("_ICC_PROFILE = 0, 0, 12, 72, …").
pub fn parse_xprop_icc(text: &str) -> Option<Vec<u8>> {
    let (_, values) = text.split_once('=')?;
    let bytes: Option<Vec<u8>> = values.split(',').map(|v| v.trim().parse::<u8>().ok()).collect();
    bytes.filter(|b| b.len() >= 128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_icc_changes_monitor_values_and_preserves_alpha() {
        let profile = ColorProfile::new_display_p3().encode().unwrap();
        let transform = DisplayTransform::from_bytes(&profile).unwrap();
        let mut pixels = [255, 0, 0, 255, 0, 255, 0, 99, 255, 255, 255, 0];
        let original = pixels;
        transform.apply(&mut pixels).unwrap();
        assert_ne!(&pixels[..3], &original[..3]);
        assert!(pixels[0] > pixels[1] && pixels[1] > 0);
        assert_eq!([pixels[3], pixels[7], pixels[11]], [255, 99, 0]);
        assert_eq!(&pixels[8..11], &[255, 255, 255]);
        assert!(transform.apply(&mut [1, 2, 3]).is_err());
    }

    #[test]
    fn display_icc_refuses_printer_profiles_and_bad_files() {
        let mut profile = ColorProfile::new_srgb();
        profile.profile_class = ProfileClass::OutputDevice;
        assert!(DisplayTransform::from_bytes(&profile.encode().unwrap()).unwrap_err().contains("RGB monitor"));
        assert!(DisplayTransform::from_bytes(b"not an ICC").is_err());
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("monitor.icc");
        std::fs::write(&p, ColorProfile::new_display_p3().encode().unwrap()).unwrap();
        DisplayTransform::load(&p).unwrap();
        std::fs::write(&p, b"bad").unwrap();
        assert!(DisplayTransform::load(&p).is_err());
    }

    #[test]
    fn the_systems_display_profile_is_read_from_xprops_answer() {
        let icc = ColorProfile::new_display_p3().encode().unwrap();
        let text = format!("_ICC_PROFILE = {}\n", icc.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", "));
        assert_eq!(parse_xprop_icc(&text).unwrap(), icc);
        DisplayTransform::from_bytes(&parse_xprop_icc(&text).unwrap()).unwrap();
        assert!(parse_xprop_icc("_ICC_PROFILE:  not found.\n").is_none());
        assert!(parse_xprop_icc("_ICC_PROFILE = 1, 2, 3").is_none(), "too short for an ICC header");
        assert!(parse_xprop_icc("_ICC_PROFILE = 1, 300, 3").is_none());
    }
}
