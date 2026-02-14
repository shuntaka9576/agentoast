use tauri::Manager;
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

tauri_panel! {
    panel!(ToastPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

pub fn init(app_handle: &tauri::AppHandle) -> tauri::Result<()> {
    if app_handle.get_webview_panel("toast").is_ok() {
        log::info!("[toast] panel already initialized");
        return Ok(());
    }

    let window = match app_handle.get_webview_window("toast") {
        Some(w) => {
            log::info!("[toast] found toast webview window");
            w
        }
        None => {
            log::warn!("[toast] toast webview window not found");
            return Ok(());
        }
    };

    log::info!("[toast] converting window to panel...");
    let panel = window.to_panel::<ToastPanel>()?;
    log::info!("[toast] panel created successfully");

    panel.set_has_shadow(true);
    panel.set_opaque(false);
    panel.set_level(PanelLevel::Floating.value() + 2);

    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .stationary()
            .full_screen_auxiliary()
            .value(),
    );

    panel.set_style_mask(StyleMask::empty().nonactivating_panel().value());
    log::info!("[toast] init complete");

    Ok(())
}

pub fn show(app_handle: &tauri::AppHandle) {
    log::info!("[toast] show() called");
    // Ensure panel is initialized
    if let Err(e) = init(app_handle) {
        log::error!("[toast] init failed: {}", e);
        return;
    }

    if let Ok(panel) = app_handle.get_webview_panel("toast") {
        position_at_top_right(app_handle);
        log::info!("[toast] showing panel");
        panel.show();
    } else {
        log::warn!("[toast] could not get toast panel");
    }
}

pub fn hide(app_handle: &tauri::AppHandle) {
    if let Ok(panel) = app_handle.get_webview_panel("toast") {
        panel.hide();
    }
}

fn position_at_top_right(app_handle: &tauri::AppHandle) {
    let window = match app_handle.get_webview_window("toast") {
        Some(w) => w,
        None => return,
    };

    let monitor = match window.primary_monitor() {
        Ok(Some(m)) => m,
        _ => return,
    };

    let scale_factor = monitor.scale_factor();
    let monitor_size = monitor.size();
    let monitor_pos = monitor.position();
    let window_size = match window.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };

    let margin_phys = (16.0 * scale_factor).round() as i32;

    let x = monitor_pos.x + monitor_size.width as i32 - window_size.width as i32 - margin_phys;
    // Top margin accounts for macOS menu bar (~25pt)
    let menu_bar_phys = (38.0 * scale_factor).round() as i32;
    let y = monitor_pos.y + menu_bar_phys;

    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}
