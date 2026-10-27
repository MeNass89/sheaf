//! Extra scripts from installed Tesseract language packs (iLovePDF's OCR language list).
//! Nothing is downloaded: only languages reported by the local executable are offered.

use super::eng::{Line, OcrError, Turn, Word};
use std::collections::HashSet;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(super) const LANGUAGES: &[(&str, &str, &str)] = &[
    ("zh", "Chinese (Simplified)", "chi_sim"),
    ("zh-Hant", "Chinese (Traditional)", "chi_tra"),
    ("ja", "Japanese", "jpn"),
    ("ko", "Korean", "kor"),
    ("zh-vert", "Chinese (Simplified, vertical)", "chi_sim_vert"),
    ("zh-Hant-vert", "Chinese (Traditional, vertical)", "chi_tra_vert"),
    ("ja-vert", "Japanese (vertical)", "jpn_vert"),
    ("ko-vert", "Korean (vertical)", "kor_vert"),
    ("ar", "Arabic", "ara"), ("he", "Hebrew", "heb"),
    ("ru", "Russian", "rus"), ("uk", "Ukrainian", "ukr"),
    ("el", "Greek", "ell"), ("hi", "Hindi", "hin"),
    ("th", "Thai", "tha"), ("vi", "Vietnamese", "vie"),
];

// ISO language codes and traineddata names differ; mixed Latin/non-Latin pages need both.
const LATIN_PACKS: &[(&str, &str)] = &[
    ("en", "eng"), ("fr", "fra"), ("de", "deu"), ("es", "spa"), ("it", "ita"),
    ("pt", "por"), ("nl", "nld"), ("pl", "pol"), ("cs", "ces"), ("sk", "slk"),
    ("hr", "hrv"), ("sl", "slv"), ("da", "dan"), ("no", "nor"), ("sv", "swe"),
    ("is", "isl"), ("et", "est"), ("lt", "lit"), ("hu", "hun"), ("sq", "sqi"),
    ("cy", "cym"), ("ga", "gle"), ("tr", "tur"), ("id", "ind"), ("ms", "msa"),
    ("af", "afr"), ("sw", "swa"), ("tl", "tgl"), ("la", "lat"),
];

pub(super) struct Language { pub code: String, pub name: String, model: String }

fn valid_model(model: &str) -> bool {
    !model.is_empty() && !model.starts_with('/') && model != "osd" && model != "equ"
        && model.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '/')
}

fn offered(packs: &HashSet<String>) -> Vec<Language> {
    let mut out: Vec<Language> = LANGUAGES.iter().filter(|(_, _, model)| packs.contains(*model))
        .map(|(code, name, model)| Language { code: (*code).into(), name: (*name).into(), model: (*model).into() }).collect();
    for model in packs.iter().filter(|model| valid_model(model)) {
        if LANGUAGES.iter().any(|(_, _, m)| *m == model.as_str()) || LATIN_PACKS.iter().any(|(_, m)| *m == model.as_str()) { continue; }
        // Do not hide installed packs merely because Sheaf has no translated display name.
        out.push(Language { code: format!("local-{model}"), name: format!("Installed pack: {model}"), model: model.clone() });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub(super) fn languages() -> &'static [Language] {
    static LANGUAGES: OnceLock<Vec<Language>> = OnceLock::new();
    LANGUAGES.get_or_init(|| offered(installed()))
}

pub(super) fn available() -> bool { installed().iter().any(|m| valid_model(m)) }

fn selection_models(selection: &str, languages: &[Language], packs: &HashSet<String>) -> Result<(String, bool), String> {
    let mut models = Vec::new();
    let mut orientations = HashSet::new();
    for code in selection.split('+') {
        let model = languages.iter().find(|l| l.code == code).map(|l| l.model.as_str())
            .or_else(|| LATIN_PACKS.iter().find(|(c, _)| *c == code).map(|(_, model)| *model))
            .filter(|model| packs.contains(*model))
            .ok_or_else(|| format!("Mixed-script OCR needs the installed Tesseract pack for {code}. Install it locally or select another language."))?;
        orientations.insert(model.ends_with("_vert"));
        if !models.contains(&model) { models.push(model); }
    }
    if orientations.len() > 1 { return Err("Recognize horizontal and vertical language packs in separate OCR jobs".into()); }
    Ok((models.join("+"), orientations.contains(&true)))
}

pub(super) fn validate_selection(selection: &str) -> Result<(), String> {
    selection_models(selection, languages(), installed()).map(|_| ())
}

/// iLovePDF's OCR Languages Folder: portable offline packs travel with the installation.
/// The explicit folder wins; an absent portable folder leaves system Tesseract unchanged.
fn data_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHEAF_TESSDATA") { return Some(PathBuf::from(path)); }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() && let Some(dir) = exe.parent() {
        candidates.extend(dirs_beside_exe(dir));
    }
    #[cfg(debug_assertions)]
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/tessdata"));
    candidates.into_iter().find(|path| has_language_packs(path))
}

fn dirs_beside_exe(dir: &Path) -> [PathBuf; 3] {
    [dir.join("tessdata"), dir.join("../Resources/tessdata"), dir.join("../share/sheaf/tessdata")]
}

fn has_language_packs(path: &Path) -> bool {
    std::fs::read_dir(path).is_ok_and(|entries| entries.flatten().any(|e| e.path().extension().is_some_and(|ext| ext == "traineddata")))
}

fn command() -> Command {
    let mut command = Command::new("tesseract");
    if let Some(dir) = data_dir() { command.arg("--tessdata-dir").arg(dir); }
    command
}

fn installed() -> &'static HashSet<String> {
    static INSTALLED: OnceLock<HashSet<String>> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        command().arg("--list-langs").output().ok().filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::trim).map(str::to_string).collect()).unwrap_or_default()
    })
}

pub(super) fn recognize(pixels: &[u8], width: u32, height: u32, code: &str) -> Result<Vec<Line>, OcrError> {
    let (model, vertical) = selection_models(code, languages(), installed()).map_err(OcrError::Recognize)?;
    let run = || -> Result<Vec<Line>, String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let image = dir.path().join("page.png");
        image::save_buffer(&image, pixels, width, height, image::ColorType::Rgba8).map_err(|e| e.to_string())?;
        let mut command = command();
        command.arg(&image).args(["stdout", "-l", model.as_str()]);
        if vertical { command.args(["--psm", "5"]); }
        // A portable language folder may contain only traineddata, without configs/tsv.
        // Request TSV directly instead of relying on that system configuration file.
        let output = command.args(["-c", "tessedit_create_tsv=1"]).output().map_err(|e| format!("Tesseract: {e}"))?;
        if !output.status.success() { return Err(format!("Tesseract: {}", String::from_utf8_lossy(&output.stderr).trim())); }
        parse_tsv(&String::from_utf8_lossy(&output.stdout), width, height, vertical)
    };
    run().map_err(OcrError::Recognize)
}

fn parse_tsv(tsv: &str, width: u32, height: u32, vertical: bool) -> Result<Vec<Line>, String> {
    let mut lines: Vec<Line> = Vec::new();
    let mut previous = String::new();
    if !tsv.lines().next().is_some_and(|s| s.starts_with("level\tpage_num\t")) { return Err("Invalid Tesseract TSV header".into()); }
    for row in tsv.lines().skip(1) {
        let columns: Vec<&str> = row.splitn(12, '\t').collect();
        if columns.first() != Some(&"5") { continue; }
        if columns.len() != 12 { return Err("Incomplete Tesseract word".into()); }
        if columns[11].trim().is_empty() { continue; }
        let number = |i: usize| columns[i].parse::<f32>().map_err(|_| "Invalid Tesseract word coordinates".to_string());
        let (x, y, w, h) = (number(6)?, number(7)?, number(8)?, number(9)?);
        if ![x, y, w, h].iter().all(|v| v.is_finite()) || x < 0.0 || y < 0.0 || w <= 0.0 || h <= 0.0 || x + w > width as f32 || y + h > height as f32 { return Err("Tesseract word outside the page".into()); }
        let confidence = number(10)?;
        if !confidence.is_finite() || !(0.0..=100.0).contains(&confidence) { return Err("Invalid Tesseract word confidence".into()); }
        let key = columns[1..5].join("/");
        if key != previous || lines.is_empty() { lines.push(Line::default()); previous = key; }
        if let Some(line) = lines.last_mut() {
            line.words.push(Word { confidence: Some(confidence / 100.0), text: columns[11].trim().into(), rect: [x, y, x + w, y + h], turn: if vertical { Turn::Down } else { Turn::Upright }, ..Default::default() });
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";

    #[test]
    fn every_installed_language_pack_is_offered_but_orientation_data_is_not() {
        let packs = ["eng", "jpn", "tam", "script/Devanagari", "osd", "equ", "../bad"].into_iter().map(str::to_string).collect();
        let offered = offered(&packs);
        assert!(offered.iter().any(|l| l.code == "ja"));
        assert!(offered.iter().any(|l| l.code == "local-tam"));
        assert!(offered.iter().any(|l| l.code == "local-script/Devanagari"));
        assert_eq!(offered.len(), 3);
    }

    #[test]
    fn mixed_scripts_use_all_selected_packs_and_refuse_missing_or_conflicting_packs() {
        let mut packs: HashSet<String> = ["eng", "jpn", "jpn_vert"].into_iter().map(str::to_string).collect();
        let offered = offered(&packs);
        assert_eq!(selection_models("en+ja+en", &offered, &packs).unwrap(), ("eng+jpn".into(), false));
        assert_eq!(selection_models("ja-vert", &offered, &packs).unwrap(), ("jpn_vert".into(), true));
        assert!(selection_models("en+ja-vert", &offered, &packs).is_err());
        packs.remove("eng");
        assert!(selection_models("en+ja", &offered, &packs).unwrap_err().contains("for en"));
    }

    #[test]
    fn local_ocr_keeps_cjk_words_lines_and_vertical_geometry() {
        let tsv = format!("{HEADER}5\t1\t1\t1\t1\t1\t10\t20\t30\t12\t91\t日本語\n5\t1\t1\t1\t2\t1\t50\t60\t15\t40\t88\t中文\n");
        let lines = parse_tsv(&tsv, 100, 120, true).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].words[0].text, "日本語");
        assert_eq!(lines[0].words[0].confidence, Some(0.91));
        assert_eq!(lines[1].words[0].rect, [50.0, 60.0, 65.0, 100.0]);
        assert_eq!(lines[0].words[0].turn, Turn::Down);
        assert_eq!(parse_tsv(&tsv, 100, 120, false).unwrap()[0].words[0].turn, Turn::Upright);
    }

    #[test]
    fn malformed_local_ocr_fails_instead_of_losing_a_page_silently() {
        assert!(parse_tsv("", 100, 100, false).is_err());
        for rect in ["NaN\t10\t2\t3", "98\t10\t20\t3", "10\t10\t-2\t3"] {
            let tsv = format!("{HEADER}5\t1\t1\t1\t1\t1\t{rect}\t90\t字\n");
            assert!(parse_tsv(&tsv, 100, 100, false).is_err());
        }
    }
    #[test]
    fn local_ocr_preserves_low_confidence_and_refuses_invalid_scores() {
        let row = |score| format!("{HEADER}5\t1\t1\t1\t1\t1\t10\t20\t30\t12\t{score}\tError\n");
        assert_eq!(parse_tsv(&row("32"), 100, 100, false).unwrap()[0].words[0].confidence, Some(0.32));
        for score in ["NaN", "-1", "101", "bad"] { assert!(parse_tsv(&row(score), 100, 100, false).is_err()); }
    }

    #[test]
    fn portable_ocr_language_folders_accept_real_packs_and_reject_empty_folders() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_language_packs(dir.path()));
        std::fs::write(dir.path().join("README.txt"), b"packs go here").unwrap();
        assert!(!has_language_packs(dir.path()));
        std::fs::write(dir.path().join("jpn.traineddata"), b"fixture").unwrap();
        assert!(has_language_packs(dir.path()));
        let packs = ["chi_sim", "chi_tra", "jpn", "kor"].into_iter().map(str::to_string).collect();
        let list = offered(&packs);
        for code in ["zh", "zh-Hant", "ja", "ko"] {
            assert!(list.iter().any(|l| l.code == code));
            assert!(selection_models(code, &list, &packs).is_ok());
        }
    }

    #[test]
    fn portable_cjk_packs_are_found_in_windows_macos_and_linux_layouts() {
        let base = tempfile::tempdir().unwrap();
        for (layout, exe, packs) in [("windows", "Sheaf", "Sheaf/tessdata"),
            ("macos", "Sheaf.app/Contents/MacOS", "Sheaf.app/Contents/Resources/tessdata"),
            ("linux", "usr/bin", "usr/share/sheaf/tessdata")] {
            let root = base.path().join(layout);
            let exe = root.join(exe);
            let packs = root.join(packs);
            std::fs::create_dir_all(&exe).unwrap();
            std::fs::create_dir_all(&packs).unwrap();
            std::fs::write(packs.join("jpn.traineddata"), b"fixture").unwrap();
            let found = dirs_beside_exe(&exe).into_iter().find(|p| has_language_packs(p)).unwrap();
            assert_eq!(found.canonicalize().unwrap(), packs.canonicalize().unwrap(), "{layout}");
        }
    }

}
