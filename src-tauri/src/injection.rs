//! Text insertion at the cursor.
//!
//! Primary path: macOS Accessibility API. Rather than reading the full
//! `AXValue` and computing an insertion offset (which needs `CFRange`
//! wrangling via `AXValueGetValue`/`AXValueCreate`), this sets
//! `kAXSelectedTextAttribute` directly on the focused element — the AX API
//! defines setting that attribute as "replace the current selection", which
//! for a collapsed (empty) selection is exactly "insert at cursor". This is
//! the same mechanism most AX-based text-insertion utilities on macOS use,
//! and it avoids the far more error-prone "read the whole value, compute an
//! offset, write the whole value back" approach.
//!
//! Fallback: clipboard swap + synthetic Cmd+V for apps that don't expose a
//! settable `AXSelectedText` — notably terminal emulators (Terminal.app,
//! iTerm2), which render text via custom drawing rather than a standard AX
//! text value. The fallback saves and restores the clipboard's plain-text
//! contents; a non-text clipboard item (an image, a file reference) is not
//! perfectly round-tripped — a known, documented limitation, not a silent
//! gap.
//!
//! **Runtime behavior cannot be verified in this environment.** Actually
//! exercising this module requires the user to grant Accessibility
//! permission via a macOS system dialog — nothing but a human clicking
//! "Allow" can do that. It compiles and the logic is structurally sound
//! against the real AX/AppKit APIs, but has not been exercised against a
//! real focused text field.
//!
//! Callers should invoke `insert_at_cursor` off the async runtime thread
//! (e.g. via `tokio::task::spawn_blocking`) — the clipboard-fallback path
//! sleeps briefly to give the target app time to read the pasteboard before
//! it's restored.

use std::thread;
use std::time::Duration;

use accessibility_sys::{
    kAXErrorSuccess, kAXFocusedUIElementAttribute, kAXSelectedTextAttribute, AXError,
    AXUIElementCopyAttributeValue, AXUIElementCreateSystemWide, AXUIElementIsAttributeSettable,
    AXUIElementRef, AXUIElementSetAttributeValue,
};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation_sys::base::CFRelease;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

#[derive(Debug, thiserror::Error)]
pub enum InjectionError {
    #[error("no focused element to insert into")]
    NoFocusedElement,
    #[error("AXValue not settable on target, and clipboard fallback failed: {0}")]
    FallbackFailed(String),
}

pub enum InjectionMethod {
    /// Direct `AXSelectedText` set on the focused element.
    Accessibility,
    /// Clipboard swap + synthetic Cmd+V, original clipboard restored after.
    ClipboardFallback,
}

/// Virtual keycode for "V" (kVK_ANSI_V), used for the synthetic Cmd+V.
const KEYCODE_V: u16 = 9;

/// Insert `text` at the current cursor position. Empty/whitespace-only text
/// is the caller's responsibility to filter before calling this (Section 3
/// — nothing is pasted, pill clears silently, on an empty transcription).
pub fn insert_at_cursor(text: &str) -> Result<InjectionMethod, InjectionError> {
    // Native FFI into ApplicationServices/AppKit — CLAUDE.md requires bridge
    // calls guarded so a panic here can never take down the whole app.
    let ax_result = std::panic::catch_unwind(|| try_ax_insert(text));
    if matches!(ax_result, Ok(Ok(()))) {
        return Ok(InjectionMethod::Accessibility);
    }
    clipboard_fallback(text).map(|_| InjectionMethod::ClipboardFallback)
}

/// `Ok(())` only if the focused element actually accepted the text via
/// `kAXSelectedTextAttribute`. Any failure (no focused element, attribute
/// not settable, the set call erroring) means "fall back to the clipboard",
/// not a hard error — the caller treats every non-`Ok` outcome uniformly.
fn try_ax_insert(text: &str) -> Result<(), InjectionError> {
    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return Err(InjectionError::NoFocusedElement);
        }
        let _system_wide_guard = AxElementGuard(system_wide);

        let focused_attr = CFString::new(kAXFocusedUIElementAttribute);
        let mut focused_ref: core_foundation_sys::base::CFTypeRef = std::ptr::null_mut();
        let err: AXError = AXUIElementCopyAttributeValue(
            system_wide,
            focused_attr.as_concrete_TypeRef(),
            &mut focused_ref,
        );
        if err != kAXErrorSuccess || focused_ref.is_null() {
            return Err(InjectionError::NoFocusedElement);
        }
        let focused = focused_ref as AXUIElementRef;
        let _focused_guard = AxElementGuard(focused);

        let selected_text_attr = CFString::new(kAXSelectedTextAttribute);
        let mut settable: u8 = 0;
        let settable_err = AXUIElementIsAttributeSettable(
            focused,
            selected_text_attr.as_concrete_TypeRef(),
            &mut settable,
        );
        if settable_err != kAXErrorSuccess || settable == 0 {
            return Err(InjectionError::NoFocusedElement);
        }

        let value = CFString::new(text);
        let set_err = AXUIElementSetAttributeValue(
            focused,
            selected_text_attr.as_concrete_TypeRef(),
            value.as_concrete_TypeRef() as core_foundation_sys::base::CFTypeRef,
        );
        if set_err != kAXErrorSuccess {
            return Err(InjectionError::NoFocusedElement);
        }
    }
    Ok(())
}

/// RAII guard that `CFRelease`s an owned ("Copy"/"Create" rule) AX
/// reference — `AXUIElementCreateSystemWide` and
/// `AXUIElementCopyAttributeValue` both hand back a +1 reference the caller
/// must release.
struct AxElementGuard(AXUIElementRef);

impl Drop for AxElementGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0 as core_foundation_sys::base::CFTypeRef) };
        }
    }
}

/// Clipboard swap + synthetic Cmd+V, original clipboard plain-text contents
/// restored afterward.
fn clipboard_fallback(text: &str) -> Result<(), InjectionError> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let string_type = unsafe { NSPasteboardTypeString };

    let original = pasteboard.stringForType(string_type).map(|s| s.to_string());

    pasteboard.clearContents();
    let ns_text = NSString::from_str(text);
    if !pasteboard.setString_forType(&ns_text, string_type) {
        return Err(InjectionError::FallbackFailed(
            "NSPasteboard rejected the write".into(),
        ));
    }

    post_cmd_v().map_err(|e| InjectionError::FallbackFailed(format!("synthetic paste failed: {e}")))?;

    // Give the target app a moment to read the pasteboard before it's
    // swapped back — paste is not synchronous from this process's view.
    thread::sleep(Duration::from_millis(150));

    pasteboard.clearContents();
    if let Some(original) = original {
        let ns_original = NSString::from_str(&original);
        pasteboard.setString_forType(&ns_original, string_type);
    }

    Ok(())
}

fn post_cmd_v() -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "could not create CGEventSource".to_string())?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KEYCODE_V, true)
        .map_err(|_| "could not create keydown event".to_string())?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, KEYCODE_V, false)
        .map_err(|_| "could not create keyup event".to_string())?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}
