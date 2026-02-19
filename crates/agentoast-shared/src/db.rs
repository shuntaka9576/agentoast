pub use rusqlite::Connection;

use rusqlite::params;
use std::collections::HashMap;
use std::path::Path;

use crate::models::{IconType, Notification};
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
    badge: &str,
    body: &str,
    badge_color: &str,
    icon: &IconType,
    metadata: &HashMap<String, String>,
    repo: &str,
    tmux_pane: &str,
    terminal_bundle_id: &str,
    force_focus: bool,
) -> rusqlite::Result<i64> {
    let metadata_json = serde_json::to_string(metadata).unwrap_or_else(|_| "{}".to_string());

    // Wrap DELETE+INSERT in a transaction so they produce a single WAL write,
    // preventing the file-watcher debounce from missing the INSERT.
    let tx = conn.unchecked_transaction()?;

    // Overwrite: remove existing notifications from the same tmux pane
    if !tmux_pane.is_empty() {
        tx.execute(
            "DELETE FROM notifications WHERE tmux_pane = ?1",
            params![tmux_pane],
        )?;
    }

    tx.execute(
        "INSERT INTO notifications (badge, body, badge_color, icon, metadata, repo, tmux_pane, terminal_bundle_id, force_focus)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![badge, body, badge_color, icon.as_str(), metadata_json, repo, tmux_pane, terminal_bundle_id, force_focus as i32],
    )?;

    let id = conn.last_insert_rowid();
    tx.commit()?;
    Ok(id)
}

fn row_to_notification(row: &rusqlite::Row) -> rusqlite::Result<Notification> {
    let metadata_str: String = row.get(5)?;
    let metadata: HashMap<String, String> = serde_json::from_str(&metadata_str).unwrap_or_default();

    Ok(Notification {
        id: row.get(0)?,
        badge: row.get(1)?,
        body: row.get(2)?,
        badge_color: row.get(3)?,
        icon: row.get(4)?,
        metadata,
        repo: row.get(6)?,
        tmux_pane: row.get(7)?,
        terminal_bundle_id: row.get(8)?,
        force_focus: row.get::<_, i32>(9)? != 0,
        is_read: row.get::<_, i32>(10)? != 0,
        created_at: row.get(11)?,
    })
}

pub fn get_notifications(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<Notification>> {
    let mut stmt = conn.prepare(
        "SELECT id, badge, body, badge_color, icon, metadata, repo, tmux_pane, terminal_bundle_id, force_focus, is_read, created_at
         FROM notifications ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], row_to_notification)?;
    rows.collect()
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

pub fn delete_notifications_by_pane(conn: &Connection, tmux_pane: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM notifications WHERE tmux_pane = ?1",
        params![tmux_pane],
    )
}

pub fn delete_notifications_by_panes(
    conn: &Connection,
    panes: &[String],
) -> rusqlite::Result<usize> {
    if panes.is_empty() {
        return Ok(0);
    }

    let placeholders: Vec<String> = (1..=panes.len()).map(|i| format!("?{}", i)).collect();
    let sql = format!(
        "DELETE FROM notifications WHERE tmux_pane IN ({})",
        placeholders.join(", ")
    );

    let params: Vec<&dyn rusqlite::types::ToSql> = panes
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();

    conn.execute(&sql, params.as_slice())
}

pub fn delete_all_notifications(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM notifications", [])?;
    Ok(())
}

pub fn get_latest_notification_by_pane(
    conn: &Connection,
    tmux_pane: &str,
) -> rusqlite::Result<Option<Notification>> {
    let mut stmt = conn.prepare(
        "SELECT id, badge, body, badge_color, icon, metadata, repo, tmux_pane, terminal_bundle_id, force_focus, is_read, created_at
         FROM notifications WHERE tmux_pane = ?1 ORDER BY id DESC LIMIT 1",
    )?;
    let mut rows = stmt.query_map(params![tmux_pane], row_to_notification)?;
    match rows.next() {
        Some(Ok(n)) => Ok(Some(n)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
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
        "SELECT id, badge, body, badge_color, icon, metadata, repo, tmux_pane, terminal_bundle_id, force_focus, is_read, created_at
         FROM notifications WHERE id > ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![after_id], row_to_notification)?;
    rows.collect()
}
