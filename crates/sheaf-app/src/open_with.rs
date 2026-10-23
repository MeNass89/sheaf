//! Files handed to Sheaf from outside the window: Finder's "Open With", a document dropped on
//! the Dock icon, or paths on the command line. The platform glue only feeds an [`Inbox`]; what
//! happens to the files is decided here, without AppKit, so it can be tested.

use std::path::{Path, PathBuf};

use crate::files::{self, Kind};

/// Holds files that arrive before the window is ready (a cold start from Finder delivers them
/// while the app is still launching), then lets everything through once it is.
#[derive(Debug, Default)]
pub struct Inbox {
    ready: bool,
    pending: Vec<PathBuf>,
}

impl Inbox {
    /// Files just received: returned to be opened now if the window is ready, queued otherwise.
    /// A file already queued is not queued again (at launch AppKit may send the command-line
    /// files that were read from argv too).
    pub fn receive(&mut self, paths: Vec<PathBuf>) -> Vec<PathBuf> {
        if self.ready {
            return paths;
        }
        for p in paths {
            if !self.pending.contains(&p) {
                self.pending.push(p);
            }
        }
        Vec::new()
    }

    /// The window is ready: returns what was queued, in arrival order. Later files pass through.
    pub fn set_ready(&mut self) -> Vec<PathBuf> {
        self.ready = true;
        std::mem::take(&mut self.pending)
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }
}

/// What to do with a batch of received files.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// PDFs to keep on Home for the next tool (same landing as a PDF dropped on Home).
    pub open: Vec<PathBuf>,
    /// Office and HTML files, each with the tool that turns it into a PDF (Word, PowerPoint,
    /// Excel or HTML to PDF), in the order they came: opened in that tool, as Acrobat creates a
    /// PDF from an Office file it is asked to open.
    pub convert: Vec<(sheaf_core::tools::ToolId, Vec<PathBuf>)>,
    /// A toast about the files left out, shown instead of "Choose a tool".
    pub notice: Option<String>,
}

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// Keeps existing PDF files, without duplicates; anything else is skipped with one toast.
pub fn plan(paths: Vec<PathBuf>) -> Plan {
    let mut open: Vec<PathBuf> = Vec::new();
    let mut convert: Vec<(sheaf_core::tools::ToolId, Vec<PathBuf>)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for p in paths {
        if !p.is_file() {
            skipped.push(format!("File not found: {}", name(&p)));
        } else if files::kind_of(&p) == Kind::Pdf {
            if !open.contains(&p) {
                open.push(p);
            }
        } else if let Some(tool) = to_pdf_tool(&p) {
            match convert.iter_mut().find(|(t, _)| *t == tool) {
                Some((_, v)) if !v.contains(&p) => v.push(p),
                Some(_) => {}
                None => convert.push((tool, vec![p])),
            }
        } else {
            skipped.push(format!("Not a PDF or a document: {}", name(&p)));
        }
    }
    let notice = match skipped.len() {
        0 => None,
        1 => Some(skipped.remove(0)),
        n => Some(format!("{n} files skipped: only PDFs and documents can be opened")),
    };
    Plan { open, convert, notice }
}

/// The tool that makes a PDF of `p` (an Office or HTML file), if one does.
fn to_pdf_tool(p: &Path) -> Option<sheaf_core::tools::ToolId> {
    use sheaf_core::tools::ToolId;
    [ToolId::WordToPdf, ToolId::PowerPointToPdf, ToolId::ExcelToPdf, ToolId::HtmlToPdf].into_iter().find(|t| files::accepts(*t, p))
}

/// Command-line arguments that name files (macOS may add a `-psn_…` switch on old systems).
pub fn launch_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Vec<PathBuf> {
    args.into_iter().filter(|a| !a.to_string_lossy().starts_with('-')).map(PathBuf::from).collect()
}

/// Canonical paths (absolute when the file is missing), so a file named relatively on the
/// command line and the same file sent by AppKit at launch are recognised as one.
pub fn absolute(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .map(|p| std::fs::canonicalize(&p).or_else(|_| std::path::absolute(&p)).unwrap_or(p))
        .collect()
}

#[cfg(target_os = "macos")]
pub use macos::install_hook;

/// Teaches winit's application delegate `application:openURLs:`, which AppKit calls for
/// Finder's "Open With", a drop on the Dock icon and `open -a Sheaf file.pdf`, both while the
/// app launches (before `applicationDidFinishLaunching:`) and while it runs.
#[cfg(target_os = "macos")]
mod macos {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use objc2::ffi::class_addMethod;
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::{sel, MainThreadMarker};
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{NSArray, NSURL};

    static HANDLER: OnceLock<fn(Vec<PathBuf>)> = OnceLock::new();

    /// The method body. AppKit calls it on the main thread with an array of file URLs.
    extern "C-unwind" fn open_urls(_this: &AnyObject, _cmd: Sel, _app: &AnyObject, urls: &NSArray<NSURL>) {
        // A panic must not unwind into AppKit's Apple Event dispatch.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let paths: Vec<PathBuf> = urls.to_vec().iter().filter_map(|u| u.to_file_path()).collect();
            if let (Some(f), false) = (HANDLER.get(), paths.is_empty()) {
                f(paths);
            }
        }));
    }

    /// Adds `application:openURLs:` to the delegate winit installed. Call after the Slint winit
    /// backend is selected (the delegate exists) and before the event loop runs (AppKit delivers
    /// launch files from `-[NSApplication finishLaunching]`). `handler` runs on the main thread.
    /// Call once: a second call keeps the first handler.
    pub fn install_hook(handler: fn(Vec<PathBuf>)) {
        let Some(mtm) = MainThreadMarker::new() else {
            eprintln!("sheaf: Open With hook must be installed on the main thread");
            return;
        };
        let _ = HANDLER.set(handler);
        let app = NSApplication::sharedApplication(mtm);
        let Some(delegate) = app.delegate() else {
            eprintln!("sheaf: no application delegate; Open With is unavailable");
            return;
        };
        let object: &AnyObject = (*delegate).as_ref();
        let class: &'static AnyClass = object.class();
        type OpenUrls = extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSURL>);
        // SAFETY: `Imp` is the type-erased `extern "C-unwind"` function pointer the runtime
        // stores; it is called back with the signature declared by the "v@:@@" encoding below
        // (void return; self, _cmd, NSApplication*, NSArray<NSURL*>*), which is `OpenUrls`.
        let imp: Imp = unsafe { std::mem::transmute::<OpenUrls, Imp>(open_urls) };
        // SAFETY: `class` is a live, registered class (classes are never freed), the selector
        // and encoding match the NSApplicationDelegate method, and adding a method to a
        // registered class is allowed by the runtime. It runs once, on the main thread, before
        // the event loop starts, so nothing is dispatching to the class concurrently.
        let added = unsafe {
            class_addMethod(
                class as *const AnyClass as *mut AnyClass,
                sel!(application:openURLs:),
                imp,
                c"v@:@@".as_ptr(),
            )
        };
        if !added.as_bool() {
            eprintln!("sheaf: the application delegate already handles openURLs; Open With unchanged");
            return;
        }
        // Defensive: in case NSApplication caches which optional methods its delegate
        // implements when the delegate is set, setting the same object again refreshes that.
        app.setDelegate(Some(&delegate));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_before_ready_are_queued_then_released_in_order() {
        let mut inbox = Inbox::default();
        assert!(!inbox.is_ready());
        assert!(inbox.receive(vec!["/a.pdf".into()]).is_empty());
        assert!(inbox.receive(vec!["/b.pdf".into(), "/c.pdf".into()]).is_empty());
        // The same file from argv and from AppKit is queued once.
        assert!(inbox.receive(vec!["/a.pdf".into()]).is_empty());
        let queued = inbox.set_ready();
        assert_eq!(queued, vec![PathBuf::from("/a.pdf"), "/b.pdf".into(), "/c.pdf".into()]);
        assert!(inbox.is_ready());
        // Nothing is released twice.
        assert!(inbox.set_ready().is_empty());
    }

    #[test]
    fn files_after_ready_pass_straight_through() {
        let mut inbox = Inbox::default();
        assert!(inbox.set_ready().is_empty());
        assert_eq!(inbox.receive(vec!["/d.pdf".into()]), vec![PathBuf::from("/d.pdf")]);
        assert!(inbox.set_ready().is_empty());
    }

    #[test]
    fn plan_keeps_existing_pdfs_and_reports_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("B.PDF");
        let txt = dir.path().join("notes.txt");
        let (doc, ppt, doc2) = (dir.path().join("r.docx"), dir.path().join("s.pptx"), dir.path().join("t.odt"));
        for p in [&a, &b, &txt, &doc, &ppt, &doc2] {
            std::fs::write(p, b"x").unwrap();
        }
        let gone = dir.path().join("gone.pdf");

        let none = Vec::new;
        assert_eq!(plan(vec![a.clone(), b.clone(), a.clone()]), Plan { open: vec![a.clone(), b.clone()], convert: none(), notice: None });
        assert_eq!(plan(vec![a.clone(), txt.clone()]), Plan { open: vec![a.clone()], convert: none(), notice: Some("Not a PDF or a document: notes.txt".into()) });
        assert_eq!(plan(vec![gone.clone()]), Plan { open: vec![], convert: none(), notice: Some("File not found: gone.pdf".into()) });
        assert_eq!(
            plan(vec![gone, txt, dir.path().to_path_buf()]),
            Plan { open: vec![], convert: none(), notice: Some("3 files skipped: only PDFs and documents can be opened".into()) }
        );
        // Documents go to the tool that converts them, Word files together.
        use sheaf_core::tools::ToolId;
        assert_eq!(
            plan(vec![doc.clone(), ppt.clone(), doc2.clone(), a.clone()]),
            Plan { open: vec![a.clone()], convert: vec![(ToolId::WordToPdf, vec![doc, doc2]), (ToolId::PowerPointToPdf, vec![ppt])], notice: None }
        );
        assert_eq!(plan(vec![]), Plan::default());
    }

    #[test]
    fn relative_and_absolute_names_of_one_file_open_it_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        std::fs::write(&a, b"x").unwrap();
        let dotted = dir.path().join(".").join("a.pdf");
        let p = plan(absolute(vec![a.clone(), dotted]));
        assert_eq!(p.open.len(), 1);
        assert!(p.notice.is_none());
    }

    #[test]
    fn launch_args_drop_switches() {
        let args = ["-psn_0_12345", "/x/a.pdf", "b.pdf"].map(std::ffi::OsString::from);
        assert_eq!(launch_args(args), vec![PathBuf::from("/x/a.pdf"), "b.pdf".into()]);
    }
}
