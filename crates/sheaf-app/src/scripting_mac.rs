//! AppleScript and Shortcuts on a Mac: the commands of Sheaf's scripting dictionary
//! (`macos/Sheaf.sdef`; Info.plist: NSAppleScriptEnabled, OSAScriptingDefinition). Cocoa
//! Scripting hands each command to `SheafScriptCommand` (the sdef's cocoa class), whose
//! performDefaultImplementation reads the Apple Event's parameters and runs the request with
//! [`crate::scripting`] on a worker thread, the command suspended meanwhile; it resumes with the
//! result or the error. So `tell application "Sheaf" to rotate pages (POSIX file "/tmp/a.pdf") by
//! 90` runs in the open app, and a shortcut reaches the same commands through Run AppleScript. The
//! standard `open` is AppKit's, which `open_with` receives.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{msg_send, sel, MainThreadMarker};
use objc2_foundation::{NSString, NSURL};

use crate::scripting::{Answer, Request};

const fn code(c: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*c)
}

thread_local! {
    /// The suspended commands, by number, until their work is done.
    static PENDING: RefCell<(u64, HashMap<u64, Retained<AnyObject>>)> = RefCell::new((0, HashMap::new()));
}

/// Registers `SheafScriptCommand`. Call once on the main thread, before the event loop runs:
/// Cocoa Scripting looks the class up when the first command comes.
pub fn install() {
    if MainThreadMarker::new().is_some() {
        let _ = command_class();
    }
}

/// `SheafScriptCommand`: an NSScriptCommand that runs the request.
fn command_class() -> &'static AnyClass {
    static CLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();
    CLASS.get_or_init(|| {
        let base = AnyClass::get(c"NSScriptCommand").expect("Foundation is loaded");
        let mut builder = ClassBuilder::new(c"SheafScriptCommand", base).expect("SheafScriptCommand is defined once");
        // SAFETY: (self, _cmd) returning an object: performDefaultImplementation's signature.
        unsafe { builder.add_method(sel!(performDefaultImplementation), perform as extern "C-unwind" fn(_, _) -> _) };
        builder.register()
    })
}

extern "C-unwind" fn perform(this: &AnyObject, _cmd: Sel) -> *mut AnyObject {
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `this` is the NSScriptCommand being executed; appleEvent is its event (or nil),
        // suspendExecution is documented for commands that finish later.
        unsafe {
            let event: Option<Retained<AnyObject>> = msg_send![this, appleEvent];
            let request = match event {
                Some(event) => read(&event),
                None => Err("Sheaf received the command without its parameters".into()),
            };
            let _: () = msg_send![this, suspendExecution];
            let retained: Retained<AnyObject> = Retained::retain(this as *const AnyObject as *mut AnyObject).expect("self is not nil");
            let key = PENDING.with_borrow_mut(|(next, map)| {
                *next += 1;
                map.insert(*next, retained);
                *next
            });
            std::thread::spawn(move || {
                let answer = request.and_then(crate::scripting::run);
                let _ = slint::invoke_from_event_loop(move || resume(key, answer));
            });
        }
    }));
    if run.is_err() {
        // SAFETY: NSScriptCommand's error setters, on the command being executed.
        unsafe {
            let _: () = msg_send![this, setScriptErrorNumber: -10000isize];
            let _: () = msg_send![this, setScriptErrorString: &*NSString::from_str("Sheaf could not read this command")];
        }
    }
    std::ptr::null_mut()
}

/// The command resumed with the answer, or with the error for the script.
fn resume(key: u64, answer: Result<Answer, String>) {
    let Some(command) = PENDING.with_borrow_mut(|(_, map)| map.remove(&key)) else { return };
    // SAFETY: NSScriptCommand's setScriptErrorNumber:, setScriptErrorString: and
    // resumeExecutionWithResult:, on the main thread, once for the suspended command.
    unsafe {
        let result: Option<Retained<AnyObject>> = match answer {
            Ok(answer) => Some(result_object(&answer)),
            Err(message) => {
                // errAEEventFailed.
                let _: () = msg_send![&*command, setScriptErrorNumber: -10000isize];
                let _: () = msg_send![&*command, setScriptErrorString: &*NSString::from_str(&message)];
                None
            }
        };
        let _: () = msg_send![&*command, resumeExecutionWithResult: result.as_deref()];
    }
}

/// The answer as Cocoa Scripting returns it to the script: a file (NSURL), a list (NSArray) of
/// files or numbers, a number (NSNumber), text.
///
/// # Safety
/// Only Foundation's constructors.
unsafe fn result_object(answer: &Answer) -> Retained<AnyObject> {
    let file = |p: &PathBuf| -> Retained<AnyObject> { Retained::into_super(Retained::into_super(NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy())))) };
    let number = |n: i64| -> Retained<AnyObject> { unsafe { msg_send![AnyClass::get(c"NSNumber").unwrap(), numberWithLongLong: n] } };
    let list = |items: Vec<Retained<AnyObject>>| -> Retained<AnyObject> {
        let list: Retained<AnyObject> = unsafe { msg_send![AnyClass::get(c"NSMutableArray").unwrap(), array] };
        for item in items {
            let _: () = unsafe { msg_send![&*list, addObject: &*item] };
        }
        list
    };
    match answer {
        Answer::File(p) => file(p),
        Answer::Files(ps) => list(ps.iter().map(file).collect()),
        Answer::Numbers(ns) => list(ns.iter().map(|&n| number(n)).collect()),
        Answer::Number(n) => number(*n),
        Answer::Text(t) => Retained::into_super(Retained::into_super(NSString::from_str(t))),
    }
}

/// The command and its parameters, from the event.
///
/// # Safety
/// `event` is an NSAppleEventDescriptor.
unsafe fn read(event: &AnyObject) -> Result<Request, String> {
    let id: u32 = unsafe { msg_send![event, eventID] };
    let param = |key: &[u8; 4]| -> Option<Retained<AnyObject>> { unsafe { msg_send![event, paramDescriptorForKeyword: code(key)] } };
    let path = |key: &[u8; 4]| param(key).and_then(|d| unsafe { path_of(&d) });
    let text = |key: &[u8; 4]| param(key).and_then(|d| unsafe { text_of(&d) });
    let int = |key: &[u8; 4]| param(key).map(|d| unsafe { msg_send![&*d, int32Value] }).map(|n: i32| n as i64);
    let file = || path(b"----").ok_or_else(|| "Give a PDF file: POSIX file \"/path/to/file.pdf\"".to_string());
    let needed = |what: Option<String>, name: &str| what.ok_or_else(|| format!("The {name} parameter is missing"));
    let out = path(b"Svin");
    Ok(match &id.to_be_bytes() {
        b"Rota" => Request::Rotate { file: file()?, degrees: int(b"by  ").unwrap_or(90), pages: text(b"Pgs "), out },
        b"Dele" => Request::Delete { file: file()?, pages: needed(text(b"Pgs "), "pages")?, out },
        b"Insp" => Request::Insert { file: file()?, from: path(b"from"), after: int(b"Aftr").unwrap_or(0).max(0) as u32, out },
        b"Getp" => Request::Extract { file: file()?, pages: needed(text(b"Pgs "), "pages")?, out: out.ok_or("Say where to save: saving in POSIX file \"/path/out.pdf\"")? },
        b"Comb" => {
            let files = param(b"----").map(|d| unsafe { paths_of(&d) }).unwrap_or_default();
            Request::Combine { files, out: out.ok_or("Say where to save: saving in POSIX file \"/path/out.pdf\"")? }
        }
        b"Bkmk" => Request::Bookmark { file: file()?, title: needed(text(b"Titl"), "titled")?, page: int(b"Page").unwrap_or(1).max(1) as u32, out },
        b"Srch" => Request::Search { file: file()?, query: needed(text(b"for "), "for")? },
        b"Text" => Request::Text { file: file()?, pages: text(b"Pgs ") },
        b"Cnt " => Request::Count { file: file()? },
        b"Imgs" => {
            // A boolean coerced to 0 or 1 (booleanValue is declared as Boolean, the runtime says BOOL).
            let png = param(b"PNG ").is_some_and(|d| { let n: i32 = unsafe { msg_send![&*d, int32Value] }; n != 0 });
            Request::Images { file: file()?, folder: path(b"Fold").ok_or("Say where: to folder POSIX file \"/path/folder\"")?, png }
        }
        _ => return Err("Sheaf does not know this command".into()),
    })
}

/// A file (alias, file, file URL) or a POSIX path given as text.
///
/// # Safety
/// `d` is an NSAppleEventDescriptor.
unsafe fn path_of(d: &AnyObject) -> Option<PathBuf> {
    let url: Option<Retained<NSURL>> = unsafe { msg_send![d, fileURLValue] };
    if let Some(p) = url.and_then(|u| u.to_file_path()) {
        return Some(p);
    }
    let text = unsafe { text_of(d) }?;
    let text = text.trim();
    let path = match text.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(rest))?,
        None => PathBuf::from(text),
    };
    path.is_absolute().then_some(path)
}

/// # Safety
/// `d` is an NSAppleEventDescriptor.
unsafe fn text_of(d: &AnyObject) -> Option<String> {
    let s: Option<Retained<NSString>> = unsafe { msg_send![d, stringValue] };
    s.map(|s| s.to_string()).filter(|s| !s.trim().is_empty())
}

/// The files of a list (or one file).
///
/// # Safety
/// `d` is an NSAppleEventDescriptor.
unsafe fn paths_of(d: &AnyObject) -> Vec<PathBuf> {
    let count: isize = unsafe { msg_send![d, numberOfItems] };
    let kind: u32 = unsafe { msg_send![d, descriptorType] };
    if kind != code(b"list") {
        return unsafe { path_of(d) }.into_iter().collect();
    }
    (1..=count)
        .filter_map(|i| {
            let item: Option<Retained<AnyObject>> = unsafe { msg_send![d, descriptorAtIndex: i] };
            item.and_then(|item| unsafe { path_of(&item) })
        })
        .collect()
}
