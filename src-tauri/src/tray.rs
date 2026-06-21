use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_nspanel::ManagerExt;

use crate::panel::position_panel_appkit;

macro_rules! get_or_init_panel {
    ($app_handle:expr) => {
        match $app_handle.get_webview_panel("main") {
            Ok(panel) => Some(panel),
            Err(_) => {
                if let Err(err) = crate::panel::init($app_handle) {
                    log::error!("Failed to init panel: {}", err);
                    None
                } else {
                    match $app_handle.get_webview_panel("main") {
                        Ok(panel) => Some(panel),
                        Err(err) => {
                            log::error!("Panel missing after init: {:?}", err);
                            None
                        }
                    }
                }
            }
        }
    };
}

pub fn toggle_panel(app_handle: &AppHandle) {
    log::info!("[panel-pos] entry=shortcut (tray::toggle_panel)");
    let Some(panel) = get_or_init_panel!(app_handle) else {
        return;
    };
    if panel.is_visible() {
        panel.hide();
    } else {
        let _ = app_handle.emit("panel:shown", ());
        crate::emit_cached_sessions(app_handle);
        let _ = app_handle.emit("notifications:refresh", ());
        panel.set_alpha_value(0.0);
        panel.show_and_make_key();
        position_panel_appkit(app_handle);
        panel.set_alpha_value(1.0);
    }
}

pub fn create(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray_icon_path = app_handle
        .path()
        .resolve("icons/tray-icon.png", BaseDirectory::Resource)?;
    let icon = Image::from_path(tray_icon_path)?;

    let settings = MenuItem::with_id(app_handle, "settings", "Settings…", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app_handle)?;
    let quit = MenuItem::with_id(app_handle, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app_handle, &[&settings, &separator, &quit])?;

    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Agentoast")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app_handle, event| match event.id.as_ref() {
            "settings" => {
                if let Some(window) = app_handle.get_webview_window("settings") {
                    let _ = window.unminimize();
                    if let Err(e) = window.show() {
                        log::error!("Failed to show settings window: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        log::error!("Failed to focus settings window: {}", e);
                    }
                } else {
                    log::error!("Settings window not registered");
                }
            }
            "quit" => {
                app_handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let app_handle = tray.app_handle();

            if let TrayIconEvent::Click { button_state, .. } = event {
                if button_state == MouseButtonState::Up {
                    log::info!("[panel-pos] entry=tray-click");
                    let Some(panel) = get_or_init_panel!(app_handle) else {
                        return;
                    };

                    if panel.is_visible() {
                        panel.hide();
                        return;
                    }

                    let _ = app_handle.emit("panel:shown", ());
                    crate::emit_cached_sessions(app_handle);
                    let _ = app_handle.emit("notifications:refresh", ());
                    panel.set_alpha_value(0.0);
                    panel.show_and_make_key();
                    position_panel_appkit(app_handle);
                    panel.set_alpha_value(1.0);
                }
            }
        })
        .build(app_handle)?;

    Ok(())
}
