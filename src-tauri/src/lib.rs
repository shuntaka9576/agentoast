#[cfg(target_os = "macos")]
mod app_nap;
#[cfg(target_os = "macos")]
mod native_toast;
mod panel;
#[cfg(target_os = "macos")]
mod sessions;
#[cfg(target_os = "macos")]
mod terminal;
mod tray;
mod watcher;

use std::collections::HashSet;
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::time::Duration;

use agentoast_shared::config::{self, AppConfig};
use agentoast_shared::db;
use agentoast_shared::models::{Notification, TmuxPaneGroup};
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[cfg(target_os = "macos")]
const FALLBACK_POLL_INTERVAL: Duration = Duration::from_secs(2);

pub struct AppState {
    pub db_path: std::path::PathBuf,
    pub config: AppConfig,
}

#[derive(Default)]
pub struct SessionsCache {
    pub groups: Option<Vec<TmuxPaneGroup>>,
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

pub fn emit_cached_sessions(app_handle: &tauri::AppHandle) {
    if let Some(cache) = app_handle.try_state::<Mutex<SessionsCache>>() {
        if let Ok(guard) = cache.lock() {
            if let Some(groups) = guard.groups.as_ref() {
                let _ = app_handle.emit("sessions:cached", groups);
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn read_show_non_agent_panes(app_handle: &tauri::AppHandle) -> bool {
    app_handle
        .try_state::<Mutex<AppState>>()
        .and_then(|s| {
            s.lock()
                .ok()
                .map(|g| g.config.notification.show_non_agent_panes)
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn refresh_and_emit(app_handle: &tauri::AppHandle) {
    let show_non_agent = read_show_non_agent_panes(app_handle);
    match sessions::list_tmux_panes_grouped(show_non_agent) {
        Ok(groups) => {
            if let Some(cache) = app_handle.try_state::<Mutex<SessionsCache>>() {
                if let Ok(mut guard) = cache.lock() {
                    guard.groups = Some(groups.clone());
                }
            }
            let _ = app_handle.emit("sessions:updated", &groups);
        }
        Err(e) => {
            log::debug!("sessions refresh: list_tmux_panes_grouped failed: {}", e);
        }
    }
}

/// Fixed-interval poller that refreshes the session list by spawning
/// `tmux list-panes` + per-pane `capture-pane`. We used to maintain a
/// long-lived `tmux -C` control client, but tmux 3.6a can wedge its
/// control-mode subsystem in a way only `kill-server` recovers from, so
/// Agentoast now sticks to plain commands.
#[cfg(target_os = "macos")]
fn start_sessions_safety_poller(app_handle: tauri::AppHandle, interval: Duration) {
    std::thread::spawn(move || loop {
        std::thread::sleep(interval);
        refresh_and_emit(&app_handle);
    });
}

pub fn do_toggle_global_mute(app_handle: &tauri::AppHandle) -> Result<MuteStatePayload, String> {
    let mute_state = app_handle.state::<Mutex<MuteState>>();
    let mut state = mute_state.lock().map_err(|e| e.to_string())?;
    state.global_muted = !state.global_muted;
    let payload = state.to_payload();
    let _ = app_handle.emit("mute:changed", &payload);
    tray::update_mute_menu(app_handle, payload.global_muted);
    if let Err(e) = config::save_notification_muted(payload.global_muted) {
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
fn hide_toast(_app_handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    native_toast::hide();
}

#[tauri::command]
fn show_panel(app_handle: tauri::AppHandle) {
    panel::init(&app_handle).ok();
    use tauri_nspanel::ManagerExt;
    if let Ok(panel) = app_handle.get_webview_panel("main") {
        let _ = app_handle.emit("panel:shown", ());
        emit_cached_sessions(&app_handle);
        let _ = app_handle.emit("notifications:refresh", ());
        panel.show();
    }
}

#[tauri::command]
fn focus_terminal(
    app_handle: tauri::AppHandle,
    tmux_pane: String,
    terminal_bundle_id: String,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Split out the tmux switch from the terminal activation so we can
        // react when the pane id is stale: if tmux errors, refresh sessions
        // immediately so the frontend's next keypress uses fresh ids instead
        // of waiting for the user to nudge tmux manually.
        let mut tmux_failed = false;
        if !tmux_pane.is_empty() {
            if let Err(e) = terminal::switch_tmux_pane(&tmux_pane) {
                log::warn!(
                    "focus_terminal: switch_tmux_pane failed pane={}: {}",
                    tmux_pane,
                    e
                );
                tmux_failed = true;
            }
        }

        if tmux_failed {
            let ctrl = app_handle
                .try_state::<sessions::TmuxCtrl>()
                .map(|s| s.inner().clone());
            refresh_and_emit(&app_handle, ctrl.as_ref());
        }

        if terminal_bundle_id.is_empty() {
            terminal::activate_any_terminal()
        } else {
            terminal::activate_terminal(&terminal_bundle_id)
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app_handle, tmux_pane, terminal_bundle_id);
        Err("focus_terminal is only supported on macOS".to_string())
    }
}

#[tauri::command]
async fn get_sessions(app_handle: tauri::AppHandle) -> Result<Vec<TmuxPaneGroup>, String> {
    #[cfg(target_os = "macos")]
    {
        let show_non_agent = read_show_non_agent_panes(&app_handle);
        let result = tauri::async_runtime::spawn_blocking(move || {
            sessions::list_tmux_panes_grouped(show_non_agent)
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Ok(ref groups) = result {
            if let Some(cache) = app_handle.try_state::<Mutex<SessionsCache>>() {
                if let Ok(mut guard) = cache.lock() {
                    guard.groups = Some(groups.clone());
                }
            }
            let _ = app_handle.emit("sessions:updated", groups);
        }
        result
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_handle;
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

fn emit_after_delete(app_handle: &tauri::AppHandle, count: i64) {
    let _ = app_handle.emit("notifications:unread-count", count);
    watcher::update_tray_icon(app_handle, count);
    let _ = app_handle.emit("notifications:refresh", ());
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
    emit_after_delete(&app_handle, db::get_unread_count(&conn).unwrap_or(0));
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
    emit_after_delete(&app_handle, db::get_unread_count(&conn).unwrap_or(0));
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
    emit_after_delete(&app_handle, db::get_unread_count(&conn).unwrap_or(0));
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
    Ok(state.config.notification.filter_notified_only)
}

#[tauri::command]
fn save_filter_notified_only(
    state: tauri::State<'_, Mutex<AppState>>,
    value: bool,
) -> Result<(), String> {
    {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        state.config.notification.filter_notified_only = value;
    }
    if let Err(e) = config::save_notification_filter_notified_only(value) {
        log::warn!("Failed to save filter_notified_only to config.toml: {}", e);
    }
    Ok(())
}

#[tauri::command]
fn get_show_non_agent_panes(state: tauri::State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state.config.notification.show_non_agent_panes)
}

#[tauri::command]
fn save_show_non_agent_panes(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    value: bool,
) -> Result<(), String> {
    {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        state.config.notification.show_non_agent_panes = value;
    }
    if let Err(e) = config::save_notification_show_non_agent_panes(value) {
        log::warn!("Failed to save show_non_agent_panes to config.toml: {}", e);
    }
    #[cfg(target_os = "macos")]
    {
        refresh_and_emit(&app_handle);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_handle;
    }
    Ok(())
}

#[tauri::command]
fn get_update_enabled(state: tauri::State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state.config.update.enabled)
}

#[tauri::command]
fn delete_all_notifications(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = db::open_reader(&state.db_path).map_err(|e| e.to_string())?;
    db::delete_all_notifications(&conn).map_err(|e| e.to_string())?;
    emit_after_delete(&app_handle, 0);
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

    let sessions_level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

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
        .level_for("agentoast_app_lib::sessions", sessions_level)
        .chain(std::io::stderr())
        .chain(file_logger)
        .apply()
        .expect("Failed to initialize logger");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_process::init())
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
            get_update_enabled,
            get_filter_notified_only,
            save_filter_notified_only,
            get_show_non_agent_panes,
            save_show_non_agent_panes,
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
            }

            let db_path = config::db_path();
            let app_config = config::load_config();
            log::info!("DB path: {:?}", db_path);
            log::info!("Config: {:?}", app_config);

            let shortcut_str = app_config.keybinding.toggle_panel.clone();
            let initial_muted = app_config.notification.muted;

            // Ensure DB is initialized
            let _ = db::open(&db_path).expect("Failed to initialize database");

            app.manage(Mutex::new(AppState {
                db_path: db_path.clone(),
                config: app_config,
            }));

            app.manage(Mutex::new(SessionsCache { groups: None }));

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

            // Register updater plugin
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            // Initialize native toast panel
            #[cfg(target_os = "macos")]
            if let Err(e) = native_toast::init(app.handle()) {
                log::error!("Failed to init native toast: {}", e);
            }

            // Start DB watcher
            watcher::start(app.handle().clone(), db_path);

            #[cfg(target_os = "macos")]
            start_sessions_safety_poller(app.handle().clone(), FALLBACK_POLL_INTERVAL);

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, _| {});
}
