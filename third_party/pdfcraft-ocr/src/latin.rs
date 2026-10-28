//! Sheaf: PaddleOCR's Latin recogniser (PP-OCRv5 mobile, Apache-2.0) reading the lines ocrs finds.
//!
//! ocrs' own recognition model knows the unaccented Latin alphabet only, so a French or German scan
//! came out with "?" for every accented letter (upstream issue #524). This model reads 32 languages
//! written in Latin script, accents included. ocrs still finds the words and the lines; each line is
//! cut out of the page with a little room around it, scaled to the model's 48 px height and read;
//! the classes are decoded greedily (CTC), and the time step of each character gives its place on
//! the line, so every word keeps its own box.

use rten_tensor::prelude::*;
use std::path::Path;

/// The model's input height, in pixels.
pub(crate) const HEIGHT: usize = 48;
/// Longest line read at once, in model pixels (a line across a large-format page).
const MAX_WIDTH: usize = 4_000;

/// One recognised word: its text, its box in image pixels `[left, top, right, bottom]`, and how
/// sure the model is of it (the mean score of its characters, 0 to 1).
pub(crate) type Read = (String, [f32; 4], f32);

pub(crate) struct Latin {
    model: Engine,
    /// The classes after the blank: the dictionary's characters, then a space.
    chars: Vec<char>,
}

/// What runs the model: onnxruntime where it is built and loads it, rten otherwise.
enum Engine {
    Onnx(crate::onnx::Lines),
    Rten(rten::Model),
}

/// Prepared PaddleOCR line pixels in contiguous BGR/CHW order (48 pixels high).
/// Exposed so the host can compare preprocessing without loading a recognition model.
pub struct RecognitionInput {
    pub width: usize,
    pub pixels: Vec<f32>,
    pub crop_left: usize,
    pub crop_width: usize,
}

/// Cache horizontal bilinear coordinates once per line, then fill contiguous channel planes.
/// This preserves the trained model's exact scaling arithmetic without tensor indexing per pixel.
pub fn recognition_input(rgb: &[u8], width: usize, height: usize, line: [f32; 4]) -> Option<RecognitionInput> {
    if width == 0 || height == 0 || width.checked_mul(height)?.checked_mul(3)? != rgb.len()
        || line.iter().any(|v| !v.is_finite()) { return None; }
    let h = (line[3] - line[1]).max(1.0);
    let pad = h * 0.25;
    let x0 = (line[0] - pad).floor().max(0.0) as usize;
    let x1 = ((line[2] + pad).ceil() as usize).min(width);
    let y0 = (line[1] - pad).floor().max(0.0) as usize;
    let y1 = ((line[3] + pad).ceil() as usize).min(height);
    if x1 <= x0.saturating_add(1) || y1 <= y0.saturating_add(1) { return None; }
    let (cw, ch) = (x1 - x0, y1 - y0);
    let w = ((cw as f32 * HEIGHT as f32 / ch as f32).round() as usize).clamp(8, MAX_WIDTH);
    let (sx, sy) = (cw as f32 / w as f32, ch as f32 / HEIGHT as f32);
    let xs: Vec<_> = (0..w).map(|x| {
        let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (cw - 1) as f32);
        let xa = fx.floor() as usize;
        (xa, (xa + 1).min(cw - 1), fx.fract())
    }).collect();
    let plane = HEIGHT * w;
    let mut pixels = vec![0.0; 3 * plane];
    for y in 0..HEIGHT {
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (ch - 1) as f32);
        let (ya, dy) = (fy.floor() as usize, fy.fract());
        let yb = (ya + 1).min(ch - 1);
        for (x, &(xa, xb, dx)) in xs.iter().enumerate() {
            let a = ((y0 + ya) * width + x0 + xa) * 3;
            let b = ((y0 + ya) * width + x0 + xb) * 3;
            let c = ((y0 + yb) * width + x0 + xa) * 3;
            let d = ((y0 + yb) * width + x0 + xb) * 3;
            for channel in 0..3 {
                let value = f32::from(rgb[a + channel]) * (1.0 - dx) * (1.0 - dy)
                    + f32::from(rgb[b + channel]) * dx * (1.0 - dy)
                    + f32::from(rgb[c + channel]) * (1.0 - dx) * dy
                    + f32::from(rgb[d + channel]) * dx * dy;
                pixels[(2 - channel) * plane + y * w + x] = value / 127.5 - 1.0;
            }
        }
    }
    Some(RecognitionInput { width: w, pixels, crop_left: x0, crop_width: cw })
}

impl Latin {
    pub(crate) fn load(model: &Path, dict: &Path, prepack: bool) -> Result<Latin, String> {
        let text = std::fs::read_to_string(dict).map_err(|e| format!("{}: {e}", dict.display()))?;
        let mut chars: Vec<char> = text.lines().filter_map(|l| l.chars().next()).collect();
        chars.push(' ');
        // onnxruntime first; rten when it is not built here, cannot run the model, or
        // SHEAF_OCR_ENGINE=rten asks for it (to compare the two).
        let onnx = || -> Option<Engine> {
            if std::env::var_os("SHEAF_OCR_ENGINE").is_some_and(|v| v == "rten") {
                return None;
            }
            let loaded = std::fs::read(model).map_err(|e| format!("{}: {e}", model.display())).and_then(|bytes| crate::onnx::Lines::load(&bytes));
            match loaded {
                Ok(lines) => Some(Engine::Onnx(lines)),
                Err(e) => {
                    eprintln!("OCR: onnxruntime unavailable ({e}); lines are read with rten");
                    None
                }
            }
        };
        let model = match onnx() {
            Some(engine) => engine,
            // The same recognition weights serve every line. Packing them once avoids repeating
            // the matrix layout conversion in every inference (the big-scan performance gap).
            None => Engine::Rten(rten::ModelOptions::with_all_ops().prepack_weights(prepack).load_file(model).map_err(|e| format!("{}: {e}", model.display()))?),
        };
        Ok(Latin { model, chars })
    }

    /// The engine reading the lines: "onnxruntime" or "rten".
    pub(crate) fn engine(&self) -> &'static str {
        match self.model {
            Engine::Onnx(_) => "onnxruntime",
            Engine::Rten(_) => "rten",
        }
    }

    /// Reads the line whose box (image pixels, `[left, top, right, bottom]`) is `line` in `rgb`
    /// (3 bytes a pixel, `width` × `height`): its words, left to right.
    pub(crate) fn read_line(&self, rgb: &[u8], width: usize, height: usize, line: [f32; 4]) -> Result<Vec<Read>, String> {
        let Some(input) = recognition_input(rgb, width, height, line) else { return Ok(Vec::new()); };
        let (x0, cw, w) = (input.crop_left, input.crop_width, input.width);
        let (shape, probs) = match &self.model {
            Engine::Onnx(lines) => lines.run(w, &input.pixels)?,
            Engine::Rten(model) => {
                let input = rten_tensor::NdTensor::from_data([1, 3, HEIGHT, w], input.pixels);
                let out: rten_tensor::Tensor<f32> = model.run_one(input.view().into(), None).map_err(|e| e.to_string())?.try_into().map_err(|e| format!("{e:?}"))?;
                (out.shape().to_vec(), out.to_vec())
            }
        };
        let (steps, classes) = match shape.as_slice() {
            [1, t, c] => (*t, *c),
            other => return Err(format!("unexpected output shape {other:?}")),
        };
        let chars: Vec<(char, usize, f32)> = ctc(&probs, steps, classes).into_iter().filter_map(|(k, t, p)| self.chars.get(k - 1).map(|c| (*c, t, p))).collect();
        // Each step covers `step` pixels of the line in the image.
        let step = cw as f32 / steps.max(1) as f32;
        let mut words = Vec::new();
        let mut word = String::new();
        let mut span = (0, 0);
        let mut score = 0.0;
        for (c, t, p) in chars.into_iter().chain(std::iter::once((' ', steps, 1.0))) {
            if c == ' ' {
                if !word.is_empty() {
                    let left = x0 as f32 + span.0 as f32 * step;
                    let right = x0 as f32 + (span.1 + 1) as f32 * step;
                    let n = word.chars().count() as f32;
                    words.push((std::mem::take(&mut word), [left.max(line[0]), line[1], right.min(line[2]).max(left + 1.0), line[3]], score / n));
                    score = 0.0;
                }
                continue;
            }
            if word.is_empty() {
                span.0 = t;
            }
            span.1 = t;
            word.push(c);
            score += p;
        }
        Ok(words)
    }
}

/// Greedy CTC decoding of `steps` rows of `classes` scores: the likeliest class at each step,
/// repeats and blanks (class 0) dropped. Returns each class kept, the step it was read at, and its
/// score there.
fn ctc(probs: &[f32], steps: usize, classes: usize) -> Vec<(usize, usize, f32)> {
    let mut out = Vec::new();
    let mut prev = 0;
    for t in 0..steps {
        let Some(row) = probs.get(t * classes..(t + 1) * classes) else { break };
        let (best, p) = row.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or((0, 0.0), |(k, p)| (k, *p));
        if best != 0 && best != prev {
            out.push((best, t, p));
        }
        prev = best;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::ctc;

    #[test]
    fn ctc_drops_blanks_and_repeats_but_keeps_doubled_letters() {
        // Steps read: a a _ b _ b c (class 0 is the blank): "abbc", "b" twice because a blank parts them.
        let classes = 4;
        let pick = |k: usize| (0..classes).map(|c| if c == k { 1.0 } else { 0.0 }).collect::<Vec<f32>>();
        let probs: Vec<f32> = [1, 1, 0, 2, 0, 2, 3].iter().flat_map(|k| pick(*k)).collect();
        assert_eq!(ctc(&probs, 7, classes), [(1, 0, 1.0), (2, 3, 1.0), (2, 5, 1.0), (3, 6, 1.0)]);
    }
}
