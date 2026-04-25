use std::collections::{HashMap, HashSet};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use objc2_app_kit::{
    NSApplicationActivationOptions, NSApplicationActivationPolicy, NSBitmapImageFileType,
    NSBitmapImageRep, NSImage, NSWorkspace,
};
use objc2_foundation::{NSDictionary, NSString};
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningApp {
    pub bundle_id: String,
    pub name: String,
    pub icon_data_url: String,
}

/// Enumerate currently-running apps that have a regular Dock presence.
/// Background daemons and accessory apps are filtered out.
pub fn list_running_apps() -> Vec<RunningApp> {
    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();

    let mut out: Vec<RunningApp> = Vec::new();
    for app in &apps {
        // Keep Regular (normal Dock apps) and Accessory (menu-bar apps like
        // Rectangle, Karabiner, Tauri-based agents). Drop Prohibited — those
        // are pure background services with no user-facing UI to focus.
        let policy = app.activationPolicy();
        if policy != NSApplicationActivationPolicy::Regular
            && policy != NSApplicationActivationPolicy::Accessory
        {
            continue;
        }
        let Some(bundle_id) = app.bundleIdentifier() else {
            continue;
        };
        let bundle_id = bundle_id.to_string();
        if bundle_id.is_empty() {
            continue;
        }
        let name = app
            .localizedName()
            .map(|s| s.to_string())
            .unwrap_or_else(|| bundle_id.clone());

        let icon_path = app
            .bundleURL()
            .and_then(|url| url.path())
            .map(|p| p.to_string());

        let icon_data_url = icon_path
            .as_deref()
            .and_then(icon_for_path_as_png_data_url)
            .unwrap_or_default();

        out.push(RunningApp {
            bundle_id,
            name,
            icon_data_url,
        });
    }

    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.bundle_id == b.bundle_id);
    out
}

/// Resolve PNG data URLs for the given bundle IDs, in one pass over
/// `runningApplications`. Far cheaper than `list_running_apps` when the caller
/// only needs icons for a small allowlist (e.g. the panel header) — we skip
/// the TIFF→PNG round-trip for every other app on the system.
pub fn resolve_app_icons(bundle_ids: &[String]) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    if bundle_ids.is_empty() {
        return out;
    }
    let wanted: HashSet<&str> = bundle_ids.iter().map(|s| s.as_str()).collect();

    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();
    for app in &apps {
        let Some(bid) = app.bundleIdentifier() else {
            continue;
        };
        let bid_str = bid.to_string();
        if !wanted.contains(bid_str.as_str()) {
            continue;
        }
        if out.contains_key(&bid_str) {
            continue;
        }
        if let Some(path) = app.bundleURL().and_then(|u| u.path()) {
            if let Some(url) = icon_for_path_as_png_data_url(&path.to_string()) {
                out.insert(bid_str, url);
            }
        }
        if out.len() == wanted.len() {
            break;
        }
    }
    out
}

fn icon_for_path_as_png_data_url(path: &str) -> Option<String> {
    let workspace = NSWorkspace::sharedWorkspace();
    let path_ns = NSString::from_str(path);
    let image: objc2::rc::Retained<NSImage> = workspace.iconForFile(&path_ns);

    let tiff = image.TIFFRepresentation()?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let empty: objc2::rc::Retained<NSDictionary<_, _>> = NSDictionary::new();
    let png =
        unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &empty) }?;

    let bytes = png.to_vec();
    let encoded = STANDARD.encode(&bytes);
    Some(format!("data:image/png;base64,{}", encoded))
}

/// Bring the app with the given bundle ID to the foreground. Does not launch
/// the app if it is not running — that is left to the user (or a future
/// extension).
pub fn activate_app(bundle_id: &str) -> Result<(), String> {
    if bundle_id.is_empty() {
        return Err("bundle_id is empty".to_string());
    }

    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();
    let target_ns = NSString::from_str(bundle_id);

    for app in &apps {
        if let Some(bid) = app.bundleIdentifier() {
            if bid.isEqualToString(&target_ns) {
                let activated =
                    app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
                if activated {
                    return Ok(());
                }
            }
        }
    }

    Err(format!("App not running: {}", bundle_id))
}
