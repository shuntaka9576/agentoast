use tauri::Manager;

pub fn disable_webview_suspension(app_handle: &tauri::AppHandle) {
    let Some(window) = app_handle.get_webview_window("toast") else {
        log::warn!("webkit_config: toast window not found");
        return;
    };

    if let Err(e) = window.with_webview(|webview| unsafe {
        use objc2_web_kit::{WKInactiveSchedulingPolicy, WKWebView};
        let wk_webview: &WKWebView = &*webview.inner().cast();
        let config = wk_webview.configuration();
        let prefs = config.preferences();
        prefs.setInactiveSchedulingPolicy(WKInactiveSchedulingPolicy::None);
        log::info!("WebKit inactiveSchedulingPolicy set to None");
    }) {
        log::warn!("Failed to configure WebKit scheduling: {e}");
    }
}
