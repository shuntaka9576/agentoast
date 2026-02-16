use std::sync::OnceLock;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_nspanel::ManagerExt;

use crate::panel::position_panel_at_tray_icon;

static MUTE_MENU_ITEM: OnceLock<MenuItem<tauri::Wry>> = OnceLock::new();

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

fn show_panel(app_handle: &AppHandle) {
    if let Some(panel) = get_or_init_panel!(app_handle) {
        let _ = app_handle.emit("notifications:refresh", ());
        panel.show_and_make_key();
    }
}

pub fn toggle_panel(app_handle: &AppHandle) {
    let Some(panel) = get_or_init_panel!(app_handle) else {
        return;
    };
    if panel.is_visible() {
        panel.hide();
    } else {
        let _ = app_handle.emit("notifications:refresh", ());
        panel.show_and_make_key();
    }
}

pub fn create(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray_icon_path = app_handle
        .path()
        .resolve("icons/tray-icon.png", BaseDirectory::Resource)?;
    let icon = Image::from_path(tray_icon_path)?;

    let show = MenuItem::with_id(app_handle, "show", "Show", true, None::<&str>)?;
    let mute = MenuItem::with_id(app_handle, "mute", "Mute Notifications", true, None::<&str>)?;
    let _ = MUTE_MENU_ITEM.set(mute.clone());
    let clear_all = MenuItem::with_id(app_handle, "clear_all", "Clear All", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app_handle)?;
    let quit = MenuItem::with_id(app_handle, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app_handle, &[&show, &mute, &clear_all, &separator, &quit])?;

    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Agentoast")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app_handle, event| match event.id.as_ref() {
            "show" => {
                show_panel(app_handle);
            }
            "mute" => {
                if let Err(e) = crate::do_toggle_global_mute(app_handle) {
                    log::error!("Failed to toggle mute: {}", e);
                }
            }
            "clear_all" => {
                let db_path = agentoast_shared::config::db_path();
                if let Ok(conn) = agentoast_shared::db::open(&db_path) {
                    let _ = agentoast_shared::db::delete_all_notifications(&conn);
                }
                let _ = app_handle.emit("notifications:refresh", ());
                let _ = app_handle.emit("notifications:unread-count", 0i64);
                crate::watcher::update_tray_icon(app_handle, 0);
            }
            "quit" => {
                app_handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let app_handle = tray.app_handle();

            if let TrayIconEvent::Click {
                button_state, rect, ..
            } = event
            {
                if button_state == MouseButtonState::Up {
                    let Some(panel) = get_or_init_panel!(app_handle) else {
                        return;
                    };

                    if panel.is_visible() {
                        panel.hide();
                        return;
                    }

                    panel.show_and_make_key();
                    position_panel_at_tray_icon(app_handle, rect.position, rect.size);
                }
            }
        })
        .build(app_handle)?;

    Ok(())
}

pub fn update_mute_menu(_app_handle: &AppHandle, global_muted: bool) {
    let mute_label = if global_muted {
        "Unmute Notifications"
    } else {
        "Mute Notifications"
    };

    if let Some(item) = MUTE_MENU_ITEM.get() {
        if let Err(e) = item.set_text(mute_label) {
            log::error!("Failed to update mute menu text: {}", e);
        }
    }
}
