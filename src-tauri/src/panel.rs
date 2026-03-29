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
    let mut found_monitor = None;

    for m in &monitors {
        let pos = m.position();
        let size = m.size();
        let x_in = icon_phys_x >= pos.x && icon_phys_x < pos.x + size.width as i32;
        let y_in = icon_phys_y >= pos.y && icon_phys_y < pos.y + size.height as i32;

        if x_in && y_in {
            found_monitor = Some(m);
            break;
        }
    }

    // Fullscreen mode may cause tray icon coordinates to fall outside monitor bounds.
    // Fall back to the first available monitor.
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

    // Clamp panel position within monitor bounds so it doesn't go off-screen
    // (e.g. fullscreen mode where tray icon coords may be above visible area).
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();
    let window_height_phys = window_size.height as i32;
    let panel_y_phys = panel_y_phys.max(monitor_pos.y);
    let panel_y_phys =
        panel_y_phys.min(monitor_pos.y + monitor_size.height as i32 - window_height_phys);
    let panel_x_phys = panel_x_phys.max(monitor_pos.x);
    let panel_x_phys =
        panel_x_phys.min(monitor_pos.x + monitor_size.width as i32 - window_width_phys);

    let final_pos = tauri::PhysicalPosition::new(panel_x_phys, panel_y_phys);
    let _ = window.set_position(final_pos);
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
fn position_panel_on_monitor(window: &tauri::WebviewWindow, monitor: &tauri::Monitor) {
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

    let _ = window.set_position(tauri::PhysicalPosition::new(panel_x, panel_y));
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
            (rect, tray_monitor_pos)
        });

    match (cursor_monitor_pos, &tray_info) {
        // Cursor and tray on the same monitor → use tray position for precise alignment
        (Some(cursor_mp), Some((rect, Some(tray_mp)))) if cursor_mp == *tray_mp => {
            position_panel_at_tray_icon(app_handle, rect.position, rect.size);
        }
        // Cursor on a different monitor (or tray monitor unknown) → position on cursor's monitor
        (Some(cursor_mp), _) => {
            if let Some(monitor) = monitors.iter().find(|m| *m.position() == cursor_mp) {
                position_panel_on_monitor(&window, monitor);
            }
        }
        // cursor_position() failed → fallback to tray position if available
        (None, Some((rect, _))) => {
            position_panel_at_tray_icon(app_handle, rect.position, rect.size);
        }
        // Both failed → fallback to first monitor's top-right
        (None, None) => {
            if let Some(monitor) = monitors.first() {
                position_panel_on_monitor(&window, monitor);
            }
        }
    }
}
