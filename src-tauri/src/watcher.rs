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

use crate::toast;
use crate::MuteState;

static LAST_KNOWN_ID: AtomicI64 = AtomicI64::new(0);

pub fn start(app_handle: AppHandle, db_path: PathBuf) {
    // Initialize last known ID
    if let Ok(conn) = db::open(&db_path) {
        if let Ok(max_id) = db::get_max_id(&conn) {
            LAST_KNOWN_ID.store(max_id, Ordering::SeqCst);
        }
    }

    let handle_for_fs = app_handle.clone();
    let db_path_for_fs = db_path.clone();

    // File system watcher
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

        let mut last_check = Instant::now() - Duration::from_secs(1);

        for event in rx {
            match event {
                Ok(event) => {
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
                                // Debounce: skip if checked within 200ms
                                let now = Instant::now();
                                if now.duration_since(last_check) < Duration::from_millis(200) {
                                    continue;
                                }
                                last_check = now;
                                check_new_notifications(&handle_for_fs, &conn);
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    log::error!("File watch error: {}", e);
                }
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
            check_new_notifications(&handle_for_poll, &conn);
        }
    });
}

fn check_new_notifications(app_handle: &AppHandle, conn: &Connection) {
    let last_id = LAST_KNOWN_ID.load(Ordering::SeqCst);

    let new_notifications = match db::get_notifications_after_id(conn, last_id) {
        Ok(n) => n,
        Err(e) => {
            log::error!("Failed to get new notifications: {}", e);
            return;
        }
    };

    if new_notifications.is_empty() {
        return;
    }

    // Update last known ID
    if let Some(last) = new_notifications.last() {
        LAST_KNOWN_ID.store(last.id, Ordering::SeqCst);
    }

    // Get mute state once for all filtering decisions
    let mute_state = app_handle.state::<Mutex<MuteState>>();
    let (is_global_muted, muted_groups) = match mute_state.lock() {
        Ok(mute) => (mute.global_muted, mute.muted_groups.clone()),
        Err(e) => {
            log::error!("Failed to lock MuteState: {}", e);
            (false, Default::default())
        }
    };

    let is_muted = |n: &agentoast_shared::models::Notification| -> bool {
        is_global_muted || muted_groups.contains(&n.group_name)
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
        let _ = app_handle.emit_to("toast", "toast:show", &filtered_toast);
        let handle = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            toast::show(&handle);
        });
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
            let _ = handle_for_focus.run_on_main_thread(move || {
                if let Err(e) = crate::terminal::focus_terminal(&tmux_pane, &terminal_bundle_id) {
                    log::debug!("force_focus: terminal focus failed (non-fatal): {}", e);
                }
            });
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
    }
}

pub fn update_tray_icon(app_handle: &AppHandle, unread_count: i64) {
    if let Some(tray) = app_handle.tray_by_id(&TrayIconId::new("tray")) {
        let tooltip = if unread_count > 0 {
            format!("Agentoast ({} unread)", unread_count)
        } else {
            "Agentoast".to_string()
        };
        let _ = tray.set_tooltip(Some(&tooltip));

        if unread_count > 0 {
            if let Ok(path) = app_handle
                .path()
                .resolve("icons/tray-icon-notification.png", BaseDirectory::Resource)
            {
                if let Ok(icon) = Image::from_path(path) {
                    let _ = tray.set_icon(Some(icon));
                    let _ = tray.set_icon_as_template(false);
                }
            }
        } else if let Ok(path) = app_handle
            .path()
            .resolve("icons/tray-icon.png", BaseDirectory::Resource)
        {
            if let Ok(icon) = Image::from_path(path) {
                let _ = tray.set_icon(Some(icon));
                let _ = tray.set_icon_as_template(true);
            }
        }
    }
}
