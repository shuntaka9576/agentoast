#[cfg(target_os = "macos")]
mod app_nap;
#[cfg(target_os = "macos")]
mod apps;
#[cfg(target_os = "macos")]
mod macos_hotkeys;
#[cfg(target_os = "macos")]
mod native_toast;
mod panel;
#[cfg(target_os = "macos")]
mod screen;
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

use agentoast_shared::config::{self, AllowedApp, AppConfig, ToastDisplay, ToastPosition};
use agentoast_shared::db;
use agentoast_shared::models::{Notification, TmuxPaneGroup};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

const README_BASE: &str = "https://github.com/shuntaka9576/agentoast";

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

/// Per-pane "did this body just change?" tracker consumed by the agent
/// status detector. Owns nothing else; lives in its own Tauri-managed state
/// so the hot path (`list_tmux_panes_grouped`) can lock it independently of
/// `SessionsCache`, whose lock is taken later to publish the rendered
/// groups.
#[cfg(target_os = "macos")]
pub type PaneHysteresisState = Mutex<sessions::hysteresis::PaneHysteresis>;

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
    let db_conn = db::open_reader(&config::db_path()).ok();
    refresh_and_emit_with_conn(app_handle, &db_conn);
}

#[cfg(target_os = "macos")]
fn refresh_and_emit_with_conn(app_handle: &tauri::AppHandle, db_conn: &Option<db::Connection>) {
    let show_non_agent = read_show_non_agent_panes(app_handle);
    let hysteresis_state = app_handle.try_state::<PaneHysteresisState>();
    let hysteresis = hysteresis_state.as_ref().map(|s| s.inner());
    match sessions::list_tmux_panes_grouped(show_non_agent, db_conn, hysteresis) {
        Ok(groups) => {
            // Reaching here means the tmux server is alive and responsive —
            // the right moment to attempt one-shot hook registration (retries
            // on every refresh until success, idempotent thereafter).
            tmux_hooks::install();
            // The 2s poller mostly re-derives an identical snapshot; skip the
            // serde + IPC of `sessions:updated` when nothing changed. The
            // frontend already drops equal payloads (shallowEqualGroups).
            let mut changed = true;
            if let Some(cache) = app_handle.try_state::<Mutex<SessionsCache>>() {
                if let Ok(mut guard) = cache.lock() {
                    changed = guard.groups.as_ref() != Some(&groups);
                    if changed {
                        guard.groups = Some(groups.clone());
                    }
                }
            }
            if changed {
                let _ = app_handle.emit("sessions:updated", &groups);
            }
        }
        Err(e) => {
            log::debug!("sessions refresh: list_tmux_panes_grouped failed: {}", e);
        }
    }
}

/// Fixed-interval poller that refreshes the session list by spawning
/// `tmux list-panes` + a single batched `capture-pane` invocation. We used to
/// maintain a long-lived `tmux -C` control client, but tmux 3.6a can wedge
/// its control-mode subsystem in a way only `kill-server` recovers from, so
/// Agentoast now sticks to plain commands.
#[cfg(target_os = "macos")]
fn start_sessions_safety_poller(app_handle: tauri::AppHandle, interval: Duration) {
    std::thread::spawn(move || {
        let db_path = config::db_path();
        // One reader connection for the thread's lifetime (WAL allows
        // concurrent readers/writer); reopened on the next cycle if the
        // first open failed (e.g. DB not created yet).
        let mut db_conn: Option<db::Connection> = None;
        loop {
            std::thread::sleep(interval);
            if db_conn.is_none() {
                db_conn = db::open_reader(&db_path).ok();
            }
            refresh_and_emit_with_conn(&app_handle, &db_conn);
        }
    });
}

/// Register / unregister the running app as a macOS Login Item via the
/// `SMAppService.mainApp` API (macOS 13+). Unlike the older
/// `osascript`-based approaches, this does NOT trigger the Automation /
/// Apple Events TCC consent prompt because the app is registering itself,
/// not controlling another process.
#[cfg(target_os = "macos")]
fn autostart_main_service() -> objc2::rc::Retained<objc2_service_management::SMAppService> {
    unsafe { objc2_service_management::SMAppService::mainAppService() }
}

#[cfg(target_os = "macos")]
fn autostart_is_enabled() -> bool {
    use objc2_service_management::SMAppServiceStatus;
    let service = autostart_main_service();
    let status = unsafe { service.status() };
    status == SMAppServiceStatus::Enabled
}

#[cfg(target_os = "macos")]
fn autostart_enable() -> Result<(), String> {
    let service = autostart_main_service();
    unsafe { service.registerAndReturnError() }
        .map_err(|e| format!("SMAppService.register failed: {}", e.localizedDescription()))
}

#[cfg(target_os = "macos")]
fn autostart_disable() -> Result<(), String> {
    let service = autostart_main_service();
    unsafe { service.unregisterAndReturnError() }.map_err(|e| {
        format!(
            "SMAppService.unregister failed: {}",
            e.localizedDescription()
        )
    })
}

#[cfg(not(target_os = "macos"))]
fn autostart_is_enabled() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
fn autostart_enable() -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn autostart_disable() -> Result<(), String> {
    Ok(())
}

/// Clean up the legacy `~/Library/LaunchAgents/Agentoast.plist` left by the
/// old `tauri-plugin-autostart` LaunchAgent mode and, if it existed, re-arm
/// autostart through SMAppService so the user's preference survives.
///
/// We deliberately do NOT auto-clean the buggy "agentoast" login item left
/// by the intermediate AppleScript-mode build: removing it would require
/// `osascript`, which would re-trigger the very TCC prompt this migration
/// is trying to avoid. Affected testers (handful of internal users) are
/// asked to remove that orphan entry manually from System Settings.
#[cfg(target_os = "macos")]
fn migrate_legacy_autostart() {
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let plist_path = std::path::PathBuf::from(home).join("Library/LaunchAgents/Agentoast.plist");
    if !plist_path.exists() {
        return;
    }

    log::info!(
        "Removing legacy LaunchAgent plist: {}",
        plist_path.display()
    );
    if let Err(e) = std::process::Command::new("launchctl")
        .arg("unload")
        .arg(&plist_path)
        .output()
    {
        log::warn!("launchctl unload failed (continuing): {}", e);
    }
    if let Err(e) = std::fs::remove_file(&plist_path) {
        log::warn!("Failed to remove {}: {}", plist_path.display(), e);
        return;
    }

    match autostart_enable() {
        Ok(_) => log::info!("Re-registered autostart via SMAppService"),
        Err(e) => log::warn!("Failed to re-enable autostart after migration: {}", e),
    }
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

/// Switch the macOS Dock presence on the fly. Used to bring agentoast forward
/// during onboarding (so the user can find it via Dock / Cmd+Tab) and then
/// hide it again once onboarding is complete to keep the menu-bar-only feel.
#[cfg(target_os = "macos")]
fn set_dock_visible(app_handle: &tauri::AppHandle, visible: bool) {
    let policy = if visible {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    };
    if let Err(e) = app_handle.set_activation_policy(policy) {
        log::warn!("Failed to set activation policy: {}", e);
    }
}

pub fn do_toggle_global_mute(app_handle: &tauri::AppHandle) -> Result<MuteStatePayload, String> {
    let mute_state = app_handle.state::<Mutex<MuteState>>();
    let mut state = mute_state.lock().map_err(|e| e.to_string())?;
    state.global_muted = !state.global_muted;
    let payload = state.to_payload();
    let _ = app_handle.emit("mute:changed", &payload);
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
        // Re-anchor at the tray icon every show so the panel never opens at
        // its last position (e.g. screen center on a fresh launch).
        panel.set_alpha_value(0.0);
        panel.show_and_make_key();
        panel::position_panel_appkit(&app_handle);
        panel.set_alpha_value(1.0);
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
        let hysteresis_handle = app_handle.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let db_conn = db::open_reader(&config::db_path()).ok();
            let hysteresis_state = hysteresis_handle.try_state::<PaneHysteresisState>();
            let hysteresis = hysteresis_state.as_ref().map(|s| s.inner());
            sessions::list_tmux_panes_grouped(show_non_agent, &db_conn, hysteresis)
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
async fn get_focused_pane() -> Result<Option<agentoast_shared::models::TmuxPane>, String> {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(sessions::find_focused_pane)
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("get_focused_pane is only supported on macOS".to_string())
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
    pub toast_positions: Vec<ToastPosition>,
    pub toast_display: ToastDisplay,
    pub toggle_panel_shortcut: String,
    pub editor: String,
    pub autostart_enabled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsResult {
    pub restart_required: bool,
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, Mutex<AppState>>) -> Result<SettingsPayload, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let autostart_enabled = autostart_is_enabled();
    Ok(SettingsPayload {
        toast_duration_ms: state.config.toast.duration_ms,
        toast_persistent: state.config.toast.persistent,
        toast_positions: state.config.toast.positions.clone(),
        toast_display: state.config.toast.display,
        toggle_panel_shortcut: state.config.keybinding.toggle_panel.clone(),
        editor: state.config.editor.clone().unwrap_or_default(),
        autostart_enabled,
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
            guard.config.toast.positions.clone(),
            guard.config.toast.display,
            guard.config.keybinding.toggle_panel.clone(),
            guard.config.editor.clone().unwrap_or_default(),
        )
    };
    let (
        old_toast_duration_ms,
        old_toast_persistent,
        old_toast_positions,
        old_toast_display,
        old_shortcut_str,
        old_editor,
    ) = snapshot;

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
        if old_toast_positions != payload.toast_positions {
            config::save_toast_positions(&payload.toast_positions).map_err(|e| e.to_string())?;
        }
        if old_toast_display != payload.toast_display {
            config::save_toast_display(payload.toast_display).map_err(|e| e.to_string())?;
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
        if let Err(e) = config::save_toast_positions(&old_toast_positions) {
            log::error!(
                "Rollback: failed to restore toast.positions={:?}: {}",
                old_toast_positions,
                e
            );
        }
        if let Err(e) = config::save_toast_display(old_toast_display) {
            log::error!(
                "Rollback: failed to restore toast.display={:?}: {}",
                old_toast_display,
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
        guard.config.toast.positions = payload.toast_positions.clone();
        guard.config.toast.display = payload.toast_display;
        guard.config.keybinding.toggle_panel = payload.toggle_panel_shortcut.clone();
        guard.config.editor = if payload.editor.is_empty() {
            None
        } else {
            Some(payload.editor.clone())
        };
    }

    // --- Phase 4: runtime-push for settings that have live consumers. ---
    #[cfg(target_os = "macos")]
    native_toast::update_settings(
        payload.toast_duration_ms,
        payload.toast_persistent,
        payload.toast_positions.clone(),
        payload.toast_display,
    );

    // --- Phase 5: autostart (System Events login item). Not mirrored in
    // config.toml, so a failure here is surfaced but does not need a toml
    // rollback. ---
    let old_autostart = autostart_is_enabled();
    if old_autostart != payload.autostart_enabled {
        let result = if payload.autostart_enabled {
            autostart_enable()
        } else {
            autostart_disable()
        };
        if let Err(e) = result {
            return Err(format!("Failed to update autostart: {}", e));
        }
    }

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
fn open_hook_readme(app_handle: tauri::AppHandle, agent: String) -> Result<(), String> {
    let anchor = match agent.as_str() {
        "claude-code" => "#claude-code",
        "codex" => "#codex",
        "copilot-cli" => "#copilot-cli",
        "opencode" => "#opencode",
        other => return Err(format!("unknown agent: {}", other)),
    };
    app_handle
        .opener()
        .open_url(format!("{}{}", README_BASE, anchor), None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn open_login_items_settings(app_handle: tauri::AppHandle) -> Result<(), String> {
    app_handle
        .opener()
        .open_url(
            "x-apple.systempreferences:com.apple.LoginItems-Settings.extension",
            None::<&str>,
        )
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn open_login_items_settings(_app_handle: tauri::AppHandle) -> Result<(), String> {
    Err("Opening Login Items settings is only supported on macOS".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CliInstallStatus {
    installed: bool,
    points_to_current_exe: bool,
    on_path: bool,
    target_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CliInstallResult {
    target_path: String,
    on_path: bool,
    replaced_existing: bool,
}

fn cli_symlink_target() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    Ok(std::path::PathBuf::from(home).join(".local/bin/agentoast"))
}

fn local_bin_on_path() -> bool {
    let Ok(home) = std::env::var("HOME") else {
        return false;
    };
    let local_bin = std::path::PathBuf::from(&home).join(".local/bin");
    let canonical_local_bin = std::fs::canonicalize(&local_bin).ok();
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':').filter(|s| !s.is_empty()).any(|entry| {
        let expanded = if let Some(stripped) = entry.strip_prefix("~/") {
            std::path::PathBuf::from(&home).join(stripped)
        } else if entry == "~" {
            std::path::PathBuf::from(&home)
        } else {
            std::path::PathBuf::from(entry)
        };
        if expanded == local_bin {
            return true;
        }
        match (
            std::fs::canonicalize(&expanded).ok(),
            canonical_local_bin.as_ref(),
        ) {
            (Some(a), Some(b)) => &a == b,
            _ => false,
        }
    })
}

fn read_symlink_target(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let target = std::fs::read_link(path).ok()?;
    if target.is_absolute() {
        Some(target)
    } else {
        path.parent().map(|p| p.join(&target))
    }
}

#[tauri::command]
fn get_cli_install_status() -> Result<CliInstallStatus, String> {
    let target = cli_symlink_target()?;
    let target_str = target.to_string_lossy().to_string();
    let symlink_meta = target.symlink_metadata();
    let installed = symlink_meta.is_ok();

    let points_to_current_exe = match (
        symlink_meta
            .as_ref()
            .ok()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        std::env::current_exe().ok(),
    ) {
        (true, Some(current_exe)) => match (
            read_symlink_target(&target).and_then(|p| std::fs::canonicalize(p).ok()),
            std::fs::canonicalize(&current_exe).ok(),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        },
        _ => false,
    };

    Ok(CliInstallStatus {
        installed,
        points_to_current_exe,
        on_path: local_bin_on_path(),
        target_path: target_str,
    })
}

#[tauri::command]
fn install_cli_symlink() -> Result<CliInstallResult, String> {
    #[cfg(not(unix))]
    {
        return Err("CLI symlink installation is only supported on Unix platforms".to_string());
    }
    #[cfg(unix)]
    {
        let current_exe = std::env::current_exe().map_err(|e| format!("current_exe: {}", e))?;
        let target = cli_symlink_target()?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create_dir_all {}: {}", parent.display(), e))?;
        }

        let mut replaced_existing = false;
        if let Ok(meta) = target.symlink_metadata() {
            if meta.file_type().is_symlink() {
                std::fs::remove_file(&target)
                    .map_err(|e| format!("remove existing symlink: {}", e))?;
                replaced_existing = true;
            } else {
                return Err(format!(
                    "{} already exists and is not a symlink. Remove it manually before retrying.",
                    target.display()
                ));
            }
        }

        std::os::unix::fs::symlink(&current_exe, &target)
            .map_err(|e| format!("create symlink: {}", e))?;

        Ok(CliInstallResult {
            target_path: target.to_string_lossy().to_string(),
            on_path: local_bin_on_path(),
            replaced_existing,
        })
    }
}

#[tauri::command]
fn complete_onboarding(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Err(e) = config::mark_onboarded() {
        log::warn!("Failed to write onboarded marker: {}", e);
    }
    if let Some(window) = app_handle.get_webview_window("onboarding") {
        let _ = window.hide();
    }
    #[cfg(target_os = "macos")]
    set_dock_visible(&app_handle, false);
    Ok(())
}

#[tauri::command]
async fn list_running_apps() -> Result<Vec<apps::RunningApp>, String> {
    // Runs on a background blocking thread so the ~2s of NSWorkspace
    // enumeration + per-app TIFF→PNG icon encoding never freezes the UI
    // thread (no macOS beach-ball cursor while the dropdown is loading).
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(apps::list_running_apps)
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("list_running_apps is only supported on macOS".to_string())
    }
}

#[tauri::command]
fn get_apps_allowed_apps(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<Vec<AllowedApp>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state.config.apps.allowed_apps.clone())
}

#[tauri::command]
fn save_apps_allowed_apps(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    allowed_apps: Vec<AllowedApp>,
) -> Result<(), String> {
    {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        state.config.apps.allowed_apps = allowed_apps.clone();
    }
    if let Err(e) = config::save_apps_allowed_apps(&allowed_apps) {
        log::warn!("Failed to save apps.allowed_apps to config.toml: {}", e);
    }
    let _ = app_handle.emit("apps:allowed_apps_changed", &allowed_apps);
    Ok(())
}

#[tauri::command]
fn activate_app(bundle_id: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        apps::activate_app(&bundle_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle_id;
        Err("activate_app is only supported on macOS".to_string())
    }
}

#[tauri::command]
async fn resolve_app_icons(
    bundle_ids: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    // Same reasoning as `list_running_apps`: keep AppKit calls off the UI
    // thread. Even with a small allowlist, encoding icons to PNG can take
    // tens of milliseconds and we don't want that on the main thread.
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(move || apps::resolve_app_icons(&bundle_ids))
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle_ids;
        Err("resolve_app_icons is only supported on macOS".to_string())
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
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            init_panel,
            hide_panel,
            hide_toast,
            show_panel,
            focus_terminal,
            get_sessions,
            get_focused_pane,
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
            complete_onboarding,
            open_hook_readme,
            open_login_items_settings,
            get_cli_install_status,
            install_cli_symlink,
            list_running_apps,
            get_apps_allowed_apps,
            save_apps_allowed_apps,
            activate_app,
            resolve_app_icons,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                // Default to menu-bar-only (Accessory). If onboarding is still
                // pending we promote to Regular so the Welcome window shows up
                // in the Dock / Cmd+Tab; complete_onboarding flips it back.
                let policy = if config::is_onboarded() {
                    tauri::ActivationPolicy::Accessory
                } else {
                    tauri::ActivationPolicy::Regular
                };
                app.set_activation_policy(policy);
            }

            #[cfg(target_os = "macos")]
            {
                app_nap::disable_app_nap();
            }

            #[cfg(target_os = "macos")]
            migrate_legacy_autostart();

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
            #[cfg(target_os = "macos")]
            app.manage::<PaneHysteresisState>(Mutex::new(
                sessions::hysteresis::PaneHysteresis::default(),
            ));

            app.manage(Mutex::new(MuteState {
                global_muted: initial_muted,
                muted_repos: HashSet::new(),
            }));

            tray::create(app.handle())?;

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

            // Show the onboarding window on first launch. Treat the close button
            // as a "complete onboarding" gesture so users aren't shown the flow
            // again next launch.
            if let Some(onboarding_window) = app.handle().get_webview_window("onboarding") {
                let window_clone = onboarding_window.clone();
                onboarding_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Err(e) = config::mark_onboarded() {
                            log::warn!("Failed to write onboarded marker on close: {}", e);
                        }
                        let _ = window_clone.hide();
                        #[cfg(target_os = "macos")]
                        set_dock_visible(window_clone.app_handle(), false);
                    }
                });

                if !config::is_onboarded() {
                    let _ = onboarding_window.show();
                    let _ = onboarding_window.set_focus();
                }
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
