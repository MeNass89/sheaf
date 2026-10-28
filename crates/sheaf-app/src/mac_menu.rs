//! A Mac's app menu, as every Mac app has it: About, then Settings… (⌘,), then Services, Hide and
//! Quit. Slint makes the app menu itself (About, Services, Hide, Quit) and has no menu roles yet, and
//! it rebuilds the whole menu bar whenever a menu's text or state changes; so Settings… is put back
//! each time items are added to the menu bar (NSMenuDidAddItemNotification).

use std::cell::RefCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, NSObject, Sel};
use objc2::{msg_send, sel, ClassType, MainThreadMarker};
use objc2_app_kit::NSApplication;
use objc2_foundation::NSString;

struct Settings {
    title: Retained<NSString>,
    target: Retained<AnyObject>,
    open: Box<dyn Fn()>,
}

thread_local! {
    static SETTINGS: RefCell<Option<Settings>> = const { RefCell::new(None) };
}

/// Adds Settings… (`title`, ⌘,) to the app menu, `open` running when it is chosen. Call once on the
/// main thread, after the window is made.
pub fn install(title: &str, open: impl Fn() + 'static) {
    if MainThreadMarker::new().is_none() {
        return;
    }
    // SAFETY: a new object of a class registered here; NSNotificationCenter's documented
    // addObserver:selector:name:object: with the target kept alive in SETTINGS for the app's life.
    unsafe {
        let target: Retained<AnyObject> = msg_send![target_class(), new];
        let center: Retained<AnyObject> = msg_send![AnyClass::get(c"NSNotificationCenter").unwrap(), defaultCenter];
        let name = NSString::from_str("NSMenuDidAddItemNotification");
        let none: *const AnyObject = std::ptr::null();
        let _: () = msg_send![&*center, addObserver: &*target, selector: sel!(menuChanged:), name: &*name, object: none];
        SETTINGS.with_borrow_mut(|s| *s = Some(Settings { title: NSString::from_str(title), target, open: Box::new(open) }));
    }
    ensure();
}

/// Puts Settings… at the app menu's third place (after About and its separator) if it is not there.
fn ensure() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    SETTINGS.with_borrow(|s| {
        let Some(s) = s else { return };
        let Some(main) = NSApplication::sharedApplication(mtm).mainMenu() else { return };
        // SAFETY: NSMenu and NSMenuItem's documented API on the main menu AppKit holds, with the
        // types they declare (NSInteger indexes, NSString titles, SEL actions, an id target).
        unsafe {
            let count: isize = msg_send![&*main, numberOfItems];
            if count == 0 {
                return;
            }
            let first: Retained<AnyObject> = msg_send![&*main, itemAtIndex: 0isize];
            let app: Option<Retained<AnyObject>> = msg_send![&*first, submenu];
            let Some(app) = app else { return };
            let at: isize = msg_send![&*app, indexOfItemWithTitle: &*s.title];
            let items: isize = msg_send![&*app, numberOfItems];
            if at >= 0 || items < 2 {
                return;
            }
            let class = AnyClass::get(c"NSMenuItem").unwrap();
            let allocated: Allocated<AnyObject> = msg_send![class, alloc];
            let item: Retained<AnyObject> = msg_send![allocated, initWithTitle: &*s.title, action: sel!(openSettings:), keyEquivalent: &*NSString::from_str(",")];
            let _: () = msg_send![&*item, setTarget: &*s.target];
            let separator: Retained<AnyObject> = msg_send![class, separatorItem];
            let _: () = msg_send![&*app, insertItem: &*item, atIndex: 2isize];
            let _: () = msg_send![&*app, insertItem: &*separator, atIndex: 3isize];
        }
    });
}

/// `SheafMenuTarget`: Settings…'s target, and the observer of menu bar changes.
fn target_class() -> &'static AnyClass {
    static CLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();
    CLASS.get_or_init(|| {
        let mut builder = ClassBuilder::new(c"SheafMenuTarget", NSObject::class()).expect("SheafMenuTarget is defined once");
        // SAFETY: both methods take (self, _cmd, an object) and return nothing, as their selectors.
        unsafe {
            builder.add_method(sel!(openSettings:), open_settings as extern "C-unwind" fn(_, _, _));
            builder.add_method(sel!(menuChanged:), menu_changed as extern "C-unwind" fn(_, _, _));
        }
        builder.register()
    })
}

extern "C-unwind" fn open_settings(_this: &AnyObject, _cmd: Sel, _sender: *mut AnyObject) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| SETTINGS.with_borrow(|s| if let Some(s) = s { (s.open)() })));
}

extern "C-unwind" fn menu_changed(_this: &AnyObject, _cmd: Sel, _notification: *mut AnyObject) {
    // Our own insertion is a change too: `ensure` finds Settings… there and stops.
    let _ = std::panic::catch_unwind(ensure);
}
