//! Query the macOS system for reserved symbolic hotkeys (Spotlight, Mission
//! Control, screenshots, etc.) via Carbon's `CopySymbolicHotKeys()`.
//!
//! The returned CFArray mirrors the structure of
//! `~/Library/Preferences/com.apple.symbolichotkeys.plist`:
//!
//! ```text
//! [{
//!     enabled = 1;
//!     value = {
//!         type = standard;
//!         parameters = (charCode, keyCode, modifierFlags);
//!     };
//! }, ...]
//! ```
//!
//! where `modifierFlags` uses `NSEventModifierFlag*` bits (1<<17..1<<20).
//! CFString dictionary keys (`enabled` / `value` / `parameters`) are built at
//! runtime so we don't depend on non-public `kHISymbolicHotKey*` externs,
//! which are not exported from the current `Carbon.framework` .tbd.

use std::ffi::{c_void, CString};
use std::sync::OnceLock;

type OSStatus = i32;
type CFIndex = isize;
type Boolean = u8;
type CFTypeRef = *const c_void;
type CFTypeID = usize;
type CFArrayRef = CFTypeRef;
type CFDictionaryRef = CFTypeRef;
type CFNumberRef = CFTypeRef;
type CFBooleanRef = CFTypeRef;
type CFStringRef = CFTypeRef;

// kCFNumberSInt64Type — safe for both keycode (SInt32) and modifiers (UInt32).
const CF_NUMBER_SINT64: i32 = 4;
const CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

// NSEventModifierFlag bits — matches what the symbolichotkeys plist stores.
const NS_FLAG_SHIFT: i64 = 1 << 17;
const NS_FLAG_CONTROL: i64 = 1 << 18;
const NS_FLAG_OPTION: i64 = 1 << 19;
const NS_FLAG_COMMAND: i64 = 1 << 20;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, idx: CFIndex) -> CFTypeRef;
    fn CFArrayGetTypeID() -> CFTypeID;
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: CFStringRef) -> CFTypeRef;
    fn CFDictionaryGetTypeID() -> CFTypeID;
    fn CFNumberGetValue(number: CFNumberRef, ty: i32, value_ptr: *mut c_void) -> Boolean;
    fn CFNumberGetTypeID() -> CFTypeID;
    fn CFBooleanGetValue(b: CFBooleanRef) -> Boolean;
    fn CFBooleanGetTypeID() -> CFTypeID;
    fn CFStringCreateWithCString(alloc: CFTypeRef, c_str: *const i8, encoding: u32) -> CFStringRef;
    fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;
    fn CFRelease(cf: CFTypeRef);
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn CopySymbolicHotKeys(out: *mut CFArrayRef) -> OSStatus;
}

static RESERVED_CACHE: OnceLock<Vec<String>> = OnceLock::new();

/// Return the list of currently-enabled macOS symbolic hotkeys serialized as
/// `super+ctrl+n` style strings, matching `tauri-plugin-global-shortcut`
/// parser expectations and the frontend `codeToToken()` output.
pub fn reserved_shortcuts() -> Vec<String> {
    RESERVED_CACHE.get_or_init(|| unsafe { compute() }).clone()
}

unsafe fn make_cfstring(s: &str) -> CFStringRef {
    let c = CString::new(s).expect("key contains NUL");
    unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), CF_STRING_ENCODING_UTF8) }
}

unsafe fn as_cfnumber_i64(value: CFTypeRef) -> Option<i64> {
    if value.is_null() {
        return None;
    }
    unsafe {
        if CFGetTypeID(value) != CFNumberGetTypeID() {
            return None;
        }
        let mut out: i64 = 0;
        let ok = CFNumberGetValue(value, CF_NUMBER_SINT64, &mut out as *mut _ as *mut c_void);
        if ok == 0 {
            None
        } else {
            Some(out)
        }
    }
}

unsafe fn is_truthy(value: CFTypeRef) -> bool {
    if value.is_null() {
        return false;
    }
    unsafe {
        let id = CFGetTypeID(value);
        if id == CFBooleanGetTypeID() {
            CFBooleanGetValue(value) != 0
        } else if id == CFNumberGetTypeID() {
            as_cfnumber_i64(value).unwrap_or(0) != 0
        } else {
            false
        }
    }
}

unsafe fn compute() -> Vec<String> {
    let enabled_key = unsafe { make_cfstring("enabled") };
    let value_key = unsafe { make_cfstring("value") };
    let parameters_key = unsafe { make_cfstring("parameters") };

    let result = unsafe { parse_all(enabled_key, value_key, parameters_key) };

    unsafe {
        CFRelease(enabled_key);
        CFRelease(value_key);
        CFRelease(parameters_key);
    }

    result
}

unsafe fn parse_all(
    enabled_key: CFStringRef,
    value_key: CFStringRef,
    parameters_key: CFStringRef,
) -> Vec<String> {
    let mut out: CFArrayRef = std::ptr::null();
    let status = unsafe { CopySymbolicHotKeys(&mut out) };
    if status != 0 || out.is_null() {
        log::debug!(
            "CopySymbolicHotKeys returned status={} (null={})",
            status,
            out.is_null()
        );
        return Vec::new();
    }

    let count = unsafe { CFArrayGetCount(out) };
    let mut result = Vec::new();

    for i in 0..count {
        let item = unsafe { CFArrayGetValueAtIndex(out, i) };
        if item.is_null() || unsafe { CFGetTypeID(item) != CFDictionaryGetTypeID() } {
            continue;
        }

        let enabled_val = unsafe { CFDictionaryGetValue(item, enabled_key) };
        if !unsafe { is_truthy(enabled_val) } {
            continue;
        }

        let value_dict = unsafe { CFDictionaryGetValue(item, value_key) };
        if value_dict.is_null() || unsafe { CFGetTypeID(value_dict) != CFDictionaryGetTypeID() } {
            continue;
        }

        let params = unsafe { CFDictionaryGetValue(value_dict, parameters_key) };
        if params.is_null() || unsafe { CFGetTypeID(params) != CFArrayGetTypeID() } {
            continue;
        }

        let param_count = unsafe { CFArrayGetCount(params) };
        if param_count < 3 {
            continue;
        }

        let keycode = match unsafe { as_cfnumber_i64(CFArrayGetValueAtIndex(params, 1)) } {
            Some(v) => v,
            None => continue,
        };
        let modifiers = match unsafe { as_cfnumber_i64(CFArrayGetValueAtIndex(params, 2)) } {
            Some(v) => v,
            None => continue,
        };

        // keycode 65535 (0xFFFF) means "no virtual key" (char-only shortcut).
        if !(0..=0xFF).contains(&keycode) {
            continue;
        }

        let Some(token) = keycode_to_token(keycode as i32) else {
            continue;
        };

        let mut parts = modifiers_to_parts(modifiers);
        if parts.is_empty() {
            continue;
        }
        parts.push(token.to_string());
        result.push(parts.join("+"));
    }

    unsafe { CFRelease(out) };
    result
}

fn modifiers_to_parts(mods: i64) -> Vec<String> {
    let mut parts = Vec::new();
    if mods & NS_FLAG_COMMAND != 0 {
        parts.push("super".to_string());
    }
    if mods & NS_FLAG_CONTROL != 0 {
        parts.push("ctrl".to_string());
    }
    if mods & NS_FLAG_OPTION != 0 {
        parts.push("alt".to_string());
    }
    if mods & NS_FLAG_SHIFT != 0 {
        parts.push("shift".to_string());
    }
    parts
}

/// Map a macOS virtual keycode (`HIToolbox/Events.h` `kVK_*`) to the token
/// we use in shortcut strings. Must stay in sync with the frontend
/// `codeToToken()` in `src/components/shortcut-recorder.tsx`.
fn keycode_to_token(code: i32) -> Option<&'static str> {
    Some(match code {
        // ANSI letters (kVK_ANSI_A..Z)
        0x00 => "a",
        0x0B => "b",
        0x08 => "c",
        0x02 => "d",
        0x0E => "e",
        0x03 => "f",
        0x05 => "g",
        0x04 => "h",
        0x22 => "i",
        0x26 => "j",
        0x28 => "k",
        0x25 => "l",
        0x2E => "m",
        0x2D => "n",
        0x1F => "o",
        0x23 => "p",
        0x0C => "q",
        0x0F => "r",
        0x01 => "s",
        0x11 => "t",
        0x20 => "u",
        0x09 => "v",
        0x0D => "w",
        0x07 => "x",
        0x10 => "y",
        0x06 => "z",

        // Digits (kVK_ANSI_0..9)
        0x1D => "0",
        0x12 => "1",
        0x13 => "2",
        0x14 => "3",
        0x15 => "4",
        0x17 => "5",
        0x16 => "6",
        0x1A => "7",
        0x1C => "8",
        0x19 => "9",

        // Symbols
        0x18 => "=",
        0x1B => "-",
        0x1E => "]",
        0x21 => "[",
        0x27 => "'",
        0x29 => ";",
        0x2A => "\\",
        0x2B => ",",
        0x2C => "/",
        0x2F => ".",
        0x32 => "`",

        // Whitespace / edit keys
        0x24 => "Enter",
        0x30 => "Tab",
        0x31 => "Space",
        0x33 => "Backspace",
        0x35 => "Escape",
        0x75 => "Delete",

        // Navigation
        0x72 => "Insert",
        0x73 => "Home",
        0x74 => "PageUp",
        0x77 => "End",
        0x79 => "PageDown",
        0x7B => "Left",
        0x7C => "Right",
        0x7D => "Down",
        0x7E => "Up",

        // Function keys
        0x7A => "F1",
        0x78 => "F2",
        0x63 => "F3",
        0x76 => "F4",
        0x60 => "F5",
        0x61 => "F6",
        0x62 => "F7",
        0x64 => "F8",
        0x65 => "F9",
        0x6D => "F10",
        0x67 => "F11",
        0x6F => "F12",
        0x69 => "F13",
        0x6B => "F14",
        0x71 => "F15",
        0x6A => "F16",
        0x40 => "F17",
        0x4F => "F18",
        0x50 => "F19",
        0x5A => "F20",

        // Keypad digits (frontend emits Num0..9)
        0x52 => "Num0",
        0x53 => "Num1",
        0x54 => "Num2",
        0x55 => "Num3",
        0x56 => "Num4",
        0x57 => "Num5",
        0x58 => "Num6",
        0x59 => "Num7",
        0x5B => "Num8",
        0x5C => "Num9",

        _ => return None,
    })
}
