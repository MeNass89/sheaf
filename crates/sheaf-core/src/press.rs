//! Acrobat's Save as PDF/X: local press exports with an explicit output condition.
//! Ghostscript performs conversion; our marker/font checks are not a full ISO validator.

use crate::{paths::atomic_write, pdfcraft::Doc, pages::Input, OpdfError, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// PDF/X variants supported by current Ghostscript; older installations may refuse X-1a/X-4.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Level { X1a, #[default] X3, X4 }

impl Level {
    pub fn label(self) -> &'static str { match self { Self::X1a => "PDF/X-1a:2001", Self::X3 => "PDF/X-3:2002", Self::X4 => "PDF/X-4" } }
    fn marker(self) -> &'static str { match self { Self::X1a => "PDF/X-1:2001", _ => self.label() } }
    fn pdf_version(self) -> &'static str { if self == Self::X4 { "1.6" } else { "1.3" } }
    fn switch(self) -> &'static str { match self { Self::X1a => "-dPDFX=1", Self::X3 => "-dPDFX=3", Self::X4 => "-dPDFX=4" } }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub level: Level,
    pub profile: PathBuf,
    pub condition: String,
}

/// A press export owns the exact unsaved document snapshot; later edits remain independent.
/// Capture on the document thread, then export on a separate worker (Acrobat's background jobs).
pub struct Snapshot {
    bytes: std::sync::Arc<Vec<u8>>,
    password: Option<String>,
}

impl Snapshot {
    pub fn capture(doc: &Doc) -> Result<Self> {
        if !doc.document().allows_modification() {
            return Err(OpdfError::Unsupported("This document's permissions do not allow a print-standard conversion".into()));
        }
        Ok(Self { bytes: doc.full_bytes()?, password: doc.document().password.clone() })
    }

    pub fn export(self, options: &Options, out: &Path) -> Result<PathBuf> {
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("snapshot.pdf");
        std::fs::write(&path, self.bytes.as_slice())?;
        let doc = Doc::open(&Input { path, password: self.password })?;
        to_pdfx3(&doc, options, out)
    }
}

/// A console Ghostscript on PATH, or an explicit local installation.
pub fn find_ghostscript() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHEAF_GHOSTSCRIPT").map(PathBuf::from).filter(|p| p.is_file()) { return Some(path); }
    let names: &[&str] = if cfg!(windows) { &["gswin64c.exe", "gswin32c.exe"] } else { &["gs"] };
    if let Some(path) = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).flat_map(|dir| names.iter().map(move |name| dir.join(name))).find(|p| p.is_file())
    }) { return Some(path); }
    #[cfg(target_os = "macos")]
    for name in ["/opt/homebrew/bin/gs", "/usr/local/bin/gs"] {
        if Path::new(name).is_file() { return Some(PathBuf::from(name)); }
    }
    None
}

/// Installed press profiles only: never substitute a generic CMYK profile for a named condition.
#[derive(Clone, Copy, Debug)]
pub enum Preset { Fogra39, Gracol, Swop }

impl Preset {
    pub fn label(self) -> &'static str { match self { Self::Fogra39 => "FOGRA39 coated", Self::Gracol => "GRACoL 2006 coated", Self::Swop => "SWOP 2006 coated" } }
    fn filenames(self) -> &'static [&'static str] {
        match self {
            Self::Fogra39 => &["FOGRA39L_coated.icc", "ISOcoated_v2_eci.icc", "CoatedFOGRA39.icc"],
            Self::Gracol => &["GRACoL2006_Coated1v2.icc", "CoatedGRACoL2006.icc"],
            Self::Swop => &["USWebCoatedSWOP.icc", "USWebCoatedSWOP.icm", "SWOP2006_Coated3v2.icc"],
        }
    }
}

pub fn installed_profile(preset: Preset) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = std::env::var_os("SHEAF_ICC_DIR") { roots.push(PathBuf::from(path)); }
    roots.extend([PathBuf::from("/usr/share/color/icc"), PathBuf::from("/usr/local/share/color/icc"), PathBuf::from("/Library/ColorSync/Profiles")]);
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.extend([home.join(".local/share/color/icc"), home.join("Library/ColorSync/Profiles")]);
    }
    if let Some(windows) = std::env::var_os("WINDIR") { roots.push(PathBuf::from(windows).join("System32/spool/drivers/color")); }
    find_profile(preset, &roots)
}

fn find_profile(preset: Preset, roots: &[PathBuf]) -> Option<PathBuf> {
    let mut pending: Vec<_> = roots.iter().map(|p| (p.clone(), 0)).collect();
    let mut remaining = 4096;
    while let Some((dir, depth)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            if remaining == 0 { return None; }
            remaining -= 1;
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() && depth < 3 { pending.push((entry.path(), depth + 1)); }
            if !kind.is_file() || !preset.filenames().iter().any(|n| entry.file_name().to_string_lossy().eq_ignore_ascii_case(n)) { continue; }
            let options = Options { level: Level::X3, profile: entry.path(), condition: preset.label().into() };
            if validate(&options).is_ok_and(|channels| channels == 4) { return Some(options.profile); }
        }
    }
    None
}

/// Reads the profile before spawning a converter. A display RGB profile is not a press condition.
pub fn validate(options: &Options) -> Result<u8> {
    if options.condition.trim().is_empty() || options.condition.chars().count() > 256 {
        return Err(OpdfError::Unsupported("Name the press output condition (1 to 256 characters)".into()));
    }
    if std::fs::metadata(&options.profile)?.len() > 16 * 1024 * 1024 {
        return Err(OpdfError::Unsupported("The press profile must be at most 16 MB".into()));
    }
    profile_channels(&std::fs::read(&options.profile)?)
}

fn profile_channels(bytes: &[u8]) -> Result<u8> {
    let valid = bytes.len() >= 128 && &bytes[36..40] == b"acsp" && &bytes[12..16] == b"prtr"
        && u32::from_be_bytes(bytes[..4].try_into().unwrap_or_default()) as usize == bytes.len();
    if valid {
        match &bytes[16..20] {
            b"CMYK" => return Ok(4),
            b"GRAY" => return Ok(1),
            _ => {}
        }
    }
    Err(OpdfError::Unsupported("Choose a valid CMYK or Gray output ICC profile for this press".into()))
}

/// Export the current working copy, without altering it, its encryption or its undo history.
pub fn to_pdfx3(doc: &Doc, options: &Options, out: &Path) -> Result<PathBuf> {
    let gs = find_ghostscript().ok_or_else(|| OpdfError::Unsupported("PDF/X export needs a local Ghostscript installation on PATH (or SHEAF_GHOSTSCRIPT)".into()))?;
    to_pdfx3_with(doc, options, out, &gs, Duration::from_secs(120))
}

/// An explicit offline runtime and time limit, also used by the integration checks.
pub fn to_pdfx3_with(doc: &Doc, options: &Options, out: &Path, gs: &Path, timeout: Duration) -> Result<PathBuf> {
    if !doc.document().allows_modification() {
        return Err(OpdfError::Unsupported("This document's permissions do not allow a print-standard conversion".into()));
    }
    let channels = validate(options)?;
    let tmp = tempfile::tempdir()?;
    let protected = tmp.path().join("source.pdf");
    std::fs::write(&protected, doc.full_bytes()?.as_slice())?;
    let mut copy = Doc::open(&Input { path: protected, password: doc.document().password.clone() })?;
    let source = copy.save_unprotected(&tmp.path().join("input.pdf"))?.0;
    drop(copy);
    let profile = tmp.path().join("press.icc");
    let bytes = std::fs::read(&options.profile)?;
    if profile_channels(&bytes)? != channels { return Err(OpdfError::Conversion("The press profile changed; choose it again".into())); }
    std::fs::write(&profile, &bytes)?;
    let definition = tmp.path().join("condition.ps");
    std::fs::write(&definition, prefix(&profile, &options.condition, channels, options.level))?;
    let rendered = tmp.path().join("press.pdf");
    let mut child = Command::new(gs)
        .args(["-dSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=pdfwrite", "-dAutoRotatePages=/None", "-dPDFACompatibilityPolicy=2"])
        .arg(options.level.switch())
        .arg(format!("-dCompatibilityLevel={}", options.level.pdf_version()))
        .arg(if channels == 4 { "-sColorConversionStrategy=CMYK" } else { "-sColorConversionStrategy=Gray" })
        // The press profile reaches the file through the PDF/X definition's OutputIntent (Ghostscript
        // 10's pdfwrite refuses -sOutputICCProfile: "undefined in .putdeviceprops").
        .arg(format!("--permit-file-read={}", profile.display()))
        .arg(format!("-sOutputFile={}", rendered.display()))
        .arg(&definition).arg(&source)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    // Ghostscript reports some failures on stdout, others on stderr; drain both with a bound.
    let stdout = drain_tail(child.stdout.take().expect("stdout is piped"));
    let stderr = drain_tail(child.stderr.take().expect("stderr is piped"));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(100)),
            polled => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OpdfError::Conversion(match polled {
                    Err(error) => error.to_string(),
                    _ => format!("PDF/X conversion timed out after {}s", timeout.as_secs()),
                }));
            }
        }
    };
    let mut tail = stdout.join().unwrap_or_default();
    tail.extend(stderr.join().unwrap_or_default());
    if !status.success() {
        let text = String::from_utf8_lossy(&tail);
        return Err(OpdfError::Conversion(format!("Ghostscript did not create PDF/X ({status}): {}", text.trim())));
    }
    check_written(&rendered, channels, &bytes, options.level)?;
    atomic_write(out, |part| { std::fs::copy(&rendered, part)?; Ok(()) })
}

fn drain_tail(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut tail = Vec::new();
        let mut chunk = [0; 4096];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 { break; }
            tail.extend_from_slice(&chunk[..n]);
            if tail.len() > 8192 { let _ = tail.drain(..tail.len() - 8192); }
        }
        tail
    })
}

fn ps_string(text: &str) -> String {
    text.as_bytes().iter().map(|b| format!("\\{b:03o}")).collect()
}

fn prefix(profile: &Path, condition: &str, channels: u8, level: Level) -> String {
    let profile = ps_string(&profile.to_string_lossy().replace('\\', "/"));
    let condition = ps_string(condition.trim());
    let marker = level.marker();
    let conformance = if level == Level::X1a { "/GTS_PDFXConformance (PDF/X-1a:2001)" } else { "" };
    format!(r#"%!
[ /GTS_PDFXVersion ({marker}) {conformance} /Trapped /False /DOCINFO pdfmark
/ICCProfile ({profile}) def
[ /_objdef {{SheafPressICC}} /type /stream /OBJ pdfmark
[ {{SheafPressICC}} << /N {channels} >> /PUT pdfmark
[ {{SheafPressICC}} ICCProfile (r) file /PUT pdfmark
[ /_objdef {{SheafPressIntent}} /type /dict /OBJ pdfmark
[ {{SheafPressIntent}} << /Type /OutputIntent /S /GTS_PDFX
  /DestOutputProfile {{SheafPressICC}} /OutputConditionIdentifier ({condition})
  /OutputCondition ({condition}) /RegistryName (http://www.color.org) >> /PUT pdfmark
[ {{Catalog}} << /OutputIntents [ {{SheafPressIntent}} ] >> /PUT pdfmark
"#)
}

/// Check the actual generated file, not just the child process's exit code.
fn check_written(path: &Path, channels: u8, expected_profile: &[u8], level: Level) -> Result<()> {
    let pdf = lopdf::Document::load(path)?;
    if pdf.is_encrypted() {
        return Err(OpdfError::Conversion("The PDF/X output is encrypted; no output saved".into()));
    }
    let info = pdf.trailer.get(b"Info").and_then(lopdf::Object::as_reference).and_then(|id| pdf.get_dictionary(id))?;
    let version = info.get(b"GTS_PDFXVersion").and_then(lopdf::Object::as_str)?;
    let intents = pdf.catalog()?.get(b"OutputIntents")?.as_array()?;
    let intent = intents.iter().filter_map(|obj| obj.as_reference().ok().and_then(|id| pdf.get_dictionary(id).ok()))
        .find(|dict| dict.get(b"S").is_ok_and(|v| v.as_name().is_ok_and(|n| n == b"GTS_PDFX")));
    let profile = intent.and_then(|dict| dict.get(b"DestOutputProfile").ok()).and_then(|obj| obj.as_reference().ok())
        .and_then(|id| pdf.get_object(id).ok()).and_then(|obj| obj.as_stream().ok());
    let conformance = level != Level::X1a || info.get(b"GTS_PDFXConformance").and_then(lopdf::Object::as_str).is_ok_and(|v| v == b"PDF/X-1a:2001");
    let valid = version == level.marker().as_bytes() && pdf.version == level.pdf_version() && conformance
        && profile.is_some_and(|s| s.dict.get(b"N").is_ok_and(|n| n.as_i64().is_ok_and(|n| n == i64::from(channels))));
    let same_profile = profile.and_then(|s| s.get_plain_content().ok()).is_some_and(|bytes| bytes == expected_profile);
    if !valid || !same_profile { return Err(OpdfError::Conversion(format!("Ghostscript output did not contain the requested {} markers and exact press ICC; no output saved. This Ghostscript version may not support the chosen level", level.label()))); }
    if level == Level::X1a && pdf.objects.values().any(non_device_colour) {
        return Err(OpdfError::Conversion("The PDF/X-1a output still declares an unsupported colour space; no output saved".into()));
    }
    let checked = Doc::open(&Input::new(path))?;
    let readiness = checked.document().print_readiness();
    if !readiness.fonts_not_embedded.is_empty() {
        return Err(OpdfError::Conversion("The PDF/X output still has unembedded fonts; no output saved".into()));
    }
    if readiness.findings.iter().any(|f| (level != Level::X4 && f.id == "transparency") || f.id == "resource-inventory-incomplete") {
        return Err(OpdfError::Conversion("The PDF/X output has forbidden transparency, or its resource inventory is incomplete; no output saved".into()));
    }
    Ok(())
}

fn non_device_colour(object: &lopdf::Object) -> bool {
    use lopdf::Object;
    match object {
        Object::Name(n) => matches!(n.as_slice(), b"DeviceRGB" | b"CalRGB" | b"Lab" | b"ICCBased" | b"DeviceN"),
        Object::Array(a) => a.iter().any(non_device_colour),
        Object::Dictionary(d) => d.iter().any(|(_, v)| non_device_colour(v)),
        Object::Stream(s) => s.dict.iter().any(|(_, v)| non_device_colour(v)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press_fixture(path: &Path, profile: &[u8], alpha: f32) {
        use lopdf::{dictionary, Object, Stream};
        let mut pdf = lopdf::Document::with_version("1.3");
        let pages = pdf.new_object_id();
        let page = pdf.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Resources" => dictionary! { "ExtGState" => dictionary! { "Alpha" => dictionary! { "ca" => alpha } } }
        });
        pdf.objects.insert(pages, dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![page.into()] }.into());
        let icc = pdf.add_object(Stream::new(dictionary! { "N" => 4 }, profile.to_vec()));
        let intent = pdf.add_object(dictionary! { "S" => "GTS_PDFX", "DestOutputProfile" => icc });
        let root = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages, "OutputIntents" => vec![intent.into()] });
        let info = pdf.add_object(dictionary! { "GTS_PDFXVersion" => Object::string_literal("PDF/X-3:2002"), "Title" => Object::string_literal("First edit") });
        pdf.trailer.set("Root", root);
        pdf.trailer.set("Info", info);
        pdf.save(path).unwrap();
    }

    #[test]
    fn press_levels_require_the_requested_markers_and_allow_transparency_only_in_x4() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("press.pdf");
        for level in [Level::X1a, Level::X3, Level::X4] {
            press_fixture(&path, b"chosen", if level == Level::X4 { 0.5 } else { 1.0 });
            assert!(check_written(&path, 4, b"chosen", if level == Level::X3 { Level::X4 } else { level }).is_err());
            let mut pdf = lopdf::Document::load(&path).unwrap();
            pdf.version = level.pdf_version().into();
            let id = pdf.trailer.get(b"Info").unwrap().as_reference().unwrap();
            let info = pdf.get_object_mut(id).unwrap().as_dict_mut().unwrap();
            info.set("GTS_PDFXVersion", lopdf::Object::string_literal(level.marker()));
            if level == Level::X1a { info.set("GTS_PDFXConformance", lopdf::Object::string_literal(level.label())); }
            pdf.save(&path).unwrap();
            check_written(&path, 4, b"chosen", level).unwrap();
            let definition = prefix(Path::new("press.icc"), "condition", 4, level);
            assert!(definition.contains(level.marker()));
        }
        assert!(non_device_colour(&lopdf::Object::Name(b"DeviceRGB".to_vec())));
        assert!(!non_device_colour(&lopdf::Object::Name(b"DeviceCMYK".to_vec())));
    }

    #[test]
    fn named_press_profiles_require_a_real_matching_installed_cmyk_profile() {
        let t = tempfile::tempdir().unwrap();
        let roots = vec![t.path().to_path_buf()];
        assert!(find_profile(Preset::Fogra39, &roots).is_none());
        let path = t.path().join("FOGRA39L_coated.icc");
        std::fs::write(&path, b"not a profile").unwrap();
        assert!(find_profile(Preset::Fogra39, &roots).is_none());
        let mut bytes = vec![0; 128];
        bytes[..4].copy_from_slice(&128u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"prtr"); bytes[16..20].copy_from_slice(b"CMYK"); bytes[36..40].copy_from_slice(b"acsp");
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(find_profile(Preset::Fogra39, &roots), Some(path));
        assert!(find_profile(Preset::Gracol, &roots).is_none());
    }

    #[test]
    fn press_snapshot_keeps_unsaved_changes_separate_from_later_edits() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("source.pdf");
        press_fixture(&path, b"test profile", 1.0);
        let mut doc = Doc::open(&Input::new(path)).unwrap();
        doc.apply(crate::pdfcraft::engine::Edit::SetInfo { key: "Title".into(), value: "Unsaved edit".into() }).unwrap();
        let snapshot = Snapshot::capture(&doc).unwrap();
        let before = snapshot.bytes.clone();
        doc.apply(crate::pdfcraft::engine::Edit::SetInfo { key: "Title".into(), value: "Later edit".into() }).unwrap();
        assert_eq!(snapshot.bytes.as_slice(), before.as_slice());
        let captured = lopdf::Document::load_mem(snapshot.bytes.as_slice()).unwrap();
        let info = captured.get_dictionary(captured.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();
        assert_eq!(info.get(b"Title").unwrap().as_str().unwrap(), b"Unsaved edit");
        assert_ne!(doc.full_bytes().unwrap().as_slice(), snapshot.bytes.as_slice());
    }

    #[test]
    fn written_press_checks_reject_substituted_icc_and_transparency_without_ghostscript() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("written.pdf");
        // Synthetic ICC bytes exercise the structural checker, not ICC colour science.
        press_fixture(&path, b"chosen profile", 1.0);
        assert!(check_written(&path, 4, b"different profile", Level::X3).unwrap_err().to_string().contains("exact press ICC"));
        check_written(&path, 4, b"chosen profile", Level::X3).unwrap();
        press_fixture(&path, b"chosen profile", 0.5);
        assert!(check_written(&path, 4, b"chosen profile", Level::X3).unwrap_err().to_string().contains("transparency"));
    }

    #[test]
    fn press_profiles_reject_rgb_and_postscript_strings_cannot_escape() {
        let mut icc = vec![0; 128];
        icc[..4].copy_from_slice(&128u32.to_be_bytes());
        icc[12..16].copy_from_slice(b"prtr");
        icc[16..20].copy_from_slice(b"CMYK");
        icc[36..40].copy_from_slice(b"acsp");
        assert_eq!(profile_channels(&icc).unwrap(), 4);
        icc[16..20].copy_from_slice(b"RGB ");
        assert!(profile_channels(&icc).is_err());
        assert!(profile_channels(b"bad").is_err());
        let encoded = ps_string("Press (A)\\\n€");
        assert!(!encoded.contains(['(', ')', '\n', '€']));
        assert_eq!(ps_string("A()"), "\\101\\050\\051");
    }
}
