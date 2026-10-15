//! Compresses a PDF at the three levels and renders page 1 of the original and each output
//! to `<out_dir>/{orig,low,rec,ext}.png` at 800 px wide, for visual review. Existing files in
//! `<out_dir>` are never deleted: PDFs get a suffixed name; PNG previews are overwritten.
//! `<out_dir>` must not be the input's own folder.
//! Usage: cargo run -p sheaf-core --example compress_preview -- <input.pdf> <out_dir>
use sheaf_core::{compress::{compress, Level}, engine::{open_pdf, pdfium_lock}, pages::Input};
use pdfium_render::prelude::*;
use std::path::Path;

fn render(pdf: &Path, png: &Path) {
    let _g = pdfium_lock();
    let doc = open_pdf(pdf, None).unwrap();
    let page = doc.pages().get(0).unwrap();
    page.render_with_config(&PdfRenderConfig::new().set_target_width(800)).unwrap().as_image().unwrap().save(png).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: compress_preview <input.pdf> <out_dir>");
        std::process::exit(2);
    }
    let (input, dir) = (Path::new(&args[1]), Path::new(&args[2]));
    std::fs::create_dir_all(dir).unwrap();
    // Outputs never replace anything (the engine suffixes taken names), but keep them out of the
    // input's own folder so a review run cannot mix with the user's files.
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    if input.parent().map(|p| canon(if p.as_os_str().is_empty() { Path::new(".") } else { p })) == Some(canon(dir)) {
        eprintln!("refusing to write into the input's folder: choose another <out_dir>");
        std::process::exit(2);
    }
    render(input, &dir.join("orig.png"));
    for (level, tag) in [(Level::Low, "low"), (Level::Recommended, "rec"), (Level::Extreme, "ext")] {
        let pdf = dir.join(format!("{tag}.pdf"));
        let r = compress(&Input::new(input), level, &pdf).unwrap();
        render(&r.path, &dir.join(format!("{tag}.png")));
        println!("{tag}: {} -> {} bytes (already_optimal: {})", r.before, r.after, r.already_optimal);
    }
}
