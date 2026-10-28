//! macOS scanning, as Preview's File ▸ Import from Scanner: ImageKit's device list and scanner
//! panel (every scanner macOS knows, USB or network, with Show Details), run in a helper process
//! (`sheaf-app --scan-to <png> <title> <cancelled> <no scanner>`, texts translated by the caller). AppKit
//! gets its own main thread there, and the caller blocks on the process as it does on SANE or WIA
//! (`sheaf_core::scanner::scan_with`). Exit status 0: a page saved as PNG; 1: the panel closed
//! without one (`<cancelled>` on stderr) or an error, said on stderr.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, NSObject, Sel};
use objc2::{msg_send, sel, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSTextAlignment, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSURL};

// IKDeviceBrowserView and IKScannerDeviceView (Quartz's ImageKit), ICScannerDevice.
#[link(name = "Quartz", kind = "framework")]
unsafe extern "C" {}
#[link(name = "ImageCaptureCore", kind = "framework")]
unsafe extern "C" {}

struct State {
    scanner: Option<Retained<AnyObject>>,
    empty: Option<Retained<AnyObject>>,
    out: PathBuf,
    done: Option<Result<(), String>>,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State { scanner: None, empty: None, out: PathBuf::new(), done: None }) };
}

/// Runs the helper when Sheaf was started as `--scan-to <png> <title> <cancelled> <no scanner>`;
/// returns its exit status.
pub fn helper(mut args: impl Iterator<Item = String>) -> Option<i32> {
    let _program = args.next();
    if args.next().as_deref() != Some("--scan-to") {
        return None;
    }
    let out = PathBuf::from(args.next()?);
    let title = args.next().unwrap_or_else(|| "Import from Scanner".into());
    let cancelled = args.next().unwrap_or_else(|| "Scan cancelled".into());
    let empty = args.next().unwrap_or_else(|| "No scanner yet: turn on a scanner connected to this Mac or on its network.".into());
    Some(match scan(&out, &title, &empty) {
        Ok(true) => 0,
        Ok(false) => {
            eprintln!("{cancelled}");
            1
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    })
}

/// Shows the panel until a page is scanned to `out` (true) or the panel is closed (false). Until a
/// scanner is chosen, `empty` says what to do in its place.
fn scan(out: &Path, title: &str, empty: &str) -> Result<bool, String> {
    let mtm = MainThreadMarker::new().ok_or("The scanner panel must run on the main thread")?;
    let (Some(browser_class), Some(scanner_class)) = (AnyClass::get(c"IKDeviceBrowserView"), AnyClass::get(c"IKScannerDeviceView")) else {
        return Err("ImageKit's scanner panel is unavailable".into());
    };
    let downloads = tempfile::tempdir().map_err(|e| e.to_string())?;
    STATE.with_borrow_mut(|s| s.out = out.to_path_buf());

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let size = NSSize::new(860.0, 560.0);
    let rect = |x: f64, w: f64| NSRect::new(NSPoint::new(x, 0.0), NSSize::new(w, size.height));
    let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable | NSWindowStyleMask::Miniaturizable;
    // SAFETY: a plain window made on the main thread; it is kept until the panel closes.
    let window = unsafe { NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), rect(0.0, size.width), style, NSBackingStoreType::Buffered, false) };
    // SAFETY: the window is owned by `window` (Retained), not released by AppKit on close.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(title));
    window.setMinSize(NSSize::new(640.0, 420.0));

    // SAFETY: the messages below are ImageKit's documented API (IKDeviceBrowserView.h,
    // IKScannerDeviceView.h) with their declared argument types: NSRect, BOOL, NSInteger and
    // NSUInteger values, NSURL, NSString and delegate objects. The delegate outlives the panel.
    unsafe {
        let delegate: Retained<AnyObject> = msg_send![delegate_class(), new];
        let allocated: Allocated<AnyObject> = msg_send![browser_class, alloc];
        let browser: Retained<AnyObject> = msg_send![allocated, initWithFrame: rect(0.0, 220.0)];
        let allocated: Allocated<AnyObject> = msg_send![scanner_class, alloc];
        let scanner: Retained<AnyObject> = msg_send![allocated, initWithFrame: rect(220.0, size.width - 220.0)];
        // NSViewHeightSizable | NSViewMaxXMargin; NSViewWidthSizable | NSViewHeightSizable.
        let _: () = msg_send![&*browser, setAutoresizingMask: 16usize | 4];
        let _: () = msg_send![&*scanner, setAutoresizingMask: 2usize | 16];
        let _: () = msg_send![&*browser, setDisplaysLocalScanners: Bool::YES];
        let _: () = msg_send![&*browser, setDisplaysNetworkScanners: Bool::YES];
        let _: () = msg_send![&*browser, setDisplaysLocalCameras: Bool::NO];
        let _: () = msg_send![&*browser, setDisplaysNetworkCameras: Bool::NO];
        // IKDeviceBrowserViewDisplayModeOutline.
        let _: () = msg_send![&*browser, setMode: 1isize];
        let _: () = msg_send![&*browser, setDelegate: &*delegate];
        // IKScannerDeviceViewTransferModeFileBased, IKScannerDeviceViewDisplayModeSimple.
        let _: () = msg_send![&*scanner, setTransferMode: 0isize];
        let _: () = msg_send![&*scanner, setMode: 0isize];
        let _: () = msg_send![&*scanner, setDisplaysDownloadsDirectoryControl: Bool::NO];
        let _: () = msg_send![&*scanner, setDisplaysPostProcessApplicationControl: Bool::NO];
        let folder = NSURL::fileURLWithPath(&NSString::from_str(&downloads.path().to_string_lossy()));
        let _: () = msg_send![&*scanner, setDownloadsDirectory: &*folder];
        let _: () = msg_send![&*scanner, setDocumentName: &*NSString::from_str("Scan")];
        let _: () = msg_send![&*scanner, setDelegate: &*delegate];
        let content: Retained<AnyObject> = msg_send![&*window, contentView];
        let _: () = msg_send![&*content, addSubview: &*browser];
        let _: () = msg_send![&*scanner, setHidden: Bool::YES];
        let _: () = msg_send![&*content, addSubview: &*scanner];
        let label: Retained<AnyObject> = msg_send![AnyClass::get(c"NSTextField").ok_or("AppKit is unavailable")?, wrappingLabelWithString: &*NSString::from_str(empty)];
        let _: () = msg_send![&*label, setFrame: NSRect::new(NSPoint::new(260.0, size.height / 2.0 - 30.0), NSSize::new(size.width - 300.0, 60.0))];
        // NSViewWidthSizable | NSViewMinYMargin | NSViewMaxYMargin below.
        let _: () = msg_send![&*label, setAlignment: NSTextAlignment::Center];
        let _: () = msg_send![&*label, setAutoresizingMask: 2usize | 8 | 32];
        let grey: Retained<AnyObject> = msg_send![AnyClass::get(c"NSColor").ok_or("AppKit is unavailable")?, secondaryLabelColor];
        let _: () = msg_send![&*label, setTextColor: &*grey];
        let _: () = msg_send![&*content, addSubview: &*label];
        let _: () = msg_send![&*window, setDelegate: &*delegate];
        STATE.with_borrow_mut(|s| (s.scanner, s.empty) = (Some(scanner), Some(label)));

        window.center();
        window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        if cfg!(debug_assertions) && let Some(shot) = std::env::var_os("SHEAF_TEST_SCAN_SHOT") {
            // gui-smoke on a Mac: the panel drawn to a PNG after the device list has had time to fill.
            let until: Retained<AnyObject> = msg_send![AnyClass::get(c"NSDate").unwrap(), dateWithTimeIntervalSinceNow: 4.0f64];
            let run_loop: Retained<AnyObject> = msg_send![AnyClass::get(c"NSRunLoop").unwrap(), currentRunLoop];
            let _: () = msg_send![&*run_loop, runUntilDate: &*until];
            return draw_window(&content, Path::new(&shot)).map(|()| false);
        }
        app.runModalForWindow(&window);
        let done = STATE.with_borrow_mut(|s| s.done.take());
        window.close();
        let _ = delegate;
        match done {
            Some(Ok(())) => Ok(true),
            Some(Err(e)) => Err(e),
            None => Ok(false),
        }
    }
}

/// `SheafScanDelegate`: the device list's, the scanner panel's and the window's delegate.
fn delegate_class() -> &'static AnyClass {
    static CLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();
    CLASS.get_or_init(|| {
        let mut builder = ClassBuilder::new(c"SheafScanDelegate", NSObject::class()).expect("SheafScanDelegate is defined once");
        // SAFETY: each function's signature matches its selector's (void return; self, _cmd and
        // the object arguments ImageKit and AppKit pass).
        unsafe {
            builder.add_method(sel!(deviceBrowserView:selectionDidChange:), selection_changed as extern "C-unwind" fn(_, _, _, _));
            builder.add_method(sel!(scannerDeviceView:didScanToURL:error:), scanned as extern "C-unwind" fn(_, _, _, _, _));
            builder.add_method(sel!(scannerDeviceView:didEncounterError:), failed as extern "C-unwind" fn(_, _, _, _));
            builder.add_method(sel!(windowWillClose:), closing as extern "C-unwind" fn(_, _, _));
        }
        builder.register()
    })
}

/// A scanner chosen in the list goes into the scanner panel (a camera never shows there).
extern "C-unwind" fn selection_changed(_this: &AnyObject, _cmd: Sel, _browser: *mut AnyObject, device: *mut AnyObject) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `device` is the ICDevice ImageKit passes, or nil; the panel accepts a scanner.
        unsafe {
            let Some(device) = device.as_ref() else { return };
            let Some(class) = AnyClass::get(c"ICScannerDevice") else { return };
            let is_scanner: bool = msg_send![device, isKindOfClass: class];
            STATE.with_borrow(|s| {
                if let (true, Some(scanner), Some(empty)) = (is_scanner, &s.scanner, &s.empty) {
                    let _: () = msg_send![&**scanner, setScannerDevice: device];
                    let _: () = msg_send![&**scanner, setHidden: Bool::NO];
                    let _: () = msg_send![&**empty, setHidden: Bool::YES];
                }
            });
        }
    }));
}

/// The scanned page, saved to the output as PNG (the scanner may write TIFF or JPEG).
extern "C-unwind" fn scanned(_this: &AnyObject, _cmd: Sel, _view: *mut AnyObject, url: *mut NSURL, error: *mut AnyObject) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `url` is the scanned file's NSURL or nil, `error` an NSError or nil.
        let result = match unsafe { url.as_ref() }.and_then(|u| u.to_file_path()) {
            Some(path) => STATE.with_borrow(|s| to_png(&path, &s.out)),
            None => Err(unsafe { error_text(error) }.unwrap_or_else(|| "No page was scanned".into())),
        };
        finish(result);
    }));
}

extern "C-unwind" fn failed(_this: &AnyObject, _cmd: Sel, _view: *mut AnyObject, error: *mut AnyObject) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `error` is the NSError ImageKit passes, or nil.
        if let Some(text) = unsafe { error_text(error) } {
            finish(Err(text));
        }
    }));
}

extern "C-unwind" fn closing(_this: &AnyObject, _cmd: Sel, _notification: *mut AnyObject) {
    let _ = std::panic::catch_unwind(stop);
}

/// # Safety
/// `error` is an NSError or nil.
unsafe fn error_text(error: *mut AnyObject) -> Option<String> {
    let error = unsafe { error.as_ref() }?;
    let text: Retained<NSString> = unsafe { msg_send![error, localizedDescription] };
    Some(text.to_string())
}

/// # Safety
/// `view` is an NSView on screen.
unsafe fn draw_window(view: &AnyObject, to: &Path) -> Result<(), String> {
    let bounds: NSRect = unsafe { msg_send![view, bounds] };
    let rep: Option<Retained<AnyObject>> = unsafe { msg_send![view, bitmapImageRepForCachingDisplayInRect: bounds] };
    let rep = rep.ok_or("The panel could not be drawn")?;
    let _: () = unsafe { msg_send![view, cacheDisplayInRect: bounds, toBitmapImageRep: &*rep] };
    // NSBitmapImageFileTypePNG.
    let props: Retained<AnyObject> = unsafe { msg_send![AnyClass::get(c"NSDictionary").unwrap(), dictionary] };
    let data: Option<Retained<AnyObject>> = unsafe { msg_send![&*rep, representationUsingType: 4usize, properties: &*props] };
    let data = data.ok_or("The panel could not be drawn")?;
    let length: usize = unsafe { msg_send![&*data, length] };
    let bytes: *const std::ffi::c_void = unsafe { msg_send![&*data, bytes] };
    std::fs::write(to, unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), length) }).map_err(|e| e.to_string())
}

fn to_png(scanned: &Path, out: &Path) -> Result<(), String> {
    let image = image::open(scanned).map_err(|_| "The scanner did not return a readable page".to_string())?;
    image.save_with_format(out, image::ImageFormat::Png).map_err(|e| e.to_string())
}

/// Keeps the first outcome and ends the panel's modal loop.
fn finish(result: Result<(), String>) {
    let first = STATE.with_borrow_mut(|s| s.done.is_none().then(|| s.done = Some(result)).is_some());
    if first {
        stop();
    }
}

fn stop() {
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        if app.modalWindow().is_some() {
            app.stopModal();
        }
    }
}
