use tauri::tray::TrayIconId;
use tauri::{Manager, Position, Size};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

tauri_panel! {
    panel!(AgentNotifyPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })

    panel_event!(AgentNotifyPanelEventHandler {
        window_did_resign_key(notification: &NSNotification) -> ()
    })
}

pub fn init(app_handle: &tauri::AppHandle) -> tauri::Result<()> {
    if app_handle.get_webview_panel("main").is_ok() {
        return Ok(());
    }

    let window = app_handle.get_webview_window("main").unwrap();
    let panel = window.to_panel::<AgentNotifyPanel>()?;

    panel.set_has_shadow(false);
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

    panel.set_event_handler(Some(event_handler.as_ref()));

    Ok(())
}

pub fn position_panel_at_tray_icon(
    app_handle: &tauri::AppHandle,
    icon_position: Position,
    icon_size: Size,
) {
    let Some(window) = app_handle.get_webview_window("main") else {
        log::warn!("position_panel_at_tray_icon: main window not found");
        return;
    };

    let (icon_phys_x, icon_phys_y) = match &icon_position {
        Position::Physical(pos) => (pos.x, pos.y),
        Position::Logical(pos) => (pos.x as i32, pos.y as i32),
    };

    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(e) => {
            log::warn!("position_panel_at_tray_icon: failed to get monitors: {}", e);
            return;
        }
    };
    // Find the monitor containing the tray icon.
    // When the menu bar is hidden (auto-hide or fullscreen), the tray icon Y coordinate
    // may fall outside monitor bounds. In that case, fall back to X-only matching so
    // that the correct monitor is used for scale factor and coordinate conversion.
    let found_monitor = monitors.iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        let x_in = icon_phys_x >= pos.x && icon_phys_x < pos.x + size.width as i32;
        let y_in = icon_phys_y >= pos.y && icon_phys_y < pos.y + size.height as i32;
        x_in && y_in
    });
    let found_monitor = found_monitor.or_else(|| {
        monitors.iter().find(|m| {
            let pos = m.position();
            let size = m.size();
            icon_phys_x >= pos.x && icon_phys_x < pos.x + size.width as i32
        })
    });

    let monitor = match found_monitor.or_else(|| monitors.first()) {
        Some(m) => m,
        None => {
            log::warn!("position_panel_at_tray_icon: no monitors available");
            return;
        }
    };
    let scale_factor = monitor.scale_factor();
    let window_size = match window.outer_size() {
        Ok(s) => s,
        Err(e) => {
            log::warn!(
                "position_panel_at_tray_icon: failed to get window size: {}",
                e
            );
            return;
        }
    };
    let window_width_phys = window_size.width as i32;

    let (icon_phys_x, icon_phys_y, icon_width_phys, icon_height_phys) =
        match (icon_position, icon_size) {
            (Position::Physical(pos), Size::Physical(size)) => {
                (pos.x, pos.y, size.width as i32, size.height as i32)
            }
            (Position::Logical(pos), Size::Logical(size)) => (
                (pos.x * scale_factor) as i32,
                (pos.y * scale_factor) as i32,
                (size.width * scale_factor) as i32,
                (size.height * scale_factor) as i32,
            ),
            (Position::Physical(pos), Size::Logical(size)) => (
                pos.x,
                pos.y,
                (size.width * scale_factor) as i32,
                (size.height * scale_factor) as i32,
            ),
            (Position::Logical(pos), Size::Physical(size)) => (
                (pos.x * scale_factor) as i32,
                (pos.y * scale_factor) as i32,
                size.width as i32,
                size.height as i32,
            ),
        };

    let icon_center_x_phys = icon_phys_x + (icon_width_phys / 2);
    let panel_x_phys = icon_center_x_phys - (window_width_phys / 2);
    let nudge_up_points: f64 = 8.0;
    let nudge_up_phys = (nudge_up_points * scale_factor).round() as i32;
    let panel_y_phys = icon_phys_y + icon_height_phys - nudge_up_phys;

    // Clamp panel position within monitor bounds.
    // Use menu-bar-bottom as the Y minimum so that the panel never sticks to
    // the very top of the screen (happens when menu bar is auto-hidden or
    // in fullscreen mode — tray.rect() returns y≈0 in these cases).
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();
    let window_height_phys = window_size.height as i32;
    let menu_bar_bottom_phys = monitor_pos.y + (25.0 * scale_factor).round() as i32 - nudge_up_phys;
    let panel_y_phys = panel_y_phys.max(menu_bar_bottom_phys);
    let panel_y_phys =
        panel_y_phys.min(monitor_pos.y + monitor_size.height as i32 - window_height_phys);
    let panel_x_phys = panel_x_phys.max(monitor_pos.x);
    let panel_x_phys =
        panel_x_phys.min(monitor_pos.x + monitor_size.width as i32 - window_width_phys);

    set_panel_position_sync(app_handle, panel_x_phys, panel_y_phys, monitor);
}

/// Set panel position directly via NSPanel (synchronous, no Tauri async dispatch).
/// Converts from Tauri global physical coordinates (top-left origin) to macOS screen
/// coordinates (bottom-left origin) using the target monitor for correct mixed-DPI
/// multi-monitor conversion.
fn set_panel_position_sync(
    app_handle: &tauri::AppHandle,
    phys_x: i32,
    phys_y: i32,
    monitor: &tauri::Monitor,
) {
    let panel_handle = match app_handle.get_webview_panel("main") {
        Ok(p) => p,
        Err(_) => {
            log::warn!("set_panel_position_sync: panel not found");
            return;
        }
    };
    let ns_panel = panel_handle.as_panel();
    let scale = monitor.scale_factor();
    let mon_pos = monitor.position();
    let mon_size = monitor.size();

    unsafe {
        use objc2::msg_send;

        // Get the panel's current frame to know window height
        let frame: tauri_nspanel::NSRect = msg_send![ns_panel, frame];
        let win_height = frame.size.height;

        // Convert global physical to local physical (relative to target monitor)
        let local_phys_x = phys_x - mon_pos.x;
        let local_phys_y = phys_y - mon_pos.y;

        // Convert local physical to local logical using the target monitor's scale
        let local_logical_x = local_phys_x as f64 / scale;
        let local_logical_y = local_phys_y as f64 / scale;

        // Find the matching NSScreen by logical dimensions
        let screens: *const objc2::runtime::AnyObject = msg_send![objc2::class!(NSScreen), screens];
        if screens.is_null() {
            return;
        }
        let count: usize = msg_send![screens, count];
        if count == 0 {
            return;
        }

        let mon_logical_w = mon_size.width as f64 / scale;
        let mon_logical_h = mon_size.height as f64 / scale;

        // Approximate expected NSScreen origin.x from Tauri physical position.
        // Exact for uniform-DPI setups (the only case where duplicate sizes occur
        // in practice); reasonable heuristic for mixed-DPI.
        let expected_x = mon_pos.x as f64 / scale;

        let mut screen_frame: Option<tauri_nspanel::NSRect> = None;
        let mut best_dist = f64::MAX;
        for i in 0..count {
            let scr: *const objc2::runtime::AnyObject = msg_send![screens, objectAtIndex: i];
            if scr.is_null() {
                continue;
            }
            let sf: tauri_nspanel::NSRect = msg_send![scr, frame];
            // Match by logical dimensions (tolerance for rounding)
            if (sf.size.width - mon_logical_w).abs() < 2.0
                && (sf.size.height - mon_logical_h).abs() < 2.0
            {
                // Among size-matched screens, pick the one closest to the
                // expected origin to disambiguate identical monitors.
                let dist = (sf.origin.x - expected_x).abs();
                if dist < best_dist {
                    best_dist = dist;
                    screen_frame = Some(sf);
                }
            }
        }

        let screen_frame = match screen_frame {
            Some(f) => f,
            None => {
                // Fallback to primary screen
                let primary: *const objc2::runtime::AnyObject =
                    msg_send![screens, objectAtIndex: 0usize];
                if primary.is_null() {
                    return;
                }
                msg_send![primary, frame]
            }
        };

        // Convert to macOS coordinates using the matched NSScreen's frame.
        // NSScreen origin is at bottom-left in macOS global logical coords.
        // local_logical_y is distance from the TOP of the screen (Tauri convention).
        let macos_x = screen_frame.origin.x + local_logical_x;
        let macos_y =
            screen_frame.origin.y + screen_frame.size.height - local_logical_y - win_height;

        let origin = tauri_nspanel::NSPoint::new(macos_x, macos_y);
        let _: () = msg_send![ns_panel, setFrameOrigin: origin];
    }
}

/// Find the monitor whose bounds contain the given physical point.
fn find_monitor_containing(
    monitors: &[tauri::Monitor],
    phys_x: i32,
    phys_y: i32,
) -> Option<&tauri::Monitor> {
    monitors.iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        phys_x >= pos.x
            && phys_x < pos.x + size.width as i32
            && phys_y >= pos.y
            && phys_y < pos.y + size.height as i32
    })
}

/// Position panel at the top-right (menu bar area) of the given monitor.
fn position_panel_on_monitor(
    app_handle: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
) {
    let scale = monitor.scale_factor();
    let mon_pos = monitor.position();
    let mon_size = monitor.size();
    let win_size = match window.outer_size() {
        Ok(s) => s,
        Err(e) => {
            log::warn!(
                "position_panel_on_monitor: failed to get window size: {}",
                e
            );
            return;
        }
    };

    // Right-aligned with margin
    let margin_phys = (16.0 * scale).round() as i32;
    let panel_x = mon_pos.x + mon_size.width as i32 - win_size.width as i32 - margin_phys;

    // Below the menu bar (~25 logical pt) with nudge up (~8 logical pt)
    let menu_bar_phys = (25.0 * scale).round() as i32;
    let nudge_phys = (8.0 * scale).round() as i32;
    let panel_y = mon_pos.y + menu_bar_phys - nudge_phys;

    // Clamp within monitor bounds
    let panel_x = panel_x.max(mon_pos.x);
    let panel_x = panel_x.min(mon_pos.x + mon_size.width as i32 - win_size.width as i32);
    let panel_y = panel_y.max(mon_pos.y);
    let panel_y = panel_y.min(mon_pos.y + mon_size.height as i32 - win_size.height as i32);

    set_panel_position_sync(app_handle, panel_x, panel_y, monitor);
}

/// Position panel for shortcut-triggered toggle.
/// Uses cursor position to determine the active monitor. If the cursor and tray icon
/// are on the same monitor, delegates to `position_panel_at_tray_icon` for precise
/// alignment. Otherwise, positions at the top-right of the cursor's monitor.
pub fn position_panel_for_shortcut(app_handle: &tauri::AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        log::warn!("position_panel_for_shortcut: main window not found");
        return;
    };

    let monitors = match window.available_monitors() {
        Ok(m) if !m.is_empty() => m,
        Ok(_) => {
            log::warn!("position_panel_for_shortcut: no monitors available");
            return;
        }
        Err(e) => {
            log::warn!("position_panel_for_shortcut: failed to get monitors: {}", e);
            return;
        }
    };

    // Determine cursor's monitor
    let cursor_monitor_pos = match window.cursor_position() {
        Ok(pos) => {
            let cx = pos.x as i32;
            let cy = pos.y as i32;
            find_monitor_containing(&monitors, cx, cy)
                .or_else(|| monitors.first())
                .map(|m| *m.position())
        }
        Err(_) => None,
    };

    // Try to get tray icon rect and its monitor
    let tray_info = app_handle
        .tray_by_id(&TrayIconId::new("tray"))
        .and_then(|tray| tray.rect().ok().flatten())
        .map(|rect| {
            let (tx, ty) = match &rect.position {
                Position::Physical(p) => (p.x, p.y),
                Position::Logical(p) => (p.x as i32, p.y as i32),
            };
            let tray_monitor_pos =
                find_monitor_containing(&monitors, tx, ty).map(|m| *m.position());
            (rect, tx, tray_monitor_pos)
        });

    match (cursor_monitor_pos, &tray_info) {
        // Cursor and tray on the same monitor → use tray position for precise alignment
        (Some(cursor_mp), Some((rect, _, Some(tray_mp)))) if cursor_mp == *tray_mp => {
            position_panel_at_tray_icon(app_handle, rect.position, rect.size);
        }
        // Tray Y out of bounds but X on cursor's monitor (hidden menu bar / fullscreen)
        // → still use tray position so the panel aligns with the tray icon horizontally
        (Some(cursor_mp), Some((rect, tx, None))) => {
            let tray_x_on_cursor_monitor = monitors
                .iter()
                .find(|m| *m.position() == cursor_mp)
                .is_some_and(|mon| {
                    let mp = mon.position();
                    let ms = mon.size();
                    *tx >= mp.x && *tx < mp.x + ms.width as i32
                });
            if tray_x_on_cursor_monitor {
                position_panel_at_tray_icon(app_handle, rect.position, rect.size);
            } else if let Some(monitor) = monitors.iter().find(|m| *m.position() == cursor_mp) {
                position_panel_on_monitor(app_handle, &window, monitor);
            }
        }
        // Cursor on a different monitor → position on cursor's monitor
        (Some(cursor_mp), _) => {
            if let Some(monitor) = monitors.iter().find(|m| *m.position() == cursor_mp) {
                position_panel_on_monitor(app_handle, &window, monitor);
            }
        }
        // cursor_position() failed → fallback to tray position if available
        (None, Some((rect, _, _))) => {
            position_panel_at_tray_icon(app_handle, rect.position, rect.size);
        }
        // Both failed → fallback to first monitor's top-right
        (None, None) => {
            if let Some(monitor) = monitors.first() {
                position_panel_on_monitor(app_handle, &window, monitor);
            }
        }
    }
}
