#[cfg(target_os = "macos")]
mod app_nap;
#[cfg(target_os = "macos")]
mod macos_hotkeys;
#[cfg(target_os = "macos")]
mod native_toast;
mod panel;
#[cfg(target_os = "macos")]
mod sessions;
#[cfg(target_os = "macos")]
mod terminal;
#[cfg(target_os = "macos")]
mod tmux_hooks;
mod tray;
mod watcher;

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
#[cfg(target_os = "macos")]
use std::time::Duration;

use agentoast_shared::config::{self, AppConfig};
use agentoast_shared::db;
use agentoast_shared::models::{Notification, TmuxPaneGroup};
use serde::{Deserialize, Serialize};
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
            // Reaching here means the tmux server is alive and responsive —
            // the right moment to attempt one-shot hook registration (retries
            // on every refresh until success, idempotent thereafter).
            tmux_hooks::install();
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

/// Currently-registered global shortcut for toggling the main panel. Kept
/// separate from `AppState.config.keybinding.toggle_panel` (which is the
/// string form) so we can unregister the exact `Shortcut` value on changes.
static CURRENT_TOGGLE_SHORTCUT: OnceLock<Mutex<Option<tauri_plugin_global_shortcut::Shortcut>>> =
    OnceLock::new();

fn current_toggle_shortcut_slot() -> &'static Mutex<Option<tauri_plugin_global_shortcut::Shortcut>>
{
    CURRENT_TOGGLE_SHORTCUT.get_or_init(|| Mutex::new(None))
}

/// Unregister the currently-bound toggle-panel shortcut (if any) and register
/// the one parsed from `shortcut_str`. An empty string disables the shortcut.
///
/// The invariant `slot == actually-registered-shortcut` is preserved across
/// every failure path:
/// - Parse failure: slot untouched, old registration stays.
/// - Unregister failure: slot untouched, new registration not attempted.
/// - Register failure: old is re-registered (rollback). If rollback also
///   fails, slot is cleared to `None` so it honestly reflects the now-empty
///   registration, and the caller still gets the original error.
pub fn apply_toggle_panel_shortcut(
    app_handle: &tauri::AppHandle,
    shortcut_str: &str,
) -> Result<(), String> {
    let slot = current_toggle_shortcut_slot();

    // 1. Parse up front; bail out without touching anything on parse failure.
    let new_shortcut: Option<tauri_plugin_global_shortcut::Shortcut> = if shortcut_str.is_empty() {
        None
    } else {
        Some(
            shortcut_str
                .parse()
                .map_err(|e| format!("Invalid shortcut '{}': {}", shortcut_str, e))?,
        )
    };

    // 2. Snapshot the currently-registered shortcut for potential rollback.
    let old_shortcut: Option<tauri_plugin_global_shortcut::Shortcut> = {
        let guard = slot.lock().map_err(|e| e.to_string())?;
        *guard
    };

    // 3. Unregister the old shortcut. On failure, leave everything as-is so the
    // old binding (which presumably still works) continues to function.
    if let Some(prev) = old_shortcut {
        app_handle
            .global_shortcut()
            .unregister(prev)
            .map_err(|e| format!("Failed to unregister previous shortcut: {}", e))?;
    }

    // 4. Clear the slot now that nothing is registered. If we return early
    // without registering a new shortcut, the slot correctly reflects "empty".
    if let Ok(mut guard) = slot.lock() {
        *guard = None;
    }

    // 5. Register the new shortcut (when non-empty).
    if let Some(shortcut) = new_shortcut {
        if let Err(e) = app_handle.global_shortcut().register(shortcut) {
            // 6. Rollback: try to re-register the old one so the user isn't
            // left without a working toggle binding.
            if let Some(prev) = old_shortcut {
                match app_handle.global_shortcut().register(prev) {
                    Ok(()) => {
                        if let Ok(mut guard) = slot.lock() {
                            *guard = Some(prev);
                        }
                        log::warn!(
                            "Failed to register '{}'; rolled back to previous shortcut: {}",
                            shortcut_str,
                            e
                        );
                    }
                    Err(e2) => {
                        log::error!(
                            "Failed to register '{}' ({}) and rollback also failed ({}); toggle panel shortcut is now unbound",
                            shortcut_str,
                            e,
                            e2
                        );
                        // slot is already None, which matches reality.
                    }
                }
            }
            return Err(format!(
                "Failed to register shortcut '{}': {}",
                shortcut_str, e
            ));
        }

        if let Ok(mut guard) = slot.lock() {
            *guard = Some(shortcut);
        }
        log::info!("Global shortcut registered: {}", shortcut_str);
    } else {
        log::info!("Global shortcut cleared (empty string)");
    }

    Ok(())
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
            refresh_and_emit(&app_handle);
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub toast_duration_ms: u64,
    pub toast_persistent: bool,
    pub toggle_panel_shortcut: String,
    pub editor: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsResult {
    pub restart_required: bool,
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, Mutex<AppState>>) -> Result<SettingsPayload, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(SettingsPayload {
        toast_duration_ms: state.config.toast.duration_ms,
        toast_persistent: state.config.toast.persistent,
        toggle_panel_shortcut: state.config.keybinding.toggle_panel.clone(),
        editor: state.config.editor.clone().unwrap_or_default(),
    })
}

/// Minimum toast display duration. Below this, a non-persistent toast is
/// dismissed before the user can notice the notification at all.
const MIN_TOAST_DURATION_MS: u64 = 500;

#[tauri::command]
fn save_settings(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    payload: SettingsPayload,
) -> Result<SaveSettingsResult, String> {
    // Input validation — reject nonsensical values before touching disk or
    // runtime registrations. Mirrors the frontend onBlur clamp.
    if payload.toast_duration_ms < MIN_TOAST_DURATION_MS {
        return Err(format!(
            "Toast duration must be at least {}ms",
            MIN_TOAST_DURATION_MS
        ));
    }

    if let Err(e) = config::ensure_config_file() {
        log::warn!("Failed to ensure config.toml exists: {}", e);
    }

    // --- Phase 0: snapshot the current on-disk / in-memory state. ---
    //
    // We persist each changed key individually, so a failure partway through
    // could leave config.toml in a mix of old and new values. By capturing a
    // snapshot up front we can deterministically restore it on failure.
    let snapshot = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        (
            guard.config.toast.duration_ms,
            guard.config.toast.persistent,
            guard.config.keybinding.toggle_panel.clone(),
            guard.config.editor.clone().unwrap_or_default(),
        )
    };
    let (old_toast_duration_ms, old_toast_persistent, old_shortcut_str, old_editor) = snapshot;

    let shortcut_changed = old_shortcut_str != payload.toggle_panel_shortcut;

    // --- Phase 1: try to apply the runtime shortcut change up front. ---
    //
    // If this fails, nothing has been written yet, so we simply propagate the
    // error. apply_toggle_panel_shortcut itself maintains the invariant
    // slot == actually-registered on every failure path.
    if shortcut_changed {
        apply_toggle_panel_shortcut(&app_handle, &payload.toggle_panel_shortcut)?;
    }

    // --- Phase 2: commit the changes to config.toml. On any write failure we
    // roll back everything — both the toml itself and the runtime shortcut
    // binding — so the user never ends up in a split-state. ---
    let write_result = (|| -> Result<(), String> {
        if old_toast_duration_ms != payload.toast_duration_ms {
            config::save_toast_duration_ms(payload.toast_duration_ms).map_err(|e| e.to_string())?;
        }
        if old_toast_persistent != payload.toast_persistent {
            config::save_toast_persistent(payload.toast_persistent).map_err(|e| e.to_string())?;
        }
        if shortcut_changed {
            config::save_keybinding_toggle_panel(&payload.toggle_panel_shortcut)
                .map_err(|e| e.to_string())?;
        }
        if old_editor != payload.editor {
            config::save_editor(&payload.editor).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();

    if let Err(commit_err) = write_result {
        // Roll back any keys we may have already written using the snapshot.
        if let Err(e) = config::save_toast_duration_ms(old_toast_duration_ms) {
            log::error!(
                "Rollback: failed to restore toast.duration_ms={}: {}",
                old_toast_duration_ms,
                e
            );
        }
        if let Err(e) = config::save_toast_persistent(old_toast_persistent) {
            log::error!(
                "Rollback: failed to restore toast.persistent={}: {}",
                old_toast_persistent,
                e
            );
        }
        if shortcut_changed {
            if let Err(e) = config::save_keybinding_toggle_panel(&old_shortcut_str) {
                log::error!(
                    "Rollback: failed to restore keybinding.toggle_panel='{}': {}",
                    old_shortcut_str,
                    e
                );
            }
            if let Err(e) = apply_toggle_panel_shortcut(&app_handle, &old_shortcut_str) {
                log::error!(
                    "Rollback: failed to re-bind previous shortcut '{}': {}",
                    old_shortcut_str,
                    e
                );
            }
        }
        if let Err(e) = config::save_editor(&old_editor) {
            log::error!("Rollback: failed to restore editor='{}': {}", old_editor, e);
        }
        return Err(commit_err);
    }

    // --- Phase 3: commit to in-memory AppState now that persistence succeeded. ---
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.config.toast.duration_ms = payload.toast_duration_ms;
        guard.config.toast.persistent = payload.toast_persistent;
        guard.config.keybinding.toggle_panel = payload.toggle_panel_shortcut.clone();
        guard.config.editor = if payload.editor.is_empty() {
            None
        } else {
            Some(payload.editor.clone())
        };
    }

    // --- Phase 4: runtime-push for settings that have live consumers. ---
    #[cfg(target_os = "macos")]
    native_toast::update_settings(payload.toast_duration_ms, payload.toast_persistent);

    Ok(SaveSettingsResult {
        restart_required: false,
    })
}

#[tauri::command]
fn show_settings(app_handle: tauri::AppHandle) -> Result<(), String> {
    let window = app_handle
        .get_webview_window("settings")
        .ok_or_else(|| "Settings window not found".to_string())?;
    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn hide_settings(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("settings") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn restart_app(app_handle: tauri::AppHandle) {
    app_handle.restart();
}

#[tauri::command]
fn get_reserved_shortcuts() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        macos_hotkeys::reserved_shortcuts()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
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
            get_settings,
            save_settings,
            show_settings,
            hide_settings,
            restart_app,
            get_reserved_shortcuts,
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

            // Register the global-shortcut plugin unconditionally so we can
            // register/unregister on the fly when the user edits Settings.
            // The handler always routes to `toggle_panel` because we only ever
            // have a single toggle-panel shortcut bound at a time.
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(|app, _sc, event| {
                        if event.state == ShortcutState::Pressed {
                            tray::toggle_panel(app);
                        }
                    })
                    .build(),
            )?;

            if let Err(e) = apply_toggle_panel_shortcut(app.handle(), &shortcut_str) {
                log::warn!("Initial shortcut setup failed: {}", e);
            }

            // Register updater plugin
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            // Hide the settings window on OS close requests (red X) so that
            // subsequent "Settings…" clicks can reuse the same WebviewWindow
            // instead of having to recreate it.
            if let Some(settings_window) = app.handle().get_webview_window("settings") {
                let window_clone = settings_window.clone();
                settings_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }

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
