#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // CLI subcommands (send, hook, list, config) are handled without Tauri initialization.
    if agentoast_cli::try_run_cli() {
        return;
    }

    // No CLI subcommand detected — launch Tauri GUI app.
    agentoast_app_lib::run()
}
