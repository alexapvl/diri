//! Swallow keys while a dictation shortcut is being recorded.
//!
//! A local monitor is enough to keep Diri from running the chord. macOS
//! itself still handles a system shortcut such as dictation on Right Option,
//! so when Accessibility is already granted a session event tap takes the
//! key before that happens. The tap exists only for the recording.

use std::ffi::c_void;
use std::ptr;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSMenu, NSMenuItem,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::NSString;
use tokio::sync::mpsc::{self, Receiver, Sender};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CapturedKey {
    pub code: u16,
    pub flags: u64,
    pub letters: Option<char>,
    pub flags_changed: bool,
    pub repeat: bool,
}

pub(crate) struct Capture {
    monitor: Option<Retained<AnyObject>>,
    tap: Option<Tap>,
    /// Cmd+N and Cmd+M are menu shortcuts, so the recorder never saw M or N.
    menu: MenuShortcuts,
    _sender: Box<Sender<CapturedKey>>,
}

struct Tap {
    port: *mut c_void,
    source: *mut c_void,
    loop_: *mut c_void,
    state: *mut TapState,
}

pub(crate) fn begin() -> Option<(Capture, Receiver<CapturedKey>)> {
    let (sender, receiver) = mpsc::channel(32);
    let sender = Box::new(sender);
    let tap = install_tap(&sender);
    // Command chords are often handled before a session tap sees them.
    // The monitor still records a letter that reaches the app.
    let monitor = install_monitor(sender.as_ref().clone());
    if tap.is_none() && monitor.is_none() {
        return None;
    }
    Some((
        Capture {
            monitor,
            tap,
            menu: MenuShortcuts::suspend(),
            _sender: sender,
        },
        receiver,
    ))
}

struct SavedEquivalent {
    item: Retained<NSMenuItem>,
    equivalent: Retained<NSString>,
    modifiers: NSEventModifierFlags,
}

struct MenuShortcuts {
    items: Vec<SavedEquivalent>,
    miniaturizable: Option<(Retained<NSWindow>, bool)>,
}

impl MenuShortcuts {
    fn suspend() -> Self {
        let Some(mtm) = MainThreadMarker::new() else {
            return Self {
                items: Vec::new(),
                miniaturizable: None,
            };
        };
        let app = NSApplication::sharedApplication(mtm);
        let mut items = Vec::new();
        if let Some(menu) = app.mainMenu() {
            clear_equivalents(&menu, &mut items);
        }
        // Cmd+M minimizes even when the Window menu item is gone.
        let miniaturizable = app.keyWindow().map(|window| {
            let mask = window.styleMask();
            let was = mask.contains(NSWindowStyleMask::Miniaturizable);
            window.setStyleMask(mask.difference(NSWindowStyleMask::Miniaturizable));
            (window, was)
        });
        Self {
            items,
            miniaturizable,
        }
    }
}

fn clear_equivalents(menu: &NSMenu, saved: &mut Vec<SavedEquivalent>) {
    for item in menu.itemArray().iter() {
        let equivalent = item.keyEquivalent();
        if equivalent.length() > 0 {
            saved.push(SavedEquivalent {
                equivalent,
                modifiers: item.keyEquivalentModifierMask(),
                item: item.clone(),
            });
            item.setKeyEquivalent(&NSString::from_str(""));
        }
        if let Some(submenu) = item.submenu() {
            clear_equivalents(&submenu, saved);
        }
    }
}

impl MenuShortcuts {
    /// Puts every cleared shortcut back. Running it twice does nothing.
    fn restore(&mut self) {
        for saved in self.items.drain(..) {
            saved.item.setKeyEquivalent(&saved.equivalent);
            saved
                .item
                .setKeyEquivalentModifierMask(saved.modifiers);
        }
        if let Some((window, was)) = self.miniaturizable.take()
            && was
        {
            window.setStyleMask(window.styleMask().union(NSWindowStyleMask::Miniaturizable));
        }
    }
}

impl Drop for MenuShortcuts {
    fn drop(&mut self) {
        self.restore();
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // Restore menu shortcuts before the monitor stops, so a key that is
        // still down is not delivered twice.
        self.menu.restore();
        if let Some(monitor) = self.monitor.take() {
            // SAFETY: this token came from addLocalMonitor on this thread.
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        if let Some(tap) = self.tap.take() {
            unsafe {
                CGEventTapEnable(tap.port, false);
                CFRunLoopRemoveSource(tap.loop_, tap.source, kCFRunLoopDefaultMode);
                CFRelease(tap.source);
                CFRelease(tap.port);
                // The callback can no longer run. The state owns the sender.
                drop(Box::from_raw(tap.state));
            }
        }
    }
}

fn install_monitor(sender: Sender<CapturedKey>) -> Option<Retained<AnyObject>> {
    let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
        let native = unsafe { event.as_ref() };
        let kind = native.r#type();
        let flags_changed = kind == NSEventType::FlagsChanged;
        if kind != NSEventType::KeyDown && !flags_changed {
            return event.as_ptr();
        }
        // AppKit throws when a flags-changed event is asked for its
        // characters or repeat state.
        let (letters, repeat) = if flags_changed {
            (None, false)
        } else {
            (
                native
                    .charactersIgnoringModifiers()
                    .and_then(|text| text.to_string().chars().next()),
                native.isARepeat(),
            )
        };
        let _ = sender.try_send(CapturedKey {
            code: native.keyCode(),
            flags: native.modifierFlags().bits() as u64,
            letters,
            flags_changed,
            repeat,
        });
        // The app must not also run the chord. System shortcuts are a
        // separate path, covered by the event tap when it is installed.
        ptr::null_mut()
    });
    // SAFETY: the block owns its sender, and AppKit calls it on the main thread.
    unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::KeyDown | NSEventMask::FlagsChanged,
            &handler,
        )
    }
}

struct TapState {
    sender: Sender<CapturedKey>,
    port: *mut c_void,
}

fn install_tap(sender: &Sender<CapturedKey>) -> Option<Tap> {
    if !trusted() {
        return None;
    }
    let state = Box::new(TapState {
        sender: sender.clone(),
        port: ptr::null_mut(),
    });
    let state = Box::into_raw(state);
    // Key-down and flags-changed only. Key-ups pass through, so a key held
    // from before recording is not left stuck in another app.
    let mask = (1u64 << 10) | (1u64 << 12);
    // SAFETY: the callback only reads the event and sends on the channel.
    // The state box stays alive until the tap is removed. HID is tried
    // first so a letter held with both Command keys is seen before AppKit
    // treats it as a key equivalent. Session is the fallback.
    let port = unsafe {
        let mut port = CGEventTapCreate(0, 0, 0, mask, tap_callback, state.cast());
        if port.is_null() {
            port = CGEventTapCreate(1, 0, 0, mask, tap_callback, state.cast());
        }
        port
    };
    if port.is_null() {
        // SAFETY: the tap was not created, so the callback cannot run.
        drop(unsafe { Box::from_raw(state) });
        return None;
    }
    unsafe { (*state).port = port };
    // SAFETY: `port` is a fresh tap. The current run loop is the main one,
    // because recording starts from a click.
    let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null_mut(), port, 0) };
    if source.is_null() {
        unsafe {
            CFRelease(port);
            drop(Box::from_raw(state));
        }
        return None;
    }
    let loop_ = unsafe { CFRunLoopGetCurrent() };
    unsafe { CFRunLoopAddSource(loop_, source, kCFRunLoopDefaultMode) };
    unsafe { CGEventTapEnable(port, true) };
    Some(Tap {
        port,
        source,
        loop_,
        state,
    })
}

unsafe extern "C" fn tap_callback(
    _proxy: *mut c_void,
    kind: u32,
    event: *mut c_void,
    user: *mut c_void,
) -> *mut c_void {
    let state = unsafe { &*(user.cast::<TapState>()) };
    // The system disables a tap that takes too long. Turn it back on, and
    // let that one event through.
    if kind == 0xFFFF_FFFE || kind == 0xFFFF_FFFF {
        unsafe { CGEventTapEnable(state.port, true) };
        return event;
    }
    // The tap is system-wide. Keys meant for another app are not ours.
    if !app_is_active() {
        return event;
    }
    let code = unsafe { CGEventGetIntegerValueField(event, 9) } as u16;
    let flags = unsafe { CGEventGetFlags(event) };
    let repeat = unsafe { CGEventGetIntegerValueField(event, 8) } != 0;
    let mut units = [0u16; 4];
    let mut count = 0usize;
    unsafe {
        CGEventKeyboardGetUnicodeString(event, units.len(), &mut count, units.as_mut_ptr());
    }
    let letters = String::from_utf16_lossy(&units[..count.min(units.len())])
        .chars()
        .next();
    let _ = state.sender.try_send(CapturedKey {
        code,
        flags,
        letters,
        flags_changed: kind == 12,
        repeat,
    });
    // Drop the event so dictation and Diri both miss it.
    ptr::null_mut()
}

fn app_is_active() -> bool {
    // The tap's run loop source is on the main run loop.
    MainThreadMarker::new()
        .is_some_and(|mtm| NSApplication::sharedApplication(mtm).isActive())
}

fn trusted() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events: u64,
        callback: unsafe extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventKeyboardGetUnicodeString(
        event: *mut c_void,
        max: usize,
        actual: *mut usize,
        buffer: *mut u16,
    );
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: *const c_void;
    fn CFMachPortCreateRunLoopSource(
        allocator: *mut c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRemoveSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRelease(value: *const c_void);
}
