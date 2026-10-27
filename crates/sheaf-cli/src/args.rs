//! The command line as clap reads it: the global options ([`Cli`]), every subcommand and its
//! arguments ([`Cmd`]), the value enums and value parsers they use, and `sheaf edit`'s items
//! regrouped by position ([`edit_items`]). `main.rs` dispatches what is parsed here.

use clap::{ArgMatches, Parser, Subcommand, ValueEnum};
use sheaf_core::compress::Level;
use sheaf_core::numbers::Std14;
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
        /// The PDFs to merge, in order
        #[arg(required = true, num_args = 1..)]
        inputs: Vec<PathBuf>,
        /// The merged PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Split a PDF by page ranges or every N pages into a directory
    #[command(group = clap::ArgGroup::new("how").required(true).args(["ranges", "every"]))]
    Split {
        /// The PDF to read
        input: PathBuf,
        /// Page ranges, one file per range, e.g. "1-3,5"
        #[arg(long)]
        ranges: Option<String>,
        /// One file every N pages
        #[arg(long)]
        every: Option<u32>,
        /// The folder to write the parts into
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Extract pages into a new PDF
    Extract {
        /// The PDF to read
        input: PathBuf,
        /// Pages to keep, e.g. "1-3,5"
        #[arg(long)]
        ranges: String,
        /// The new PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Remove pages
    Remove {
        /// The PDF to read
        input: PathBuf,
        /// Pages to remove, e.g. "2,4-6"
        #[arg(long)]
        pages: String,
        /// The PDF to write without them
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Reorder pages: 1-based page numbers, 0 inserts a blank page, e.g. "3,0,1"
    Organize {
        /// The PDF to read
        input: PathBuf,
        /// The new page order, 1-based; 0 inserts a blank page, e.g. "3,0,1,2"
        #[arg(long, value_delimiter = ',', required = true)]
        order: Vec<u32>,
        /// The reordered PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Rotate pages clockwise by 90, 180 or 270 degrees (negative = counter-clockwise)
    Rotate {
        /// The PDF to read
        input: PathBuf,
        /// 90, 180 or 270 (negative turns counter-clockwise)
        #[arg(long, allow_hyphen_values = true)]
        degrees: i64,
        /// Pages to rotate (default: all)
        #[arg(long)]
        pages: Option<String>,
        /// The rotated PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Compress a PDF
    Compress {
        /// The PDF to read
        input: PathBuf,
        /// How hard to compress
        #[arg(long, value_enum, default_value_t = CliLevel::Recommended)]
        level: CliLevel,
        /// The compressed PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Encrypt a PDF with a password (AES-256)
    Protect {
        /// The PDF to read
        input: PathBuf,
        /// The password needed to open it
        #[arg(long)]
        user_password: String,
        /// The password needed to change its permissions (default: random, not shown)
        #[arg(long)]
        owner_password: Option<String>,
        /// The protected PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Save portable security settings without passwords (Acrobat's Security Policies)
    SaveSecurityPolicy {
        /// Policy name (1 to 200 characters)
        name: String,
        /// Disallow printing
        #[arg(long)]
        no_printing: bool,
        /// Disallow copying or extracting text
        #[arg(long)]
        no_copying: bool,
        /// Disallow editing and assembling pages
        #[arg(long)]
        no_editing: bool,
        /// Disallow comments and annotations
        #[arg(long)]
        no_commenting: bool,
        /// Disallow filling form fields
        #[arg(long)]
        no_forms: bool,
        /// Keep XMP metadata readable without the password
        #[arg(long)]
        leave_metadata_clear: bool,
        /// Print quality when printing is allowed
        #[arg(long, default_value = "high", value_enum)]
        print_quality: CliPolicyPrint,
        /// Maximum changes allowed, further limited by the action switches
        #[arg(long, default_value = "any", value_enum)]
        changes: CliPolicyChanges,
        /// AES compatibility for modern or older readers
        #[arg(long, default_value = "aes256", value_enum)]
        algorithm: CliPolicyAlgorithm,
        /// The settings JSON or protected PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Apply imported security settings, asking for passwords separately
    ApplySecurityPolicy {
        /// The PDF to protect
        input: PathBuf,
        /// A saved security policy JSON file
        #[arg(long)]
        policy: PathBuf,
        /// Password to open the PDF (may be empty with an owner password)
        #[arg(long, default_value = "")]
        user_password: String,
        /// Password to change security settings
        #[arg(long)]
        owner_password: Option<String>,
        /// The settings JSON or protected PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Remove the password from a PDF (needs --password)
    Unlock {
        /// The protected PDF
        input: PathBuf,
        /// The PDF to write without its password
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Rebuild a damaged PDF
    Repair {
        /// The damaged PDF
        input: PathBuf,
        /// The repaired PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Render every page to JPEG (or PNG with --png) in a directory
    ToJpg {
        /// The PDF to read
        input: PathBuf,
        /// Resolution in dots per inch (150 normal, 300 print, 600 fine)
        #[arg(long, default_value_t = 150)]
        dpi: u32,
        /// JPEG quality, 1 to 100
        #[arg(long, default_value_t = 85, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: u8,
        /// Lossless PNG images instead of JPEG
        #[arg(long, conflicts_with = "quality")]
        png: bool,
        /// The folder to write the images into
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Build a PDF from images, one per page
    FromImages {
        /// The images, one page each, in order (PNG, JPEG, WebP, TIFF, BMP, GIF)
        #[arg(required = true, num_args = 1..)]
        images: Vec<PathBuf>,
        /// Place each image on an A4 page with a 0.5 in margin (default: page fits the image)
        #[arg(long, group = "paper")]
        a4: bool,
        /// Place each image on a US Letter page with a 0.5 in margin
        #[arg(long, group = "paper")]
        letter: bool,
        /// Landscape paper (with --a4 or --letter)
        #[arg(long, requires = "paper")]
        landscape: bool,
        /// The PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Stamp page numbers
    Number {
        /// The PDF to read
        input: PathBuf,
        /// Where the numbers go
        #[arg(long, value_enum, default_value_t = Position::BottomCenter)]
        position: Position,
        /// Double-sided: even pages mirror the left/right position
        #[arg(long)]
        double_sided: bool,
        /// First page is a cover page (not numbered)
        #[arg(long)]
        cover: bool,
        /// Distance from the page edges
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
        /// What each number reads
        #[arg(long, value_enum, default_value_t = NumberFormat::Number)]
        text: NumberFormat,
        /// With --text custom: any text, {page} standing for the page number and {pages} for
        /// the page count, e.g. "Sheet {page} of {pages}"
        #[arg(long, required_if_eq("text", "custom"))]
        custom: Option<String>,
        /// Text size in points
        #[arg(long, default_value_t = 12.0)]
        font_size: f64,
        /// The font of the numbers
        #[arg(long, value_enum, default_value_t = CliFont::Helvetica)]
        font: CliFont,
        /// Text colour as RRGGBB
        #[arg(long, default_value = "000000")]
        color: String,
        /// Underline the numbers
        #[arg(long)]
        underline: bool,
        /// The numbered PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Stamp a text or image watermark
    #[command(group = clap::ArgGroup::new("mark").required(true).args(["text", "image"]))]
    Watermark {
        /// The PDF to read
        input: PathBuf,
        /// Text to stamp (or --image)
        #[arg(long)]
        text: Option<String>,
        /// PNG or JPEG file
        #[arg(long)]
        image: Option<PathBuf>,
        /// Text size in points (1 to 500), or "auto"
        #[arg(long, default_value = "48", value_parser = parse_font_size)]
        font_size: f64,
        /// The font of --text
        #[arg(long, value_enum, default_value_t = CliFont::Helvetica)]
        font: CliFont,
        /// Underline --text
        #[arg(long)]
        underline: bool,
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
        /// First page to mark, 1-based (default: the first)
        #[arg(long)]
        from: Option<u32>,
        /// Last page to mark, 1-based (default: the last)
        #[arg(long)]
        to: Option<u32>,
        /// The watermarked PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Crop pages by margins in millimetres (as the page is displayed), or remove a crop
    Crop {
        /// The PDF to read
        input: PathBuf,
        /// Millimetres to cut from the top
        #[arg(long, default_value_t = 0.0)]
        top: f64,
        /// Millimetres to cut from the bottom
        #[arg(long, default_value_t = 0.0)]
        bottom: f64,
        /// Millimetres to cut from the left
        #[arg(long, default_value_t = 0.0)]
        left: f64,
        /// Millimetres to cut from the right
        #[arg(long, default_value_t = 0.0)]
        right: f64,
        /// Remove the crop instead (back to the full page)
        #[arg(long, conflicts_with_all = ["top", "bottom", "left", "right"])]
        remove: bool,
        /// Pages to crop (default: all)
        #[arg(long)]
        pages: Option<String>,
        /// The cropped PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Recognise the text of scanned pages and add a searchable text layer
    Ocr {
        /// The scanned PDF
        input: PathBuf,
        /// Use installed Tesseract instead of bundled recognition, including Latin text
        #[arg(long)]
        local_engine: bool,
        /// Language codes joined with + (for example en+ja); extra scripts need local Tesseract packs
        #[arg(long, default_value = "en")]
        language: String,
        /// Also read pages that already contain text (only new words are added)
        #[arg(long)]
        all_pages: bool,
        /// The searchable PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Review the invisible OCR text, with page boxes and possible decoding errors
    ReviewOcr {
        /// The PDF whose OCR layer to review or correct
        input: PathBuf,
    },
    /// Correct an OCR run or mark it as not text, without repainting the scan
    #[command(group = clap::ArgGroup::new("correction").required(true).args(["text", "not_text"]))]
    CorrectOcr {
        /// The PDF whose OCR layer to review or correct
        input: PathBuf,
        /// Page number from review-ocr (1-based)
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        page: u32,
        /// Run number from review-ocr (1-based)
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        line: u32,
        /// The original text from review-ocr; refuses a stale selection
        #[arg(long)]
        expected: String,
        /// Correct text for the selected hidden run
        #[arg(long)]
        text: Option<String>,
        /// Remove the run from the searchable text layer
        #[arg(long)]
        not_text: bool,
        /// The corrected PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Export to an editable Word document (.docx); the layout is approximate
    ToWord {
        /// The PDF to read
        input: PathBuf,
        /// Recognise the text of scanned pages first
        #[arg(long)]
        ocr: bool,
        /// The .docx to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Export to PowerPoint (.pptx), one slide per page; the layout is approximate
    ToPowerpoint {
        /// The PDF to read
        input: PathBuf,
        /// Recognise the text of scanned pages first
        #[arg(long)]
        ocr: bool,
        /// The .pptx to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Export the tables to Excel (.xlsx); pages without a table keep their text in column A
    ToExcel {
        /// The PDF to read
        input: PathBuf,
        /// How the tables go into sheets
        #[arg(long, value_enum, default_value_t = CliSheets::OneSheet)]
        layout: CliSheets,
        /// Recognise the text of scanned pages first
        #[arg(long)]
        ocr: bool,
        /// The .xlsx to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Compare an original and a changed PDF: a report (text) or the changes marked (overlay)
    Compare {
        /// The original version
        old: PathBuf,
        /// The changed version
        new: PathBuf,
        /// What to produce
        #[arg(long, value_enum, default_value_t = CliCompare::Text)]
        mode: CliCompare,
        /// The report or marked PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Black out text, patterns or whole pages for good, then check no text remains under them
    Redact {
        /// The PDF to read
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
        /// The redacted PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Place a signature (typed, drawn or an image) on a page, optionally signed with a certificate
    #[command(group = clap::ArgGroup::new("graphic").required(true).args(["text", "image", "strokes"]))]
    Sign {
        /// The PDF to read
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
        /// The name to sign with, in a handwriting font
        #[arg(long)]
        text: Option<String>,
        /// The handwriting font of --text
        #[arg(long, value_enum, default_value_t = CliScript::Dancing)]
        script_font: CliScript,
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
        /// Certify the document, allowing after it: 1 no changes, 2 filling in forms and signing,
        /// 3 those and comments
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
        certify: Option<u8>,
        /// The signed PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Add text and images on top of pages. Each --text or --image starts an item; the --page,
    /// --at, --size, --color and --width after it apply to that item
    #[command(after_help = "Example: sheaf edit in.pdf --text \"Approved\" --page 1 --at 72,72 --size 18 --color #c00000 \\\n    --image logo.png --page 2 --at 400,40 --width 120 -o out.pdf")]
    Edit {
        /// The PDF to read
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
        /// The edited PDF to write
        #[arg(short, long)]
        out: PathBuf,
        /// The items in command-line order (filled from the argument positions).
        #[arg(skip)]
        items: Vec<CliItem>,
    },
    /// Convert to PDF/A for archiving, then check the result
    #[command(after_help = "Exit status: 0 when the output conforms. When rules of the standard are still broken, each is\nprinted and the file is still written (everything fixable was fixed), but the exit status is 1.")]
    Pdfa {
        /// The PDF to read
        input: PathBuf,
        /// The PDF/A level
        #[arg(long, value_enum, default_value_t = CliPdfA::A2b)]
        level: CliPdfA,
        /// The PDF/A file to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Convert an Office or HTML document to PDF through LibreOffice
    Office {
        /// The Office (Word, Excel, PowerPoint, OpenDocument) or HTML document
        input: PathBuf,
        /// The PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Add a bookmark that opens a page, after the last top-level one (`info` lists them)
    Bookmark {
        /// The PDF to read
        input: PathBuf,
        /// The bookmark's title
        #[arg(long)]
        title: String,
        /// The page it opens (1-based)
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        page: u32,
        /// The PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Describe a PDF: page count and sizes, encryption, metadata, forms, bookmarks (JSON)
    Info {
        /// The PDF to read
        input: PathBuf,
    },
    /// Print the text of pages as Sheaf's tools read it (what redact searches)
    Text {
        /// The PDF to read
        input: PathBuf,
        /// Pages to read, e.g. "1-3,5" (default: all); pages are separated by a form feed
        #[arg(long)]
        pages: Option<String>,
    },
    /// Render one page to a PNG, to look at a result
    Render {
        /// The PDF to read
        input: PathBuf,
        /// Page to render (1-based)
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        page: u32,
        /// Resolution in dots per inch (72 is one pixel per point)
        #[arg(long, default_value_t = 110, value_parser = clap::value_parser!(u32).range(18..=600))]
        dpi: u32,
        /// The PNG to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Encrypt a PDF for local recipients using RSA certificates and AES-256
    EncryptCertificates {
        /// The unsigned PDF to encrypt
        input: PathBuf,
        /// Recipient X.509 certificate files (PEM or DER); include your own to reopen
        #[arg(long = "recipient", required = true, num_args = 1..)]
        recipients: Vec<PathBuf>,
        /// Encrypt all contents, leave metadata clear, or encrypt embedded files only
        #[arg(long, value_enum, default_value = "all")]
        scope: CliCertificateScope,
        /// The certificate-encrypted PDF to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Save an authorized unprotected copy using a local PKCS#12 recipient digital ID
    UnlockCertificate {
        /// The Adobe.PubSec S5 AES-256 PDF to unlock
        input: PathBuf,
        /// A recipient's local PKCS#12 digital ID file (.p12 or .pfx)
        #[arg(long)]
        identity: PathBuf,
        /// Password for the digital ID file
        #[arg(long)]
        id_password: String,
        /// The unprotected PDF copy to write; the encrypted original is retained
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Build a reusable full-text catalog for a set of PDFs (Acrobat's Catalog)
    Index {
        /// PDF files or folders to include recursively in the catalog
        #[arg(required = true, num_args = 1..)]
        inputs: Vec<PathBuf>,
        /// Reuse unchanged pages from this catalog; omitted sources are purged
        #[arg(long)]
        update: Option<PathBuf>,
        /// The catalog JSON file to write
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Search a previously built full-text catalog; rebuild if a source PDF changed
    SearchIndex {
        /// A previously built catalog JSON file
        index: PathBuf,
        /// Words, phrase, or Boolean expression to find
        query: String,
        /// Phrase, all words, any words, Boolean expression, or unordered proximity
        #[arg(long, value_enum, default_value = "phrase")]
        mode: CliCatalogMode,
        /// Match complete words rather than substrings in phrase mode
        #[arg(long)]
        whole_words: bool,
        /// Match letter case (cannot combine with English stemming)
        #[arg(long, conflicts_with = "english_stems")]
        match_case: bool,
        /// Match English word inflections using Snowball stemming
        #[arg(long)]
        english_stems: bool,
        /// Maximum intervening words for proximity mode
        #[arg(long, default_value_t = 10)]
        proximity: u32,
    },
    /// Describe every command as a JSON schema, for agents
    Tools,
    /// Serve every command as a tool over the Model Context Protocol (JSON-RPC on stdio)
    Mcp,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliCertificateScope {
    /// Encrypt every string and content stream
    All,
    /// Encrypt contents, leaving XMP metadata readable
    ExceptMetadata,
    /// Encrypt embedded file streams while leaving page contents readable
    Attachments,
}
impl CliCertificateScope {
    pub fn core(self) -> sheaf_core::certificate_security::Scope {
        use sheaf_core::certificate_security::Scope;
        match self { Self::All => Scope::All, Self::ExceptMetadata => Scope::ExceptMetadata, Self::Attachments => Scope::Attachments }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliCatalogMode {
    /// Exact phrase, including across line ends
    Phrase,
    /// Every word on the same page, in any order
    AllWords,
    /// At least one word on the page
    AnyWords,
    /// AND, OR, NOT, parentheses and quoted phrases
    Boolean,
    /// Words near each other in any order
    Proximity,
}
impl CliCatalogMode {
    pub fn core(self) -> sheaf_core::search_index::SearchMode {
        use sheaf_core::search_index::SearchMode;
        match self { Self::Phrase => SearchMode::Phrase, Self::AllWords => SearchMode::AllWords, Self::AnyWords => SearchMode::AnyWords, Self::Boolean => SearchMode::Boolean, Self::Proximity => SearchMode::Proximity }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliPolicyPrint {
    /// Allow high-resolution printing
    High,
    /// Allow low-resolution printing only
    Low,
}
impl CliPolicyPrint {
    pub fn core(self) -> sheaf_core::security::PrintQuality {
        match self { Self::High => sheaf_core::security::PrintQuality::High, Self::Low => sheaf_core::security::PrintQuality::Low }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliPolicyChanges {
    /// Allow all checked actions
    Any,
    /// Disallow all document changes
    None,
    /// Allow page assembly only
    Assemble,
    /// Allow form filling only
    FillForms,
    /// Allow comments and form filling
    CommentForms,
}
impl CliPolicyChanges {
    pub fn core(self) -> sheaf_core::security::ChangeLevel {
        use sheaf_core::security::ChangeLevel;
        match self { Self::Any => ChangeLevel::Any, Self::None => ChangeLevel::None, Self::Assemble => ChangeLevel::Assemble, Self::FillForms => ChangeLevel::FillForms, Self::CommentForms => ChangeLevel::CommentForms }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliPolicyAlgorithm {
    /// AES-256 for modern readers
    #[value(name = "aes256")]
    Aes256,
    /// AES-128 for older readers
    #[value(name = "aes128")]
    Aes128,
}
impl CliPolicyAlgorithm {
    pub fn core(self) -> sheaf_core::security::EncryptionAlgorithm {
        match self { Self::Aes256 => sheaf_core::security::EncryptionAlgorithm::Aes256, Self::Aes128 => sheaf_core::security::EncryptionAlgorithm::Aes128 }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliPattern {
    /// Email addresses
    Email,
    /// Phone numbers
    Phone,
    /// Payment card numbers
    CreditCard,
    /// US Social Security numbers
    Ssn,
    /// Dates
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
    /// Each page of the original beside the marked one, on one sheet
    SideBySide,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliSheets {
    /// Every table on one sheet
    OneSheet,
    /// One sheet per table
    SheetPerTable,
}

/// The handwriting fonts of a typed signature.
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliScript {
    /// Dancing Script
    Dancing,
    /// Great Vibes
    GreatVibes,
    /// Satisfy
    Satisfy,
    /// Caveat
    Caveat,
}

impl CliScript {
    pub(crate) fn font(self) -> sheaf_core::script::ScriptFont {
        use sheaf_core::script::ScriptFont;
        match self {
            CliScript::Dancing => ScriptFont::Dancing,
            CliScript::GreatVibes => ScriptFont::GreatVibes,
            CliScript::Satisfy => ScriptFont::Satisfy,
            CliScript::Caveat => ScriptFont::Caveat,
        }
    }
}

/// The twelve standard text fonts, by their PDF names.
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliFont {
    /// Helvetica
    #[value(name = "Helvetica")]
    Helvetica,
    /// Helvetica bold
    #[value(name = "Helvetica-Bold")]
    HelveticaBold,
    /// Helvetica italic
    #[value(name = "Helvetica-Oblique")]
    HelveticaOblique,
    /// Helvetica bold italic
    #[value(name = "Helvetica-BoldOblique")]
    HelveticaBoldOblique,
    /// Times
    #[value(name = "Times-Roman")]
    TimesRoman,
    /// Times bold
    #[value(name = "Times-Bold")]
    TimesBold,
    /// Times italic
    #[value(name = "Times-Italic")]
    TimesItalic,
    /// Times bold italic
    #[value(name = "Times-BoldItalic")]
    TimesBoldItalic,
    /// Courier
    #[value(name = "Courier")]
    Courier,
    /// Courier bold
    #[value(name = "Courier-Bold")]
    CourierBold,
    /// Courier italic
    #[value(name = "Courier-Oblique")]
    CourierOblique,
    /// Courier bold italic
    #[value(name = "Courier-BoldOblique")]
    CourierBoldOblique,
}

impl CliFont {
    /// The engine's font; `None` for Helvetica, its default.
    pub(crate) fn std14(self) -> Option<Std14> {
        use Std14::*;
        let f = [Helvetica, HelveticaBold, HelveticaOblique, HelveticaBoldOblique, TimesRoman, TimesBold, TimesItalic, TimesBoldItalic, Courier, CourierBold, CourierOblique, CourierBoldOblique][self as usize];
        Some(f).filter(|f| *f != Helvetica)
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliPdfA {
    /// PDF/A-1b (older archives; no transparency or attachments)
    #[value(name = "1b")]
    A1b,
    /// PDF/A-2b (recommended)
    #[value(name = "2b")]
    A2b,
    /// PDF/A-3b (allows attachments)
    #[value(name = "3b")]
    A3b,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliLevel {
    /// Best image quality, a modest saving
    Low,
    /// Small file, sharp enough to read
    Recommended,
    /// Smallest file, softer images
    Extreme,
}

/// A cell of the 3×3 position grid.
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Position {
    /// Top left
    TopLeft,
    /// Top centre
    TopCenter,
    /// Top right
    TopRight,
    /// Middle left
    MiddleLeft,
    /// Centre
    Center,
    /// Middle right
    MiddleRight,
    /// Bottom left
    BottomLeft,
    /// Bottom centre
    BottomCenter,
    /// Bottom right
    BottomRight,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum WmPosition {
    /// Top left
    TopLeft,
    /// Top centre
    TopCenter,
    /// Top right
    TopRight,
    /// Middle left
    MiddleLeft,
    /// Centre
    Center,
    /// Middle right
    MiddleRight,
    /// Bottom left
    BottomLeft,
    /// Bottom centre
    BottomCenter,
    /// Bottom right
    BottomRight,
    /// Repeated across the page (3 × 3)
    Mosaic,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Margin {
    /// 18 pt (1/4 in)
    Small,
    /// 36 pt (1/2 in)
    Recommended,
    /// 54 pt (3/4 in)
    Big,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub(crate) enum NumberFormat {
    /// The number alone: 3
    Number,
    /// Page 3
    Page,
    /// Page 3 of 10
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

#[cfg(test)]
mod help_tests {
    use super::*;
    use clap::CommandFactory;

    /// Every option, input and enumerated value of every command says what it is in `--help`.
    #[test]
    fn offline_certificate_commands_require_recipients_identity_and_explicit_scope() {
        assert!(Cli::try_parse_from(["sheaf", "encrypt-certificates", "input.pdf", "--recipient", "alice.cer", "bob.pem", "--scope", "except-metadata", "-o", "encrypted.pdf"]).is_ok());
        assert!(Cli::try_parse_from(["sheaf", "encrypt-certificates", "input.pdf", "-o", "encrypted.pdf"]).is_err());
        assert!(Cli::try_parse_from(["sheaf", "unlock-certificate", "encrypted.pdf", "--identity", "alice.p12", "--id-password", "secret", "-o", "copy.pdf"]).is_ok());
        assert!(Cli::try_parse_from(["sheaf", "unlock-certificate", "encrypted.pdf", "-o", "copy.pdf"]).is_err());
    }
    #[test]
    fn catalog_advanced_cli_modes_and_conflicting_case_options_are_validated() {
        assert!(Cli::try_parse_from(["sheaf", "search-index", "catalog.json", "red AND blue", "--mode", "boolean", "--whole-words"]).is_ok());
        assert!(Cli::try_parse_from(["sheaf", "search-index", "catalog.json", "red blue", "--mode", "proximity", "--proximity", "3", "--english-stems"]).is_ok());
        assert!(Cli::try_parse_from(["sheaf", "search-index", "catalog.json", "red", "--match-case", "--english-stems"]).is_err());
    }
    #[test]
    fn every_option_and_value_has_help() {
        let cli = Cli::command();
        let mut missing = Vec::new();
        for cmd in cli.get_subcommands() {
            for arg in cmd.get_arguments().filter(|a| !matches!(a.get_id().as_str(), "help" | "version")) {
                if arg.get_help().is_none() {
                    missing.push(format!("{} {}", cmd.get_name(), arg.get_id()));
                }
                for v in arg.get_possible_values() {
                    if v.get_help().is_none() && !matches!(v.get_name(), "true" | "false") {
                        missing.push(format!("{} {}={}", cmd.get_name(), arg.get_id(), v.get_name()));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "no help for: {missing:?}");
    }

    /// The page-number help reads as written (clap turns "{n}" into a line break: the help
    /// names the placeholders {page} and {pages}).
    #[test]
    fn page_number_help_reads_as_written() {
        let help = Cli::command().find_subcommand_mut("number").unwrap().render_long_help().to_string();
        assert!(help.contains("Page 3 of 10") && help.contains("\"Sheet {page} of {pages}\""), "{help}");
    }

    #[test]
    fn ocr_correction_requires_one_action_and_positive_review_indexes() {
        let args = ["sheaf", "correct-ocr", "scan.pdf", "--page", "1", "--line", "1", "--expected", "T?xt", "-o", "fixed.pdf"];
        assert!(Cli::try_parse_from(args).is_err(), "a correction action is required");
        let mut remove = args.to_vec();
        remove.push("--not-text");
        assert!(Cli::try_parse_from(&remove).is_ok());
        remove.extend(["--text", "Text"]);
        assert!(Cli::try_parse_from(&remove).is_err(), "replace and remove are exclusive");
        let mut zero = args.to_vec();
        zero[4] = "0";
        zero.push("--not-text");
        assert!(Cli::try_parse_from(zero).is_err());
    }

    #[test]
    fn catalog_and_security_policy_commands_are_accessible_from_the_cli() {
        assert!(Cli::try_parse_from(["sheaf", "index", "a.pdf", "b.pdf", "-o", "catalog.json"]).is_ok());
        assert!(Cli::try_parse_from(["sheaf", "search-index", "catalog.json", "signed invoice"]).is_ok());
        assert!(Cli::try_parse_from(["sheaf", "save-security-policy", "Review", "--no-copying", "--leave-metadata-clear", "-o", "policy.json"]).is_ok());
        assert!(Cli::try_parse_from(["sheaf", "apply-security-policy", "a.pdf", "--policy", "policy.json", "--owner-password", "secret", "-o", "protected.pdf"]).is_ok());
    }

    #[test]
    fn local_ocr_engine_and_mixed_languages_are_explicit_cli_options() {
        let cli = Cli::try_parse_from(["sheaf", "ocr", "scan.pdf", "--local-engine", "--language", "en+ja", "-o", "out.pdf"]).unwrap();
        assert!(matches!(cli.cmd, Cmd::Ocr { local_engine: true, language, .. } if language == "en+ja"));
    }

    #[test]
    fn policy_compatibility_choices_and_incremental_folder_index_are_accessible() {
        assert!(Cli::try_parse_from(["sheaf", "save-security-policy", "Review", "--print-quality", "low", "--changes", "fill-forms", "--algorithm", "aes128", "-o", "policy.json"]).is_ok());
        assert!(Cli::try_parse_from(["sheaf", "save-security-policy", "Review", "--algorithm", "rc4", "-o", "policy.json"]).is_err());
        assert!(Cli::try_parse_from(["sheaf", "index", "folder", "--update", "previous.json", "-o", "updated.json"]).is_ok());
    }

}
