use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use agentoast_shared::db;
use agentoast_shared::db::Connection;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::image::Image;
use tauri::path::BaseDirectory;
use tauri::tray::TrayIconId;
use tauri::{AppHandle, Emitter, Manager};

use crate::native_toast;
use crate::MuteState;

static LAST_KNOWN_ID: AtomicI64 = AtomicI64::new(0);
// Snapshot of the last unread count we observed. -1 marks "not yet initialized"
// so the first tick only seeds the baseline without firing a spurious refresh.
// Any drop below the stored value means another process (typically
// `agentoast dismiss` via a tmux hook) deleted rows, and we need to update the
// tray icon / main panel ourselves because the INSERT-based watcher path
// below won't notice a DELETE-only change.
static LAST_UNREAD_COUNT: AtomicI64 = AtomicI64::new(-1);

pub fn start(app_handle: AppHandle, db_path: PathBuf) {
    // Initialize last known ID
    if let Ok(conn) = db::open(&db_path) {
        if let Ok(max_id) = db::get_max_id(&conn) {
            LAST_KNOWN_ID.store(max_id, Ordering::SeqCst);
        }
    }

    let handle_for_fs = app_handle.clone();
    let db_path_for_fs = db_path.clone();

    // File system watcher (trailing-edge debounce)
    //
    // Uses recv_timeout to wait 300ms after the last DB file event before checking.
    // This ensures the check runs AFTER the CLI's transaction has committed,
    // preventing the watcher from reading uncommitted WAL data and missing
    // the new notification.
    std::thread::spawn(move || {
        let conn = match db::open_reader(&db_path_for_fs) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to open DB for file watcher: {}", e);
                return;
            }
        };

        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher: RecommendedWatcher =
            Watcher::new(tx, notify::Config::default()).expect("Failed to create file watcher");

        // Watch the directory containing the DB file
        if let Some(parent) = db_path_for_fs.parent() {
            if let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive) {
                log::error!("Failed to watch DB directory: {}", e);
            }
        }

        let db_file_name = db_path_for_fs
            .file_name()
            .map(|n| n.to_string_lossy().to_string());

        let debounce = Duration::from_millis(300);
        let mut last_event: Option<Instant> = None;

        loop {
            let timeout = match last_event {
                Some(t) => {
                    let elapsed = t.elapsed();
                    if elapsed >= debounce {
                        check_new_notifications(&handle_for_fs, &conn, "file-watcher");
                        last_event = None;
                        Duration::from_secs(3600)
                    } else {
                        debounce - elapsed
                    }
                }
                None => Duration::from_secs(3600),
            };

            match rx.recv_timeout(timeout) {
                Ok(Ok(event)) => {
                    let is_db_event = match &db_file_name {
                        Some(name) => event.paths.iter().any(|p| {
                            p.file_name()
                                .map(|n| {
                                    let n = n.to_string_lossy();
                                    n == name.as_str()
                                        || n.starts_with(&format!("{}-", name))
                                        || n == format!("{}-wal", name)
                                        || n == format!("{}-shm", name)
                                })
                                .unwrap_or(false)
                        }),
                        None => false,
                    };

                    if is_db_event {
                        match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) => {
                                last_event = Some(Instant::now());
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::error!("File watch error: {}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if last_event.is_some() {
                        check_new_notifications(&handle_for_fs, &conn, "file-watcher");
                        last_event = None;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    // Polling fallback (every 5 seconds)
    let handle_for_poll = app_handle.clone();
    let db_path_for_poll = db_path.clone();
    std::thread::spawn(move || {
        let conn = match db::open_reader(&db_path_for_poll) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to open DB for polling: {}", e);
                return;
            }
        };

        loop {
            std::thread::sleep(Duration::from_secs(5));
            check_new_notifications(&handle_for_poll, &conn, "polling");
        }
    });
}

/// Resolve the repository path for a tmux pane.
/// Uses `tmux display-message` to get the pane's cwd, then on-disk .git metadata
/// to find the git repo root. Falls back to cwd if not a git repo.
#[cfg(target_os = "macos")]
fn resolve_pane_repo(tmux_pane: &str) -> Option<String> {
    use std::process::Command;

    let tmux_path = crate::terminal::find_tmux()?;

    let output = Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args([
            "display-message",
            "-p",
            "-t",
            tmux_pane,
            "#{pane_current_path}",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let cwd = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if cwd.is_empty() {
        return None;
    }

    // Resolve the repo root from on-disk .git metadata (no git spawn).
    if let Some(info) = agentoast_shared::git_info::resolve_git_info(&cwd) {
        return Some(info.repo_root);
    }

    Some(cwd)
}

fn check_new_notifications(app_handle: &AppHandle, conn: &Connection, source: &str) {
    // External DELETE detection (e.g. `agentoast dismiss` invoked from a tmux
    // hook). The INSERT-based code path below exits early on a DELETE-only
    // change, so rely on a count snapshot and emit here when it shrinks.
    let mut current_count = db::get_unread_count(conn).unwrap_or(0);
    let last_count = LAST_UNREAD_COUNT.load(Ordering::SeqCst);
    if last_count >= 0 && current_count < last_count {
        log::info!(
            "check_new: external DELETE detected ({} -> {}) via {}, refreshing tray + panel",
            last_count,
            current_count,
            source
        );
        let _ = app_handle.emit("notifications:refresh", ());
        let _ = app_handle.emit("notifications:unread-count", current_count);
        update_tray_icon(app_handle, current_count);
    }

    let last_id = LAST_KNOWN_ID.load(Ordering::SeqCst);

    let new_notifications = match db::get_notifications_after_id(conn, last_id) {
        Ok(n) => n,
        Err(e) => {
            log::error!("Failed to get new notifications: {}", e);
            LAST_UNREAD_COUNT.store(current_count, Ordering::SeqCst);
            return;
        }
    };

    if new_notifications.is_empty() {
        LAST_UNREAD_COUNT.store(current_count, Ordering::SeqCst);
        return;
    }

    log::debug!(
        "Detected {} new notification(s) via {}",
        new_notifications.len(),
        source
    );

    // Update last known ID
    if let Some(last) = new_notifications.last() {
        LAST_KNOWN_ID.store(last.id, Ordering::SeqCst);
    }

    // Active pane suppression: remove notifications where the user is looking at the pane
    #[cfg(target_os = "macos")]
    let new_notifications = {
        let (suppressed, remaining): (Vec<_>, Vec<_>) =
            new_notifications.into_iter().partition(|n| {
                let visible =
                    crate::terminal::is_pane_visible_to_user(&n.terminal_bundle_id, &n.tmux_pane);
                log::debug!(
                    "Suppression check: id={} pane={} bundle_id={} visible={}",
                    n.id,
                    n.tmux_pane,
                    n.terminal_bundle_id,
                    visible
                );
                visible
            });

        for n in &suppressed {
            if let Err(e) = db::delete_notification(conn, n.id) {
                log::error!("Failed to delete suppressed notification {}: {}", n.id, e);
            }
        }

        if !suppressed.is_empty() {
            log::debug!(
                "Suppressed {} notification(s) (active pane)",
                suppressed.len()
            );
        }

        remaining
    };

    if new_notifications.is_empty() {
        if let Ok(count) = db::get_unread_count(conn) {
            let _ = app_handle.emit("notifications:unread-count", count);
            update_tray_icon(app_handle, count);
            current_count = count;
        }
        LAST_UNREAD_COUNT.store(current_count, Ordering::SeqCst);
        return;
    }

    // Get mute state once for all filtering decisions
    let mute_state = app_handle.state::<Mutex<MuteState>>();
    let (is_global_muted, muted_repos) = match mute_state.lock() {
        Ok(mute) => (mute.global_muted, mute.muted_repos.clone()),
        Err(e) => {
            log::error!("Failed to lock MuteState: {}", e);
            (false, Default::default())
        }
    };

    let is_muted = |n: &agentoast_shared::models::Notification| -> bool {
        if is_global_muted {
            return true;
        }
        // Short-circuit: if no repos are muted, skip expensive repo resolution
        if muted_repos.is_empty() {
            return false;
        }
        // Resolve the pane's repo and check if it's muted
        #[cfg(target_os = "macos")]
        {
            if !n.tmux_pane.is_empty() {
                if let Some(repo) = resolve_pane_repo(&n.tmux_pane) {
                    return muted_repos.contains(&repo);
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = n;
        false
    };

    // Separate force_focus and normal notifications
    let (focus_notifications, normal_notifications): (Vec<_>, Vec<_>) =
        new_notifications.into_iter().partition(|n| n.force_focus);

    // Collect all notifications that need toast display
    let mut toast_notifications = normal_notifications.clone();
    toast_notifications.extend(focus_notifications.iter().cloned());

    // Show toast (respecting mute state)
    let filtered_toast: Vec<_> = toast_notifications
        .into_iter()
        .filter(|n| !is_muted(n))
        .collect();

    if !filtered_toast.is_empty() {
        let notifications = filtered_toast;
        if let Err(e) = app_handle.run_on_main_thread(move || {
            native_toast::show_notifications(notifications);
        }) {
            log::warn!("Failed to show toast notifications: {e}");
        }
    }

    // Emit notifications:new only for normal notifications (not force_focus)
    if !normal_notifications.is_empty() {
        let _ = app_handle.emit("notifications:new", &normal_notifications);
    }

    // force_focus notifications: when muted, skip focus + DB delete (demote to regular notification)
    let (muted_focus, active_focus): (Vec<_>, Vec<_>) =
        focus_notifications.into_iter().partition(|n| is_muted(n));

    // Muted force_focus notifications are kept in DB as regular notifications
    if !muted_focus.is_empty() {
        let _ = app_handle.emit("notifications:new", &muted_focus);
    }

    // Active (non-muted) force_focus notifications: focus terminal + delete from DB
    #[cfg(target_os = "macos")]
    {
        if let Some(focus_notification) = active_focus.last() {
            let tmux_pane = focus_notification.tmux_pane.clone();
            let terminal_bundle_id = focus_notification.terminal_bundle_id.clone();
            let handle_for_focus = app_handle.clone();
            if let Err(e) = handle_for_focus.run_on_main_thread(move || {
                if let Err(e) = crate::terminal::focus_terminal(&tmux_pane, &terminal_bundle_id) {
                    log::debug!("force_focus: terminal focus failed (non-fatal): {}", e);
                }
            }) {
                log::warn!("Failed to run terminal focus on main thread: {e}");
            }
        }
    }

    for n in &active_focus {
        if let Err(e) = db::delete_notification(conn, n.id) {
            log::error!("Failed to delete force_focus notification {}: {}", n.id, e);
        }
    }

    // Also update unread count + tray icon
    if let Ok(count) = db::get_unread_count(conn) {
        let _ = app_handle.emit("notifications:unread-count", count);
        update_tray_icon(app_handle, count);
        current_count = count;
    }
    LAST_UNREAD_COUNT.store(current_count, Ordering::SeqCst);
}

pub fn update_tray_icon(app_handle: &AppHandle, unread_count: i64) {
    if let Some(tray) = app_handle.tray_by_id(&TrayIconId::new("tray")) {
        let tooltip = if unread_count > 0 {
            format!("Agentoast ({} unread)", unread_count)
        } else {
            "Agentoast".to_string()
        };
        if let Err(e) = tray.set_tooltip(Some(&tooltip)) {
            log::warn!("Failed to set tray tooltip: {e}");
        }

        if unread_count > 0 {
            if let Ok(path) = app_handle
                .path()
                .resolve("icons/tray-icon-notification.png", BaseDirectory::Resource)
            {
                if let Ok(icon) = Image::from_path(path) {
                    if let Err(e) = tray.set_icon(Some(icon)) {
                        log::warn!("Failed to set tray notification icon: {e}");
                    }
                    if let Err(e) = tray.set_icon_as_template(false) {
                        log::warn!("Failed to set tray icon as template (notification): {e}");
                    }
                }
            }
        } else if let Ok(path) = app_handle
            .path()
            .resolve("icons/tray-icon.png", BaseDirectory::Resource)
        {
            if let Ok(icon) = Image::from_path(path) {
                if let Err(e) = tray.set_icon(Some(icon)) {
                    log::warn!("Failed to set tray default icon: {e}");
                }
                if let Err(e) = tray.set_icon_as_template(true) {
                    log::warn!("Failed to set tray icon as template (default): {e}");
                }
            }
        }
    }
}
