pub use rusqlite::Connection;

use rusqlite::params;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::models::{IconType, Notification, NotificationGroup};
use crate::schema;

pub fn open(db_path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    schema::initialize(&conn)?;
    Ok(conn)
}

/// Read-only connection without schema initialization.
/// Use this for long-lived reader threads (watcher, polling) to avoid
/// repeated CREATE TABLE / migration checks on every query.
pub fn open_reader(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(conn)
}

#[allow(clippy::too_many_arguments)]
pub fn insert_notification(
    conn: &Connection,
    title: &str,
    body: &str,
    color: &str,
    icon: &IconType,
    group_name: &str,
    metadata: &HashMap<String, String>,
    tmux_pane: &str,
    force_focus: bool,
) -> rusqlite::Result<i64> {
    let metadata_json = serde_json::to_string(metadata).unwrap_or_else(|_| "{}".to_string());

    conn.execute(
        "INSERT INTO notifications (title, body, color, icon, group_name, metadata, tmux_pane, force_focus)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![title, body, color, icon.as_str(), group_name, metadata_json, tmux_pane, force_focus as i32],
    )?;
    Ok(conn.last_insert_rowid())
}

fn row_to_notification(row: &rusqlite::Row) -> rusqlite::Result<Notification> {
    let metadata_str: String = row.get(5)?;
    let metadata: HashMap<String, String> = serde_json::from_str(&metadata_str).unwrap_or_default();

    Ok(Notification {
        id: row.get(0)?,
        title: row.get(1)?,
        body: row.get(2)?,
        color: row.get(3)?,
        icon: row.get(4)?,
        group_name: row.get(6)?,
        metadata,
        tmux_pane: row.get(7)?,
        force_focus: row.get::<_, i32>(8)? != 0,
        is_read: row.get::<_, i32>(9)? != 0,
        created_at: row.get(10)?,
    })
}

pub fn get_notifications(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<Notification>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, body, color, icon, metadata, group_name, tmux_pane, force_focus, is_read, created_at
         FROM notifications ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], row_to_notification)?;
    rows.collect()
}

pub fn get_notifications_grouped(
    conn: &Connection,
    limit: i64,
    group_limit: usize,
) -> rusqlite::Result<Vec<NotificationGroup>> {
    let notifications = get_notifications(conn, limit)?;
    let mut groups: BTreeMap<String, Vec<Notification>> = BTreeMap::new();

    for n in notifications {
        groups.entry(n.group_name.clone()).or_default().push(n);
    }

    let result = groups
        .into_iter()
        .map(|(group_name, notifications)| {
            let unread_count = notifications.iter().filter(|n| !n.is_read).count() as i64;
            let truncated = if group_limit > 0 && notifications.len() > group_limit {
                notifications.into_iter().take(group_limit).collect()
            } else {
                notifications
            };
            NotificationGroup {
                group_name,
                notifications: truncated,
                unread_count,
            }
        })
        .collect();

    Ok(result)
}

pub fn get_unread_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM notifications WHERE is_read = 0",
        [],
        |row| row.get(0),
    )
}

pub fn delete_notification(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM notifications WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn delete_notifications_by_group_tmux(
    conn: &Connection,
    group_name: &str,
    tmux_pane: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM notifications WHERE group_name = ?1 AND tmux_pane = ?2",
        params![group_name, tmux_pane],
    )
}

pub fn delete_notifications_by_group(
    conn: &Connection,
    group_name: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM notifications WHERE group_name = ?1",
        params![group_name],
    )
}

pub fn delete_all_notifications(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM notifications", [])?;
    Ok(())
}

pub fn get_max_id(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(id), 0) FROM notifications",
        [],
        |row| row.get(0),
    )
}

pub fn get_notifications_after_id(
    conn: &Connection,
    after_id: i64,
) -> rusqlite::Result<Vec<Notification>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, body, color, icon, metadata, group_name, tmux_pane, force_focus, is_read, created_at
         FROM notifications WHERE id > ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![after_id], row_to_notification)?;
    rows.collect()
}
