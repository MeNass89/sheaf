//! A Mac's app menu, as every Mac app has it: About, Check for Updates…, then Settings… (⌘,), then
//! Services, Hide and Quit. Slint makes the app menu itself (About, Services, Hide, Quit) and has no
//! menu roles yet, and it rebuilds the whole menu bar whenever a menu's text or state changes; so
//! Check for Updates… and Settings… are put back each time items are added to the menu bar
//! (NSMenuDidAddItemNotification).

use std::cell::RefCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, NSObject, Sel};
use objc2::{msg_send, sel, ClassType, MainThreadMarker};
use objc2_app_kit::NSApplication;
use objc2_foundation::NSString;

struct Items {
    settings: Retained<NSString>,
    updates: Retained<NSString>,
    target: Retained<AnyObject>,
    open: Box<dyn Fn()>,
    check: Box<dyn Fn()>,
}

thread_local! {
    static ITEMS: RefCell<Option<Items>> = const { RefCell::new(None) };
}

/// Adds Check for Updates… (`updates`, `check` running when it is chosen) after About, and
/// Settings… (`settings`, ⌘,; `open`) after them, to the app menu. Call once on the main thread,
/// after the window is made.
pub fn install(settings: &str, open: impl Fn() + 'static, updates: &str, check: impl Fn() + 'static) {
    if MainThreadMarker::new().is_none() {
        return;
    }
    // SAFETY: a new object of a class registered here; NSNotificationCenter's documented
    // addObserver:selector:name:object: with the target kept alive in ITEMS for the app's life.
    unsafe {
        let target: Retained<AnyObject> = msg_send![target_class(), new];
        let center: Retained<AnyObject> = msg_send![AnyClass::get(c"NSNotificationCenter").unwrap(), defaultCenter];
        let name = NSString::from_str("NSMenuDidAddItemNotification");
        let none: *const AnyObject = std::ptr::null();
        let _: () = msg_send![&*center, addObserver: &*target, selector: sel!(menuChanged:), name: &*name, object: none];
        ITEMS.with_borrow_mut(|s| {
            *s = Some(Items {
                settings: NSString::from_str(settings),
                updates: NSString::from_str(updates),
                target,
                open: Box::new(open),
                check: Box::new(check),
            })
        });
    }
    ensure();
}

/// Makes the app menu read About (0), Check for Updates… (1), a separator (2), Settings… (3), a
/// separator (4), then Slint's own items: each missing item is put back at its place.
fn ensure() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    ITEMS.with_borrow(|s| {
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
            let items: isize = msg_send![&*app, numberOfItems];
            // Slint's app menu not made yet (About and its separator first).
            if items < 2 {
                return;
            }
            let class = AnyClass::get(c"NSMenuItem").unwrap();
            let make = |title: &NSString, action: Sel, key: &str| -> Retained<AnyObject> {
                let allocated: Allocated<AnyObject> = msg_send![class, alloc];
                let item: Retained<AnyObject> = msg_send![allocated, initWithTitle: title, action: action, keyEquivalent: &*NSString::from_str(key)];
                let _: () = msg_send![&*item, setTarget: &*s.target];
                item
            };
            // Check for Updates… right after About, before About's separator.
            let at: isize = msg_send![&*app, indexOfItemWithTitle: &*s.updates];
            if at < 0 {
                let item = make(&s.updates, sel!(checkForUpdates:), "");
                let _: () = msg_send![&*app, insertItem: &*item, atIndex: 1isize];
            }
            // Settings… after that separator (now at 2), with a separator of its own.
            let at: isize = msg_send![&*app, indexOfItemWithTitle: &*s.settings];
            if at < 0 {
                let item = make(&s.settings, sel!(openSettings:), ",");
                let separator: Retained<AnyObject> = msg_send![class, separatorItem];
                let _: () = msg_send![&*app, insertItem: &*item, atIndex: 3isize];
                let _: () = msg_send![&*app, insertItem: &*separator, atIndex: 4isize];
            }
        }
    });
}

/// `SheafMenuTarget`: the target of Check for Updates… and Settings…, and the observer of menu
/// bar changes.
fn target_class() -> &'static AnyClass {
    static CLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();
    CLASS.get_or_init(|| {
        let mut builder = ClassBuilder::new(c"SheafMenuTarget", NSObject::class()).expect("SheafMenuTarget is defined once");
        // SAFETY: the three methods take (self, _cmd, an object) and return nothing, as their selectors.
        unsafe {
            builder.add_method(sel!(openSettings:), open_settings as extern "C-unwind" fn(_, _, _));
            builder.add_method(sel!(checkForUpdates:), check_for_updates as extern "C-unwind" fn(_, _, _));
            builder.add_method(sel!(menuChanged:), menu_changed as extern "C-unwind" fn(_, _, _));
        }
        builder.register()
    })
}

extern "C-unwind" fn open_settings(_this: &AnyObject, _cmd: Sel, _sender: *mut AnyObject) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ITEMS.with_borrow(|s| if let Some(s) = s { (s.open)() })));
}

extern "C-unwind" fn check_for_updates(_this: &AnyObject, _cmd: Sel, _sender: *mut AnyObject) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ITEMS.with_borrow(|s| if let Some(s) = s { (s.check)() })));
}

extern "C-unwind" fn menu_changed(_this: &AnyObject, _cmd: Sel, _notification: *mut AnyObject) {
    // Our own insertions are changes too: `ensure` finds both items there and stops.
    let _ = std::panic::catch_unwind(ensure);
}
