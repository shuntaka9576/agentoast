use rusqlite::Connection;

pub fn initialize(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        DROP TABLE IF EXISTS notifications;

        CREATE TABLE notifications (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            badge         TEXT NOT NULL DEFAULT '',
            body          TEXT NOT NULL DEFAULT '',
            badge_color   TEXT NOT NULL DEFAULT 'gray',
            icon          TEXT NOT NULL DEFAULT 'agentoast',
            metadata      TEXT NOT NULL DEFAULT '{}',
            repo          TEXT NOT NULL DEFAULT '',
            tmux_pane     TEXT NOT NULL DEFAULT '',
            terminal_bundle_id TEXT NOT NULL DEFAULT '',
            force_focus   INTEGER NOT NULL DEFAULT 0,
            is_read       INTEGER NOT NULL DEFAULT 0,
            created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_notifications_tmux_pane ON notifications(tmux_pane);
        ",
    )?;

    Ok(())
}
