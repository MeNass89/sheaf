//! `sheaf`: a thin command-line front-end over the sheaf-core engine.
//!
//! Every subcommand prints the path or paths it wrote, one per line, on stdout.
//! Exit codes: 0 success, 1 any error, 2 password required or wrong.

use clap::{Parser, Subcommand, ValueEnum};
use sheaf_core::compress::{self, Level};
use sheaf_core::images::{self, PageLayout};
use sheaf_core::pages::{self, Input};
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
    /// Convert an Office or HTML document to PDF through LibreOffice
    Office {
        input: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CliLevel {
    Low,
    Recommended,
    Extreme,
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
    }
}

/// How much larger `after` is than `before`, in whole percent (at least 1).
fn grew_pct(before: u64, after: u64) -> u64 {
    if before == 0 { return 0; }
    ((after.saturating_sub(before) as f64 / before as f64 * 100.0).round() as u64).max(1)
}

fn main() {
    // clap exits 2 on usage errors; keep 2 reserved for password errors.
    let cli = Cli::try_parse().unwrap_or_else(|e| {
        let code = if e.use_stderr() { 1 } else { 0 };
        let _ = e.print();
        std::process::exit(code);
    });
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
