#[cfg(target_os = "macos")]
mod app_nap;
mod panel;
#[cfg(target_os = "macos")]
mod terminal;
mod toast;
mod tray;
mod watcher;
#[cfg(target_os = "macos")]
mod webkit_config;

use std::collections::HashSet;
use std::sync::Mutex;

use agentoast_shared::config::{self, AppConfig};
use agentoast_shared::db;
use agentoast_shared::models::{Notification, NotificationGroup};
use serde::Serialize;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db_path: std::path::PathBuf,
    pub config: AppConfig,
}

#[derive(Default)]
pub struct MuteState {
    pub global_muted: bool,
    pub muted_groups: HashSet<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MuteStatePayload {
    pub global_muted: bool,
    pub muted_groups: Vec<String>,
}

impl MuteState {
    pub fn to_payload(&self) -> MuteStatePayload {
        let mut groups: Vec<String> = self.muted_groups.iter().cloned().collect();
        groups.sort();
        MuteStatePayload {
            global_muted: self.global_muted,
            muted_groups: groups,
        }
    }
}

pub fn do_toggle_global_mute(app_handle: &tauri::AppHandle) -> Result<MuteStatePayload, String> {
    let mute_state = app_handle.state::<Mutex<MuteState>>();
    let mut state = mute_state.lock().map_err(|e| e.to_string())?;
    state.global_muted = !state.global_muted;
    let payload = state.to_payload();
    let _ = app_handle.emit("mute:changed", &payload);
    tray::update_mute_menu(app_handle, payload.global_muted);
    Ok(payload)
}

#[tauri::command]
fn init_panel(app_handle: tauri::AppHandle) {
    panel::init(&app_handle).expect("Failed to initialize panel");
}

#[tauri::command]
fn hide_panel(app_handle: tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    if let Ok(panel) = app_handle.get_webview_panel("main") {
        panel.hide();
    }
}

#[tauri::command]
fn hide_toast(app_handle: tauri::AppHandle) {
    toast::hide(&app_handle);
}

#[tauri::command]
fn show_panel(app_handle: tauri::AppHandle) {
    panel::init(&app_handle).ok();
    use tauri_nspanel::ManagerExt;
    if let Ok(panel) = app_handle.get_webview_panel("main") {
        let _ = app_handle.emit("notifications:refresh", ());
        panel.show();
    }
}

#[tauri::command]
fn focus_terminal(tmux_pane: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        terminal::focus_terminal(&tmux_pane)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = tmux_pane;
        Err("focus_terminal is only supported on macOS".to_string())
    }
}

#[tauri::command]
fn get_notifications(
    state: tauri::State<'_, Mutex<AppState>>,
    limit: Option<i64>,
) -> Result<Vec<Notification>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::get_notifications(&conn, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_notifications_grouped(
    state: tauri::State<'_, Mutex<AppState>>,
    limit: Option<i64>,
) -> Result<Vec<NotificationGroup>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::get_notifications_grouped(&conn, limit.unwrap_or(100), state.config.panel.group_limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_unread_count(state: tauri::State<'_, Mutex<AppState>>) -> Result<i64, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::get_unread_count(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_notification(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    id: i64,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::delete_notification(&conn, id).map_err(|e| e.to_string())?;
    if let Ok(count) = db::get_unread_count(&conn) {
        let _ = app_handle.emit("notifications:unread-count", count);
        watcher::update_tray_icon(&app_handle, count);
    }
    Ok(())
}

#[tauri::command]
fn delete_notifications_by_group_tmux(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    group_name: String,
    tmux_pane: String,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::delete_notifications_by_group_tmux(&conn, &group_name, &tmux_pane)
        .map_err(|e| e.to_string())?;
    if let Ok(count) = db::get_unread_count(&conn) {
        let _ = app_handle.emit("notifications:unread-count", count);
        watcher::update_tray_icon(&app_handle, count);
    }
    Ok(())
}

#[tauri::command]
fn delete_notifications_by_group(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    group_name: String,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::delete_notifications_by_group(&conn, &group_name).map_err(|e| e.to_string())?;
    if let Ok(count) = db::get_unread_count(&conn) {
        let _ = app_handle.emit("notifications:unread-count", count);
        watcher::update_tray_icon(&app_handle, count);
    }
    Ok(())
}

#[tauri::command]
fn get_mute_state(state: tauri::State<'_, Mutex<MuteState>>) -> Result<MuteStatePayload, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state.to_payload())
}

#[tauri::command]
fn toggle_global_mute(app_handle: tauri::AppHandle) -> Result<MuteStatePayload, String> {
    do_toggle_global_mute(&app_handle)
}

#[tauri::command]
fn toggle_group_mute(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<MuteState>>,
    group_name: String,
) -> Result<MuteStatePayload, String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    if state.muted_groups.contains(&group_name) {
        state.muted_groups.remove(&group_name);
    } else {
        state.muted_groups.insert(group_name);
    }
    let payload = state.to_payload();
    let _ = app_handle.emit("mute:changed", &payload);
    Ok(payload)
}

#[tauri::command]
fn get_toast_duration(state: tauri::State<'_, Mutex<AppState>>) -> Result<u64, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state.config.toast.duration_ms)
}

#[tauri::command]
fn get_toast_persistent(state: tauri::State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state.config.toast.persistent)
}

#[tauri::command]
fn delete_all_notifications(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::delete_all_notifications(&conn).map_err(|e| e.to_string())?;
    let _ = app_handle.emit("notifications:unread-count", 0i64);
    watcher::update_tray_icon(&app_handle, 0);
    Ok(())
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![
            init_panel,
            hide_panel,
            hide_toast,
            show_panel,
            focus_terminal,
            get_notifications,
            get_notifications_grouped,
            get_unread_count,
            delete_notification,
            delete_notifications_by_group_tmux,
            delete_notifications_by_group,
            delete_all_notifications,
            get_toast_duration,
            get_toast_persistent,
            get_mute_state,
            toggle_global_mute,
            toggle_group_mute,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            {
                app_nap::disable_app_nap();
                webkit_config::disable_webview_suspension(app.handle());
            }

            let db_path = config::db_path();
            let app_config = config::load_config();
            log::info!("DB path: {:?}", db_path);
            log::info!("Config: {:?}", app_config);

            // Ensure DB is initialized
            let _ = db::open(&db_path).expect("Failed to initialize database");

            app.manage(Mutex::new(AppState {
                db_path: db_path.clone(),
                config: app_config,
            }));

            app.manage(Mutex::new(MuteState::default()));

            tray::create(app.handle())?;

            // Initialize toast panel
            toast::init(app.handle())?;

            // Start DB watcher
            watcher::start(app.handle().clone(), db_path);

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, _| {});
}
