//! `sheaf`: a thin command-line front-end over the sheaf-core engine.
//!
//! Every subcommand prints the path or paths it wrote, one per line, on stdout.
//! Exit codes: 0 success, 1 any error, 2 password required or wrong.

use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use sheaf_core::compress::{self, Level};
use sheaf_core::images::{self, PageLayout};
use sheaf_core::pages::{self, Input};
use sheaf_core::crop::{self, CropSpec};
use sheaf_core::numbers::{self, NumberOptions, NumberText};
use sheaf_core::compare::{self, CompareMode};
use sheaf_core::excel::{self, SheetLayout};
use sheaf_core::export::{self, ExportOptions, Office};
use sheaf_core::ocr::{self, OcrOptions};
use sheaf_core::pdfa::{self, PdfALevel};
use sheaf_core::redact::{self, Pattern, Query};
use sheaf_core::edit::{self, EditSpec, Item};
use sheaf_core::sign::{self, DigitalSpec, Graphic, SignSpec};
use sheaf_core::watermark::{self, Mark, Placement, WatermarkOptions};
use sheaf_core::{office, repair, rotate, security, OpdfError};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "sheaf", version, about = "Sheaf: merge, split, compress, protect and convert PDF files")]
struct Cli {
    /// Password used to open every input PDF
    #[arg(long, global = true)]
    password: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Merge several PDFs into one, in the given order
    Merge {
        #[arg(required = true, num_args = 1..)]
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Split a PDF by page ranges or every N pages into a directory
    #[command(group = clap::ArgGroup::new("how").required(true).args(["ranges", "every"]))]
    Split {
        input: PathBuf,
        /// Page ranges, one file per range, e.g. "1-3,5"
        #[arg(long)]
        ranges: Option<String>,
        /// One file every N pages
        #[arg(long)]
        every: Option<u32>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Extract pages into a new PDF
    Extract {
        input: PathBuf,
        #[arg(long)]
        ranges: String,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Remove pages
    Remove {
        input: PathBuf,
        #[arg(long)]
        pages: String,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Reorder pages: 1-based page numbers, 0 inserts a blank page, e.g. "3,0,1"
    Organize {
        input: PathBuf,
        #[arg(long, value_delimiter = ',', required = true)]
        order: Vec<u32>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Rotate pages clockwise by 90, 180 or 270 degrees (negative = counter-clockwise)
    Rotate {
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        degrees: i64,
        /// Pages to rotate (default: all)
        #[arg(long)]
        pages: Option<String>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Compress a PDF
    Compress {
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = CliLevel::Recommended)]
        level: CliLevel,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Encrypt a PDF with a password (AES-256)
    Protect {
        input: PathBuf,
        #[arg(long)]
        user_password: String,
        #[arg(long)]
        owner_password: Option<String>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Remove the password from a PDF (needs --password)
    Unlock {
        input: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Rebuild a damaged PDF
    Repair {
        input: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Render every page to JPEG in a directory
    ToJpg {
        input: PathBuf,
        #[arg(long, default_value_t = 150)]
        dpi: u32,
        #[arg(long, default_value_t = 85, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: u8,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Build a PDF from images, one per page
    FromImages {
        #[arg(required = true, num_args = 1..)]
        images: Vec<PathBuf>,
        /// Place each image on an A4 page with a 0.5 in margin (default: page fits the image)
        #[arg(long)]
        a4: bool,
        /// Landscape A4 (with --a4)
        #[arg(long, requires = "a4")]
        landscape: bool,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Stamp page numbers
    Number {
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = Position::BottomCenter)]
        position: Position,
        /// Double-sided: even pages mirror the left/right position
        #[arg(long)]
        double_sided: bool,
        /// First page is a cover page (not numbered)
        #[arg(long)]
        cover: bool,
        #[arg(long, value_enum, default_value_t = Margin::Recommended)]
        margin: Margin,
        /// First page to number (default: 1)
        #[arg(long)]
        from: Option<u32>,
        /// Last page to number (default: the last)
        #[arg(long)]
        to: Option<u32>,
        /// Number shown on the first numbered page
        #[arg(long, default_value_t = 1)]
        start: u32,
        #[arg(long, value_enum, default_value_t = NumberFormat::Number)]
        text: NumberFormat,
        /// With --text custom: text with {n} (page number) and {p} (page count)
        #[arg(long, required_if_eq("text", "custom"))]
        custom: Option<String>,
        #[arg(long, default_value_t = 12.0)]
        font_size: f64,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Stamp a text or image watermark
    #[command(group = clap::ArgGroup::new("mark").required(true).args(["text", "image"]))]
    Watermark {
        input: PathBuf,
        #[arg(long)]
        text: Option<String>,
        /// PNG or JPEG file
        #[arg(long)]
        image: Option<PathBuf>,
        /// Text size in points (1 to 500), or "auto"
        #[arg(long, default_value = "48", value_parser = parse_font_size)]
        font_size: f64,
        /// Text colour as RRGGBB
        #[arg(long, default_value = "000000")]
        color: String,
        /// Image size relative to the page, 0.05 to 1 (1 fits the page)
        #[arg(long, default_value_t = 0.5)]
        scale: f64,
        /// A grid position, or mosaic (repeated over the page)
        #[arg(long, value_enum, default_value_t = WmPosition::Center)]
        position: WmPosition,
        /// Transparency in percent: 0, 25, 50 or 75
        #[arg(long, default_value_t = 0, value_parser = parse_transparency)]
        transparency: u8,
        /// Rotation in degrees: 0, 45, 90, 180, 270 or -45
        #[arg(long, default_value_t = 0, allow_hyphen_values = true, value_parser = parse_rotation)]
        rotate: i32,
        /// Draw below the page content
        #[arg(long)]
        below: bool,
        #[arg(long)]
        from: Option<u32>,
        #[arg(long)]
        to: Option<u32>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Crop pages by margins in millimetres (as the page is displayed), or remove a crop
    Crop {
        input: PathBuf,
        #[arg(long, default_value_t = 0.0)]
        top: f64,
        #[arg(long, default_value_t = 0.0)]
        bottom: f64,
        #[arg(long, default_value_t = 0.0)]
        left: f64,
        #[arg(long, default_value_t = 0.0)]
        right: f64,
        /// Remove the crop instead (back to the full page)
        #[arg(long, conflicts_with_all = ["top", "bottom", "left", "right"])]
        remove: bool,
        /// Pages to crop (default: all)
        #[arg(long)]
        pages: Option<String>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Recognise the text of scanned pages and add a searchable text layer
    Ocr {
        input: PathBuf,
        /// Document language code (the models read Latin script)
        #[arg(long, default_value = "en")]
        language: String,
        /// Also read pages that already contain text (only new words are added)
        #[arg(long)]
        all_pages: bool,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Export to an editable Word document (.docx); the layout is approximate
    ToWord {
        input: PathBuf,
        /// Recognise the text of scanned pages first
        #[arg(long)]
        ocr: bool,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Export to PowerPoint (.pptx), one slide per page; the layout is approximate
    ToPowerpoint {
        input: PathBuf,
        /// Recognise the text of scanned pages first
        #[arg(long)]
        ocr: bool,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Export the tables to Excel (.xlsx); pages without a table keep their text in column A
    ToExcel {
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = CliSheets::OneSheet)]
        layout: CliSheets,
        /// Recognise the text of scanned pages first
        #[arg(long)]
        ocr: bool,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Compare an original and a changed PDF: a report (text) or the changes marked (overlay)
    Compare {
        /// The original version
        old: PathBuf,
        /// The changed version
        new: PathBuf,
        #[arg(long, value_enum, default_value_t = CliCompare::Text)]
        mode: CliCompare,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Black out text, patterns or whole pages for good, then check no text remains under them
    Redact {
        input: PathBuf,
        /// Text to redact wherever it appears (case-insensitive); repeat for several
        #[arg(long)]
        text: Vec<String>,
        /// Kind of content to redact wherever it appears; repeat for several
        #[arg(long, value_enum)]
        pattern: Vec<CliPattern>,
        /// Pages to redact entirely, e.g. "3" or "1-2,5"
        #[arg(long)]
        pages: Option<String>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Place a signature (typed, drawn or an image) on a page, optionally signed with a certificate
    #[command(group = clap::ArgGroup::new("graphic").required(true).args(["text", "image", "strokes"]))]
    Sign {
        input: PathBuf,
        /// Page to sign (1-based)
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        page: u32,
        /// Top-left corner of the signature, in points from the top-left of the page as displayed
        #[arg(long, value_parser = parse_point, allow_hyphen_values = true)]
        at: (f64, f64),
        /// Width of the signature in points (its height follows the graphic)
        #[arg(long, default_value_t = sign::DEFAULT_WIDTH, value_parser = parse_width)]
        width: f64,
        /// The name to sign with, in a script font
        #[arg(long)]
        text: Option<String>,
        /// A PNG or JPEG image of the signature
        #[arg(long)]
        image: Option<PathBuf>,
        /// A drawn signature: JSON strokes [[[x, y], …], …], y up, where the pad is 1 wide
        #[arg(long)]
        strokes: Option<PathBuf>,
        /// Also sign digitally with this certificate (a .p12 or .pfx digital ID)
        #[arg(long)]
        p12: Option<PathBuf>,
        /// Password of the certificate
        #[arg(long, requires = "p12")]
        p12_password: Option<String>,
        /// Reason for signing, shown in the certificate signature
        #[arg(long, requires = "p12")]
        reason: Option<String>,
        /// Where it was signed, shown in the certificate signature
        #[arg(long, requires = "p12")]
        location: Option<String>,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Add text and images on top of pages. Each --text or --image starts an item; the --page,
    /// --at, --size, --color and --width after it apply to that item
    #[command(after_help = "Example: sheaf edit in.pdf --text \"Approved\" --page 1 --at 72,72 --size 18 --color #c00000 \\\n    --image logo.png --page 2 --at 400,40 --width 120 -o out.pdf")]
    Edit {
        input: PathBuf,
        /// Text to add (\n starts a new line)
        #[arg(long)]
        text: Vec<String>,
        /// A PNG or JPEG image to add
        #[arg(long)]
        image: Vec<PathBuf>,
        /// Page of the item (1-based, default 1)
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        page: Vec<u32>,
        /// Top-left corner of the item, in points from the top-left of the page as displayed
        #[arg(long, value_parser = parse_point, allow_hyphen_values = true)]
        at: Vec<(f64, f64)>,
        /// Font size of a text item in points (default 12)
        #[arg(long)]
        size: Vec<f64>,
        /// Colour of a text item, #RRGGBB (default black)
        #[arg(long, value_parser = parse_rgb)]
        color: Vec<[u8; 3]>,
        /// Width of an image item in points (its height follows the image)
        #[arg(long, value_parser = parse_width)]
        width: Vec<f64>,
        #[arg(short, long)]
        out: PathBuf,
        /// The items in command-line order (filled from the argument positions).
        #[arg(skip)]
        items: Vec<CliItem>,
    },
    /// Convert to PDF/A for archiving, then check the result
    Pdfa {
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = CliPdfA::A2b)]
        level: CliPdfA,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Convert an Office or HTML document to PDF through LibreOffice
    Office {
        input: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CliPattern {
    Email,
    Phone,
    CreditCard,
    Ssn,
    Date,
}

impl From<CliPattern> for Pattern {
    fn from(p: CliPattern) -> Self {
        match p {
            CliPattern::Email => Pattern::Email,
            CliPattern::Phone => Pattern::Phone,
            CliPattern::CreditCard => Pattern::CreditCard,
            CliPattern::Ssn => Pattern::Ssn,
            CliPattern::Date => Pattern::Date,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum CliCompare {
    /// A report PDF listing the word changes
    Text,
    /// The changed PDF with the differences highlighted
    Overlay,
}

#[derive(Clone, Copy, ValueEnum)]
enum CliSheets {
    /// Every table on one sheet
    OneSheet,
    /// One sheet per table
    SheetPerTable,
}

#[derive(Clone, Copy, ValueEnum)]
enum CliPdfA {
    /// PDF/A-2b (recommended)
    #[value(name = "2b")]
    A2b,
    /// PDF/A-3b (allows attachments)
    #[value(name = "3b")]
    A3b,
}

#[derive(Clone, Copy, ValueEnum)]
enum CliLevel {
    Low,
    Recommended,
    Extreme,
}

/// A cell of the 3×3 position grid.
#[derive(Clone, Copy, ValueEnum)]
enum Position {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Clone, Copy, ValueEnum)]
enum WmPosition {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Mosaic,
}

#[derive(Clone, Copy, ValueEnum)]
enum Margin {
    Small,
    Recommended,
    Big,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum NumberFormat {
    /// "{n}"
    Number,
    /// "Page {n}"
    Page,
    /// "Page {n} of {p}"
    PageOf,
    /// --custom text
    Custom,
}

/// "auto" is 0 (automatic size); any other value is checked by the core (1 to 500).
fn parse_font_size(s: &str) -> Result<f64, String> {
    if s.eq_ignore_ascii_case("auto") {
        return Ok(0.0);
    }
    s.parse().map_err(|_| "a font size in points, or auto".into())
}

fn parse_point(s: &str) -> Result<(f64, f64), String> {
    let bad = || format!("expected X,Y in points (e.g. 72,600), got \"{s}\"");
    let (x, y) = s.split_once(',').ok_or_else(bad)?;
    let (x, y) = (x.trim().parse::<f64>().map_err(|_| bad())?, y.trim().parse::<f64>().map_err(|_| bad())?);
    if x.is_finite() && y.is_finite() { Ok((x, y)) } else { Err(bad()) }
}

fn parse_rgb(s: &str) -> Result<[u8; 3], String> {
    let h = s.trim().trim_start_matches('#');
    let bad = || format!("expected a colour as #RRGGBB, got \"{s}\"");
    if h.len() != 6 || !h.is_ascii() {
        return Err(bad());
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| bad());
    Ok([c(0)?, c(2)?, c(4)?])
}

/// One `sheaf edit` item as given on the command line.
#[derive(Clone, Debug, Default)]
struct CliItem {
    /// The text, or the image path.
    text: Option<String>,
    image: Option<PathBuf>,
    page: Option<u32>,
    at: Option<(f64, f64)>,
    size: Option<f64>,
    color: Option<[u8; 3]>,
    width: Option<f64>,
}

/// Groups `sheaf edit`'s options into items by their positions: each --text or --image starts
/// an item, and the options after it (up to the next one) belong to it.
fn edit_items(m: &ArgMatches) -> Result<Vec<CliItem>, String> {
    fn at<T: Clone + Send + Sync + 'static>(m: &ArgMatches, id: &str) -> Vec<(usize, T)> {
        match (m.indices_of(id), m.get_many::<T>(id)) {
            (Some(i), Some(v)) => i.zip(v.cloned()).collect(),
            _ => Vec::new(),
        }
    }
    let mut starts: Vec<(usize, CliItem)> = at::<String>(m, "text").into_iter().map(|(i, t)| (i, CliItem { text: Some(t), ..Default::default() })).collect();
    starts.extend(at::<PathBuf>(m, "image").into_iter().map(|(i, p)| (i, CliItem { image: Some(p), ..Default::default() })));
    starts.sort_by_key(|(i, _)| *i);
    if starts.is_empty() {
        return Err("nothing to add: give at least one --text or --image".into());
    }
    let mut set = |id: &str, i: usize, f: &mut dyn FnMut(&mut CliItem) -> bool| -> Result<(), String> {
        let item = starts.iter_mut().rev().find(|(s, _)| *s < i).map(|(_, it)| it).ok_or_else(|| format!("--{id} must come after the --text or --image it applies to"))?;
        if f(item) { Ok(()) } else { Err(format!("--{id} is given twice for one item")) }
    };
    fn put<T>(slot: &mut Option<T>, v: T) -> bool {
        slot.replace(v).is_none()
    }
    for (i, v) in at::<u32>(m, "page") {
        set("page", i, &mut |it| put(&mut it.page, v))?;
    }
    for (i, v) in at::<(f64, f64)>(m, "at") {
        set("at", i, &mut |it| put(&mut it.at, v))?;
    }
    for (i, v) in at::<f64>(m, "size") {
        set("size", i, &mut |it| put(&mut it.size, v))?;
    }
    for (i, v) in at::<[u8; 3]>(m, "color") {
        set("color", i, &mut |it| put(&mut it.color, v))?;
    }
    for (i, v) in at::<f64>(m, "width") {
        set("width", i, &mut |it| put(&mut it.width, v))?;
    }
    Ok(starts.into_iter().map(|(_, it)| it).collect())
}

fn parse_width(s: &str) -> Result<f64, String> {
    match s.parse::<f64>() {
        Ok(w) if (10.0..=2000.0).contains(&w) => Ok(w),
        _ => Err(format!("expected a width between 10 and 2000 points, got \"{s}\"")),
    }
}

/// Drawn strokes from a JSON file: an array of strokes, each an array of [x, y] points.
fn read_strokes(path: &Path) -> sheaf_core::Result<Vec<Vec<[f64; 2]>>> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| OpdfError::Unsupported(format!("{}: not a list of strokes ({e})", path.display())))
}

fn parse_transparency(s: &str) -> Result<u8, String> {
    s.parse().ok().filter(|v| [0, 25, 50, 75].contains(v)).ok_or_else(|| "use 0, 25, 50 or 75".into())
}

fn parse_rotation(s: &str) -> Result<i32, String> {
    s.parse().ok().filter(|v| [0, 45, 90, 180, 270, -45].contains(v)).ok_or_else(|| "use 0, 45, 90, 180, 270 or -45".into())
}

/// "RRGGBB" → RGB in 0..=1.
fn parse_color(s: &str) -> sheaf_core::Result<[f64; 3]> {
    let s = s.trim_start_matches('#');
    let bad = || OpdfError::Unsupported(format!("colour \"{s}\" (use RRGGBB)"));
    if s.len() != 6 { return Err(bad()); }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).map(|v| v as f64 / 255.0).map_err(|_| bad());
    Ok([c(0)?, c(2)?, c(4)?])
}

impl From<CliLevel> for Level {
    fn from(l: CliLevel) -> Self {
        match l {
            CliLevel::Low => Level::Low,
            CliLevel::Recommended => Level::Recommended,
            CliLevel::Extreme => Level::Extreme,
        }
    }
}

impl Cmd {
    /// The PDFs whose protection the output loses: every PDF input, except for Protect (output
    /// encrypted by design), Unlock (removing protection is the point) and Compress (it reports
    /// encryption itself).
    fn unprotecting_inputs(&self) -> Vec<&Path> {
        match self {
            Cmd::Merge { inputs, .. } => inputs.iter().map(PathBuf::as_path).collect(),
            Cmd::Split { input, .. }
            | Cmd::Extract { input, .. }
            | Cmd::Remove { input, .. }
            | Cmd::Organize { input, .. }
            | Cmd::Rotate { input, .. }
            | Cmd::Repair { input, .. }
            | Cmd::ToJpg { input, .. } => vec![input],
            // These report it themselves (from the engine), like Compress.
            Cmd::Number { .. } | Cmd::Watermark { .. } | Cmd::Crop { .. } | Cmd::Pdfa { .. } | Cmd::Ocr { .. } |
            Cmd::ToWord { .. } | Cmd::ToPowerpoint { .. } | Cmd::ToExcel { .. } | Cmd::Compare { .. } | Cmd::Redact { .. } |
            // Sign refuses protected inputs.
            Cmd::Sign { .. } |
            Cmd::Edit { .. } |
            Cmd::Compress { .. } | Cmd::Protect { .. } | Cmd::Unlock { .. } | Cmd::FromImages { .. } | Cmd::Office { .. } => vec![],
        }
    }

    /// The directory that must exist before core writes: `-o` itself for
    /// directory outputs (`split`, `to-jpg`), else the parent of `-o`.
    fn out_dir(&self) -> Option<&Path> {
        match self {
            Cmd::Split { out, .. } | Cmd::ToJpg { out, .. } => Some(out),
            Cmd::Merge { out, .. }
            | Cmd::Extract { out, .. }
            | Cmd::Remove { out, .. }
            | Cmd::Organize { out, .. }
            | Cmd::Rotate { out, .. }
            | Cmd::Compress { out, .. }
            | Cmd::Protect { out, .. }
            | Cmd::Unlock { out, .. }
            | Cmd::Repair { out, .. }
            | Cmd::FromImages { out, .. }
            | Cmd::Number { out, .. }
            | Cmd::Watermark { out, .. }
            | Cmd::Crop { out, .. }
            | Cmd::Pdfa { out, .. }
            | Cmd::Ocr { out, .. }
            | Cmd::ToWord { out, .. }
            | Cmd::ToPowerpoint { out, .. }
            | Cmd::ToExcel { out, .. }
            | Cmd::Compare { out, .. }
            | Cmd::Redact { out, .. }
            | Cmd::Sign { out, .. }
            | Cmd::Edit { out, .. }
            | Cmd::Office { out, .. } => out.parent().filter(|p| !p.as_os_str().is_empty()),
        }
    }
}

fn run(cli: Cli) -> sheaf_core::Result<Vec<PathBuf>> {
    if let Some(dir) = cli.cmd.out_dir() {
        std::fs::create_dir_all(dir)?;
    }
    let pw = cli.password;
    let input = |p: PathBuf| Input { path: p, password: pw.clone() };
    let unprotects = cli.cmd.unprotecting_inputs().into_iter().any(|p| input(p.to_path_buf()).is_encrypted());
    let r = dispatch(cli.cmd, &input);
    if unprotects && r.is_ok() {
        eprintln!("{NOT_PROTECTED}");
    }
    r
}

const NOT_PROTECTED: &str = "note: the output is not password-protected";

fn dispatch(cmd: Cmd, input: &dyn Fn(PathBuf) -> Input) -> sheaf_core::Result<Vec<PathBuf>> {
    let one = |r: sheaf_core::Result<PathBuf>| r.map(|p| vec![p]);
    match cmd {
        Cmd::Merge { inputs, out } => {
            let inputs: Vec<Input> = inputs.into_iter().map(input).collect();
            one(pages::merge(&inputs, &out))
        }
        Cmd::Split { input: i, ranges, every, out } => match (ranges, every) {
            (Some(r), _) => pages::split_ranges(&input(i), &r, &out),
            (None, Some(n)) => pages::split_every(&input(i), n, &out),
            (None, None) => unreachable!("clap requires --ranges or --every"),
        },
        Cmd::Extract { input: i, ranges, out } => one(pages::extract(&input(i), &ranges, &out)),
        Cmd::Remove { input: i, pages: p, out } => one(pages::remove(&input(i), &p, &out)),
        Cmd::Organize { input: i, order, out } => one(pages::organize(&input(i), &order, &out)),
        Cmd::Rotate { input: i, degrees, pages: p, out } => one(rotate::rotate(&input(i), p.as_deref(), degrees.rem_euclid(360), &out)),
        Cmd::Compress { input: i, level, out } => {
            let r = compress::compress(&input(i), level.into(), &out)?;
            eprintln!("{} -> {} bytes", r.before, r.after);
            if r.was_encrypted {
                eprintln!("{NOT_PROTECTED}");
            }
            if r.grew {
                eprintln!("note: the unprotected copy is {}% larger than the protected original", grew_pct(r.before, r.after));
            }
            Ok(vec![r.path])
        }
        Cmd::Protect { input: i, user_password, owner_password, out } => {
            one(security::protect(&input(i), &user_password, owner_password.as_deref(), &out))
        }
        Cmd::Unlock { input: i, out } => one(security::unlock(&input(i), &out)),
        Cmd::Repair { input: i, out } => one(repair::repair(&input(i), &out)),
        Cmd::ToJpg { input: i, dpi, quality, out } => images::pdf_to_jpg(&input(i), dpi, quality, &out),
        Cmd::FromImages { images: imgs, a4, landscape, out } => {
            let layout = if a4 { PageLayout::A4 { landscape, margin_pt: 36.0 } } else { PageLayout::FitImage };
            one(images::images_to_pdf(&imgs, layout, &out))
        }
        Cmd::Office { input: i, out } => one(office::office_to_pdf(&i, &out)),
        Cmd::Number { input: i, position, double_sided, cover, margin, from, to, start, text, custom, font_size, out } => {
            let text = match text {
                NumberFormat::Number => NumberText::Number,
                NumberFormat::Page => NumberText::Page,
                NumberFormat::PageOf => NumberText::PageOfTotal,
                NumberFormat::Custom => NumberText::Custom(custom.unwrap_or_default()),
            };
            let o = NumberOptions {
                position: position as u8,
                double_sided,
                cover,
                margin: numbers::MARGINS[margin as usize],
                from,
                to,
                start,
                text,
                font_size,
                ..Default::default()
            };
            warned(numbers::number_pages(&input(i), &o, &out))
        }
        Cmd::Watermark { input: i, text, image, font_size, color, scale, position, transparency, rotate, below, from, to, out } => {
            let mark = match (text, image) {
                (_, Some(path)) => Mark::Image { path, scale },
                (Some(text), None) => Mark::Text { text, font_size, color: parse_color(&color)? },
                (None, None) => unreachable!("clap requires --text or --image"),
            };
            let placement = match position {
                WmPosition::Mosaic => Placement::Mosaic,
                p => Placement::Grid(p as u8),
            };
            let o = WatermarkOptions { mark, placement, opacity: 1.0 - transparency as f64 / 100.0, rotation: rotate as f64, behind: below, from, to };
            warned(watermark::watermark(&input(i), &o, &out))
        }
        Cmd::Crop { input: i, top, bottom, left, right, remove, pages: p, out } => {
            let spec = if remove {
                CropSpec::Remove
            } else if top == 0.0 && bottom == 0.0 && left == 0.0 && right == 0.0 {
                return Err(OpdfError::Unsupported("give a margin (--top, --bottom, --left, --right) or --remove".into()));
            } else {
                CropSpec::Margins { top: crop::mm(top), bottom: crop::mm(bottom), left: crop::mm(left), right: crop::mm(right) }
            };
            warned(crop::crop(&input(i), p.as_deref(), &spec, &out))
        }
        Cmd::Ocr { input: i, language, all_pages, out } => {
            let opts = OcrOptions { language, read_text_pages: all_pages, ..Default::default() };
            let r = ocr::ocr(&input(i), &opts, &out, &mut |_| true)?;
            let page = |n: usize| if n == 1 { "page" } else { "pages" };
            eprintln!("{} {} read, {} skipped, {} words added", r.pages_read, page(r.pages_read), r.pages_skipped, r.words);
            warned(Ok((r.path, r.was_encrypted)))
        }
        Cmd::ToWord { input: i, ocr, out } => office_export(&input(i), Office::Word, ocr, &out),
        Cmd::ToPowerpoint { input: i, ocr, out } => office_export(&input(i), Office::PowerPoint, ocr, &out),
        Cmd::ToExcel { input: i, layout, ocr, out } => {
            let layout = match layout {
                CliSheets::OneSheet => SheetLayout::OneSheet,
                CliSheets::SheetPerTable => SheetLayout::SheetPerTable,
            };
            let r = excel::to_excel(&input(i), layout, &ExportOptions { ocr, ..Default::default() }, &out, &mut |_| true)?;
            eprintln!("{} table{} found", r.tables, if r.tables == 1 { "" } else { "s" });
            Ok(vec![r.path])
        }
        Cmd::Compare { old, new, mode, out } => {
            let mode = match mode {
                CliCompare::Text => CompareMode::Text,
                CliCompare::Overlay => CompareMode::Overlay,
            };
            let r = compare::compare(&input(old), &input(new), mode, &out)?;
            if r.identical() {
                eprintln!("No differences: the documents' text is identical.");
            } else {
                eprintln!("{}", r.counts());
            }
            if r.regions > 0 {
                eprintln!("{} other changed area{} outlined", r.regions, if r.regions == 1 { "" } else { "s" });
            }
            warned(Ok((r.path, r.was_encrypted)))
        }
        Cmd::Redact { input: i, text, pattern, pages: whole, out } => {
            let i = input(i);
            let mut queries: Vec<Query> = text.into_iter().map(|t| Query::Literal { text: t, case_insensitive: true }).collect();
            queries.extend(pattern.into_iter().map(|p| Query::Pattern(p.into())));
            let mut marks = redact::mark_text(&redact::find_all(&i, &queries)?);
            if let Some(spec) = whole {
                let n = sheaf_core::pdfcraft::Doc::open(&i)?.document().info.pages.len() as u32;
                let pages: Vec<usize> = sheaf_core::ranges::parse_ranges(&spec, n)?.into_iter().flatten().map(|p| p as usize - 1).collect();
                marks.extend(redact::mark_pages(&i, &pages)?);
            }
            if marks.is_empty() {
                return Err(OpdfError::Unsupported("nothing to redact: no match and no page given".into()));
            }
            let r = redact::apply(&i, &marks, &out)?;
            eprintln!("{} area{} redacted", r.areas, if r.areas == 1 { "" } else { "s" });
            eprintln!("Verified: no hidden text remains under the redactions");
            warned(Ok((r.path, r.was_encrypted)))
        }
        Cmd::Sign { input: i, page, at, width, text, image, strokes, p12, p12_password, reason, location, out } => {
            let i = input(i);
            let graphic = match (text, image, strokes) {
                (Some(t), _, _) => Graphic::Typed(t),
                (_, Some(p), _) => Graphic::Image(p),
                (_, _, Some(p)) => Graphic::Drawn(read_strokes(&p)?),
                _ => unreachable!("clap requires one graphic"),
            };
            let page = page as usize - 1;
            let info = sign::page_info(&i, page)?;
            let aspect = sign::aspect(&graphic)?;
            let center = [at.0 + width / 2.0, at.1 + width * aspect / 2.0];
            let digital = p12.map(|p12| DigitalSpec { p12, password: p12_password.unwrap_or_default(), reason, location });
            let spec = SignSpec { page, rect: sign::box_at(&info, center, width, aspect), graphic, digital };
            let r = sign::sign(&i, &spec, &out)?;
            match &r.signer {
                Some(cn) => eprintln!("Signed digitally (certificate: {cn})"),
                None => eprintln!("Signed"),
            }
            Ok(vec![r.path])
        }
        Cmd::Edit { input: i, items, out, .. } => {
            let i = input(i);
            let mut spec = EditSpec::default();
            let mut infos = std::collections::HashMap::new();
            for it in items {
                let page = it.page.unwrap_or(1) as usize - 1;
                let what = it.text.as_deref().map(|t| format!("--text \"{t}\"")).unwrap_or_else(|| format!("--image {}", it.image.as_ref().map(|p| p.display().to_string()).unwrap_or_default()));
                let (x, y) = it.at.ok_or_else(|| OpdfError::Unsupported(format!("{what}: --at X,Y is missing")))?;
                let info = match infos.entry(page) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => e.insert(edit::page_info(&i, page)?),
                };
                spec.items.push(match (it.text, it.image) {
                    (Some(text), _) => {
                        if it.width.is_some() {
                            return Err(OpdfError::Unsupported(format!("{what}: --width is for images (a text's width follows its size)")));
                        }
                        let text = text.replace("\\n", "\n");
                        Item::Text { page, at: edit::view_to_user(info, [x, y]), text, font_size: it.size.unwrap_or(edit::DEFAULT_FONT_SIZE), color: it.color.unwrap_or([0; 3]) }
                    }
                    (None, Some(path)) => {
                        if it.size.is_some() || it.color.is_some() {
                            return Err(OpdfError::Unsupported(format!("{what}: --size and --color are for text")));
                        }
                        let width = it.width.ok_or_else(|| OpdfError::Unsupported(format!("{what}: --width is missing")))?;
                        let aspect = edit::Picture::read(&path)?.aspect();
                        Item::Image { page, rect: edit::image_rect(info, [x, y], width, aspect), path }
                    }
                    (None, None) => unreachable!("an item starts at --text or --image"),
                });
            }
            let r = edit::edit(&i, &spec, &out)?;
            eprintln!("{} item{} added", r.added, if r.added == 1 { "" } else { "s" });
            warned(Ok((r.path, r.was_encrypted)))
        }
        Cmd::Pdfa { input: i, level, out } => {
            let level = match level {
                CliPdfA::A2b => PdfALevel::A2b,
                CliPdfA::A3b => PdfALevel::A3b,
            };
            let r = pdfa::to_pdfa(&input(i), level, &out)?;
            if r.issues.is_empty() {
                eprintln!("Conforms to {}", level.label());
            }
            for issue in &r.issues {
                eprintln!("warning: {} ({}): {}", level.label(), issue.clause, issue.message);
            }
            warned(Ok((r.path, r.was_encrypted)))
        }
    }
}

/// Word or PowerPoint export (the output is not a PDF: no protection note).
fn office_export(input: &Input, format: Office, ocr: bool, out: &Path) -> sheaf_core::Result<Vec<PathBuf>> {
    Ok(vec![export::to_office(input, format, &ExportOptions { ocr, ..Default::default() }, out, &mut |_| true)?.path])
}

/// A PdfCraft tool's result: the path, with the note when the protected input lost protection.
fn warned(r: sheaf_core::Result<(PathBuf, bool)>) -> sheaf_core::Result<Vec<PathBuf>> {
    let (p, was_encrypted) = r?;
    if was_encrypted {
        eprintln!("{NOT_PROTECTED}");
    }
    Ok(vec![p])
}

/// How much larger `after` is than `before`, in whole percent (at least 1).
fn grew_pct(before: u64, after: u64) -> u64 {
    if before == 0 { return 0; }
    ((after.saturating_sub(before) as f64 / before as f64 * 100.0).round() as u64).max(1)
}

fn main() {
    // clap exits 2 on usage errors; keep 2 reserved for password errors.
    let fail = |e: clap::Error| -> ! {
        let code = if e.use_stderr() { 1 } else { 0 };
        let _ = e.print();
        std::process::exit(code);
    };
    let matches = Cli::command().try_get_matches().unwrap_or_else(|e| fail(e));
    let mut cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| fail(e));
    if let (Cmd::Edit { items, .. }, Some(("edit", m))) = (&mut cli.cmd, matches.subcommand()) {
        *items = edit_items(m).unwrap_or_else(|e| fail(Cli::command().error(clap::error::ErrorKind::ArgumentConflict, e)));
    }
    match run(cli) {
        Ok(paths) => {
            for p in paths {
                println!("{}", p.display());
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            let code = match e {
                OpdfError::PasswordRequired(_) | OpdfError::WrongPassword(_) => 2,
                _ => 1,
            };
            std::process::exit(code);
        }
    }
}
