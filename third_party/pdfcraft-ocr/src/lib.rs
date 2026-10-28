//! pdfcraft-ocr — Scan & OCR ▸ Recognize text (L4).
//!
//! Recognition runs the ocrs engine (MIT/Apache-2.0) with its pre-trained models (CC-BY-SA-4.0,
//! fetched by `cargo xtask models`; see ATTRIBUTION.toml). The caller renders a page to pixels;
//! [`Ocr::recognize`] finds the words in them, and [`text_layer`] turns words placed in user space
//! into page content: invisible text (rendering mode 3) over each word, so the page becomes a
//! searchable image (Acrobat's "Searchable Image (Exact)": the image is left untouched).
//!
//! The models read the Latin alphabet (English and other languages written without accents).
//! Sheaf: with PaddleOCR's Latin model beside them ([`LATIN_MODEL`], [`LATIN_DICT`]), lines are read
//! by it instead, accents included (32 languages: [`LANGUAGES`]); see `latin.rs`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

pub use pdfcraft_fonts::helvetica_width;

/// The model files, as named in ATTRIBUTION.toml.
pub const DETECTION_MODEL: &str = "text-detection.rten";
pub const RECOGNITION_MODEL: &str = "text-recognition.rten";
/// Sheaf: PaddleOCR's PP-OCRv5 Latin recognition model and its dictionary (Apache-2.0).
pub const LATIN_MODEL: &str = "latin-recognition.onnx";
pub const LATIN_DICT: &str = "latin-dict.txt";

mod latin;
pub use latin::{RecognitionInput, recognition_input};

/// The recognition model's alphabet, as it was trained (ocrs-models' `DEFAULT_ALPHABET`): its
/// output classes follow this order. The ocrs crate's own copy has an `E` where the training had
/// `€`, which made a scanned "€250" read as "E250".
pub const ALPHABET: &str = " 0123456789!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~€ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// The languages the models read (ISO 639-1). Sheaf: those of PaddleOCR's Latin model, the one
/// that reads accents (without it, the models read English only: [`Models::reads_accents`]).
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("fr", "French"),
    ("de", "German"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("cs", "Czech"),
    ("sk", "Slovak"),
    ("hr", "Croatian"),
    ("sl", "Slovenian"),
    ("da", "Danish"),
    ("no", "Norwegian"),
    ("sv", "Swedish"),
    ("is", "Icelandic"),
    ("et", "Estonian"),
    ("lt", "Lithuanian"),
    ("hu", "Hungarian"),
    ("sq", "Albanian"),
    ("cy", "Welsh"),
    ("ga", "Irish"),
    ("tr", "Turkish"),
    ("id", "Indonesian"),
    ("ms", "Malay"),
    ("af", "Afrikaans"),
    ("sw", "Swahili"),
    ("tl", "Tagalog"),
    ("la", "Latin"),
];

#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    /// Shown to end users (release packages ship the models; a source build fetches them with
    /// `cargo xtask models`). The UI catalogs key this exact text.
    #[error("the text recognition models are not installed (reinstall PdfCraft, or set PDFCRAFT_MODELS to the folder that holds them)")]
    NoModels,
    #[error("loading {0}: {1}")]
    Load(String, String),
    #[error("text recognition failed: {0}")]
    Recognize(String),
    #[error("the image is empty")]
    EmptyImage,
}

/// Where the two model files are.
#[derive(Clone, Debug, PartialEq)]
pub struct Models {
    pub detection: PathBuf,
    pub recognition: PathBuf,
    /// Sheaf: PaddleOCR's Latin model and dictionary, when installed beside the others.
    pub latin: Option<(PathBuf, PathBuf)>,
}

impl Models {
    /// Detection plus either Paddle recognition/dictionary or the legacy English recognizer.
    pub fn in_dir(dir: &Path) -> Option<Models> {
        let latin = (dir.join(LATIN_MODEL), dir.join(LATIN_DICT));
        let latin = (latin.0.is_file() && latin.1.is_file()).then_some(latin);
        let m = Models { detection: dir.join(DETECTION_MODEL), recognition: dir.join(RECOGNITION_MODEL), latin };
        (m.detection.is_file() && (m.recognition.is_file() || m.latin.is_some())).then_some(m)
    }

    /// Sheaf: whether accented letters are read (PaddleOCR's Latin model is there).
    pub fn reads_accents(&self) -> bool {
        self.latin.is_some()
    }

    /// Look for the models: `$PDFCRAFT_MODELS`, then where the release packages install them
    /// beside the executable ([`Models::dirs_beside_exe`]), then the source tree's `assets/models/`.
    pub fn find() -> Option<Models> {
        Self::search_dirs().iter().find_map(|d| Self::in_dir(d))
    }

    /// The directories [`Models::find`] looks in, in order.
    pub fn search_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(d) = std::env::var_os("PDFCRAFT_MODELS") {
            dirs.push(PathBuf::from(d));
        }
        if let Some(exe) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
            dirs.extend(Self::dirs_beside_exe(&exe));
        }
        // The source tree's models, for development builds only: a release must not carry the
        // builder's paths (bundle-macos.sh checks the binaries for them).
        #[cfg(debug_assertions)]
        dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models"));
        dirs
    }

    /// Where the release packages put the models, relative to the directory holding the
    /// executable (packaging/*):
    /// - `models/` beside it: the Windows MSI and portable zip;
    /// - `../Resources/models`: the macOS app bundle (`Contents/MacOS` → `Contents/Resources`);
    /// - `../share/pdfcraft/models`: the FHS-style trees (`bin/` → `share/`) of the Linux
    ///   deb/rpm/tar.gz/AppImage/Flatpak and the FreeBSD tarball, wherever they are installed.
    pub fn dirs_beside_exe(exe_dir: &Path) -> Vec<PathBuf> {
        vec![exe_dir.join("models"), exe_dir.join("../Resources/models"), exe_dir.join("../share/pdfcraft/models")]
    }
}

/// A recognised word: its text and its box in image pixels `[left, top, right, bottom]`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Word {
    pub text: String,
    pub rect: [f32; 4],
    /// Sheaf: which way it reads (a table's label may stand on its side).
    pub turn: Turn,
    /// Sheaf: a scan's tilt (radians): `rect` is in the image straightened about `pivot`; the word
    /// is placed turned back by `tilt` about it, where the scan draws it.
    pub tilt: f32,
    pub pivot: [f32; 2],
    /// Mean character recognition confidence, when supplied by the recognizer (0..1).
    pub confidence: Option<f32>,
}

/// Sheaf: the way a word reads on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Turn {
    #[default]
    Upright,
    /// Bottom to top, the letters' tops to the left (Word's "rotate text 270°").
    Up,
    /// Top to bottom, the letters' tops to the right.
    Down,
}

/// A recognised line of words, in reading order.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Line {
    pub words: Vec<Word>,
}

impl Line {
    pub fn text(&self) -> String {
        self.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
    }
}

/// A loaded recogniser. Loading takes a moment; keep one and reuse it.
pub struct Ocr {
    engine: ocrs::OcrEngine,
    /// Sheaf: reads the lines when installed.
    latin: Option<latin::Latin>,
}

impl Ocr {
    pub fn load(models: &Models) -> Result<Ocr, OcrError> {
        Self::load_with_prepacked_weights(models, true)
    }

    /// Sheaf: configure recognition weight prepacking for accuracy comparisons and release
    /// benchmarks. The default packs once at load time, rather than for every scanned line.
    pub fn load_with_prepacked_weights(models: &Models, prepack: bool) -> Result<Ocr, OcrError> {
        let load = |p: &Path| rten::ModelOptions::with_all_ops().prepack_weights(prepack).load_file(p).map_err(|e| OcrError::Load(p.display().to_string(), e.to_string()));
        let params = ocrs::OcrEngineParams {
            detection_model: Some(load(&models.detection)?),
            // Paddle reads every line when installed; the old English recognizer is unused.
            recognition_model: if models.latin.is_none() { Some(load(&models.recognition)?) } else { None },
            alphabet: Some(ALPHABET.into()),
            ..Default::default()
        };
        let engine = ocrs::OcrEngine::new(params).map_err(|e| OcrError::Load("ocr engine".into(), e.to_string()))?;
        let latin = match &models.latin {
            Some((model, dict)) => Some(latin::Latin::load(model, dict, prepack).map_err(|e| OcrError::Load(LATIN_MODEL.into(), e))?),
            None => None,
        };
        Ok(Ocr { engine, latin })
    }

    /// Load the models found by [`Models::find`].
    pub fn find() -> Result<Ocr, OcrError> {
        Self::load(&Models::find().ok_or(OcrError::NoModels)?)
    }

    /// Recognise the text in an RGBA (or RGB, or grey) image, `width` × `height` pixels.
    pub fn recognize(&self, pixels: &[u8], width: u32, height: u32) -> Result<Vec<Line>, OcrError> {
        if width == 0 || height == 0 || pixels.is_empty() {
            return Err(OcrError::EmptyImage);
        }
        // ocrs wants 1 or 3 channels.
        let rgb: std::borrow::Cow<[u8]> = if pixels.len() == (width * height * 4) as usize {
            pixels.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect::<Vec<u8>>().into()
        } else {
            pixels.into()
        };
        let err = |e: &dyn std::fmt::Display| OcrError::Recognize(e.to_string());
        let source = ocrs::ImageSource::from_bytes(&rgb, (width, height)).map_err(|e| err(&e))?;
        let mut input = self.engine.prepare_input(source).map_err(|e| err(&e))?;
        let mut found = self.engine.detect_words(&input).map_err(|e| err(&e))?;
        // Sheaf: a scan fed in a little askew (2.5° left 8 % of a page's words) is straightened, read
        // again, and its words turned back to where the scan draws them.
        let tilt = if slant(&found) == 0.0 && found.len() < 8 { 0.0 } else { skew(&rgb, width as usize, height as usize) };
        let mut rgb = rgb;
        if tilt.abs() > 0.004 {
            rgb = straightened(&rgb, width as usize, height as usize, tilt).into();
            let source = ocrs::ImageSource::from_bytes(&rgb, (width, height)).map_err(|e| err(&e))?;
            input = self.engine.prepare_input(source).map_err(|e| err(&e))?;
            found = self.engine.detect_words(&input).map_err(|e| err(&e))?;
        }
        let pivot = [width as f32 / 2.0, height as f32 / 2.0];
        let tilted = |mut lines: Vec<Line>| {
            if tilt.abs() > 0.004 {
                for w in lines.iter_mut().flat_map(|l| l.words.iter_mut()) {
                    (w.tilt, w.pivot) = (tilt, pivot);
                }
            }
            lines
        };
        let lines = self.engine.find_text_lines(&input, &found);
        if let Some(latin) = &self.latin {
            return self.read_latin(latin, &rgb, width, height, &lines).map(tilted);
        }
        let read = self.engine.recognize_text(&input, &lines).map_err(|e| err(&e))?;
        use ocrs::TextItem;
        Ok(tilted(read
            .into_iter()
            .flatten()
            .map(|line| Line {
                words: line
                    .words()
                    .map(|w| {
                        let r = w.bounding_rect();
                        Word { text: w.to_string(), rect: [r.left() as f32, r.top() as f32, r.right() as f32, r.bottom() as f32], ..Default::default() }
                    })
                    .filter(|w| !w.text.trim().is_empty())
                    .collect(),
            })
            .filter(|l| !l.words.is_empty())
            .collect()))
    }
}

impl Ocr {
    /// Sheaf: `lines` (ocrs' words, line by line) read by PaddleOCR's Latin model. A word read keeps
    /// the box of the words found under it (the model's place for it otherwise).
    fn read_latin(&self, latin: &latin::Latin, rgb: &[u8], width: u32, height: u32, lines: &[Vec<rten_imageproc::RotatedRect>]) -> Result<Vec<Line>, OcrError> {
        // Grey images (one byte a pixel) as RGB.
        let rgb: std::borrow::Cow<[u8]> =
            if rgb.len() == (width * height) as usize { rgb.iter().flat_map(|v| [*v; 3]).collect::<Vec<u8>>().into() } else { rgb.into() };
        let boxes = |line: &Vec<rten_imageproc::RotatedRect>| -> Vec<[f32; 4]> {
            line.iter()
                .map(|r| {
                    let c = r.corners();
                    let xs = c.iter().map(|p| p.x);
                    let ys = c.iter().map(|p| p.y);
                    [xs.clone().fold(f32::MAX, f32::min), ys.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max), ys.fold(f32::MIN, f32::max)]
                })
                .collect()
        };
        // One line at a time on each core (a line is too small for the model to use them all).
        // Each line with how sure the model is of each word.
        let read_one = |line: &Vec<rten_imageproc::RotatedRect>| -> Result<Option<(Line, Vec<f32>)>, OcrError> {
            let found = boxes(line);
            if found.is_empty() {
                return Ok(None);
            }
            let rect = found.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]);
            let read = latin.read_line(&rgb, width as usize, height as usize, rect).map_err(OcrError::Recognize)?;
            let scores: Vec<f32> = read.iter().map(|r| r.2).collect();
            let mut words = place_words(read, &found);
            apart(&mut words);
            Ok((!words.is_empty()).then_some((Line { words }, scores)))
        };
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 8).min(lines.len().max(1));
        let next = std::sync::atomic::AtomicUsize::new(0);
        let mut slots: Vec<Option<Result<Option<(Line, Vec<f32>)>, OcrError>>> = (0..lines.len()).map(|_| None).collect();
        let results = std::sync::Mutex::new(&mut slots);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(line) = lines.get(k) else { break };
                        let r = read_one(line);
                        if let Ok(mut slots) = results.lock()
                            && let Some(slot) = slots.get_mut(k)
                        {
                            *slot = Some(r);
                        }
                    }
                });
            }
        });
        let mut out: Vec<(Line, Vec<f32>)> = Vec::new();
        for r in slots.into_iter().flatten() {
            if let Some(line) = r? {
                out.push(line);
            }
        }
        // Words standing on their side (a table's turned labels): ocrs finds them as boxes much
        // taller than wide, or as a stack of letters. Each is cut out, turned both ways and read as a
        // line; the surer reading is kept when it reads more than was read there upright (single
        // letters, or nothing sure), which it then replaces.
        let tall: Vec<[f32; 4]> = tall_boxes(&lines.iter().flat_map(boxes).collect::<Vec<_>>()).into_iter().map(|b| along_ink(&rgb, width as usize, height as usize, b)).collect();
        let tall = tall_boxes(&tall);
        for b in tall {
            let mut best: Option<(f32, f32, Vec<Word>)> = None;
            for turn in [Turn::Up, Turn::Down] {
                // Reading up first (Word's turned cells read so); the other way only when that was unsure.
                if best.as_ref().is_some_and(|(s, _, _)| *s >= 0.95) {
                    break;
                }
                let (img, w, h, back) = turned(&rgb, width as usize, height as usize, b, turn);
                let read = latin.read_line(&img, w, h, [0.0, 0.0, w as f32, h as f32]).map_err(OcrError::Recognize)?;
                let letters: usize = read.iter().map(|r| r.0.chars().filter(|c| c.is_alphanumeric()).count()).sum();
                if read.is_empty() || letters < 2 {
                    continue;
                }
                let score = read.iter().map(|r| r.2).sum::<f32>() / read.len() as f32;
                if score >= 0.8 && best.as_ref().is_none_or(|(s, _, _)| score > *s) {
                    best = Some((score, score * letters as f32, read.into_iter().map(|(text, at, confidence)| Word { text, rect: back(at), confidence: Some(confidence), turn, ..Default::default() }).collect()));
                }
            }
            let inside = |w: &Word| {
                let (cx, cy) = ((w.rect[0] + w.rect[2]) / 2.0, (w.rect[1] + w.rect[3]) / 2.0);
                cx >= b[0] && cx <= b[2] && cy >= b[1] && cy <= b[3]
            };
            // What the upright reading found there: its letters, each counted as sure as its word.
            let upright: f32 = out
                .iter()
                .flat_map(|(l, s)| l.words.iter().zip(s))
                .filter(|(w, _)| inside(w))
                .map(|(w, s)| s * w.text.chars().filter(|c| c.is_alphanumeric()).count() as f32)
                .sum();
            if let Some((_, _, words)) = best.filter(|(_, weight, _)| *weight > upright) {
                for (l, s) in out.iter_mut() {
                    let keep: Vec<bool> = l.words.iter().map(|w| !inside(w)).collect();
                    let mut k = keep.iter();
                    l.words.retain(|_| k.next().copied().unwrap_or(true));
                    let mut k = keep.iter();
                    s.retain(|_| k.next().copied().unwrap_or(true));
                }
                out.retain(|(l, _)| !l.words.is_empty());
                out.push((Line { words }, Vec::new()));
            }
        }
        Ok(out.into_iter().map(|(l, _)| l).collect())
    }
}

/// Sheaf: whether the page has text to go by (words clearly longer than tall): 0 when too few.
fn slant(found: &[rten_imageproc::RotatedRect]) -> f32 {
    let long = found
        .iter()
        .filter(|r| {
            let c = r.corners();
            let (a, b) = ((c[1].x - c[0].x).hypot(c[1].y - c[0].y), (c[2].x - c[1].x).hypot(c[2].y - c[1].y));
            a.max(b) >= 2.0 * a.min(b)
        })
        .count();
    if long < 8 { 0.0 } else { 1.0 }
}

/// Sheaf: how much a page's text slants (radians, y down), within 5°: the angle at which its dark
/// pixels fall into the sharpest rows (the projection profile, as deskew tools measure it), searched
/// by quarter degrees, then by fortieths around the best, on the page reduced to 800 px across.
fn skew(img: &[u8], width: usize, height: usize) -> f32 {
    let ch = (img.len() / (width * height).max(1)).max(1);
    let step = (width / 800).max(1);
    let mut dark: Vec<(f32, f32)> = Vec::new();
    for y in (0..height).step_by(step) {
        for x in (0..width).step_by(step) {
            let at = (y * width + x) * ch;
            let v: u32 = img.get(at..at + ch).map_or(255, |p| p.iter().map(|v| u32::from(*v)).sum::<u32>() / ch as u32);
            if v < 128 {
                dark.push(((x / step) as f32, (y / step) as f32));
            }
        }
    }
    if dark.len() < 100 {
        return 0.0;
    }
    let rows = (width.max(height) / step) * 2 + 2;
    let sharpness = |a: f32| -> f64 {
        let (sin, cos) = a.sin_cos();
        let mut hist = vec![0u32; rows];
        for &(x, y) in &dark {
            // The row the pixel falls in once the page is turned by -a.
            let r = (-x * sin + y * cos + rows as f32 / 4.0) as usize;
            if let Some(h) = hist.get_mut(r) {
                *h += 1;
            }
        }
        hist.iter().map(|h| f64::from(*h).powi(2)).sum()
    };
    let best = |from: f32, to: f32, by: f32| {
        let mut a = from;
        let mut out = (0.0, f64::MIN);
        while a <= to + by / 2.0 {
            let s = sharpness(a.to_radians());
            if s > out.1 {
                out = (a, s);
            }
            a += by;
        }
        out.0
    };
    let coarse = best(-5.0, 5.0, 0.25);
    best(coarse - 0.25, coarse + 0.25, 0.025).to_radians()
}

/// Sheaf: `img` (1 or 3 bytes a pixel) turned by `-tilt` about its centre, so text slanting by `tilt`
/// runs level; what comes in from beyond the edges is white.
fn straightened(img: &[u8], width: usize, height: usize, tilt: f32) -> Vec<u8> {
    let ch = (img.len() / (width * height).max(1)).max(1);
    let mut out = vec![255u8; width * height * ch];
    let (sin, cos) = tilt.sin_cos();
    let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
    for y in 0..height {
        for x in 0..width {
            // Where this straightened pixel lies on the scan.
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let (sx, sy) = (cx + dx * cos - dy * sin - 0.5, cy + dx * sin + dy * cos - 0.5);
            if sx < 0.0 || sy < 0.0 || sx >= (width - 1) as f32 || sy >= (height - 1) as f32 {
                continue;
            }
            let (x0, y0) = (sx as usize, sy as usize);
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            for c in 0..ch {
                let at = |xx: usize, yy: usize| f32::from(img[(yy * width + xx) * ch + c]);
                let v = at(x0, y0) * (1.0 - fx) * (1.0 - fy) + at(x0 + 1, y0) * fx * (1.0 - fy) + at(x0, y0 + 1) * (1.0 - fx) * fy + at(x0 + 1, y0 + 1) * fx * fy;
                out[(y * width + x) * ch + c] = v.round() as u8;
            }
        }
    }
    out
}

/// Sheaf: the boxes standing on their side among `found` (taller than 1.5 widths), once the boxes
/// stacked in one column with little between them are joined (ocrs may cut a long turned label in
/// two, or find each of its letters on its own).
fn tall_boxes(found: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut sorted = found.to_vec();
    sorted.sort_by(|a, b| a[1].total_cmp(&b[1]));
    let mut out: Vec<[f32; 4]> = Vec::new();
    for b in sorted {
        // One above the other, sharing most of the wider one's width (a column of letters, not lines
        // of words of all lengths), with less than about half a letter between them.
        let stacked = |o: &&mut [f32; 4]| {
            let (wo, wb) = (o[2] - o[0], b[2] - b[0]);
            let (narrow, wide) = (wo.min(wb).max(1.0), wo.max(wb));
            o[2].min(b[2]) - o[0].max(b[0]) >= 0.6 * wide && o[2].max(b[2]) - o[0].min(b[0]) <= 1.2 * wide && b[1] - o[3] < 0.6 * narrow
        };
        match out.iter_mut().find(stacked) {
            Some(o) => *o = [o[0].min(b[0]), o[1].min(b[1]), o[2].max(b[2]), o[3].max(b[3])],
            None => out.push(b),
        }
    }
    out.retain(|b| b[3] - b[1] > 1.5 * (b[2] - b[0]) && b[3] - b[1] > 12.0);
    out
}

/// Sheaf: `b` (a box standing on its side) grown up and down its column while there is ink: ocrs
/// often finds only part of a turned label ("cha" of "Prise en charge"). It stops at a blank gap
/// longer than the box is wide, or at a rule across the column reaching past its sides (a table's
/// border, not a turned letter's stroke).
fn along_ink(rgb: &[u8], width: usize, height: usize, b: [f32; 4]) -> [f32; 4] {
    let (x0, x1) = (b[0].max(0.0) as usize, (b[2].max(0.0) as usize).min(width));
    if x1 <= x0 || height == 0 {
        return b;
    }
    let gap = (b[2] - b[0]).max(4.0) as usize;
    let grey = |x: usize, y: usize| rgb.get((y * width + x) * 3..(y * width + x) * 3 + 3).map_or(255, |p| (u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 3);
    // The label's ground (light paper, or a dark shaded cell with light letters): the seed box's
    // median grey. Ink is what differs from it.
    let (y0, y1) = (b[1].max(0.0) as usize, (b[3].max(0.0) as usize).min(height));
    let mut seen: Vec<u16> = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| grey(x, y)).collect();
    seen.sort_unstable();
    let ground = seen.get(seen.len() / 2).copied().unwrap_or(255);
    let is_ink = |x: usize, y: usize| grey(x, y).abs_diff(ground) > 70;
    let ink = |y: usize| (x0..x1).filter(|&x| is_ink(x, y)).count();
    let grow = |from: usize, step: isize| -> usize {
        let (mut y, mut last, mut blank) = (from as isize, from, 0);
        loop {
            y += step;
            if y < 0 || y >= height as isize || blank > gap {
                return last;
            }
            let n = ink(y as usize);
            // A rule across the column reaches past both its sides (a letter's stroke does not).
            let reach = gap / 2;
            if n * 10 >= (x1 - x0) * 8 && x0 >= reach && is_ink(x0 - reach, y as usize) && is_ink((x1 + reach).min(width - 1), y as usize) {
                return last;
            }
            if n > 0 {
                last = y as usize;
                blank = 0;
            } else {
                blank += 1;
            }
        }
    };
    let top = grow(b[1].max(0.0) as usize, -1);
    let bottom = grow((b[3].max(0.0) as usize).min(height - 1), 1);
    [b[0], top as f32, b[2], bottom as f32 + 1.0]
}

/// Sheaf: the region `b` of `rgb` (with room around it) turned so a word reading `turn` reads
/// upright, its size, and the way back from a box in it to one on the page.
fn turned(rgb: &[u8], width: usize, height: usize, b: [f32; 4], turn: Turn) -> (Vec<u8>, usize, usize, impl Fn([f32; 4]) -> [f32; 4]) {
    let w = b[2] - b[0];
    let x0 = (b[0] - w * 0.3).floor().max(0.0) as usize;
    let x1 = ((b[2] + w * 0.3).ceil() as usize).min(width).max(x0 + 1);
    let y0 = (b[1] - w * 0.3).floor().max(0.0) as usize;
    let y1 = ((b[3] + w * 0.3).ceil() as usize).min(height).max(y0 + 1);
    let (cw, ch) = (x1 - x0, y1 - y0);
    let mut out = vec![255u8; cw * ch * 3];
    // Up: turned a quarter clockwise (out is ch wide, cw high); Down: a quarter the other way.
    for y in 0..ch {
        for x in 0..cw {
            let (ox, oy) = match turn {
                Turn::Down => (y, cw - 1 - x),
                _ => (ch - 1 - y, x),
            };
            let from = ((y0 + y) * width + x0 + x) * 3;
            let to = (oy * ch + ox) * 3;
            if let (Some(src), Some(dst)) = (rgb.get(from..from + 3), out.get_mut(to..to + 3)) {
                dst.copy_from_slice(src);
            }
        }
    }
    let (fx, fy, fcw, fch) = (x0 as f32, y0 as f32, cw as f32, ch as f32);
    let back = move |r: [f32; 4]| match turn {
        Turn::Down => [fx + fcw - r[3], fy + r[0], fx + fcw - r[1], fy + r[2]],
        _ => [fx + r[1], fy + fch - r[2], fx + r[3], fy + fch - r[0]],
    };
    (out, ch, cw, back)
}

/// Sheaf: words of a line that touch or overlap (two read in one box ocrs found) pulled apart, a
/// fifth of their height between them, so a reader of the text sees two words ("de" "cause", not
/// "decause").
fn apart(words: &mut [Word]) {
    for k in 1..words.len() {
        let (a, b) = (words[k - 1].rect, words[k].rect);
        let gap = 0.2 * (a[3] - a[1]).max(b[3] - b[1]);
        if b[0] - a[2] < gap && b[0] > a[0] && b[2] > a[2] {
            let mid = ((a[2] + b[0]) / 2.0).clamp(a[0] + gap, b[2] - gap);
            words[k - 1].rect[2] = (mid - gap / 2.0).max(a[0] + 1.0);
            words[k].rect[0] = (mid + gap / 2.0).min(b[2] - 1.0);
        }
    }
}

/// Sheaf: the words read on a line, placed: with the boxes ocrs found, in order, when there are as
/// many as words (the usual case); else (ocrs found "cause inconnue" as one word) each where the
/// model read it across the line, as tall as the boxes it overlaps.
fn place_words(read: Vec<latin::Read>, found: &[[f32; 4]]) -> Vec<Word> {
    let mut sorted = found.to_vec();
    sorted.sort_by(|a, b| a[0].total_cmp(&b[0]));
    if read.len() == sorted.len() {
        return read.into_iter().zip(sorted).map(|((text, _, confidence), rect)| Word { text, rect, confidence: Some(confidence), ..Default::default() }).collect();
    }
    read.into_iter()
        .map(|(text, at, confidence)| {
            let over = sorted.iter().filter(|b| b[0] < at[2] && b[2] > at[0]);
            let (top, bottom) = over.fold((f32::MAX, f32::MIN), |(t, d), b| (t.min(b[1]), d.max(b[3])));
            let rect = if top < bottom { [at[0], top, at[2], bottom] } else { at };
            Word { text, rect, confidence: Some(confidence), ..Default::default() }
        })
        .collect()
}

/// A word placed on a page, in PDF user space: the bottom-left corner of its box, and the
/// vectors along its bottom edge (`across`, left → right as read) and its left edge (`up`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedWord {
    pub text: String,
    pub confidence: Option<f32>,
    pub origin: [f64; 2],
    pub across: [f64; 2],
    pub up: [f64; 2],
}

impl PlacedWord {
    /// Place a pixel-space word with `to_user`, which maps an image pixel (x right, y down) to
    /// user space.
    pub fn place(word: &Word, to_user: impl Fn(f32, f32) -> [f64; 2]) -> PlacedWord {
        let [l, t, r, b] = word.rect;
        // Sheaf: the start of its baseline, the end, and the top of its first letter, as it reads.
        let ((ox, oy), (ex, ey), (ux, uy)) = match word.turn {
            Turn::Upright => ((l, b), (r, b), (l, t)),
            Turn::Up => ((r, b), (r, t), (l, b)),
            Turn::Down => ((l, t), (l, b), (r, t)),
        };
        // Sheaf: on a tilted scan, back from the straightened image to the scan.
        let (sin, cos) = word.tilt.sin_cos();
        let [px, py] = word.pivot;
        let on_scan = |x: f32, y: f32| {
            let (dx, dy) = (x - px, y - py);
            to_user(px + dx * cos - dy * sin, py + dx * sin + dy * cos)
        };
        let o = on_scan(ox, oy);
        let br = on_scan(ex, ey);
        let tl = on_scan(ux, uy);
        PlacedWord { text: word.text.clone(), confidence: word.confidence, origin: o, across: [br[0] - o[0], br[1] - o[1]], up: [tl[0] - o[0], tl[1] - o[1]] }
    }
}

/// Helvetica's ascender and descender (em fractions): a word box spans about this much.
const BOX_EM: f64 = 0.93;
const DESCENT: f64 = 0.21;
/// Helvetica's cap height and x-height (em fractions).
const CAP_HEIGHT: f64 = 0.718;
const X_HEIGHT: f64 = 0.523;

/// How far `text`'s ink reaches above and below the baseline, in em fractions: a word box is
/// tight around the ink, so "DT" spans the cap height only, "sun" the x-height, and "gap" reaches
/// down to the descender. Text without letters or digits keeps the full ascender-descender span.
fn ink_extent(text: &str) -> (f64, f64) {
    if !text.chars().any(char::is_alphanumeric) {
        return (BOX_EM - DESCENT, -DESCENT);
    }
    let tall = text
        .chars()
        .any(|c| c.is_uppercase() || c.is_ascii_digit() || "bdfhijklt!?/\\()[]{}|'\"#$%&@".contains(c) || (c.is_alphabetic() && !c.is_ascii()));
    let low = text.chars().any(|c| "gjpqy()[]{}|_".contains(c));
    (if tall { CAP_HEIGHT } else { X_HEIGHT }, if low { -DESCENT } else { 0.0 })
}

fn num(v: f64) -> String {
    let s = format!("{:.4}", if v.abs() < 1e-9 { 0.0 } else { v });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// Page content that writes `words` as invisible text in `/PCHelv` (standard Helvetica,
/// WinAnsiEncoding; Sheaf: other characters in `/PCUni`), each word stretched to its box so selection and search highlight the
/// right place. Marked content `/OCR` so it can be told apart from the page's own text.
pub fn text_layer(words: &[PlacedWord]) -> Vec<u8> {
    let mut out = b"/OCR BMC\nBT\n3 Tr\n/PCHelv 1 Tf\n".to_vec();
    let mut in_uni = false;
    for w in words {
        // Sheaf: a word WinAnsi cannot carry (α, →) in `/PCUni`, the glyphless Unicode font
        // pdfcraft-edit adds (UTF-16 codes, each glyph half an em wide).
        let uni = !w.text.chars().all(|c| c == '?' || pdfcraft_fonts::win_ansi_byte(c).is_some());
        if uni != in_uni {
            out.extend_from_slice(if uni { b"/PCUni 1 Tf\n" } else { b"/PCHelv 1 Tf\n" });
            in_uni = uni;
        }
        let width = if uni { w.text.encode_utf16().count() as f64 * 0.5 } else { helvetica_width(&w.text, 1.0) };
        let height = w.up[0].hypot(w.up[1]);
        if width <= 0.0 || height <= 0.0 {
            continue;
        }
        // The em square: the word's own ink (cap height or x-height, descenders or not) spans the
        // box, so the text is as tall as the scanned word; its width is stretched to fit the word.
        let (top, bottom) = ink_extent(&w.text);
        let em = [w.up[0] / (top - bottom), w.up[1] / (top - bottom)];
        let ax = [w.across[0] / width, w.across[1] / width];
        let o = [w.origin[0] - em[0] * bottom, w.origin[1] - em[1] * bottom];
        let m = [ax[0], ax[1], em[0], em[1], o[0], o[1]];
        // Separate marked scopes retain confidence and prevent adjacent OCR words being
        // merged into a single editable line (Acrobat's word-by-word suspects).
        let properties = w.confidence.filter(|c| c.is_finite() && (0.0..=1.0).contains(c))
            .map_or_else(String::new, |c| format!(" /SheafConfidence {}", num(c as f64)));
        out.extend_from_slice(format!("/SheafOCRWord <<{properties} >> BDC\n").as_bytes());
        let nums: Vec<String> = m.iter().map(|v| num(*v)).collect();
        out.extend_from_slice(nums.join(" ").as_bytes());
        out.extend_from_slice(b" Tm ");
        if uni {
            out.push(b'<');
            out.extend(w.text.encode_utf16().flat_map(|u| format!("{u:04X}").into_bytes()));
            out.push(b'>');
        } else {
            out.extend(pdfcraft_fonts::literal(&pdfcraft_fonts::win_ansi(&w.text)));
        }
        // Sheaf: a space after it, where it ends, so a reader of the text need not guess where words
        // part (on a tilted scan, pdftotext ran some together: "causeinconnue").
        out.extend_from_slice(if uni { b" Tj <0020> Tj\nEMC\n" } else { b" Tj ( ) Tj\nEMC\n" });
    }
    out.extend_from_slice(b"ET\nEMC\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_slant_is_measured_and_straightened() {
        // Rows of dark dashes ("lines of text") slanting by -2°, y down.
        let (w, h) = (800, 600);
        let t = (-2.0f32).to_radians();
        let mut img = vec![255u8; w * h];
        for row in 0..12 {
            for x in 50..750 {
                if x % 40 < 30 {
                    let y0 = 60.0 + row as f32 * 40.0 + (x as f32 - 400.0) * t.tan();
                    for dy in 0..8 {
                        img[(y0 as usize + dy) * w + x] = 0;
                    }
                }
            }
        }
        let got = skew(&img, w, h);
        assert!((got - t).abs() < 0.1f32.to_radians(), "{} for {}", got.to_degrees(), t.to_degrees());
        assert!(skew(&straightened(&img, w, h, got), w, h).abs() < 0.1f32.to_radians());
    }

    #[test]
    fn a_region_turned_and_back_is_where_it_was() {
        // A 4 × 2 region at (10, 20) of a 40 × 40 image; its pixel (1, 0) is black.
        let mut rgb = vec![255u8; 40 * 40 * 3];
        rgb[(20 * 40 + 11) * 3..(20 * 40 + 11) * 3 + 3].fill(0);
        for turn in [Turn::Up, Turn::Down] {
            let (img, w, h, back) = turned(&rgb, 40, 40, [10.0, 20.0, 14.0, 22.0], turn);
            assert_eq!(w * h * 3, img.len());
            let at = img.chunks(3).position(|p| p[0] == 0).expect("the black pixel is in the turned region");
            let (x, y) = ((at % w) as f32, (at / w) as f32);
            let r = back([x, y, x + 1.0, y + 1.0]);
            assert_eq!([r[0], r[1]], [11.0, 20.0], "{turn:?}: {r:?}");
        }
    }

    #[test]
    fn a_turned_label_grows_along_its_ink_on_a_dark_ground() {
        // A dark column (a shaded cell) with light letters from y = 10 to 90, a rule across at y = 100.
        let (w, h) = (40, 120);
        let mut rgb = vec![255u8; w * h * 3];
        for y in 0..h {
            for x in 10..30 {
                let light = (10..90).contains(&y) && (17..23).contains(&x) && y % 7 != 0;
                let v = if light { 250 } else { 60 };
                rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].fill(v);
            }
        }
        for x in 0..w {
            rgb[(100 * w + x) * 3..(100 * w + x) * 3 + 3].fill(0);
        }
        // ocrs found only the middle of it.
        let b = along_ink(&rgb, w, h, [10.0, 40.0, 30.0, 60.0]);
        assert!(b[1] <= 11.0 && (89.0..=99.0).contains(&b[3]), "{b:?}");
    }

    #[test]
    fn a_turned_word_is_placed_reading_up_the_page() {
        // Bottom to top: its baseline runs up the right side of its box, its letters' tops to the left.
        let to_user = |x: f32, y: f32| [x as f64, 100.0 - y as f64];
        let w = PlacedWord::place(&Word { text: "Bio".into(), rect: [10.0, 20.0, 20.0, 60.0], turn: Turn::Up, ..Default::default() }, to_user);
        assert_eq!((w.origin, w.across, w.up), ([20.0, 40.0], [0.0, 40.0], [-10.0, 0.0]));
    }

    #[test]
    fn words_that_touch_are_pulled_apart() {
        let w = |x0: f32, x1: f32| Word { text: "w".into(), rect: [x0, 0.0, x1, 10.0], ..Default::default() };
        let mut words = vec![w(0.0, 20.0), w(18.0, 40.0), w(60.0, 80.0)];
        apart(&mut words);
        assert!(words[1].rect[0] - words[0].rect[2] >= 1.9, "{words:?}");
        assert_eq!(words[2].rect, [60.0, 0.0, 80.0, 10.0], "words already apart are left");
    }

    #[test]
    fn words_read_take_the_boxes_found_in_order() {
        let read = vec![("de".to_string(), [0.0, 0.0, 10.0, 12.0], 1.0), ("cause".to_string(), [18.0, 0.0, 52.0, 12.0], 1.0), ("inconnue".to_string(), [58.0, 0.0, 99.0, 12.0], 1.0)];
        let found = [[60.0, 2.0, 100.0, 10.0], [0.0, 2.0, 15.0, 10.0], [20.0, 2.0, 55.0, 10.0]];
        let words = place_words(read.clone(), &found);
        assert_eq!(words.iter().map(|w| w.rect).collect::<Vec<_>>(), [found[1], found[2], found[0]]);
        // "cause inconnue" found as one box: each word where it was read, as tall as that box.
        let words = place_words(read, &[[0.0, 2.0, 15.0, 10.0], [20.0, 1.0, 100.0, 11.0]]);
        assert_eq!(words.iter().map(|w| w.rect).collect::<Vec<_>>(), [[0.0, 2.0, 10.0, 10.0], [18.0, 1.0, 52.0, 11.0], [58.0, 1.0, 99.0, 11.0]]);
    }

    #[test]
    fn places_words_through_the_mapping() {
        // 2 pixels per point, page 612 × 792, y flipped.
        let to_user = |x: f32, y: f32| [x as f64 / 2.0, 792.0 - y as f64 / 2.0];
        let w = PlacedWord::place(&Word { text: "Hello".into(), rect: [100.0, 200.0, 300.0, 240.0], ..Default::default() }, to_user);
        assert_eq!(w.origin, [50.0, 672.0]);
        assert_eq!(w.across, [100.0, 0.0]);
        assert_eq!(w.up, [0.0, 20.0]);
    }

    /// The text layer is Helvetica in WinAnsiEncoding: every character the model can recognise
    /// must be one it can hold, or recognised text would be saved as `?`. A model with a wider
    /// alphabet (Chinese, for example) needs a text layer in another font first.
    #[test]
    fn the_model_alphabet_fits_the_text_layer_font() {
        assert_eq!(pdfcraft_fonts::first_undrawable(ALPHABET), None);
    }

    #[test]
    fn text_layer_is_invisible_text_fitted_to_each_box() {
        let w = PlacedWord { text: "Hi".into(), confidence: None, origin: [10.0, 20.0], across: [30.0, 0.0], up: [0.0, 9.3] };
        let s = String::from_utf8(text_layer(&[w])).unwrap();
        assert!(s.contains("3 Tr") && s.contains("/PCHelv 1 Tf"), "{s}");
        let width = helvetica_width("Hi", 1.0);
        // "Hi" has no descender: its box spans the cap height, from the baseline up.
        assert!(s.contains(&format!("{} 0 0 {} 10 20 Tm (Hi) Tj", num(30.0 / width), num(9.3 / CAP_HEIGHT))), "{s}");
        assert!(s.starts_with("/OCR BMC") && s.ends_with("EMC\n"));
    }

    /// A word box is tight around the ink, so the text's size and baseline come from the letters:
    /// capitals, lowercase without ascenders, descenders.
    #[test]
    fn the_text_is_as_tall_as_the_scanned_word() {
        assert_eq!(ink_extent("DT"), (CAP_HEIGHT, 0.0));
        assert_eq!(ink_extent("402337"), (CAP_HEIGHT, 0.0));
        assert_eq!(ink_extent("sun"), (X_HEIGHT, 0.0));
        assert_eq!(ink_extent("gap"), (X_HEIGHT, -DESCENT));
        assert_eq!(ink_extent("Gallus.Ze@camtel.cm"), (CAP_HEIGHT, 0.0));
        assert_eq!(ink_extent("Corporation"), (CAP_HEIGHT, -DESCENT));
        assert_eq!(ink_extent("--"), (BOX_EM - DESCENT, -DESCENT));
        let w = PlacedWord { text: "gap".into(), confidence: None, origin: [0.0, 100.0], across: [20.0, 0.0], up: [0.0, 7.3] };
        let s = String::from_utf8(text_layer(&[w])).unwrap();
        let em = 7.3 / (X_HEIGHT + DESCENT);
        assert!(s.contains(&format!(" 0 0 {} 0 {} Tm (gap) Tj", num(em), num(100.0 + em * DESCENT))), "{s}");
    }

    #[test]
    fn rotated_pages_keep_the_reading_direction() {
        // A page turned 90°: image x runs up the page.
        let to_user = |x: f32, y: f32| [y as f64, x as f64];
        let w = PlacedWord::place(&Word { text: "Up".into(), rect: [0.0, 0.0, 50.0, 10.0], ..Default::default() }, to_user);
        assert_eq!(w.across, [0.0, 50.0]);
        assert_eq!(w.up, [-10.0, 0.0]);
    }

    /// #103: every release package's layout puts the models where the installed app looks.
    /// Mirrors packaging/{windows/package.ps1,macos/package.sh,linux/package.sh,freebsd/package.sh}.
    #[test]
    fn finds_the_models_in_every_release_package_layout() {
        let base = std::env::temp_dir().join(format!("pdfcraft-ocr-layouts-{}", std::process::id()));
        let layouts = [
            ("windows", "PdfCraft", "PdfCraft/models"),
            ("macos", "PdfCraft.app/Contents/MacOS", "PdfCraft.app/Contents/Resources/models"),
            ("linux-deb-rpm", "usr/bin", "usr/share/pdfcraft/models"),
            ("linux-tar", "pdfcraft-1.0.0-linux-x86_64/bin", "pdfcraft-1.0.0-linux-x86_64/share/pdfcraft/models"),
            ("appimage", "PdfCraft.AppDir/usr/bin", "PdfCraft.AppDir/usr/share/pdfcraft/models"),
            ("flatpak", "app/bin", "app/share/pdfcraft/models"),
            ("freebsd", "usr/local/bin", "usr/local/share/pdfcraft/models"),
        ];
        for (name, exe_dir, models_dir) in layouts {
            let root = base.join(name);
            let (exe_dir, models_dir) = (root.join(exe_dir), root.join(models_dir));
            std::fs::create_dir_all(&exe_dir).unwrap();
            std::fs::create_dir_all(&models_dir).unwrap();
            let found = || Models::dirs_beside_exe(&exe_dir).iter().find_map(|d| Models::in_dir(d));
            assert_eq!(found(), None, "{name}: no models yet");
            for file in [DETECTION_MODEL, RECOGNITION_MODEL] {
                std::fs::write(models_dir.join(file), b"model").unwrap();
            }
            let models = found().unwrap_or_else(|| panic!("{name}: models in {} not found", models_dir.display()));
            assert_eq!(models.detection.canonicalize().unwrap(), models_dir.join(DETECTION_MODEL).canonicalize().unwrap(), "{name}");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The message tells end users what to do; release builds have no cargo.
    #[test]
    fn the_alphabet_is_the_trained_one() {
        let chars: Vec<char> = ALPHABET.chars().collect();
        assert_eq!(chars.len(), 96);
        // The euro sign follows the ASCII punctuation (class 44; 0 is the CTC blank), not a second E.
        assert_eq!(chars.iter().position(|&c| c == '€'), Some(43));
        assert_eq!(chars.iter().filter(|&&c| c == 'E').count(), 1);
    }

    #[test]
    fn missing_models_message_is_for_end_users() {
        let message = OcrError::NoModels.to_string();
        assert!(!message.contains("cargo") && message.contains("PDFCRAFT_MODELS"), "{message}");
    }

    /// End to end on rendered text, when the models are installed (`cargo xtask models`).
    #[test]
    fn reads_rendered_text() {
        let Some(models) = Models::find() else {
            eprintln!("skipped: OCR models not installed");
            return;
        };
        let ocr = Ocr::load(&models).unwrap();
        let (w, h, px) = test_image::hello();
        let lines = ocr.recognize(&px, w, h).unwrap();
        let text: Vec<String> = lines.iter().map(Line::text).collect();
        assert!(text.iter().any(|t| t.contains("HELL") && t.contains("WOR")), "{text:?}");
        let word = lines.iter().flat_map(|l| &l.words).find(|w| w.text.contains("HELL")).unwrap();
        assert!(word.rect[0] >= 10.0 && word.rect[0] < 60.0, "{word:?}");
    }

    mod test_image {
        /// "HELLO WORLD" drawn with blocky 5×7 letters, 4 pixels per dot, black on white.
        pub fn hello() -> (u32, u32, Vec<u8>) {
            const GLYPHS: &[(char, [&str; 7])] = &[
                ('H', ["X...X", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X"]),
                ('E', ["XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "XXXXX"]),
                ('L', ["X....", "X....", "X....", "X....", "X....", "X....", "XXXXX"]),
                ('O', [".XXX.", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."]),
                ('W', ["X...X", "X...X", "X...X", "X.X.X", "X.X.X", "XX.XX", "X...X"]),
                ('R', ["XXXX.", "X...X", "X...X", "XXXX.", "X.X..", "X..X.", "X...X"]),
                ('D', ["XXXX.", "X...X", "X...X", "X...X", "X...X", "X...X", "XXXX."]),
            ];
            let text = "HELLO WORLD";
            let (dot, x0, y0) = (4u32, 40u32, 40u32);
            let (w, h) = (x0 * 2 + text.len() as u32 * 6 * dot, y0 * 2 + 7 * dot);
            let mut px = vec![255u8; (w * h * 4) as usize];
            for (i, c) in text.chars().enumerate() {
                let Some((_, rows)) = GLYPHS.iter().find(|g| g.0 == c) else { continue };
                for (ry, row) in rows.iter().enumerate() {
                    for (rx, b) in row.bytes().enumerate() {
                        if b != b'X' {
                            continue;
                        }
                        for dy in 0..dot {
                            for dx in 0..dot {
                                let x = x0 + (i as u32 * 6 + rx as u32) * dot + dx;
                                let y = y0 + ry as u32 * dot + dy;
                                let o = ((y * w + x) * 4) as usize;
                                px[o..o + 3].copy_from_slice(&[0, 0, 0]);
                            }
                        }
                    }
                }
            }
            (w, h, px)
        }
    }
}
