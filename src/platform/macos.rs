//! macOS-specific code

// fuck Objective-C
#![allow(clippy::let_unit_value)]

use cocoa::appkit::{NSApp, NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use cocoa::base::id;
use cocoa::base::nil;
use cocoa::foundation::{NSAutoreleasePool, NSBundle, NSString, NSUInteger};

use objc::*;

#[allow(non_upper_case_globals)]
const NSEventModifierFlagOption: NSUInteger = 1 << 19;

#[allow(non_upper_case_globals)]
const NSEventModifierFlagCommand: NSUInteger = 1 << 20;

use objc::runtime::Object;
use core_foundation::runloop::{CFRunLoopRef, CFRunLoopStop};

pub unsafe fn handle_panic(info: &std::panic::PanicInfo) {
    let main_loop: id = msg_send![class!(NSRunLoop), mainRunLoop];
    let main_loop: CFRunLoopRef = msg_send![main_loop, getCFRunLoop];
    CFRunLoopStop(main_loop);

    let _: id = msg_send![NSApp(), stopModal];

    let alert: id = msg_send![class!(NSAlert), alloc];
    let alert: id = msg_send![alert, init];

    let _: id = msg_send![alert, setMessageText: NSString::alloc(nil).init_str("Error").autorelease()];
    let _: id = msg_send![alert, setInformativeText: NSString::alloc(nil)
                                                        .init_str(&format!("{}", info))
                                                        .autorelease()];
    let _: id = msg_send![alert, runModal];

    std::process::exit(1);
}

pub(super) fn setup_panic_handler() {
    std::panic::set_hook(Box::new(|p| unsafe { handle_panic(p) }));
}

#[allow(non_snake_case)]
unsafe fn NSLocalizedString(key: &str) -> *mut Object {
    let bundle: *mut Object = NSBundle::mainBundle();
    let key = NSString::alloc(nil).init_str(key);

    msg_send![bundle, localizedStringForKey: key
                    value: nil
                    table: nil]
}

// create macOS menu-bar
pub(crate) unsafe fn create_menu_bar() {
    // assume that an NSAutoreleasePool is generated by winit-rs.
    let app = NSApp();

    let menubar = NSMenu::new(nil).autorelease();

    let app_menu_item = NSMenuItem::new(nil).autorelease();
    let file_menu_item = NSMenuItem::new(nil).autorelease();
    let window_menu_item = NSMenuItem::new(nil).autorelease();

    menubar.addItem_(app_menu_item);
    menubar.addItem_(file_menu_item);
    menubar.addItem_(window_menu_item);

    app.setMainMenu_(menubar);

    // ReizeiinTohka
    //  - About ReizeiinTohka...
    //  -
    //  - Hide ReizeiinTohka
    //  - Hide others
    //  - Show All
    //  - --
    //  - Quit ReizeiinTohka
    let app_menu = NSMenu::new(nil).autorelease();

    app_menu.addItem_(
        NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSLocalizedString(&format!("Hide {}", crate::constants::GAME_ENGINE_NAME)),
                sel![hide:],
                NSString::alloc(nil).init_str("h"),
            )
            .autorelease(),
    );

    let hide_others = NSMenuItem::alloc(nil)
        .initWithTitle_action_keyEquivalent_(
            NSLocalizedString("Hide Others"),
            sel![hideOtherApplications:],
            NSString::alloc(nil).init_str("h"),
        )
        .autorelease();

    hide_others.setKeyEquivalentModifierMask_(
        NSEventModifierFlags::from_bits(NSEventModifierFlagOption | NSEventModifierFlagCommand)
            .unwrap(),
    );

    app_menu.addItem_(hide_others);

    app_menu.addItem_(
        NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSLocalizedString("Show All"),
                sel![unhideAllApplications:],
                NSString::alloc(nil).init_str(""),
            )
            .autorelease(),
    );

    app_menu.addItem_(NSMenuItem::separatorItem(nil).autorelease());

    let services = NSMenuItem::alloc(nil)
        .initWithTitle_action_keyEquivalent_(
            NSLocalizedString("Services"),
            std::mem::zeroed(), // null-selector
            NSString::alloc(nil).init_str(""),
        )
        .autorelease();

    let services_menu = NSMenu::new(nil).autorelease();
    let _: () = msg_send![app, setServicesMenu: services_menu];
    services.setSubmenu_(services_menu);

    app_menu.addItem_(services);

    app_menu.addItem_(NSMenuItem::separatorItem(nil).autorelease());

    // NOTE: the actual "Quit" should be done by winit.
    //       currently, this performed by closing window.
    let quit_item = NSMenuItem::alloc(nil)
        .initWithTitle_action_keyEquivalent_(
            NSLocalizedString(&format!("Quit {}", crate::constants::GAME_ENGINE_NAME)),
            sel![performClose:],
            NSString::alloc(nil).init_str("q"),
        )
        .autorelease();
    app_menu.addItem_(quit_item);

    app_menu_item.setSubmenu_(app_menu);

    // File
    //  - Close
    let file_menu = NSMenu::new(nil).autorelease();
    let _: () = msg_send![file_menu, setTitle: (NSLocalizedString("File"))];

    file_menu.addItem_(
        NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSLocalizedString("Close"),
                sel![performClose:],
                NSString::alloc(nil).init_str("w"),
            )
            .autorelease(),
    );

    file_menu_item.setSubmenu_(file_menu);

    // File
    //  - Close
    let window_menu = NSMenu::new(nil).autorelease();
    let _: () = msg_send![window_menu, setTitle: (NSLocalizedString("Window"))];
    let _: () = msg_send![app, setWindowsMenu: window_menu];

    window_menu_item.setSubmenu_(window_menu);
}
