//! Active-screen resolution shared by every NSPanel positioner (main panel,
//! toast). Centralizing this means a single fallback chain — and a single
//! place to fix whatever the next "which monitor?" edge case turns out to be.

use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEvent, NSScreen};
use objc2_foundation::MainThreadMarker;

/// Resolve "the screen the user is currently looking at". Falls back through
/// the cursor's screen, then the key window's screen, then the main screen,
/// so callers always get *something* usable as long as macOS has any screen
/// attached.
pub fn current_active_screen(mtm: MainThreadMarker) -> Option<Retained<NSScreen>> {
    let mouse_loc = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    for screen in screens.iter() {
        let f = screen.frame();
        if mouse_loc.x >= f.origin.x
            && mouse_loc.x < f.origin.x + f.size.width
            && mouse_loc.y >= f.origin.y
            && mouse_loc.y < f.origin.y + f.size.height
        {
            return Some(screen);
        }
    }

    let app = NSApplication::sharedApplication(mtm);
    if let Some(key_window) = app.keyWindow() {
        if let Some(screen) = key_window.screen() {
            return Some(screen);
        }
    }

    NSScreen::mainScreen(mtm)
}
