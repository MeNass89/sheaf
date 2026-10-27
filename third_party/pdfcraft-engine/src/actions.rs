//! Action Wizard: named sequences of steps run over files (Acrobat's guided actions), the
//! built-in actions, and running an action over many files (batch processing).

use std::sync::Arc;

use crate::{DocId, Edit, EditError, Session};

/// One step of an action.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Scan & OCR ▸ Recognize text (pages without text).
    RecognizeText,
    /// Reduce file size.
    ReduceFileSize,
    /// Sheaf native optimization presets; no Ghostscript process.
    Compress(String),
    /// Native PDF/A conversion, refused if verification still finds nonconformities.
    ConvertPdfA(String),
    /// position|start|size|format, configured by the Action Wizard controls.
    PageNumbers(String),
    /// Remove hidden information (every category).
    RemoveHiddenInformation,
    /// Sanitize document.
    Sanitize,
    FlattenComments,
    FlattenFields,
    /// Add a text watermark (diagonal, 50% opaque).
    AddWatermark(String),
    /// Add a footer with this centre text (tokens such as `<<Page 1 of n>>` work).
    AddFooter(String),
    /// Add a header with this centre text.
    AddHeader(String),
    /// Set the document title.
    SetTitle(String),
    SetAuthor(String),
    SetSubject(String),
    SetKeywords(String),
    RotateClockwise,
    RotateCounterclockwise,
    RotateHalfTurn,
    RemoveImages,
    RemoveLinks,
    /// Prepare a form: detect fields.
    DetectFormFields,
    /// Run a JavaScript (console context).
    RunJavaScript(String),
}

impl Step {
    /// Every step, with an empty argument where it takes one (for the step picker).
    pub fn all() -> Vec<Step> {
        vec![
            Step::RecognizeText,
            Step::ReduceFileSize,
            Step::Compress("recommended".into()),
            Step::ConvertPdfA("2b".into()),
            Step::PageNumbers("4|1|8|Page <<1>> of <<n>>".into()),
            Step::RemoveHiddenInformation,
            Step::Sanitize,
            Step::FlattenComments,
            Step::FlattenFields,
            Step::AddWatermark(String::new()),
            Step::AddHeader(String::new()),
            Step::AddFooter(String::new()),
            Step::SetTitle(String::new()),
            Step::SetAuthor(String::new()),
            Step::SetSubject(String::new()),
            Step::SetKeywords(String::new()),
            Step::RotateClockwise,
            Step::RotateCounterclockwise,
            Step::RotateHalfTurn,
            Step::RemoveImages,
            Step::RemoveLinks,
            Step::DetectFormFields,
            Step::RunJavaScript(String::new()),
        ]
    }

    /// A stable id (agents, saved actions).
    pub fn id(&self) -> &'static str {
        match self {
            Step::RecognizeText => "recognize_text",
            Step::ReduceFileSize => "reduce_file_size",
            Step::Compress(_) => "compress",
            Step::ConvertPdfA(_) => "pdfa",
            Step::PageNumbers(_) => "page_numbers",
            Step::RemoveHiddenInformation => "remove_hidden_information",
            Step::Sanitize => "sanitize",
            Step::FlattenComments => "flatten_comments",
            Step::FlattenFields => "flatten_fields",
            Step::AddWatermark(_) => "add_watermark",
            Step::AddHeader(_) => "add_header",
            Step::AddFooter(_) => "add_footer",
            Step::SetTitle(_) => "set_title",
            Step::SetAuthor(_) => "set_author",
            Step::SetSubject(_) => "set_subject",
            Step::SetKeywords(_) => "set_keywords",
            Step::RotateClockwise => "rotate_clockwise",
            Step::RotateCounterclockwise => "rotate_counterclockwise",
            Step::RotateHalfTurn => "rotate_half_turn",
            Step::RemoveImages => "remove_images",
            Step::RemoveLinks => "remove_links",
            Step::DetectFormFields => "detect_form_fields",
            Step::RunJavaScript(_) => "run_javascript",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Step::RecognizeText => "Recognize text",
            Step::ReduceFileSize => "Reduce file size",
            Step::Compress(_) => "Compress PDF",
            Step::ConvertPdfA(_) => "Convert to PDF/A",
            Step::PageNumbers(_) => "Add page numbers",
            Step::RemoveHiddenInformation => "Remove hidden information",
            Step::Sanitize => "Sanitize document",
            Step::FlattenComments => "Flatten comments",
            Step::FlattenFields => "Flatten form fields",
            Step::AddWatermark(_) => "Add watermark",
            Step::AddHeader(_) => "Add header",
            Step::AddFooter(_) => "Add footer",
            Step::SetTitle(_) => "Set document title",
            Step::SetAuthor(_) => "Set document author",
            Step::SetSubject(_) => "Set document subject",
            Step::SetKeywords(_) => "Set document keywords",
            Step::RotateClockwise => "Rotate pages clockwise",
            Step::RotateCounterclockwise => "Rotate pages counterclockwise",
            Step::RotateHalfTurn => "Turn pages upside down",
            Step::RemoveImages => "Remove all images",
            Step::RemoveLinks => "Remove all links",
            Step::DetectFormFields => "Detect form fields",
            Step::RunJavaScript(_) => "Run a JavaScript",
        }
    }

    /// The step's text argument, if it takes one.
    pub fn arg(&self) -> Option<&str> {
        match self {
            Step::AddWatermark(s) | Step::AddHeader(s) | Step::AddFooter(s) | Step::SetTitle(s) | Step::SetAuthor(s) | Step::SetSubject(s) | Step::SetKeywords(s) | Step::RunJavaScript(s) | Step::Compress(s) | Step::ConvertPdfA(s) | Step::PageNumbers(s) => Some(s),
            _ => None,
        }
    }

    pub fn arg_mut(&mut self) -> Option<&mut String> {
        match self {
            Step::AddWatermark(s) | Step::AddHeader(s) | Step::AddFooter(s) | Step::SetTitle(s) | Step::SetAuthor(s) | Step::SetSubject(s) | Step::SetKeywords(s) | Step::RunJavaScript(s) | Step::Compress(s) | Step::ConvertPdfA(s) | Step::PageNumbers(s) => Some(s),
            _ => None,
        }
    }

    /// Build a step from its id and argument.
    pub fn from_id(id: &str, arg: &str) -> Option<Step> {
        let mut s = Step::all().into_iter().find(|s| s.id() == id)?;
        if let Some(a) = s.arg_mut() {
            *a = arg.to_string();
        }
        if matches!(&s, Step::Compress(v) if !["low", "recommended", "extreme", "lossless"].contains(&v.as_str())) { return None; }
        if matches!(&s, Step::ConvertPdfA(v) if !["2b", "3b"].contains(&v.as_str())) { return None; }
        if let Step::PageNumbers(v) = &s { number_settings(v).ok()?; }
        Some(s)
    }
}

/// Parse the Wizard's bounded page-number options before changing any pages.
pub fn number_settings(value: &str) -> Result<crate::HeaderFooter, EditError> {
    let fields: Vec<_> = value.splitn(4, '|').collect();
    let bad = || EditError::Invalid("Page numbers need position 0–5, start 1–1000000, size 4–144 and text".into());
    if fields.len() != 4 { return Err(bad()); }
    let position = fields[0].parse::<usize>().map_err(|_| bad())?;
    let start = fields[1].parse::<u32>().map_err(|_| bad())?;
    let size = fields[2].parse::<f64>().map_err(|_| bad())?;
    if position > 5 || !(1..=1_000_000).contains(&start) || !size.is_finite() || !(4.0..=144.0).contains(&size) || fields[3].is_empty() { return Err(bad()); }
    let mut settings = crate::HeaderFooter { start_number: start, font_size: size, ..Default::default() };
    settings.text[position] = fields[3].into();
    Ok(settings)
}

/// A named action.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub name: String,
    pub description: String,
    pub steps: Vec<Step>,
    /// One of PdfCraft's own actions (can't be edited or deleted).
    pub builtin: bool,
}

/// The built-in actions.
pub fn builtin() -> Vec<Action> {
    let a = |name: &str, description: &str, steps: Vec<Step>| Action { name: name.into(), description: description.into(), steps, builtin: true };
    vec![
        a(
            "Prepare for Distribution",
            "Remove hidden information and flatten comments and form fields, so recipients see only the pages.",
            vec![Step::RemoveHiddenInformation, Step::FlattenComments, Step::FlattenFields],
        ),
        a("Optimize Scanned Documents", "Make scans searchable, then make them smaller.", vec![Step::RecognizeText, Step::ReduceFileSize]),
        a("Add Page Numbers", "Number every page in the footer.", vec![Step::AddFooter("Page <<1>> of <<n>>".into())]),
        a("Mark as Confidential", "A diagonal CONFIDENTIAL watermark on every page.", vec![Step::AddWatermark("CONFIDENTIAL".into())]),
        a("Sanitize Documents", "Remove hidden information, metadata, scripts and embedded content, and rewrite the files.", vec![Step::Sanitize]),
        a("Archive Documents (PDF/A-2b)", "Convert with the native PDF/A engine. Publish only if the built-in conformance check passes; unresolved font or colour issues stop the file.", vec![Step::ConvertPdfA("2b".into())]),
        a("Archive Documents (PDF/A-3b)", "Create a verified PDF/A-3b archive copy. Native conversion may refuse files needing font or colour repair.", vec![Step::ConvertPdfA("3b".into())]),
        a("Prepare a Form", "Detect form fields on the pages. Open the result to review names, positions and reading order before distributing.", vec![Step::DetectFormFields]),
        a("Optimize for Sharing", "Reduce image resolution to 150 ppi and use JPEG quality 70. Keeps tags and document content; does not linearize the PDF for fast web view.", vec![Step::Compress("recommended".into())]),
    ]
}

/// What running an action on one file did.
#[derive(Clone, Debug, PartialEq)]
pub struct FileResult {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    /// One line per step.
    pub log: Vec<String>,
}

/// Commands which can be applied to the open reader document as one undoable edit.
/// Replacement/export jobs must keep their own progress and publication flow.
pub fn command_edit(step: &Step, pages: usize) -> Result<Edit, EditError> {
    let all: Vec<usize> = (0..pages).collect();
    Ok(match step {
        Step::PageNumbers(value) => Edit::AddHeaderFooter { pages: all, settings: number_settings(value)?, replace: false },
        Step::RemoveHiddenInformation => Edit::RemoveHidden { which: crate::HIDDEN.to_vec() },
        Step::Sanitize => Edit::Sanitize,
        Step::FlattenComments => Edit::Flatten { comments: true, fields: false },
        Step::FlattenFields => Edit::Flatten { comments: false, fields: true },
        Step::AddWatermark(t) => {
            Edit::AddWatermark { pages: all, settings: crate::Watermark { text: t.clone(), ..Default::default() }, replace: false, file: None }
        }
        Step::AddHeader(t) | Step::AddFooter(t) => {
            let mut settings = crate::HeaderFooter::default();
            settings.text[if matches!(step, Step::AddHeader(_)) { 1 } else { 4 }] = t.clone();
            Edit::AddHeaderFooter { pages: all, settings, replace: false }
        }
        Step::SetTitle(t) => Edit::SetInfo { key: "Title".into(), value: t.clone() },
        Step::SetAuthor(t) => Edit::SetInfo { key: "Author".into(), value: t.clone() },
        Step::SetSubject(t) => Edit::SetInfo { key: "Subject".into(), value: t.clone() },
        Step::SetKeywords(t) => Edit::SetInfo { key: "Keywords".into(), value: t.clone() },
        Step::RotateClockwise => Edit::RotatePages { pages: all, degrees: 90 },
        Step::RotateCounterclockwise => Edit::RotatePages { pages: all, degrees: 270 },
        Step::RotateHalfTurn => Edit::RotatePages { pages: all, degrees: 180 },
        Step::RemoveImages => Edit::RemoveImages { pages: all },
        Step::RemoveLinks => Edit::RemoveLinks { pages: None },
        _ => return Err(EditError::Invalid(format!("{} requires Run on PDFs or its dedicated reader tool", step.label()))),
    })
}

fn apply_step(s: &mut Session, id: DocId, step: &Step, log: &mut Vec<String>) -> Result<Option<DocId>, EditError> {
    let pages = s.get(id).map_or(0, |d| d.info.pages.len());
    let edit = match step {
        Step::RecognizeText => {
            let found = s.recognize_text(id, &[], crate::ocr::OcrSettings::default()).map_err(EditError::Invalid)?;
            log.push(format!("Recognize text: {} words", found.iter().map(|p| p.words.len()).sum::<usize>()));
            return Ok(None);
        }
        Step::ConvertPdfA(value) => {
            let level = match value.as_str() {
                "2b" => crate::pdfa::Level::A2b, "3b" => crate::pdfa::Level::A3b,
                _ => return Err(EditError::Invalid("Choose PDF/A-2b or PDF/A-3b".into())),
            };
            s.apply(id, Edit::ConvertPdfA { level })?;
            let issues = s.get(id).ok_or(EditError::NoDocument)?.pdfa_verify(level);
            if !issues.is_empty() {
                return Err(EditError::Invalid(format!("Native PDF/A conversion still has {} conformance issue(s): {}. No output was published.", issues.len(), issues[0].message)));
            }
            log.push(format!("Verified PDF/A-{value}"));
            return Ok(None);
        }
        Step::ReduceFileSize | Step::Compress(_) => {
            let mut settings = crate::optimize::Settings::default();
            if let Step::Compress(level) = step {
                let (ppi, quality) = match level.as_str() {
                    "low" => (300.0, crate::optimize::Compression::Jpeg(85)),
                    "recommended" => (150.0, crate::optimize::Compression::Jpeg(70)),
                    "extreme" => (96.0, crate::optimize::Compression::Jpeg(45)),
                    "lossless" => (300.0, crate::optimize::Compression::Flate),
                    _ => return Err(EditError::Invalid("Choose low, recommended, extreme or lossless compression".into())),
                };
                settings.color = crate::optimize::ImageSettings { downsample: level != "lossless", target_ppi: ppi, above_ppi: ppi * 1.5, compression: quality };
                settings.gray = settings.color;
            }
            let (bytes, _) = s.optimized_bytes(id, &settings, &[])?;
            let before = s.get(id).map_or(0, |d| d.bytes.len());
            log.push(format!("Reduce file size: {} → {} bytes", before, bytes.len()));
            let name = s.get(id).map(|d| d.name.clone()).unwrap_or_default();
            let password = s.get(id).and_then(|d| d.editor.as_ref()).and_then(|e| e.keys.reopen.clone());
            s.close(id);
            let new = s.open(name, None, bytes, password.as_deref()).map_err(|e| EditError::Reopen(e.to_string()))?;
            return Ok(Some(new));
        }
        Step::DetectFormFields => {
            let n = s.auto_detect_fields(id, &[])?.len();
            log.push(format!("Detect form fields: {n}"));
            return Ok(None);
        }
        Step::RunJavaScript(js) => {
            let o = s.run_javascript(id, js, None)?;
            if let Some(e) = o.error { return Err(EditError::Invalid(e)); }
            log.push("Run a JavaScript".into());
            return Ok(None);
        }
        // A document with no links has nothing to remove: the step is done, not failed (Acrobat's
        // Remove all links in an action).
        Step::RemoveLinks if s.get(id).is_some_and(|d| d.info.links.is_empty()) => {
            log.push(format!("{}: none", step.label()));
            return Ok(None);
        }
        _ => command_edit(step, pages)?,
    };
    // Sheaf: permissions, unsupported content and write failures must stop an action.
    // Flattening an empty set already succeeds in the editor; do not swallow real errors.
    s.apply(id, edit)?;
    log.push(step.label().to_string());
    Ok(None)
}

/// Run `action` on one file. `progress(step, steps)` is called before each step.
pub fn run_on(action: &Action, name: &str, bytes: Arc<Vec<u8>>, mut progress: impl FnMut(usize, usize)) -> Result<FileResult, String> {
    run_guided(action, name, bytes, None, |i, step| {
        progress(i, action.steps.len());
        Some(step.clone())
    })
}

/// Sheaf's guided runner keeps the document open for the sequence. A prompt can replace a
/// step's settings or cancel; passwords stay in memory and are never part of an action file.
pub fn run_guided(
    action: &Action, name: &str, bytes: Arc<Vec<u8>>, password: Option<&str>,
    mut before: impl FnMut(usize, &Step) -> Option<Step>,
) -> Result<FileResult, String> {
    let mut s = Session::new();
    let mut id = s.open(name, None, bytes, password).map_err(|e| e.to_string())?;
    if let Some(why) = s.get(id).and_then(|d| d.read_only_reason.clone()) {
        return Err(why);
    }
    let mut log = Vec::new();
    for (i, step) in action.steps.iter().enumerate() {
        let step = before(i, step).ok_or("Action cancelled")?;
        if let Some(new) = apply_step(&mut s, id, &step, &mut log).map_err(|e| format!("{}: {e}", step.label()))? {
            id = new;
        }
        // Batch outputs cannot be undone in this private session. Drop prior revisions now,
        // rather than retaining up to 100 full document states until the last step.
        if let Ok(doc) = s.doc_mut(id) && let Some(editor) = doc.editor.as_mut() {
            editor.undo.clear(); editor.redo.clear();
        }
    }
    let d = s.get(id).ok_or("the document was lost")?;
    // A signed file keeps its signatures: incremental. Otherwise a clean full rewrite.
    let bytes = if d.is_signed() { s.save_bytes(id) } else { s.save_full_bytes(id) }.map_err(|e| e.to_string())?;
    Ok(FileResult { name: name.to_string(), bytes, log })
}
