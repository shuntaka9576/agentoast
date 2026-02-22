use std::io::Write;
use std::process::{Command, Output, Stdio};

use agentoast_shared::db;

fn run_hook_claude(json: &str, data_dir: &std::path::Path, config_dir: &std::path::Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_agentoast"))
        .args(["hook", "claude"])
        .env("XDG_DATA_HOME", data_dir)
        .env("XDG_CONFIG_HOME", config_dir)
        .env_remove("TMUX_PANE")
        .env_remove("__CFBundleIdentifier")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn agentoast");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(json.as_bytes())
        .expect("Failed to write stdin");

    child.wait_with_output().expect("Failed to wait for output")
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
fn stop_event() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let json = serde_json::json!({
        "hook_event_name": "Stop",
        "cwd": env!("CARGO_MANIFEST_DIR")
    })
    .to_string();

    let output = run_hook_claude(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["success"], true);

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].badge, "Stop");
    assert_eq!(notifications[0].badge_color, "green");
    assert_eq!(notifications[0].icon, "claude-code");
    assert!(!notifications[0].force_focus);
}

#[test]
fn notification_event_permission_prompt() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let json = serde_json::json!({
        "hook_event_name": "Notification",
        "notification_type": "permission_prompt",
        "message": "Tool execution requires permission",
        "cwd": env!("CARGO_MANIFEST_DIR")
    })
    .to_string();

    let output = run_hook_claude(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["success"], true);

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].badge, "Notification");
    assert_eq!(notifications[0].body, "Tool execution requires permission");
    assert_eq!(notifications[0].badge_color, "blue");
}

#[test]
fn invalid_json() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    let output = run_hook_claude("not valid json{", data_dir.path(), config_dir.path());
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
[notification.agents.claude]
events = ["Stop"]
"#,
    );

    // permission_prompt should be filtered out since events only includes "Stop"
    let json = serde_json::json!({
        "hook_event_name": "Notification",
        "notification_type": "permission_prompt",
        "message": "Permission needed",
        "cwd": env!("CARGO_MANIFEST_DIR")
    })
    .to_string();

    let output = run_hook_claude(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["success"], true);

    let notifications = get_notifications(data_dir.path());
    assert!(
        notifications.is_empty(),
        "permission_prompt should be filtered out"
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
[notification.agents.claude]
focus_events = ["Stop"]
"#,
    );

    let json = serde_json::json!({
        "hook_event_name": "Stop",
        "cwd": env!("CARGO_MANIFEST_DIR")
    })
    .to_string();

    let output = run_hook_claude(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert!(
        notifications[0].force_focus,
        "Stop should have force_focus=true"
    );
}

#[test]
fn git_info_from_cwd() {
    let data_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    setup_db(data_dir.path());

    // Use workspace root which is a git repo
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let json = serde_json::json!({
        "hook_event_name": "Stop",
        "cwd": workspace_root.to_str().unwrap()
    })
    .to_string();

    let output = run_hook_claude(&json, data_dir.path(), config_dir.path());
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
        "hook_event_name": "Stop"
    })
    .to_string();

    let output = run_hook_claude(&json, data_dir.path(), config_dir.path());
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["success"], true);

    let notifications = get_notifications(data_dir.path());
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].repo, "");
}
