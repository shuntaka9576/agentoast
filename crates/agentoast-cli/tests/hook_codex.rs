use std::process::{Command, Output, Stdio};

use agentoast_shared::db;

fn run_hook_codex(json: &str, data_dir: &std::path::Path, config_dir: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_agentoast"))
        .args(["hook", "codex", json])
        .env("XDG_DATA_HOME", data_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .env_remove("TMUX_PANE")
        .env_remove("__CFBundleIdentifier")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn agentoast")
        .wait_with_output()
        .expect("Failed to wait for output")
}

fn setup_db(data_dir: &std::path::Path) {
    let db_path = data_dir.join("agentoast").join("notifications.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    let _conn = db::open(&db_path).unwrap();
}

fn get_notifications(data_dir: &std::path::Path) -> Vec<agentoast_shared::models::Notification> {
    let db_path = data_dir.join("agentoast").join("notifications.db");
    let conn = db::open_reader(&db_path).unwrap();
    db::get_notifications(&conn, 100).unwrap()
}

fn write_config(config_dir: &std::path::Path, content: &str) {
    let config_path = config_dir.join("agentoast").join("config.toml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(config_path, content).unwrap();
}

#[test]
fn agent_turn_complete_event() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let json = serde_json::json!({
        "type": "agent-turn-complete",
        "thread-id": "test-thread",
        "turn-id": "test-turn",
        "cwd": env!("CARGO_MANIFEST_DIR"),
        "input-messages": ["test message"],
        "last-assistant-message": "Task completed successfully"
    })
    .to_string();

    let output = run_hook_codex(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["success"], true);

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].badge, "Stop");
    assert_eq!(notifications[0].badge_color, "green");
    assert_eq!(notifications[0].icon, "codex");
    assert_eq!(notifications[0].body, "Task completed successfully");
    assert!(!notifications[0].force_focus);
}

#[test]
fn invalid_json() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let output = run_hook_codex("not valid json{", data_dir.path(), config_dir.path());
    assert!(
        output.status.success(),
        "should exit 0 even on invalid JSON"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["success"], false);
    assert!(result["error"]
        .as_str()
        .unwrap()
        .contains("Failed to parse JSON"));

    let notifications = get_notifications(data_dir.path());
    assert!(notifications.is_empty());
}

#[test]
fn event_filtered_by_config() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    write_config(
        config_dir.path(),
        r#"
[hook.codex]
events = []
"#,
    );

    let json = serde_json::json!({
        "type": "agent-turn-complete",
        "cwd": env!("CARGO_MANIFEST_DIR")
    })
    .to_string();

    let output = run_hook_codex(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["success"], true);

    let notifications = get_notifications(data_dir.path());
    assert!(
        notifications.is_empty(),
        "agent-turn-complete should be filtered out when events is empty"
    );
}

#[test]
fn focus_events_config() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    write_config(
        config_dir.path(),
        r#"
[hook.codex]
focus_events = ["agent-turn-complete"]
"#,
    );

    let json = serde_json::json!({
        "type": "agent-turn-complete",
        "cwd": env!("CARGO_MANIFEST_DIR")
    })
    .to_string();

    let output = run_hook_codex(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert!(
        notifications[0].force_focus,
        "agent-turn-complete should have force_focus=true"
    );
}

#[test]
fn git_info_from_cwd() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let json = serde_json::json!({
        "type": "agent-turn-complete",
        "cwd": workspace_root.to_str().unwrap()
    })
    .to_string();

    let output = run_hook_codex(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert!(
        notifications[0].metadata.contains_key("branch"),
        "metadata should contain branch info when cwd is a git repo"
    );
}

#[test]
fn missing_cwd() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let json = serde_json::json!({
        "type": "agent-turn-complete"
    })
    .to_string();

    let output = run_hook_codex(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["success"], true);

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].repo, "");
}

#[test]
fn body_truncation() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let long_message = "a".repeat(500);
    let json = serde_json::json!({
        "type": "agent-turn-complete",
        "cwd": env!("CARGO_MANIFEST_DIR"),
        "last-assistant-message": long_message
    })
    .to_string();

    let output = run_hook_codex(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert!(
        notifications[0].body.len() <= 203, // 200 + "..."
        "body should be truncated to ~200 chars, got {}",
        notifications[0].body.len()
    );
    assert!(notifications[0].body.ends_with("..."));
}

#[test]
fn include_body_false() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    write_config(
        config_dir.path(),
        r#"
[hook.codex]
include_body = false
"#,
    );

    let json = serde_json::json!({
        "type": "agent-turn-complete",
        "cwd": env!("CARGO_MANIFEST_DIR"),
        "last-assistant-message": "This should not appear"
    })
    .to_string();

    let output = run_hook_codex(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert_eq!(
        notifications[0].body, "",
        "body should be empty when include_body=false"
    );
}

#[test]
fn tmux_pane_from_env() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let json = serde_json::json!({
        "type": "agent-turn-complete",
        "cwd": env!("CARGO_MANIFEST_DIR")
    })
    .to_string();

    let output = Command::new(env!("CARGO_BIN_EXE_agentoast"))
        .args(["hook", "codex", &json])
        .env("XDG_DATA_HOME", data_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("TMUX_PANE", "%42")
        .env_remove("__CFBundleIdentifier")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn agentoast")
        .wait_with_output()
        .expect("Failed to wait for output");

    assert!(output.status.success());

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].tmux_pane, "%42");
}
