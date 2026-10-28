//! macOS captures from a camera, as Preview has them. File ▸ Import from iPhone or iPad
//! (Continuity Camera: Take Photo, Scan Documents, Add Sketch on a phone or tablet near the Mac)
//! and Signatures ▸ Camera (the Mac's own camera, ImageKit's picture taker). Each runs in a helper
//! process, as the scanner panel does (`scan_mac`): AppKit gets its own main thread there and the
//! caller waits for the process.
//!
//! - `sheaf-app --iphone-to <out> <pdf|photo> <title> <hint> <menu title>`: a small window whose
//!   button opens AppKit's own Import from iPhone or iPad menu (the device list, then Take Photo,
//!   Scan Documents, Add Sketch). With `pdf` the result is a PDF at `<out>` (the scanned pages, or
//!   the photo or sketch on a page); with `photo` a PNG (photos and sketches only).
//! - `sheaf-app --camera-to <png> <title>`: ImageKit's picture taker, its camera first; the
//!   picture taken saved as PNG.
//!
//! Exit status 0: the result written; 1: closed without one, or an error, said on stderr.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, NSObject, Sel};
use objc2::{msg_send, sel, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSTextAlignment, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

// IKPictureTaker (Quartz's ImageKit).
#[link(name = "Quartz", kind = "framework")]
unsafe extern "C" {}

const PDF_TYPE: &str = "com.adobe.pdf";

struct State {
    out: PathBuf,
    pdf: bool,
    menu: Option<Retained<AnyObject>>,
    done: Option<Result<(), String>>,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State { out: PathBuf::new(), pdf: false, menu: None, done: None }) };
}

/// Runs a helper when Sheaf was started as `--iphone-to …` or `--camera-to …`; returns its exit
/// status.
pub fn helper(mut args: impl Iterator<Item = String>) -> Option<i32> {
    let _program = args.next();
    let result = match args.next().as_deref() {
        Some("--iphone-to") => {
            let out = PathBuf::from(args.next()?);
            let pdf = args.next().as_deref() != Some("photo");
            let title = args.next().unwrap_or_else(|| "Import from iPhone or iPad".into());
            let hint = args.next().unwrap_or_default();
            let menu = args.next().unwrap_or_else(|| title.clone());
            from_phone(&out, pdf, &title, &hint, &menu)
        }
        Some("--camera-to") => {
            let out = PathBuf::from(args.next()?);
            let title = args.next().unwrap_or_else(|| "Camera".into());
            from_camera(&out, &title)
        }
        _ => return None,
    };
    Some(match result {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    })
}

/// The window with AppKit's Import from iPhone or iPad menu, until a photo, scan or sketch comes
/// back (true) or the window is closed (false).
fn from_phone(out: &Path, pdf: bool, title: &str, hint: &str, menu_title: &str) -> Result<bool, String> {
    let mtm = MainThreadMarker::new().ok_or("Continuity Camera must run on the main thread")?;
    STATE.with_borrow_mut(|s| (s.out, s.pdf) = (out.to_path_buf(), pdf));
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let size = NSSize::new(460.0, 170.0);
    let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable;
    // SAFETY: a plain window made on the main thread; it is kept until the helper ends.
    let window = unsafe { NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), NSRect::new(NSPoint::new(0.0, 0.0), size), style, NSBackingStoreType::Buffered, false) };
    // SAFETY: the window is owned by `window` (Retained), not released by AppKit on close.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(title));

    // SAFETY: the messages below are AppKit's documented API (NSApplication, NSMenu, NSMenuItem,
    // NSView, NSButton, NSTextField, NSPasteboard) with their declared argument types; the
    // objects made here live until the helper ends.
    unsafe {
        let ns = |name: &std::ffi::CStr| AnyClass::get(name).ok_or("AppKit is unavailable");
        // The types the window takes back: AppKit asks only for registered ones.
        let image_types: Retained<AnyObject> = msg_send![ns(c"NSImage")?, imageTypes];
        let returns: Retained<AnyObject> = if pdf { msg_send![&*image_types, arrayByAddingObject: &*NSString::from_str(PDF_TYPE)] } else { image_types };
        let sends: Retained<AnyObject> = msg_send![ns(c"NSArray")?, array];
        let _: () = msg_send![&*app, registerServicesMenuSendTypes: &*sends, returnTypes: &*returns];

        // AppKit fills an item marked NSMenuItemImportFromDeviceIdentifier only in the main menu:
        // the device list (with Take Photo, Scan Documents, Add Sketch) appears when its menu opens.
        let menu_class = ns(c"NSMenu")?;
        let item_class = ns(c"NSMenuItem")?;
        let empty = NSString::from_str("");
        let main: Retained<AnyObject> = msg_send![menu_class, new];
        let file: Retained<AnyObject> = msg_send![menu_class, new];
        let _: () = msg_send![&*file, setTitle: &*NSString::from_str(menu_title)];
        for submenu in [msg_send![menu_class, new], file.clone()] {
            let submenu: Retained<AnyObject> = submenu;
            let allocated: Allocated<AnyObject> = msg_send![item_class, alloc];
            let top: Retained<AnyObject> = msg_send![allocated, initWithTitle: &*NSString::from_str(menu_title), action: None::<Sel>, keyEquivalent: &*empty];
            let _: () = msg_send![&*top, setSubmenu: &*submenu];
            let _: () = msg_send![&*main, addItem: &*top];
        }
        let allocated: Allocated<AnyObject> = msg_send![item_class, alloc];
        let import: Retained<AnyObject> = msg_send![allocated, initWithTitle: &*NSString::from_str(menu_title), action: None::<Sel>, keyEquivalent: &*empty];
        let _: () = msg_send![&*import, setIdentifier: &*NSString::from_str("NSMenuItemImportFromDeviceIdentifier")];
        let _: () = msg_send![&*file, addItem: &*import];
        let _: () = msg_send![&*app, setMainMenu: &*main];

        // The content view takes the result (NSServicesMenuRequestor), from the first responder.
        let view: Retained<AnyObject> = msg_send![requestor_class(), new];
        let _: () = msg_send![&*window, setContentView: &*view];
        let delegate: Retained<AnyObject> = msg_send![delegate_class(), new];
        let label: Retained<AnyObject> = msg_send![ns(c"NSTextField")?, wrappingLabelWithString: &*NSString::from_str(hint)];
        let _: () = msg_send![&*label, setFrame: NSRect::new(NSPoint::new(24.0, 70.0), NSSize::new(size.width - 48.0, 76.0))];
        let _: () = msg_send![&*label, setAlignment: NSTextAlignment::Center];
        let grey: Retained<AnyObject> = msg_send![ns(c"NSColor")?, secondaryLabelColor];
        let _: () = msg_send![&*label, setTextColor: &*grey];
        let _: () = msg_send![&*view, addSubview: &*label];
        let button: Retained<AnyObject> = msg_send![ns(c"NSButton")?, buttonWithTitle: &*NSString::from_str(&format!("{menu_title}…")), target: &*delegate, action: sel!(choose:)];
        let _: () = msg_send![&*button, sizeToFit];
        let frame: NSRect = msg_send![&*button, frame];
        let width = frame.size.width.max(200.0);
        let _: () = msg_send![&*button, setFrame: NSRect::new(NSPoint::new((size.width - width) / 2.0, 24.0), NSSize::new(width, frame.size.height))];
        let _: () = msg_send![&*button, setKeyEquivalent: &*NSString::from_str("\r")];
        let _: () = msg_send![&*view, addSubview: &*button];
        let _: () = msg_send![&*window, setDelegate: &*delegate];
        STATE.with_borrow_mut(|s| s.menu = Some(file.clone()));

        window.center();
        window.makeKeyAndOrderFront(None);
        let _: Bool = msg_send![&*window, makeFirstResponder: &*view];
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        let checked = cfg!(debug_assertions) && (std::env::var_os("SHEAF_TEST_IPHONE_SHOT").is_some() || std::env::var_os("SHEAF_TEST_IPHONE_PASTE").is_some());
        if checked {
            // As `run` would: AppKit's Continuity Camera starts with the app.
            app.finishLaunching();
            if let Some(shot) = std::env::var_os("SHEAF_TEST_IPHONE_SHOT") {
                // A check on a Mac: the menu opened under the button, its items (as AppKit filled
                // them with the devices near) said on stderr, then the window drawn to a PNG.
                run_for(1.0);
                let timer: Retained<AnyObject> = msg_send![ns(c"NSTimer")?, timerWithTimeInterval: 2.0f64, target: &*delegate, selector: sel!(menuShown:), userInfo: &*file, repeats: Bool::NO];
                let run_loop: Retained<AnyObject> = msg_send![ns(c"NSRunLoop")?, currentRunLoop];
                let _: () = msg_send![&*run_loop, addTimer: &*timer, forMode: &*NSString::from_str("kCFRunLoopCommonModes")];
                let _: () = msg_send![&*delegate, choose: &*button];
                let content: Retained<AnyObject> = msg_send![&*window, contentView];
                return crate::scan_mac::draw_view(&content, Path::new(&shot)).map(|()| false);
            }
            if let Some(sample) = std::env::var_os("SHEAF_TEST_IPHONE_PASTE") {
                // A check on a Mac without a phone at hand: the sample handed to the window as
                // Continuity Camera hands its result, through a pasteboard.
                run_for(1.0);
                let sample = Path::new(&sample);
                let bytes = std::fs::read(sample).map_err(|e| e.to_string())?;
                let kind = match sample.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
                    Some("pdf") => PDF_TYPE,
                    Some("png") => "public.png",
                    Some("heic") => "public.heic",
                    _ => "public.jpeg",
                };
                let pasteboard: Retained<AnyObject> = msg_send![ns(c"NSPasteboard")?, pasteboardWithUniqueName];
                let types: Retained<AnyObject> = msg_send![ns(c"NSArray")?, arrayWithObject: &*NSString::from_str(kind)];
                let _: isize = msg_send![&*pasteboard, declareTypes: &*types, owner: None::<&AnyObject>];
                let data: Retained<AnyObject> = msg_send![ns(c"NSData")?, dataWithBytes: bytes.as_ptr().cast::<std::ffi::c_void>(), length: bytes.len()];
                let _: Bool = msg_send![&*pasteboard, setData: &*data, forType: &*NSString::from_str(kind)];
                let requestor: *mut AnyObject = msg_send![&*view, validRequestorForSendType: None::<&AnyObject>, returnType: &*NSString::from_str(kind)];
                if requestor.is_null() {
                    STATE.with_borrow_mut(|s| s.done = Some(Err(format!("The window does not take {kind}"))));
                } else {
                    let _: Bool = msg_send![&*view, readSelectionFromPasteboard: &*pasteboard];
                }
                let _: () = msg_send![&*pasteboard, releaseGlobally];
            }
        }
        if STATE.with_borrow(|s| s.done.is_none()) {
            app.run();
        }
        window.close();
        let _ = (delegate, import, main);
    }
    match STATE.with_borrow_mut(|s| s.done.take()) {
        Some(Ok(())) => Ok(true),
        Some(Err(e)) => Err(e),
        None => Ok(false),
    }
}

/// ImageKit's picture taker, the Mac's camera first, until a picture is taken (true) or the
/// panel is cancelled (false).
fn from_camera(out: &Path, title: &str) -> Result<bool, String> {
    let mtm = MainThreadMarker::new().ok_or("The camera panel must run on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let taker_class = AnyClass::get(c"IKPictureTaker").ok_or("ImageKit's picture taker is unavailable")?;
    // SAFETY: IKPictureTaker's documented API (IKPictureTaker.h): its option keys (their values are
    // their names), an NSValue size, runModal's NSModalResponse and outputImage's NSImage.
    unsafe {
        let taker: Retained<AnyObject> = msg_send![taker_class, pictureTaker];
        let yes: Retained<AnyObject> = msg_send![AnyClass::get(c"NSNumber").unwrap(), numberWithBool: Bool::YES];
        let no: Retained<AnyObject> = msg_send![AnyClass::get(c"NSNumber").unwrap(), numberWithBool: Bool::NO];
        let set = |key: &str, value: &AnyObject| {
            let _: () = msg_send![&*taker, setValue: value, forKey: &*NSString::from_str(key)];
        };
        set("IKPictureTakerAllowsVideoCaptureKey", &yes);
        set("IKPictureTakerAllowsFileChoosingKey", &yes);
        set("IKPictureTakerShowRecentPictureKey", &no);
        set("IKPictureTakerShowEffectsKey", &no);
        // A signature is wide: the crop is three times as wide as high, the output large enough
        // to sign with.
        let crop: Retained<AnyObject> = msg_send![AnyClass::get(c"NSValue").unwrap(), valueWithSize: NSSize::new(480.0, 160.0)];
        set("IKPictureTakerCropAreaSizeKey", &crop);
        let largest: Retained<AnyObject> = msg_send![AnyClass::get(c"NSValue").unwrap(), valueWithSize: NSSize::new(1500.0, 500.0)];
        set("IKPictureTakerOutputImageMaxSizeKey", &largest);
        let _: () = msg_send![&*taker, setTitle: &*NSString::from_str(title)];
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        let image: Option<Retained<AnyObject>> = if cfg!(debug_assertions) && let Some(shot) = std::env::var_os("SHEAF_TEST_CAMERA_SHOT") {
            // A check on a Mac: the panel shown (not modal) and its views said on stderr; the
            // picture is drawn by another process (NSRemoteView), so the PNG holds only the frame.
            let delegate: Retained<AnyObject> = msg_send![delegate_class(), new];
            let none: *mut std::ffi::c_void = std::ptr::null_mut();
            let _: () = msg_send![&*taker, beginPictureTakerWithDelegate: &*delegate, didEndSelector: sel!(pictureTaker:didEnd:context:), contextInfo: none];
            run_for(4.0);
            let content: Retained<AnyObject> = msg_send![&*taker, contentView];
            let frame: Option<Retained<AnyObject>> = msg_send![&*content, superview];
            describe(frame.as_deref().unwrap_or(&content), 0);
            let visible: Bool = msg_send![&*taker, isVisible];
            eprintln!("panel visible: {}", visible.as_bool());
            crate::scan_mac::draw_view(&content, Path::new(&shot))?;
            return Ok(false);
        } else if cfg!(debug_assertions) && let Some(sample) = std::env::var_os("SHEAF_TEST_CAMERA_IMAGE") {
            // A check on a Mac: the sample as the picture taken, through the same saving.
            let path = NSString::from_str(&sample.to_string_lossy());
            let allocated: Allocated<AnyObject> = msg_send![AnyClass::get(c"NSImage").unwrap(), alloc];
            msg_send![allocated, initWithContentsOfFile: &*path]
        } else {
            let response: isize = msg_send![&*taker, runModal];
            // NSModalResponseOK.
            if response != 1 {
                return Ok(false);
            }
            msg_send![&*taker, outputImage]
        };
        let image = image.ok_or("The camera gave no picture")?;
        save_png(&image, out)?;
    }
    Ok(true)
}

/// Runs the main run loop for `seconds`.
fn run_for(seconds: f64) {
    // SAFETY: NSDate and NSRunLoop's documented class and instance methods.
    unsafe {
        let until: Retained<AnyObject> = msg_send![AnyClass::get(c"NSDate").unwrap(), dateWithTimeIntervalSinceNow: seconds];
        let run_loop: Retained<AnyObject> = msg_send![AnyClass::get(c"NSRunLoop").unwrap(), currentRunLoop];
        let _: () = msg_send![&*run_loop, runUntilDate: &*until];
    }
}

/// A view tree, one class (and title) per line, on stderr.
///
/// # Safety
/// `view` is an NSView.
unsafe fn describe(view: &AnyObject, depth: usize) {
    let class = view.class().name().to_string_lossy().into_owned();
    let titled: Bool = unsafe { msg_send![view, respondsToSelector: sel!(title)] };
    let title = if titled.as_bool() {
        let t: Option<Retained<NSString>> = unsafe { msg_send![view, title] };
        t.map(|t| format!(" \"{t}\"")).unwrap_or_default()
    } else {
        String::new()
    };
    eprintln!("{}{class}{title}", "  ".repeat(depth));
    let subviews: Retained<AnyObject> = unsafe { msg_send![view, subviews] };
    let count: usize = unsafe { msg_send![&*subviews, count] };
    for i in 0..count {
        let child: Retained<AnyObject> = unsafe { msg_send![&*subviews, objectAtIndex: i] };
        unsafe { describe(&child, depth + 1) };
    }
}

/// An NSImage saved as PNG at `out`.
///
/// # Safety
/// `image` is an NSImage.
unsafe fn save_png(image: &AnyObject, out: &Path) -> Result<(), String> {
    let tiff: Option<Retained<AnyObject>> = unsafe { msg_send![image, TIFFRepresentation] };
    let tiff = tiff.ok_or("The picture could not be read")?;
    let rep: Option<Retained<AnyObject>> = unsafe { msg_send![AnyClass::get(c"NSBitmapImageRep").unwrap(), imageRepWithData: &*tiff] };
    let rep = rep.ok_or("The picture could not be read")?;
    let props: Retained<AnyObject> = unsafe { msg_send![AnyClass::get(c"NSDictionary").unwrap(), dictionary] };
    // NSBitmapImageFileTypePNG.
    let png: Option<Retained<AnyObject>> = unsafe { msg_send![&*rep, representationUsingType: 4usize, properties: &*props] };
    unsafe { write_data(&*png.ok_or("The picture could not be saved")?, out) }
}

/// # Safety
/// `data` is an NSData.
unsafe fn write_data(data: &AnyObject, to: &Path) -> Result<(), String> {
    let length: usize = unsafe { msg_send![data, length] };
    let bytes: *const std::ffi::c_void = unsafe { msg_send![data, bytes] };
    let slice = if length == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), length) } };
    std::fs::write(to, slice).map_err(|e| e.to_string())
}

/// What Continuity Camera handed back, written to the output: the PDF as it is (Scan Documents),
/// a picture as PNG, or put on a page when a PDF is wanted.
///
/// # Safety
/// `pasteboard` is an NSPasteboard.
unsafe fn take(pasteboard: &AnyObject) -> Result<(), String> {
    let (out, pdf) = STATE.with_borrow(|s| (s.out.clone(), s.pdf));
    let pdf_type = NSString::from_str(PDF_TYPE);
    let types: Retained<AnyObject> = unsafe { msg_send![AnyClass::get(c"NSArray").unwrap(), arrayWithObject: &*pdf_type] };
    let found: Option<Retained<NSString>> = unsafe { msg_send![pasteboard, availableTypeFromArray: &*types] };
    if pdf && found.is_some() {
        let data: Option<Retained<AnyObject>> = unsafe { msg_send![pasteboard, dataForType: &*pdf_type] };
        unsafe { write_data(&*data.ok_or("The scan could not be read")?, &out)? };
        let readable = std::fs::read(&out).is_ok_and(|b| sheaf_core::pages::count_in(&b) > 0);
        return if readable { Ok(()) } else { Err("The scan is not a readable PDF".into()) };
    }
    let image_class = AnyClass::get(c"NSImage").unwrap();
    let readable: Bool = unsafe { msg_send![image_class, canInitWithPasteboard: pasteboard] };
    if !readable.as_bool() {
        return Err("The picture could not be read".into());
    }
    let allocated: Allocated<AnyObject> = unsafe { msg_send![image_class, alloc] };
    let image: Option<Retained<AnyObject>> = unsafe { msg_send![allocated, initWithPasteboard: pasteboard] };
    let image = image.ok_or("The picture could not be read")?;
    if !pdf {
        return unsafe { save_png(&image, &out) };
    }
    let png = out.with_extension("png");
    unsafe { save_png(&image, &png)? };
    let made = sheaf_core::images::images_to_pdf(std::slice::from_ref(&png), sheaf_core::images::PageLayout::FitImage, &out);
    let _ = std::fs::remove_file(&png);
    made.map(|_| ()).map_err(|e| e.to_string())
}

/// `SheafPhoneView`: the window's content view, which Continuity Camera hands its result to.
fn requestor_class() -> &'static AnyClass {
    static CLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();
    CLASS.get_or_init(|| {
        let view = AnyClass::get(c"NSView").expect("AppKit is loaded");
        let mut builder = ClassBuilder::new(c"SheafPhoneView", view).expect("SheafPhoneView is defined once");
        // SAFETY: each function's signature matches its selector's (NSServicesMenuRequestor and
        // NSResponder: self, _cmd, then object arguments; BOOL or id returns).
        unsafe {
            builder.add_method(sel!(acceptsFirstResponder), accepts as extern "C-unwind" fn(_, _) -> _);
            builder.add_method(sel!(validRequestorForSendType:returnType:), requestor as extern "C-unwind" fn(_, _, _, _) -> _);
            builder.add_method(sel!(readSelectionFromPasteboard:), read as extern "C-unwind" fn(_, _, _) -> _);
        }
        builder.register()
    })
}

extern "C-unwind" fn accepts(_this: &AnyObject, _cmd: Sel) -> Bool {
    Bool::YES
}

/// The view itself for nothing sent and a picture (or, for a PDF, a PDF) returned.
extern "C-unwind" fn requestor(this: &AnyObject, _cmd: Sel, send: *mut AnyObject, ret: *mut NSString) -> *mut AnyObject {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `ret` is the pasteboard type AppKit asks about (an NSString) or nil.
        let Some(ret) = (unsafe { ret.as_ref() }) else { return std::ptr::null_mut() };
        let pdf = STATE.with_borrow(|s| s.pdf);
        let image_types: Retained<AnyObject> = unsafe { msg_send![AnyClass::get(c"NSImage").unwrap(), imageTypes] };
        let image: Bool = unsafe { msg_send![&*image_types, containsObject: ret] };
        // NSImage's types are UTIs; AppKit also asks for TIFF under its old name.
        let ret_text = ret.to_string();
        let tiff = ret_text == "NeXT TIFF v4.0 pasteboard type";
        let taken = send.is_null() && (image.as_bool() || tiff || (pdf && ret_text == PDF_TYPE));
        if cfg!(debug_assertions) && std::env::var_os("SHEAF_TEST_IPHONE_SHOT").is_some() {
            eprintln!("asked: return {ret}, taken {taken}");
        }
        if taken { this as *const AnyObject as *mut AnyObject } else { std::ptr::null_mut() }
    }))
    .unwrap_or(std::ptr::null_mut())
}

extern "C-unwind" fn read(_this: &AnyObject, _cmd: Sel, pasteboard: *mut AnyObject) -> Bool {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `pasteboard` is the NSPasteboard AppKit passes.
        match unsafe { pasteboard.as_ref() } {
            Some(p) => unsafe { take(p) },
            None => Err("Nothing came back".into()),
        }
    }))
    .unwrap_or_else(|_| Err("The result could not be read".into()));
    let ok = result.is_ok();
    finish(result);
    Bool::new(ok)
}

/// `SheafCaptureDelegate`: the button's target, the window's delegate, the checks' timer target.
fn delegate_class() -> &'static AnyClass {
    static CLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();
    CLASS.get_or_init(|| {
        let mut builder = ClassBuilder::new(c"SheafCaptureDelegate", NSObject::class()).expect("SheafCaptureDelegate is defined once");
        // SAFETY: each function's signature matches its selector's (void return; self, _cmd and
        // the object or integer arguments AppKit and ImageKit pass).
        unsafe {
            builder.add_method(sel!(choose:), choose as extern "C-unwind" fn(_, _, _));
            builder.add_method(sel!(menuShown:), menu_shown as extern "C-unwind" fn(_, _, _));
            builder.add_method(sel!(windowWillClose:), closing as extern "C-unwind" fn(_, _, _));
            builder.add_method(sel!(pictureTaker:didEnd:context:), picture_ended as extern "C-unwind" fn(_, _, _, _, _));
        }
        builder.register()
    })
}

/// The button: AppKit's import menu opened under it (the device, then Take Photo, Scan Documents,
/// Add Sketch).
extern "C-unwind" fn choose(_this: &AnyObject, _cmd: Sel, sender: *mut AnyObject) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        STATE.with_borrow(|s| {
            // SAFETY: `sender` is the NSButton; the menu is the main menu's, which AppKit fills.
            if let (Some(menu), Some(button)) = (&s.menu, unsafe { sender.as_ref() }) {
                let bounds: NSRect = unsafe { msg_send![button, bounds] };
                let at = NSPoint::new(0.0, bounds.size.height + 4.0);
                let _: Bool = unsafe { msg_send![&**menu, popUpMenuPositioningItem: None::<&AnyObject>, atLocation: at, inView: button] };
            }
        });
    }));
}

/// SHEAF_TEST_IPHONE_SHOT: the open menu's items, then it closes.
extern "C-unwind" fn menu_shown(_this: &AnyObject, _cmd: Sel, timer: *mut AnyObject) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `timer` is the NSTimer whose userInfo is the NSMenu shown.
        unsafe {
            let Some(timer) = timer.as_ref() else { return };
            let menu: Retained<AnyObject> = msg_send![timer, userInfo];
            let app: Retained<AnyObject> = msg_send![AnyClass::get(c"NSApplication").unwrap(), sharedApplication];
            let key: Option<Retained<AnyObject>> = msg_send![&*app, keyWindow];
            let active: Bool = msg_send![&*app, isActive];
            let responder = key.as_ref().and_then(|k| -> Option<Retained<AnyObject>> { msg_send![&**k, firstResponder] });
            eprintln!("active {}, key window {}, first responder {}", active.as_bool(), key.is_some(), responder.map(|r| r.class().name().to_string_lossy().into_owned()).unwrap_or_default());
            list_menu(&menu, 0);
            let _: () = msg_send![&*menu, cancelTracking];
        }
    }));
}

/// # Safety
/// `menu` is an NSMenu.
unsafe fn list_menu(menu: &AnyObject, depth: usize) {
    let count: isize = unsafe { msg_send![menu, numberOfItems] };
    for i in 0..count {
        let item: Retained<AnyObject> = unsafe { msg_send![menu, itemAtIndex: i] };
        let title: Retained<NSString> = unsafe { msg_send![&*item, title] };
        let enabled: Bool = unsafe { msg_send![&*item, isEnabled] };
        eprintln!("menu: {}{title}{}", "  ".repeat(depth), if enabled.as_bool() { "" } else { " (disabled)" });
        let submenu: Option<Retained<AnyObject>> = unsafe { msg_send![&*item, submenu] };
        if let Some(submenu) = submenu {
            // Validated as when it opens: the items AppKit can run with this window enabled.
            let _: () = unsafe { msg_send![&*submenu, update] };
            unsafe { list_menu(&submenu, depth + 1) };
        }
    }
}

extern "C-unwind" fn closing(_this: &AnyObject, _cmd: Sel, _notification: *mut AnyObject) {
    let _ = std::panic::catch_unwind(stop);
}

extern "C-unwind" fn picture_ended(_this: &AnyObject, _cmd: Sel, _taker: *mut AnyObject, _code: isize, _context: *mut std::ffi::c_void) {}

/// Keeps the first outcome and ends the helper's run loop.
fn finish(result: Result<(), String>) {
    let first = STATE.with_borrow_mut(|s| s.done.is_none().then(|| s.done = Some(result)).is_some());
    if first {
        stop();
    }
}

/// Stops `[NSApp run]` (an event posted so it notices at once).
fn stop() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    app.stop(None);
    // SAFETY: NSEvent's documented otherEventWithType:… with NSEventTypeApplicationDefined (15)
    // and plain values; postEvent:atStart: on the shared application.
    unsafe {
        let event: Option<Retained<AnyObject>> = msg_send![
            AnyClass::get(c"NSEvent").unwrap(),
            otherEventWithType: 15usize,
            location: NSPoint::new(0.0, 0.0),
            modifierFlags: 0usize,
            timestamp: 0.0f64,
            windowNumber: 0isize,
            context: None::<&AnyObject>,
            subtype: 0i16,
            data1: 0isize,
            data2: 0isize
        ];
        if let Some(event) = event {
            let _: () = msg_send![&*app, postEvent: &*event, atStart: Bool::YES];
        }
    }
}
