//! The command line as clap reads it: the global options ([`Cli`]), every subcommand and its
//! arguments ([`Cmd`]), the value enums and value parsers they use, and `sheaf edit`'s items
//! regrouped by position ([`edit_items`]). `main.rs` dispatches what is parsed here.

use clap::{ArgMatches, Parser, Subcommand, ValueEnum};
use sheaf_core::compress::Level;
use sheaf_core::redact::Pattern;
use sheaf_core::sign;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sheaf", version, about = "Sheaf: merge, split, compress, protect and convert PDF files")]
pub(crate) struct Cli {
    /// Password used to open every input PDF
    #[arg(long, global = true)]
    pub(crate) password: Option<String>,
    /// Print one JSON object instead of text: {"ok", "outputs", "notes", "data"} or {"ok": false, "error", "code"}
    #[arg(long, global = true)]
    pub(crate) json: bool,
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
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
    /// Render every page to JPEG (or PNG with --png) in a directory
    ToJpg {
        input: PathBuf,
        #[arg(long, default_value_t = 150)]
        dpi: u32,
        #[arg(long, default_value_t = 85, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: u8,
        /// Lossless PNG images instead of JPEG
        #[arg(long, conflicts_with = "quality")]
        png: bool,
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
        /// Text to redact wherever it appears (any case, unless --match-case); repeat for several
        #[arg(long)]
        text: Vec<String>,
        /// Match --text only with the same upper and lower case
        #[arg(long)]
        match_case: bool,
        /// Match --text only as whole words, not inside longer ones
        #[arg(long)]
        whole_words: bool,
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
    #[command(after_help = "Exit status: 0 when the output conforms. When rules of the standard are still broken, each is\nprinted and the file is still written (everything fixable was fixed), but the exit status is 1.")]
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
    /// Describe a PDF: page count and sizes, encryption, metadata, forms, bookmarks (JSON)
    Info {
        input: PathBuf,
    },
    /// Print the text of pages as Sheaf's tools read it (what redact searches)
    Text {
        input: PathBuf,
        /// Pages to read, e.g. "1-3,5" (default: all); pages are separated by a form feed
        #[arg(long)]
        pages: Option<String>,
    },
    /// Render one page to a PNG, to look at a result
    Render {
        input: PathBuf,
        /// Page to render (1-based)
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        page: u32,
        /// Resolution in dots per inch (72 is one pixel per point)
        #[arg(long, default_value_t = 110, value_parser = clap::value_parser!(u32).range(18..=600))]
        dpi: u32,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Describe every command as a JSON schema, for agents
    Tools,
    /// Serve every command as a tool over the Model Context Protocol (JSON-RPC on stdio)
    Mcp,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliPattern {
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
pub(crate) enum CliCompare {
    /// A report PDF listing the word changes
    Text,
    /// The changed PDF with the differences highlighted
    Overlay,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliSheets {
    /// Every table on one sheet
    OneSheet,
    /// One sheet per table
    SheetPerTable,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliPdfA {
    /// PDF/A-2b (recommended)
    #[value(name = "2b")]
    A2b,
    /// PDF/A-3b (allows attachments)
    #[value(name = "3b")]
    A3b,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliLevel {
    Low,
    Recommended,
    Extreme,
}

/// A cell of the 3×3 position grid.
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Position {
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
pub(crate) enum WmPosition {
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
pub(crate) enum Margin {
    Small,
    Recommended,
    Big,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub(crate) enum NumberFormat {
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
pub(crate) struct CliItem {
    /// The text, or the image path.
    pub(crate) text: Option<String>,
    pub(crate) image: Option<PathBuf>,
    pub(crate) page: Option<u32>,
    pub(crate) at: Option<(f64, f64)>,
    pub(crate) size: Option<f64>,
    pub(crate) color: Option<[u8; 3]>,
    pub(crate) width: Option<f64>,
}

/// Groups `sheaf edit`'s options into items by their positions: each --text or --image starts
/// an item, and the options after it (up to the next one) belong to it.
pub(crate) fn edit_items(m: &ArgMatches) -> Result<Vec<CliItem>, String> {
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

fn parse_transparency(s: &str) -> Result<u8, String> {
    s.parse().ok().filter(|v| [0, 25, 50, 75].contains(v)).ok_or_else(|| "use 0, 25, 50 or 75".into())
}

fn parse_rotation(s: &str) -> Result<i32, String> {
    s.parse().ok().filter(|v| [0, 45, 90, 180, 270, -45].contains(v)).ok_or_else(|| "use 0, 45, 90, 180, 270 or -45".into())
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
