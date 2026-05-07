use tauri::tray::TrayIconId;
use tauri::{Manager, PhysicalPosition, PhysicalSize, Position, Size};
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
    // Block NSPanel's default Esc → cancelOperation: → performClose: → close
    // path. Hiding always goes through `panel.hide()` (orderOut:), so blocking
    // close has no functional cost — but stops the panel from disappearing when
    // the user presses Esc inside the SearchBar input.
    event_handler.window_should_close(|_window| Bool::new(false));

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

    log::info!(
        "[panel-pos] at_tray_icon: icon_pos=({}, {}) icon_size={:?}",
        icon_phys_x,
        icon_phys_y,
        icon_size
    );

    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(e) => {
            log::warn!("position_panel_at_tray_icon: failed to get monitors: {}", e);
            return;
        }
    };
    // Find the monitor containing the tray icon.
    // Tray icons live in the macOS menu bar, which sits *above* the usable
    // rect returned by Tauri, so `icon_phys_y < monitor.position().y` is the
    // normal case. We first try an exact (X ∧ Y) match for completeness, then
    // fall back to "X is inside and the monitor's top is closest-above the
    // tray icon". That tiebreak is critical on setups where two monitors'
    // X ranges overlap (e.g. a huge secondary display positioned so its X
    // range covers the primary): the previous `iter().find(X-only)` would
    // return the first such monitor in the list, sending the panel to the
    // wrong screen even though the tray icon is visually on the other one.
    let in_x_range = |m: &tauri::Monitor| {
        let pos = m.position();
        let size = m.size();
        icon_phys_x >= pos.x && icon_phys_x < pos.x + size.width as i32
    };

    let found_monitor = monitors.iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        let y_in = icon_phys_y >= pos.y && icon_phys_y < pos.y + size.height as i32;
        in_x_range(m) && y_in
    });
    let found_monitor = found_monitor.or_else(|| {
        monitors.iter().filter(|m| in_x_range(m)).min_by_key(|m| {
            let dy = m.position().y - icon_phys_y;
            // Prefer monitors whose top edge sits below the tray icon
            // (the menu-bar case, dy ≥ 0). Among those, pick the one
            // closest to the icon. Monitors whose top is above the icon
            // fall back to distance-only ranking after the preferred set.
            (if dy < 0 { 1 } else { 0 }, dy.unsigned_abs())
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
    // Was 8.0 — that nudged the panel top *into* the menu bar by 8 logical pt
    // so the chrome notch could "kiss" the tray icon visually, but it also
    // made the panel body cover the bottom half of the icon. Sit just below
    // the menu bar instead.
    let nudge_up_points: f64 = 0.0;
    let nudge_up_phys = (nudge_up_points * scale_factor).round() as i32;
    let panel_y_phys = icon_phys_y + icon_height_phys - nudge_up_phys;

    log::info!(
        "[panel-pos] at_tray_icon: monitor_pos={:?} scale={} win_phys=({}x{}) icon_phys=({},{} {}x{}) center_x={} pre_clamp=({},{})",
        monitor.position(),
        scale_factor,
        window_size.width,
        window_size.height,
        icon_phys_x,
        icon_phys_y,
        icon_width_phys,
        icon_height_phys,
        icon_center_x_phys,
        panel_x_phys,
        panel_y_phys
    );

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

    log::info!(
        "[panel-pos] at_tray_icon: final=({},{})",
        panel_x_phys,
        panel_y_phys
    );

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

    log::info!(
        "[panel-pos] on_monitor: monitor_pos={:?} mon_size=({}x{}) scale={} final=({},{})",
        mon_pos,
        mon_size.width,
        mon_size.height,
        scale,
        panel_x,
        panel_y
    );

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
    let cursor_pos_raw = window.cursor_position().ok();
    let cursor_monitor_pos = match &cursor_pos_raw {
        Some(pos) => {
            let cx = pos.x as i32;
            let cy = pos.y as i32;
            find_monitor_containing(&monitors, cx, cy)
                .or_else(|| monitors.first())
                .map(|m| *m.position())
        }
        None => None,
    };

    // Try to get tray icon rect and its monitor.
    //
    // Important: `tray.rect()` and `TrayIconEvent::Click`'s `rect` may report
    // `Position`/`Size` in *different* units — empirically, the click event
    // delivers `Physical(68, 48)` while `tray.rect()` returns `Logical(34, 43)`
    // for the same icon. We normalize everything to physical here using a
    // best-effort scale (the monitor that contains the tray icon's X), so the
    // mirror math operates on a single coordinate system.
    let tray_info = app_handle
        .tray_by_id(&TrayIconId::new("tray"))
        .and_then(|tray| tray.rect().ok().flatten())
        .map(|rect| {
            // Approximate the tray's monitor first using a position-only guess
            // so we can pick a sensible scale. If the position is already
            // physical this is exact; if it's logical we'll re-find later.
            let (raw_x, raw_y) = match &rect.position {
                Position::Physical(p) => (p.x, p.y),
                Position::Logical(p) => (p.x as i32, p.y as i32),
            };
            let approx_monitor =
                find_monitor_containing(&monitors, raw_x, raw_y).or_else(|| monitors.first());
            let scale = approx_monitor.map(|m| m.scale_factor()).unwrap_or(1.0);

            let (px, py, pw, ph) = rect_to_physical(&rect.position, &rect.size, scale);
            let tray_monitor_pos =
                find_monitor_containing(&monitors, px, py).map(|m| *m.position());

            (px, py, pw, ph, tray_monitor_pos)
        });

    log::info!(
        "[panel-pos] for_shortcut: cursor_raw={:?} cursor_monitor={:?} tray_info_phys={:?}",
        cursor_pos_raw.as_ref().map(|p| (p.x, p.y)),
        cursor_monitor_pos,
        tray_info,
    );

    match (cursor_monitor_pos, &tray_info) {
        // Cursor and tray on the same monitor → use tray position directly.
        (Some(cursor_mp), Some((px, py, pw, ph, Some(tray_mp)))) if cursor_mp == *tray_mp => {
            log::info!("[panel-pos] for_shortcut: branch=same-monitor → at_tray_icon");
            position_panel_at_tray_icon(
                app_handle,
                Position::Physical(PhysicalPosition { x: *px, y: *py }),
                Size::Physical(PhysicalSize {
                    width: *pw as u32,
                    height: *ph as u32,
                }),
            );
        }
        // Tray on a different monitor than the cursor → mirror the tray icon
        // onto the cursor's monitor (preserve its distance from the right edge
        // and its vertical offset within the monitor) and reuse the tray-icon
        // positioner. This makes the shortcut feel like "open the panel from
        // the tray, but on the screen I'm currently looking at" instead of
        // jumping the panel to the menu bar's actual screen.
        (Some(cursor_mp), Some((px, py, pw, ph, tray_mp_opt))) => {
            let cursor_monitor = monitors.iter().find(|m| *m.position() == cursor_mp);
            // Resolve a "source monitor" for the tray icon. When tray_mp is
            // None (tray Y outside any monitor — auto-hide menu bar /
            // fullscreen), fall back to "X-in-range, Y closest from below"
            // — same heuristic as `position_panel_at_tray_icon`. Plain
            // `iter().find()` would return the first X-matching monitor in
            // declaration order, which on overlapping multi-monitor setups
            // wrongly attributes the tray to the primary even when its true
            // home is the secondary (and vice-versa).
            let source_monitor = tray_mp_opt
                .as_ref()
                .and_then(|mp| monitors.iter().find(|m| m.position() == mp))
                .or_else(|| {
                    monitors
                        .iter()
                        .filter(|m| {
                            let mp = m.position();
                            let ms = m.size();
                            *px >= mp.x && *px < mp.x + ms.width as i32
                        })
                        .min_by_key(|m| {
                            let dy = m.position().y - *py;
                            (if dy < 0 { 1 } else { 0 }, dy.unsigned_abs())
                        })
                });

            // If the resolved source monitor turns out to be the cursor's
            // monitor, this is really a same-monitor case and we should just
            // use the tray rect directly. Mirroring src==dst would still pick
            // up the original X — which on tray icons near the right edge
            // gets clamped to the screen's right side, making the panel
            // appear to overshoot.
            let is_same_monitor = match (cursor_monitor, source_monitor) {
                (Some(c), Some(s)) => c.position() == s.position(),
                _ => false,
            };

            if is_same_monitor {
                log::info!(
                    "[panel-pos] for_shortcut: branch=mirror-resolved-same-monitor → at_tray_icon"
                );
                position_panel_at_tray_icon(
                    app_handle,
                    Position::Physical(PhysicalPosition { x: *px, y: *py }),
                    Size::Physical(PhysicalSize {
                        width: *pw as u32,
                        height: *ph as u32,
                    }),
                );
            } else if let (Some(cursor_monitor), Some(source_monitor)) =
                (cursor_monitor, source_monitor)
            {
                let (mx, my, mw, mh) =
                    mirror_tray_rect_to_monitor(*px, *py, *pw, *ph, source_monitor, cursor_monitor);
                log::info!(
                    "[panel-pos] for_shortcut: branch=mirrored src_mon={:?} dst_mon={:?} mirrored_phys=({},{} {}x{})",
                    source_monitor.position(),
                    cursor_monitor.position(),
                    mx,
                    my,
                    mw,
                    mh,
                );
                position_panel_at_tray_icon(
                    app_handle,
                    Position::Physical(PhysicalPosition { x: mx, y: my }),
                    Size::Physical(PhysicalSize {
                        width: mw as u32,
                        height: mh as u32,
                    }),
                );
            } else if let Some(monitor) = cursor_monitor {
                log::info!("[panel-pos] for_shortcut: branch=mirror-fallback → on_monitor");
                position_panel_on_monitor(app_handle, &window, monitor);
            }
        }
        // No tray rect available (tray hidden / not registered yet) → just put
        // the panel on the cursor's monitor in the conventional top-right slot.
        (Some(cursor_mp), None) => {
            if let Some(monitor) = monitors.iter().find(|m| *m.position() == cursor_mp) {
                log::info!("[panel-pos] for_shortcut: branch=no-tray → on_monitor");
                position_panel_on_monitor(app_handle, &window, monitor);
            }
        }
        // cursor_position() failed → fallback to tray position if available
        (None, Some((px, py, pw, ph, _))) => {
            log::info!("[panel-pos] for_shortcut: branch=no-cursor-fallback → at_tray_icon");
            position_panel_at_tray_icon(
                app_handle,
                Position::Physical(PhysicalPosition { x: *px, y: *py }),
                Size::Physical(PhysicalSize {
                    width: *pw as u32,
                    height: *ph as u32,
                }),
            );
        }
        // Both failed → fallback to first monitor's top-right
        (None, None) => {
            log::info!("[panel-pos] for_shortcut: branch=all-fallback → on_monitor(first)");
            if let Some(monitor) = monitors.first() {
                position_panel_on_monitor(app_handle, &window, monitor);
            }
        }
    }
}

/// Convert a Tauri `Position`+`Size` pair into physical pixels using the
/// supplied `scale` factor (only consulted when the value is `Logical`).
/// `tray.rect()` and the click event report units inconsistently — this
/// helper lets all downstream math operate in a single coordinate system.
fn rect_to_physical(position: &Position, size: &Size, scale: f64) -> (i32, i32, i32, i32) {
    let (x, y) = match position {
        Position::Physical(p) => (p.x, p.y),
        Position::Logical(p) => ((p.x * scale).round() as i32, (p.y * scale).round() as i32),
    };
    let (w, h) = match size {
        Size::Physical(s) => (s.width as i32, s.height as i32),
        Size::Logical(s) => (
            (s.width * scale).round() as i32,
            (s.height * scale).round() as i32,
        ),
    };
    (x, y, w, h)
}

/// Mirror a tray icon (already in physical pixels) from `src_monitor` onto
/// `dst_monitor`, keeping its distance from the monitor's right edge and
/// placing it at the destination monitor's top-of-screen (menu-bar slot).
///
/// The reported icon height varies between sources (click event vs
/// `tray.rect()`), so we override it with a stable 24-logical-pt value scaled
/// to the destination monitor. That makes the downstream
/// `panel_y = icon_y + icon_h - nudge` formula in `position_panel_at_tray_icon`
/// produce the same vertical position the click path produces (panel sits
/// just under the menu bar).
fn mirror_tray_rect_to_monitor(
    icon_phys_x: i32,
    _icon_phys_y: i32,
    icon_w_phys: i32,
    _icon_h_phys: i32,
    src_monitor: &tauri::Monitor,
    dst_monitor: &tauri::Monitor,
) -> (i32, i32, i32, i32) {
    let src_pos = src_monitor.position();
    let src_size = src_monitor.size();
    let dst_pos = dst_monitor.position();
    let dst_size = dst_monitor.size();
    let dst_scale = dst_monitor.scale_factor();

    let src_right = src_pos.x + src_size.width as i32;
    let dst_right = dst_pos.x + dst_size.width as i32;
    // Distance from the icon's *right edge* to its monitor's right edge.
    let right_offset = src_right - (icon_phys_x + icon_w_phys);
    let new_x = dst_right - right_offset - icon_w_phys;

    // Stable 24-logical-pt icon height in physical pixels at the destination
    // scale. Matches what `TrayIconEvent::Click` reports for the same tray
    // icon, so the panel ends up at the same Y the click path produces.
    let stable_h = (24.0 * dst_scale).round() as i32;
    // Place the (mirrored) icon at the destination monitor's top — the
    // downstream Y formula adds icon_h then subtracts the nudge, which lands
    // the panel just below the menu bar.
    let new_y = dst_pos.y;

    (new_x, new_y, icon_w_phys, stable_h)
}
