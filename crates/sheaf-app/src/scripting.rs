//! The PDF actions Sheaf's AppleScript dictionary offers (`macos/Sheaf.sdef`), named as Preview's
//! Shortcuts actions are: Rotate Pages, Delete Pages, Insert Pages, Get Pages, Combine, Add
//! Bookmark, Search, Get Text, Count Pages, Save as Images. `scripting_mac` reads them from Apple
//! Events (so Shortcuts' Run AppleScript and `osascript` reach them) and runs them here, with the
//! functions the `sheaf` command line uses. A file changed without `saving in` is replaced, as
//! Preview's actions change the file they are given.

use std::path::{Path, PathBuf};

use sheaf_core::pages::Input;

/// One command, its parameters read.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Pages turned by `degrees` (90, 180, 270, or −90): `pages` such as "1-3,5", else all.
    Rotate { file: PathBuf, degrees: i64, pages: Option<String>, out: Option<PathBuf> },
    Delete { file: PathBuf, pages: String, out: Option<PathBuf> },
    /// Another PDF's pages (or, without `from`, a blank page) put after page `after` (0: first).
    Insert { file: PathBuf, from: Option<PathBuf>, after: u32, out: Option<PathBuf> },
    /// Chosen pages taken out into a new PDF.
    Extract { file: PathBuf, pages: String, out: PathBuf },
    Combine { files: Vec<PathBuf>, out: PathBuf },
    Bookmark { file: PathBuf, title: String, page: u32, out: Option<PathBuf> },
    /// The pages (1-based) where `query` is, case ignored.
    Search { file: PathBuf, query: String },
    /// The text of the chosen pages (else all), pages apart by a form feed.
    Text { file: PathBuf, pages: Option<String> },
    Count { file: PathBuf },
    /// Every page as a PNG (else JPEG) file in `folder`.
    Images { file: PathBuf, folder: PathBuf, png: bool },
}

/// What a command gives back to the script.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    File(PathBuf),
    Files(Vec<PathBuf>),
    Numbers(Vec<i64>),
    Number(i64),
    Text(String),
}

/// Runs `request`. Err: what went wrong, for the script's error message.
pub fn run(request: Request) -> Result<Answer, String> {
    use sheaf_core::{bookmarks, images, pages, rotate};
    let text = |e: sheaf_core::OpdfError| e.to_string();
    match request {
        Request::Rotate { file, degrees, pages: spec, out } => in_place(&file, out, |to| rotate::rotate(&Input::new(&file), spec.as_deref(), degrees.rem_euclid(360), to)),
        Request::Delete { file, pages: spec, out } => in_place(&file, out, |to| pages::remove(&Input::new(&file), &spec, to)),
        Request::Insert { file, from, after, out } => {
            let n = page_count(&file)?;
            if after > n {
                return Err(format!("page {after} does not exist (the document has {n} pages)"));
            }
            in_place(&file, out, |to| match &from {
                Some(other) => {
                    let m = page_count(other).map_err(sheaf_core::OpdfError::Unsupported)?;
                    let seq: Vec<(usize, u32)> = (1..=after).map(|p| (0, p)).chain((1..=m).map(|p| (1, p))).chain((after + 1..=n).map(|p| (0, p))).collect();
                    pages::assemble(&[Input::new(&file), Input::new(other)], &seq, to)
                }
                // 0 is a blank page in `organize`'s order.
                None => pages::organize(&Input::new(&file), &(1..=after).chain([0]).chain(after + 1..=n).collect::<Vec<_>>(), to),
            })
        }
        Request::Extract { file, pages: spec, out } => pages::extract(&Input::new(&file), &spec, &out).map(Answer::File).map_err(text),
        Request::Combine { files, out } => {
            if files.is_empty() {
                return Err("Combine needs at least one PDF".into());
            }
            pages::merge(&files.iter().map(Input::new).collect::<Vec<_>>(), &out).map(Answer::File).map_err(text)
        }
        Request::Bookmark { file, title, page, out } => in_place(&file, out, |to| bookmarks::add(&Input::new(&file), &title, page, to)),
        Request::Search { file, query } => {
            let q = query.trim().to_lowercase();
            if q.is_empty() {
                return Err("Search needs a word or phrase".into());
            }
            let texts = sheaf_core::text::page_texts(&Input::new(&file), &[]).map_err(text)?;
            Ok(Answer::Numbers(texts.iter().enumerate().filter(|(_, t)| t.to_lowercase().contains(&q)).map(|(k, _)| k as i64 + 1).collect()))
        }
        Request::Text { file, pages: spec } => {
            let chosen = match spec {
                Some(spec) => {
                    let n = page_count(&file)?;
                    sheaf_core::ranges::parse_ranges(&spec, n).map_err(text)?.into_iter().flatten().map(|p| p as usize - 1).collect()
                }
                None => vec![],
            };
            sheaf_core::text::page_texts(&Input::new(&file), &chosen).map(|t| Answer::Text(t.join("\u{c}"))).map_err(text)
        }
        Request::Count { file } => page_count(&file).map(|n| Answer::Number(n.into())),
        Request::Images { file, folder, png } => {
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            images::pdf_to_jpg(&Input::new(&file), 150, (!png).then_some(90), &folder).map(Answer::Files).map_err(text)
        }
    }
}

/// The pages of the PDF at `file`.
fn page_count(file: &Path) -> Result<u32, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    match sheaf_core::pages::count_in(&bytes) {
        0 => Err(format!("{} is not a PDF Sheaf can read", file.display())),
        n => Ok(n as u32),
    }
}

/// `make` writes to `out`, or, without one, to a file beside `file` that then replaces it.
fn in_place(file: &Path, out: Option<PathBuf>, make: impl FnOnce(&Path) -> sheaf_core::Result<PathBuf>) -> Result<Answer, String> {
    if let Some(out) = out {
        return make(&out).map(Answer::File).map_err(|e| e.to_string());
    }
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temp = file.with_file_name(format!(".{name}.sheaf-{}.pdf", std::process::id()));
    let made = make(&temp).map_err(|e| e.to_string()).and_then(|_| std::fs::rename(&temp, file).map_err(|e| e.to_string()));
    if made.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    made.map(|()| Answer::File(file.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PDF of `n` pages, each 100 pt wide and 100 + page points high.
    fn pdf(dir: &Path, name: &str, n: u32, word: &str) -> PathBuf {
        use lopdf::{dictionary, Document, Object, Stream};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
        let kids: Vec<Object> = (1..=n)
            .map(|k| {
                let content = format!("BT /F1 12 Tf 10 50 Td ({word} {k}) Tj ET");
                let c = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
                doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 100.into(), (100 + k as i64).into()], "Contents" => c, "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } } }).into()
            })
            .collect();
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => n as i64 }));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let path = dir.join(name);
        doc.save(&path).unwrap();
        path
    }

    fn heights(path: &Path) -> Vec<i64> {
        let doc = lopdf::Document::load(path).unwrap();
        doc.get_pages().values().map(|id| doc.get_object(*id).unwrap().as_dict().unwrap().get(b"MediaBox").unwrap().as_array().unwrap()[3].as_i64().unwrap()).collect()
    }

    #[test]
    fn pages_go_in_and_out_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let a = pdf(dir.path(), "a.pdf", 3, "alpha");
        let b = pdf(dir.path(), "b.pdf", 2, "beta");
        assert_eq!(run(Request::Insert { file: a.clone(), from: Some(b.clone()), after: 1, out: None }), Ok(Answer::File(a.clone())));
        assert_eq!(heights(&a), vec![101, 101, 102, 102, 103]);
        assert_eq!(run(Request::Delete { file: a.clone(), pages: "2-3".into(), out: None }), Ok(Answer::File(a.clone())));
        assert_eq!(heights(&a), vec![101, 102, 103]);
        assert_eq!(run(Request::Insert { file: a.clone(), from: None, after: 3, out: None }), Ok(Answer::File(a.clone())));
        assert_eq!(run(Request::Count { file: a.clone() }), Ok(Answer::Number(4)));
        assert!(run(Request::Insert { file: a.clone(), from: None, after: 9, out: None }).is_err());
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 2, "no temporary file is left: {names:?}");
    }

    #[test]
    fn a_copy_is_written_when_asked_and_text_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let a = pdf(dir.path(), "a.pdf", 3, "alpha");
        let out = dir.path().join("out.pdf");
        assert_eq!(run(Request::Extract { file: a.clone(), pages: "2-3".into(), out: out.clone() }), Ok(Answer::File(out.clone())));
        assert_eq!(heights(&out), vec![102, 103]);
        assert_eq!(heights(&a), vec![101, 102, 103], "the original is left as it was");
        assert_eq!(run(Request::Search { file: a.clone(), query: "ALPHA 2".into() }), Ok(Answer::Numbers(vec![2])));
        assert!(run(Request::Search { file: a.clone(), query: " ".into() }).is_err());
        let Ok(Answer::Text(t)) = run(Request::Text { file: a, pages: Some("3".into()) }) else { panic!() };
        assert!(t.contains("alpha 3") && !t.contains("alpha 2"), "{t}");
    }
}
