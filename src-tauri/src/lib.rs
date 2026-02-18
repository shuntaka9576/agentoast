#[cfg(target_os = "macos")]
mod app_nap;
mod panel;
#[cfg(target_os = "macos")]
mod sessions;
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
use agentoast_shared::models::{Notification, TmuxPaneGroup};
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub struct AppState {
    pub db_path: std::path::PathBuf,
    pub config: AppConfig,
}

#[derive(Default)]
pub struct MuteState {
    pub global_muted: bool,
    pub muted_repos: HashSet<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MuteStatePayload {
    pub global_muted: bool,
    pub muted_repos: Vec<String>,
}

impl MuteState {
    pub fn to_payload(&self) -> MuteStatePayload {
        let mut repos: Vec<String> = self.muted_repos.iter().cloned().collect();
        repos.sort();
        MuteStatePayload {
            global_muted: self.global_muted,
            muted_repos: repos,
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
    if let Err(e) = config::save_panel_muted(payload.global_muted) {
        log::warn!("Failed to save mute state to config.toml: {}", e);
    }
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
fn focus_terminal(tmux_pane: String, terminal_bundle_id: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        terminal::focus_terminal(&tmux_pane, &terminal_bundle_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (tmux_pane, terminal_bundle_id);
        Err("focus_terminal is only supported on macOS".to_string())
    }
}

#[tauri::command]
fn get_sessions() -> Result<Vec<TmuxPaneGroup>, String> {
    #[cfg(target_os = "macos")]
    {
        sessions::list_tmux_panes_grouped()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Sessions are only supported on macOS".to_string())
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
fn delete_notifications_by_pane(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    tmux_pane: String,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::delete_notifications_by_pane(&conn, &tmux_pane).map_err(|e| e.to_string())?;
    if let Ok(count) = db::get_unread_count(&conn) {
        let _ = app_handle.emit("notifications:unread-count", count);
        watcher::update_tray_icon(&app_handle, count);
    }
    Ok(())
}

#[tauri::command]
fn delete_notifications_by_panes(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    pane_ids: Vec<String>,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::delete_notifications_by_panes(&conn, &pane_ids).map_err(|e| e.to_string())?;
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
fn toggle_repo_mute(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<MuteState>>,
    repo_path: String,
) -> Result<MuteStatePayload, String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    if state.muted_repos.contains(&repo_path) {
        state.muted_repos.remove(&repo_path);
    } else {
        state.muted_repos.insert(repo_path);
    }
    let payload = state.to_payload();
    let _ = app_handle.emit("mute:changed", &payload);
    Ok(payload)
}

#[tauri::command]
fn get_filter_notified_only(state: tauri::State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state.config.panel.filter_notified_only)
}

#[tauri::command]
fn save_filter_notified_only(
    state: tauri::State<'_, Mutex<AppState>>,
    value: bool,
) -> Result<(), String> {
    {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        state.config.panel.filter_notified_only = value;
    }
    if let Err(e) = config::save_panel_filter_notified_only(value) {
        log::warn!("Failed to save filter_notified_only to config.toml: {}", e);
    }
    Ok(())
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
    let log_dir = config::data_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("agentoast.log");

    // Rotate log file if it exceeds 5 MB
    const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024;
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() > MAX_LOG_SIZE {
            let old_path = log_dir.join("agentoast.log.old");
            let _ = std::fs::rename(&log_path, &old_path);
        }
    }

    let file_logger = fern::log_file(&log_path).expect("Failed to create log file");

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}",
                humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .level_for("agentoast_app_lib::sessions", log::LevelFilter::Debug)
        .chain(std::io::stderr())
        .chain(file_logger)
        .apply()
        .expect("Failed to initialize logger");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![
            init_panel,
            hide_panel,
            hide_toast,
            show_panel,
            focus_terminal,
            get_sessions,
            get_notifications,
            get_unread_count,
            delete_notification,
            delete_notifications_by_pane,
            delete_notifications_by_panes,
            delete_all_notifications,
            get_toast_duration,
            get_toast_persistent,
            get_filter_notified_only,
            save_filter_notified_only,
            get_mute_state,
            toggle_global_mute,
            toggle_repo_mute,
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

            let shortcut_str = app_config.shortcut.toggle_panel.clone();
            let initial_muted = app_config.panel.muted;

            // Ensure DB is initialized
            let _ = db::open(&db_path).expect("Failed to initialize database");

            app.manage(Mutex::new(AppState {
                db_path: db_path.clone(),
                config: app_config,
            }));

            app.manage(Mutex::new(MuteState {
                global_muted: initial_muted,
                muted_repos: HashSet::new(),
            }));

            tray::create(app.handle())?;
            if initial_muted {
                tray::update_mute_menu(app.handle(), true);
            }

            // Register global shortcut for panel toggle
            if !shortcut_str.is_empty() {
                match shortcut_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    Ok(shortcut) => {
                        app.handle().plugin(
                            tauri_plugin_global_shortcut::Builder::new()
                                .with_handler(move |app, sc, event| {
                                    if event.state == ShortcutState::Pressed && sc == &shortcut {
                                        tray::toggle_panel(app);
                                    }
                                })
                                .build(),
                        )?;
                        if let Err(e) = app.global_shortcut().register(shortcut) {
                            log::error!(
                                "Failed to register global shortcut '{}': {}",
                                shortcut_str,
                                e
                            );
                        } else {
                            log::info!("Global shortcut registered: {}", shortcut_str);
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Invalid shortcut '{}' in config.toml: {}, shortcut disabled",
                            shortcut_str,
                            e
                        );
                    }
                }
            } else {
                log::info!("Global shortcut disabled (empty string in config)");
            }

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
