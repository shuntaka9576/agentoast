#![allow(deprecated)] // msg_send_id! is deprecated in objc2 0.6, but still works

use std::sync::OnceLock;

use block2::RcBlock;
use objc2::msg_send_id;
// NB: NSPoint, NSRect, NSEvent, MainThreadMarker, Retained, Bool are pulled in
// at module scope by `tauri_panel!` below — DON'T re-import them or rustc
// trips E0252. Use bare names downstream.
use objc2_app_kit::NSEventMask;

use crate::screen::current_active_screen;
use tauri::tray::TrayIconId;
use tauri::Emitter;
use tauri::Manager;
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

static ESC_MONITOR_INSTALLED: OnceLock<()> = OnceLock::new();

// Intercept Esc at the AppKit local-event-monitor layer, before NSWindow's
// `cancelOperation:` (which on NSPanel calls into the close path even when
// `windowShouldClose:` returns false in some configurations) and before the
// keystroke reaches WebKit. We swallow it and re-deliver as a Tauri event so
// the JS side stays the single source of truth for Esc semantics inside the
// panel (close help / cancel search / leave apps view / hide panel).
fn install_esc_monitor(app_handle: &tauri::AppHandle) {
    let handle = app_handle.clone();
    ESC_MONITOR_INSTALLED.get_or_init(|| {
        unsafe {
            let block = RcBlock::new(
                move |event: std::ptr::NonNull<objc2_app_kit::NSEvent>| -> *mut objc2_app_kit::NSEvent {
                    let event_ref = event.as_ref();
                    let key_code: u16 = objc2::msg_send![event_ref, keyCode];
                    if key_code != 0x35 {
                        return event.as_ptr();
                    }

                    // Only intercept when the event targets our main panel.
                    let panel_window_num: i64 = match handle.get_webview_panel("main") {
                        Ok(p) => objc2::msg_send![p.as_panel(), windowNumber],
                        Err(_) => return event.as_ptr(),
                    };
                    let event_window_num: i64 = objc2::msg_send![event_ref, windowNumber];
                    if event_window_num != panel_window_num {
                        return event.as_ptr();
                    }

                    let _ = handle.emit("panel:esc", ());
                    std::ptr::null_mut()
                },
            );

            let mask = NSEventMask::KeyDown;
            let _monitor: Option<objc2::rc::Retained<objc2_foundation::NSObject>> = msg_send_id![
                objc2_app_kit::NSEvent::class(),
                addLocalMonitorForEventsMatchingMask: mask.0,
                handler: &*block
            ];
            std::mem::forget(_monitor);
            std::mem::forget(block);
        }
    });
}

tauri_panel! {
    panel!(AgentNotifyPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })

    panel_event!(AgentNotifyPanelEventHandler {
        window_did_resign_key(notification: &NSNotification) -> (),
        window_should_close(window: &NSWindow) -> Bool
    })
}

pub fn init(app_handle: &tauri::AppHandle) -> tauri::Result<()> {
    if app_handle.get_webview_panel("main").is_ok() {
        return Ok(());
    }

    let window = app_handle.get_webview_window("main").unwrap();
    let panel = window.to_panel::<AgentNotifyPanel>()?;

    // Native macOS window shadow. AppKit derives the shadow shape from the
    // window's rendered alpha, so it hugs the rounded panel + tray arrow the
    // same way NSMenu/NSPopover shadows do. A CSS box-shadow can't do this:
    // it gets clipped at the window bounds (the padding around the panel),
    // leaving a visible rectangular gradient edge on light backgrounds.
    panel.set_has_shadow(true);
    panel.set_opaque(false);
    panel.set_level(PanelLevel::MainMenu.value() + 1);

    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .stationary()
            .full_screen_auxiliary()
            .value(),
    );

    panel.set_style_mask(StyleMask::empty().nonactivating_panel().value());

    let event_handler = AgentNotifyPanelEventHandler::new();
    let handle = app_handle.clone();
    event_handler.window_did_resign_key(move |_notification| {
        if let Ok(panel) = handle.get_webview_panel("main") {
            panel.hide();
        }
    });
    // Defensive: any future programmatic close request goes through this
    // delegate, which always returns false. Hiding always goes through
    // `panel.hide()` (orderOut:), so blocking close has no functional cost.
    event_handler.window_should_close(|_window| Bool::new(false));

    panel.set_event_handler(Some(event_handler.as_ref()));

    install_esc_monitor(app_handle);

    Ok(())
}

/// Position the main panel on the screen currently containing the cursor (or
/// the key window's screen, or main screen) such that the panel sits just
/// below the menu bar, horizontally centered on the tray icon's right-edge
/// offset.
///
/// Both the tray-click and shortcut-trigger paths funnel through this — the
/// positioning rule is the same in either case: "drop the panel under the
/// tray icon on the screen the user is currently looking at." On
/// multi-monitor setups where macOS replicates the menu bar on every screen
/// (System Settings → Displays → "Show menu bar on all displays"), this puts
/// the panel exactly under the focused screen's tray icon. On "main display
/// only" setups, it puts the panel where the icon *would be* on the active
/// screen (same right-edge offset).
///
/// All coordinates are AppKit (bottom-left origin, logical pt). We never
/// round-trip through Tauri's coordinate API, so no Physical/Logical
/// normalization or scale-factor math is needed.
pub fn position_panel_appkit(app_handle: &tauri::AppHandle) {
    let Some(mtm) = MainThreadMarker::new() else {
        log::warn!("position_panel_appkit: not on main thread");
        return;
    };

    let panel_handle = match app_handle.get_webview_panel("main") {
        Ok(p) => p,
        Err(_) => {
            log::warn!("position_panel_appkit: panel not found");
            return;
        }
    };
    let ns_panel = panel_handle.as_panel();

    let panel_frame: NSRect = unsafe { objc2::msg_send![ns_panel, frame] };
    let panel_width = panel_frame.size.width;
    let panel_height = panel_frame.size.height;

    let Some(active_screen) = current_active_screen(mtm) else {
        log::warn!("position_panel_appkit: no usable screen");
        return;
    };
    let active_frame = active_screen.frame();
    let active_visible = active_screen.visibleFrame();

    let tray_frames = get_tray_frames_appkit(app_handle);

    let (panel_x, panel_y) = if let Some((tray_win_frame, tray_screen_frame)) = tray_frames {
        // Preserve the tray icon's distance from the right edge of its own
        // screen, then apply that same offset on the active screen. This is
        // what makes the panel feel like it pops out from "where the tray
        // would be on this screen" regardless of which monitor the tray
        // physically lives on.
        let tray_right_offset = (tray_screen_frame.origin.x + tray_screen_frame.size.width)
            - (tray_win_frame.origin.x + tray_win_frame.size.width);
        let tray_width = tray_win_frame.size.width;

        let target_tray_right = active_frame.origin.x + active_frame.size.width - tray_right_offset;
        let target_tray_center = target_tray_right - tray_width / 2.0;
        let x = target_tray_center - panel_width / 2.0;
        // visibleFrame's top edge is the bottom of the menu bar — placing
        // panel.y so its top sits exactly there lands it just under the
        // menu bar, no hardcoded height needed.
        let y = active_visible.origin.y + active_visible.size.height - panel_height;
        (x, y)
    } else {
        // Tray status item unavailable (menu bar hidden / not yet registered).
        // Fall back to the same top-right convention the toast panel uses.
        let margin = 16.0;
        let x = active_frame.origin.x + active_frame.size.width - panel_width - margin;
        let y = active_visible.origin.y + active_visible.size.height - panel_height;
        (x, y)
    };

    // Clamp inside the active screen so a tray icon near a screen edge
    // (or a panel wider than expected) can't push the panel off-screen.
    let min_x = active_frame.origin.x;
    let max_x = active_frame.origin.x + active_frame.size.width - panel_width;
    let panel_x = clamp_into(panel_x, min_x, max_x);
    let min_y = active_visible.origin.y;
    let max_y = active_visible.origin.y + active_visible.size.height - panel_height;
    let panel_y = clamp_into(panel_y, min_y, max_y);

    log::info!(
        "[panel-pos] appkit: tray={} active=({:.0},{:.0} {:.0}x{:.0}) final=({:.0},{:.0}) panel=({:.0}x{:.0})",
        tray_frames.is_some(),
        active_frame.origin.x,
        active_frame.origin.y,
        active_frame.size.width,
        active_frame.size.height,
        panel_x,
        panel_y,
        panel_width,
        panel_height
    );

    let origin = NSPoint::new(panel_x, panel_y);
    unsafe {
        let _: () = objc2::msg_send![ns_panel, setFrameOrigin: origin];
    }
}

/// Get the tray icon's window frame and its screen's frame, both in AppKit
/// global coordinates (bottom-left origin, logical pt).
///
/// Uses `tauri::TrayIcon::with_inner_tray_icon` → `tray_icon::TrayIcon::ns_status_item`
/// — a documented but internal-ish path. The `tray-icon` crate is pinned by
/// Tauri minor versions; if Tauri bumps it incompatibly, this will fail to
/// compile (caught in CI).
fn get_tray_frames_appkit(app_handle: &tauri::AppHandle) -> Option<(NSRect, NSRect)> {
    let tray = app_handle.tray_by_id(&TrayIconId::new("tray"))?;
    tray.with_inner_tray_icon(|inner| {
        let item = inner.ns_status_item()?;
        let mtm = MainThreadMarker::new()?;
        let button = item.button(mtm)?;
        let window = button.window()?;
        let screen = window.screen()?;
        Some((window.frame(), screen.frame()))
    })
    .ok()
    .flatten()
}

fn clamp_into(v: f64, lo: f64, hi: f64) -> f64 {
    // Tolerate inverted ranges (panel wider than screen) by anchoring to
    // the lower bound rather than producing NaN.
    if hi < lo {
        return lo;
    }
    v.clamp(lo, hi)
}
