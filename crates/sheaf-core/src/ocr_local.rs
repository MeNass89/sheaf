//! Extra scripts from installed Tesseract language packs (iLovePDF's OCR language list).
//! Nothing is downloaded: only languages reported by the local executable are offered.

use super::eng::{Line, OcrError, Turn, Word};
use std::collections::HashSet;
use std::process::Command;
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

fn installed() -> &'static HashSet<String> {
    static INSTALLED: OnceLock<HashSet<String>> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        Command::new("tesseract").arg("--list-langs").output().ok().filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::trim).map(str::to_string).collect()).unwrap_or_default()
    })
}

pub(super) fn supports(code: &str) -> bool {
    LANGUAGES.iter().any(|(c, _, model)| *c == code && installed().contains(*model))
}

pub(super) fn recognize(pixels: &[u8], width: u32, height: u32, code: &str) -> Result<Vec<Line>, OcrError> {
    let model = LANGUAGES.iter().find(|(c, _, _)| *c == code).map(|(_, _, m)| *m).ok_or_else(|| OcrError::Recognize(format!("Unsupported OCR language {code}")))?;
    let run = || -> Result<Vec<Line>, String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let image = dir.path().join("page.png");
        image::save_buffer(&image, pixels, width, height, image::ColorType::Rgba8).map_err(|e| e.to_string())?;
        let mut command = Command::new("tesseract");
        command.arg(&image).args(["stdout", "-l", model]);
        if code.ends_with("-vert") { command.args(["--psm", "5"]); }
        let output = command.arg("tsv").output().map_err(|e| format!("Tesseract: {e}"))?;
        if !output.status.success() { return Err(format!("Tesseract: {}", String::from_utf8_lossy(&output.stderr).trim())); }
        parse_tsv(&String::from_utf8_lossy(&output.stdout), width, height, code.ends_with("-vert"))
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
        let key = columns[1..5].join("/");
        if key != previous || lines.is_empty() { lines.push(Line::default()); previous = key; }
        if let Some(line) = lines.last_mut() {
            line.words.push(Word { text: columns[11].trim().into(), rect: [x, y, x + w, y + h], turn: if vertical { Turn::Down } else { Turn::Upright }, ..Default::default() });
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";

    #[test]
    fn local_ocr_keeps_cjk_words_lines_and_vertical_geometry() {
        let tsv = format!("{HEADER}5\t1\t1\t1\t1\t1\t10\t20\t30\t12\t91\t日本語\n5\t1\t1\t1\t2\t1\t50\t60\t15\t40\t88\t中文\n");
        let lines = parse_tsv(&tsv, 100, 120, true).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].words[0].text, "日本語");
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
}
