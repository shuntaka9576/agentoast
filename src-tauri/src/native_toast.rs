#![allow(deprecated)] // msg_send_id! is deprecated in objc2 0.6, but still works

use std::sync::{Mutex, OnceLock};

use agentoast_shared::config;
use agentoast_shared::models::Notification;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2::{msg_send, msg_send_id, AnyThread, ClassType, MainThreadOnly};
use objc2_app_kit::{
    NSAnimationContext, NSApplication, NSBackingStoreType, NSColor, NSEvent, NSEventMask, NSFont,
    NSImage, NSImageView, NSLineBreakMode, NSPanel, NSScreen, NSTextAlignment, NSTextField, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectView,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{MainThreadMarker, NSData, NSString, NSTimer};
use tauri::Emitter;

use crate::terminal;

// SAFETY: NSPanel and NSTimer are only ever accessed from the main thread.
// We enforce this by only calling show_notifications / hide / init from the main thread.
struct SendSyncWrapper<T>(T);
unsafe impl<T> Send for SendSyncWrapper<T> {}
unsafe impl<T> Sync for SendSyncWrapper<T> {}

static TOAST_PANEL: OnceLock<SendSyncWrapper<Retained<NSPanel>>> = OnceLock::new();
static TOAST_STATE: OnceLock<Mutex<ToastState>> = OnceLock::new();
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();
static TOAST_TIMER: Mutex<Option<SendSyncWrapper<Retained<NSTimer>>>> = Mutex::new(None);
static FADE_TIMER: Mutex<Option<SendSyncWrapper<Retained<NSTimer>>>> = Mutex::new(None);
static EVENT_MONITOR_INSTALLED: OnceLock<()> = OnceLock::new();

const PANEL_WIDTH: f64 = 380.0;
const PADDING: f64 = 8.0;
const CORNER_RADIUS: f64 = 12.0;
const FADE_DURATION: f64 = 0.3;

// Shared layout constants (used by both compute_panel_height and build_toast_view)
const TOP_MARGIN: f64 = 12.0;
const LINE1_HEIGHT: f64 = 18.0;
const META_HEIGHT: f64 = 16.0;
const BODY_HEIGHT: f64 = 28.0;
const LINE_GAP: f64 = 6.0;
const BOTTOM_GAP: f64 = 10.0;
const BOTTOM_SECTION_H: f64 = 27.0;
const BOTTOM_MARGIN: f64 = 5.0;

fn compute_panel_height(has_meta: bool, has_body: bool) -> f64 {
    let meta_section = if has_meta { LINE_GAP + META_HEIGHT } else { 0.0 };
    let body_section = if has_body { LINE_GAP + BODY_HEIGHT } else { 0.0 };
    let effect_h = TOP_MARGIN + LINE1_HEIGHT + meta_section + body_section
        + BOTTOM_GAP + BOTTOM_SECTION_H + BOTTOM_MARGIN;
    effect_h + PADDING * 2.0
}

const GIT_BRANCH_ICON: &[u8] = include_bytes!("../icons/toast/git-branch.png");
const TMUX_ICON: &[u8] = include_bytes!("../icons/toast/tmux.png");
const X_ICON: &[u8] = include_bytes!("../icons/toast/x.png");
const TRASH_ICON: &[u8] = include_bytes!("../icons/toast/trash.png");

struct ToastState {
    queue: Vec<Notification>,
    current_index: usize,
    is_visible: bool,
    duration_ms: u64,
    persistent: bool,
}

// --- Color definitions ---

struct ToastColors {
    bg: (f64, f64, f64, f64),
    border: (f64, f64, f64, f64),
    focus_bg: (f64, f64, f64, f64),
    focus_border: (f64, f64, f64, f64),
    text_secondary: (f64, f64, f64, f64),
    text_muted: (f64, f64, f64, f64),
    badge_stop_bg: (f64, f64, f64, f64),
    badge_stop_text: (f64, f64, f64, f64),
    badge_notif_bg: (f64, f64, f64, f64),
    badge_notif_text: (f64, f64, f64, f64),
    badge_red_bg: (f64, f64, f64, f64),
    badge_red_text: (f64, f64, f64, f64),
    badge_gray_bg: (f64, f64, f64, f64),
    badge_gray_text: (f64, f64, f64, f64),
    focus_badge_bg: (f64, f64, f64, f64),
    focus_badge_text: (f64, f64, f64, f64),
}

fn is_dark_mode() -> bool {
    let mtm = match MainThreadMarker::new() {
        Some(m) => m,
        None => return false,
    };
    let app = NSApplication::sharedApplication(mtm);
    unsafe {
        let appearance: Option<Retained<objc2_app_kit::NSAppearance>> =
            msg_send_id![&app, effectiveAppearance];
        if let Some(appearance) = appearance {
            let name: Option<Retained<NSString>> = msg_send_id![&appearance, name];
            if let Some(name) = name {
                return name.to_string().contains("Dark");
            }
        }
    }
    false
}

fn colors() -> ToastColors {
    if is_dark_mode() {
        ToastColors {
            bg: (0.173, 0.173, 0.18, 0.95),
            border: (1.0, 1.0, 1.0, 0.10),
            focus_bg: (0.216, 0.157, 0.314, 0.95),
            focus_border: (0.545, 0.361, 0.965, 0.40),
            text_secondary: (1.0, 1.0, 1.0, 0.70),
            text_muted: (1.0, 1.0, 1.0, 0.40),
            badge_stop_bg: (0.133, 0.773, 0.369, 0.20),
            badge_stop_text: (0.290, 0.855, 0.502, 1.0),
            badge_notif_bg: (0.231, 0.510, 0.965, 0.20),
            badge_notif_text: (0.376, 0.647, 0.980, 1.0),
            badge_red_bg: (0.961, 0.259, 0.259, 0.20),
            badge_red_text: (0.973, 0.443, 0.443, 1.0),
            badge_gray_bg: (1.0, 1.0, 1.0, 0.10),
            badge_gray_text: (1.0, 1.0, 1.0, 0.50),
            focus_badge_bg: (0.545, 0.361, 0.965, 0.25),
            focus_badge_text: (0.655, 0.545, 0.980, 1.0),
        }
    } else {
        ToastColors {
            bg: (1.0, 1.0, 1.0, 0.95),
            border: (0.0, 0.0, 0.0, 0.10),
            focus_bg: (0.929, 0.914, 0.996, 0.95),
            focus_border: (0.545, 0.361, 0.965, 0.35),
            text_secondary: (0.0, 0.0, 0.0, 0.70),
            text_muted: (0.0, 0.0, 0.0, 0.40),
            badge_stop_bg: (0.133, 0.773, 0.369, 0.15),
            badge_stop_text: (0.086, 0.639, 0.290, 1.0),
            badge_notif_bg: (0.231, 0.510, 0.965, 0.15),
            badge_notif_text: (0.145, 0.388, 0.929, 1.0),
            badge_red_bg: (0.961, 0.259, 0.259, 0.15),
            badge_red_text: (0.937, 0.267, 0.267, 1.0),
            badge_gray_bg: (0.0, 0.0, 0.0, 0.10),
            badge_gray_text: (0.0, 0.0, 0.0, 0.50),
            focus_badge_bg: (0.545, 0.361, 0.965, 0.15),
            focus_badge_text: (0.486, 0.227, 0.929, 1.0),
        }
    }
}

fn nscolor(r: f64, g: f64, b: f64, a: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a)
}

fn nscolor_tuple(t: (f64, f64, f64, f64)) -> Retained<NSColor> {
    nscolor(t.0, t.1, t.2, t.3)
}

fn font_medium(size: f64) -> Retained<NSFont> {
    // NSFontWeightMedium = 0.23
    unsafe { msg_send_id![NSFont::class(), systemFontOfSize: size, weight: 0.23_f64] }
}

fn font_regular(size: f64) -> Retained<NSFont> {
    NSFont::systemFontOfSize(size)
}

// --- Panel creation ---

pub fn init(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let _ = APP_HANDLE.set(app_handle.clone());
    let _ = TOAST_STATE.set(Mutex::new({
        let cfg = config::load_config();
        ToastState {
            queue: Vec::new(),
            current_index: 0,
            is_visible: false,
            duration_ms: cfg.toast.duration_ms,
            persistent: cfg.toast.persistent,
        }
    }));

    let mtm = MainThreadMarker::new().ok_or_else(|| "Must be called on main thread".to_string())?;

    let panel = create_panel(mtm);
    TOAST_PANEL
        .set(SendSyncWrapper(panel))
        .map_err(|_| "Toast panel already initialized".to_string())?;

    install_event_monitor();

    log::info!("[native_toast] init complete");
    Ok(())
}

fn create_panel(mtm: MainThreadMarker) -> Retained<NSPanel> {
    unsafe {
        let frame = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(PANEL_WIDTH, 144.0));

        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;

        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        );

        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(true);
        panel.setMovable(false);

        // Level: floating + 2 (above main panel). NSFloatingWindowLevel = 5
        let _: () = msg_send![&panel, setLevel: 7i64];

        // Collection behavior
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );

        // Don't steal key focus
        let _: () = msg_send![&panel, setBecomesKeyOnlyIfNeeded: Bool::YES];

        panel
    }
}

// --- Public API ---

pub fn show_notifications(notifications: Vec<Notification>) {
    if notifications.is_empty() {
        return;
    }

    let Some(state_mutex) = TOAST_STATE.get() else {
        return;
    };

    // Clear any pending timers
    cancel_timer();
    cancel_fade_timer();

    let mut state = match state_mutex.lock() {
        Ok(s) => s,
        Err(e) => {
            log::error!("[native_toast] Failed to lock state: {}", e);
            return;
        }
    };

    if state.queue.is_empty() || !state.is_visible {
        // Fresh queue (LIFO: newest first)
        let mut reversed = notifications;
        reversed.reverse();
        state.queue = reversed;
        state.current_index = 0;
        state.is_visible = true;
        drop(state);
        update_and_show();
    } else {
        // Merge into existing queue
        let current_idx = state.current_index;
        let remaining: Vec<Notification> = state.queue[current_idx..]
            .iter()
            .filter(|q| {
                !notifications
                    .iter()
                    .any(|n| !n.tmux_pane.is_empty() && q.tmux_pane == n.tmux_pane)
            })
            .cloned()
            .collect();

        let mut new_items: Vec<Notification> = notifications;
        new_items.reverse();
        new_items.extend(remaining);

        state.queue = new_items;
        state.current_index = 0;
        drop(state);
        update_and_show();
    }
}

pub fn hide() {
    let Some(wrapper) = TOAST_PANEL.get() else {
        return;
    };
    let panel = &wrapper.0;
    cancel_timer();
    cancel_fade_timer();

    panel.orderOut(None);

    if let Some(state_mutex) = TOAST_STATE.get() {
        if let Ok(mut state) = state_mutex.lock() {
            state.is_visible = false;
            state.queue.clear();
            state.current_index = 0;
        }
    }
}

// --- Internal ---

fn update_and_show() {
    let Some(wrapper) = TOAST_PANEL.get() else {
        return;
    };
    let panel = &wrapper.0;
    let Some(state_mutex) = TOAST_STATE.get() else {
        return;
    };

    let state = match state_mutex.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    let current = match state.queue.get(state.current_index) {
        Some(n) => n.clone(),
        None => {
            drop(state);
            hide();
            return;
        }
    };

    let queue_len = state.queue.len();
    let current_index = state.current_index;
    let duration_ms = state.duration_ms;
    let persistent = state.persistent;
    drop(state);

    let mtm = match MainThreadMarker::new() {
        Some(m) => m,
        None => return,
    };

    // Determine dynamic height
    let has_meta =
        !current.tmux_pane.is_empty() || current.metadata.values().any(|v| !v.is_empty());
    let has_body = !current.body.is_empty();
    let panel_height = compute_panel_height(has_meta, has_body);

    // Resize panel BEFORE setting content view to prevent layout distortion in release builds.
    // Setting content view on a panel with stale size causes AppKit to resize subviews incorrectly.
    position_at_top_right(mtm, panel, panel_height);

    let content_view = build_toast_view(mtm, &current, current_index, queue_len, panel_height);
    panel.setContentView(Some(&content_view));

    // Show with fade-in animation
    panel.setAlphaValue(0.0);
    panel.orderFrontRegardless();

    let panel_ptr = Retained::as_ptr(panel) as usize;
    NSAnimationContext::runAnimationGroup(&RcBlock::new(
        move |context: std::ptr::NonNull<NSAnimationContext>| {
            let ctx = unsafe { context.as_ref() };
            ctx.setDuration(FADE_DURATION);
            let panel_ref: &NSPanel = unsafe { &*(panel_ptr as *const NSPanel) };
            let animator: Retained<NSPanel> = unsafe { msg_send_id![panel_ref, animator] };
            animator.setAlphaValue(1.0);
        },
    ));

    // Start auto-advance timer
    if !persistent {
        start_timer(duration_ms);
    }
}

fn build_toast_view(
    mtm: MainThreadMarker,
    notification: &Notification,
    current_index: usize,
    queue_len: usize,
    panel_height: f64,
) -> Retained<NSView> {
    let colors = colors();
    let is_focus = notification.force_focus;
    let (bg_color, border_color) = if is_focus {
        (colors.focus_bg, colors.focus_border)
    } else {
        (colors.bg, colors.border)
    };

    // Root view (transparent)
    let root = NSView::initWithFrame(
        NSView::alloc(mtm),
        CGRect::new(
            CGPoint::new(0.0, 0.0),
            CGSize::new(PANEL_WIDTH, panel_height),
        ),
    );

    // Visual effect view (blur background)
    let effect_frame = CGRect::new(
        CGPoint::new(PADDING, PADDING),
        CGSize::new(PANEL_WIDTH - PADDING * 2.0, panel_height - PADDING * 2.0),
    );
    let effect_view =
        NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), effect_frame);
    effect_view.setMaterial(NSVisualEffectMaterial::Popover);
    effect_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    effect_view.setWantsLayer(true);

    if let Some(layer) = effect_view.layer() {
        layer.setCornerRadius(CORNER_RADIUS);
        layer.setMasksToBounds(true);
        layer.setBorderWidth(1.0);
        let border_cg = nscolor_tuple(border_color).CGColor();
        layer.setBorderColor(Some(&border_cg));
        let bg_cg = nscolor_tuple(bg_color).CGColor();
        layer.setBackgroundColor(Some(&bg_cg));
    }

    let effect_w = effect_frame.size.width;
    let effect_h = effect_frame.size.height;
    let muted_color = nscolor_tuple(colors.text_muted);

    // --- Icon (16x16, no container, directly on effect_view) ---
    let icon_size = 16.0;
    let icon_x = 12.0;
    let icon_y = effect_h - TOP_MARGIN - icon_size;

    let png_bytes: &[u8] = match notification.icon.as_str() {
        "claude-code" => include_bytes!("../icons/toast/claude-code.png"),
        "codex" => include_bytes!("../icons/toast/codex.png"),
        "opencode" => include_bytes!("../icons/toast/opencode.png"),
        _ => include_bytes!("../icons/toast/agentoast.png"),
    };
    let ns_data = NSData::with_bytes(png_bytes);
    let image: Option<Retained<NSImage>> =
        unsafe { msg_send_id![NSImage::alloc(), initWithData: &*ns_data] };
    if let Some(image) = image {
        unsafe {
            let _: () = msg_send![&image, setTemplate: Bool::YES];
        }
        let image_view = NSImageView::initWithFrame(
            NSImageView::alloc(mtm),
            CGRect::new(
                CGPoint::new(icon_x, icon_y),
                CGSize::new(icon_size, icon_size),
            ),
        );
        image_view.setImage(Some(&image));
        unsafe {
            let _: () = msg_send![&image_view, setContentTintColor: &*muted_color];
        }
        effect_view.addSubview(&image_view);
    }

    // Text content starts after icon
    let text_x = 32.0;
    let text_width = effect_w - text_x - 12.0;

    // --- Line 1: Badge + repo name + relative time ---
    let line1_y = effect_h - TOP_MARGIN - LINE1_HEIGHT;
    let mut line1_x = 0.0_f64;

    // Badge pill
    if !notification.badge.is_empty() {
        let (badge_bg, badge_text) = badge_colors(&notification.badge_color, &colors);
        let (badge_pill, badge_w) = make_pill(
            mtm,
            &notification.badge,
            CGPoint::new(text_x + line1_x, line1_y),
            &nscolor_tuple(badge_text),
            &nscolor_tuple(badge_bg),
            10.0,
            18.0,
        );
        line1_x += badge_w + 4.0;
        effect_view.addSubview(&badge_pill);
    }

    // Repo name (plain text, no background)
    if !notification.repo.is_empty() {
        let repo_font = font_medium(12.0);
        let repo_label = make_label(
            mtm,
            &notification.repo,
            CGRect::new(
                CGPoint::new(text_x + line1_x, line1_y),
                CGSize::new(text_width - line1_x, 18.0),
            ),
            &nscolor_tuple(colors.text_secondary),
            &repo_font,
        );
        repo_label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        effect_view.addSubview(&repo_label);
    }

    // Relative time (right-aligned)
    let time_text = format_relative_time(&notification.created_at);
    if !time_text.is_empty() {
        let time_font = font_regular(10.0);
        let time_label = make_label(
            mtm,
            &time_text,
            CGRect::new(CGPoint::new(0.0, line1_y + 2.0), CGSize::new(200.0, 16.0)),
            &muted_color,
            &time_font,
        );
        unsafe {
            let _: () = msg_send![&time_label, sizeToFit];
        }
        let fitted: CGRect = time_label.frame();
        time_label.setFrame(CGRect::new(
            CGPoint::new(effect_w - fitted.size.width - 12.0, line1_y + 2.0),
            CGSize::new(fitted.size.width, 16.0),
        ));
        effect_view.addSubview(&time_label);
    }

    // --- Line 2: Metadata (below badge line) ---
    let meta_y = line1_y - LINE_GAP - META_HEIGHT;
    let meta_height = META_HEIGHT;
    let meta_icon_size = 12.0;
    let meta_gap = 4.0;
    let meta_icon_text_gap = 2.0;

    let mut meta_entries: Vec<(Option<&[u8]>, String)> = Vec::new();
    for (key, value) in &notification.metadata {
        if !value.is_empty() {
            if key == "branch" {
                meta_entries.push((Some(GIT_BRANCH_ICON), value.clone()));
            } else {
                meta_entries.push((None, format!("{}:{}", key, value)));
            }
        }
    }
    if !notification.tmux_pane.is_empty() {
        meta_entries.push((Some(TMUX_ICON), notification.tmux_pane.clone()));
    }

    let has_meta = !meta_entries.is_empty();
    if has_meta {
        let meta_x = 12.0; // icon_x と同じ（アイコン左端揃え）
        let meta_width = effect_w - meta_x - 12.0;
        let meta_container = NSView::initWithFrame(
            NSView::alloc(mtm),
            CGRect::new(
                CGPoint::new(meta_x, meta_y),
                CGSize::new(meta_width, meta_height),
            ),
        );

        let mut cursor_x = 0.0_f64;
        for (icon_bytes, text) in &meta_entries {
            if cursor_x > 0.0 {
                cursor_x += meta_gap;
            }

            if let Some(png_bytes) = icon_bytes {
                if let Some(icon_view) =
                    make_meta_icon(mtm, png_bytes, cursor_x, 2.0, meta_icon_size, &muted_color)
                {
                    meta_container.addSubview(&icon_view);
                    cursor_x += meta_icon_size + meta_icon_text_gap;
                }
            }

            let meta_font = font_regular(11.0);
            let label = make_label(
                mtm,
                text,
                CGRect::new(
                    CGPoint::new(cursor_x, 0.0),
                    CGSize::new(text_width - cursor_x, meta_height),
                ),
                &muted_color,
                &meta_font,
            );
            unsafe {
                let _: () = msg_send![&label, sizeToFit];
            }
            let fitted: CGRect = label.frame();
            label.setFrame(CGRect::new(
                CGPoint::new(cursor_x, 0.0),
                CGSize::new(fitted.size.width, meta_height),
            ));
            meta_container.addSubview(&label);
            cursor_x += fitted.size.width;
        }

        effect_view.addSubview(&meta_container);
    }

    // --- Line 3: Body (up to 2 lines) ---
    let has_body = !notification.body.is_empty();
    if has_body {
        let body_font = font_regular(11.0);
        let body_top = if has_meta {
            meta_y - LINE_GAP
        } else {
            line1_y - LINE_GAP
        };
        let body_h = BODY_HEIGHT;
        let body_x = 12.0; // icon_x と同じ（アイコン左端揃え）
        let body_width = effect_w - body_x - 12.0;
        let body_y = body_top - body_h;
        log::debug!(
            "[native_toast] layout: effect_h={}, line1_y={}, meta_y={}, body_top={}, body_y={}, body_h={}",
            effect_h, line1_y, meta_y, body_top, body_y, body_h
        );
        let body_label = make_label(
            mtm,
            &notification.body,
            CGRect::new(
                CGPoint::new(body_x, body_y),
                CGSize::new(body_width, body_h),
            ),
            &nscolor_tuple(colors.text_secondary),
            &body_font,
        );
        body_label.setMaximumNumberOfLines(2);
        body_label.setLineBreakMode(NSLineBreakMode::ByCharWrapping);
        effect_view.addSubview(&body_label);
    }

    // --- Queue counter (bottom-right, plain text) ---
    let bottom_y = 8.0;
    if queue_len > 1 {
        let counter_str = format!("{}/{}", current_index + 1, queue_len);
        let counter_font = font_medium(10.0);
        let counter_label = make_label(
            mtm,
            &counter_str,
            CGRect::new(CGPoint::new(0.0, bottom_y), CGSize::new(200.0, 12.0)),
            &muted_color,
            &counter_font,
        );
        unsafe {
            let _: () = msg_send![&counter_label, sizeToFit];
        }
        let fitted: CGRect = counter_label.frame();
        counter_label.setFrame(CGRect::new(
            CGPoint::new(effect_w - fitted.size.width - 12.0, bottom_y),
            CGSize::new(fitted.size.width, 12.0),
        ));
        effect_view.addSubview(&counter_label);
    }

    // --- Focused: no history badge (bottom-right) ---
    if is_focus {
        let (focus_pill, focus_w) = make_pill(
            mtm,
            "Focused: no history",
            CGPoint::new(0.0, 0.0),
            &nscolor_tuple(colors.focus_badge_text),
            &nscolor_tuple(colors.focus_badge_bg),
            10.0,
            14.0,
        );
        focus_pill.setFrameOrigin(CGPoint::new(effect_w - focus_w - 12.0, 6.0));
        effect_view.addSubview(&focus_pill);
    }

    // --- Dismiss buttons (bottom-left, pill background) ---
    let btn_w = 28.0;
    let btn_h = 22.0;
    let btn_icon_size = 14.0;
    let btn_y = 5.0;
    let btn_gap = 6.0;
    if let Some(v) = make_dismiss_button(
        mtm,
        X_ICON,
        8.0,
        btn_y,
        btn_w,
        btn_h,
        btn_icon_size,
        &colors,
    ) {
        effect_view.addSubview(&v);
    }
    if let Some(v) = make_dismiss_button(
        mtm,
        TRASH_ICON,
        8.0 + btn_w + btn_gap,
        btn_y,
        btn_w,
        btn_h,
        btn_icon_size,
        &colors,
    ) {
        effect_view.addSubview(&v);
    }

    root.addSubview(&effect_view);
    root
}

type ColorTuple = (f64, f64, f64, f64);

fn badge_colors(badge_color: &str, colors: &ToastColors) -> (ColorTuple, ColorTuple) {
    match badge_color {
        "green" => (colors.badge_stop_bg, colors.badge_stop_text),
        "blue" => (colors.badge_notif_bg, colors.badge_notif_text),
        "red" => (colors.badge_red_bg, colors.badge_red_text),
        _ => (colors.badge_gray_bg, colors.badge_gray_text),
    }
}

fn make_label(
    mtm: MainThreadMarker,
    text: &str,
    frame: CGRect,
    color: &NSColor,
    font: &NSFont,
) -> Retained<NSTextField> {
    let label = NSTextField::initWithFrame(NSTextField::alloc(mtm), frame);
    label.setStringValue(&NSString::from_str(text));
    label.setBezeled(false);
    label.setDrawsBackground(false);
    label.setEditable(false);
    label.setSelectable(false);
    label.setTextColor(Some(color));
    label.setFont(Some(font));
    label
}

fn make_pill(
    mtm: MainThreadMarker,
    text: &str,
    origin: CGPoint,
    text_color: &NSColor,
    bg_color: &NSColor,
    font_size: f64,
    pill_height: f64,
) -> (Retained<NSView>, f64) {
    let font = font_medium(font_size);
    let label = make_label(mtm, text, CGRect::ZERO, text_color, &font);
    label.setAlignment(NSTextAlignment::Center);
    unsafe {
        let _: () = msg_send![&label, sizeToFit];
    }
    let fitted: CGRect = label.frame();

    let pill_w = fitted.size.width + 10.0;
    let text_y = (pill_height - fitted.size.height) / 2.0;

    label.setFrame(CGRect::new(
        CGPoint::new(0.0, text_y),
        CGSize::new(pill_w, fitted.size.height),
    ));

    let pill = NSView::initWithFrame(
        NSView::alloc(mtm),
        CGRect::new(origin, CGSize::new(pill_w, pill_height)),
    );
    pill.setWantsLayer(true);
    if let Some(layer) = pill.layer() {
        let cg = bg_color.CGColor();
        layer.setBackgroundColor(Some(&cg));
        layer.setCornerRadius(4.0);
    }
    pill.addSubview(&label);

    (pill, pill_w)
}

fn format_relative_time(created_at: &str) -> String {
    // Parse ISO 8601: "2025-01-01T12:00:00.000Z"
    // No chrono dependency (binary size optimization), manual parse
    let parts: Vec<&str> = created_at.split('T').collect();
    if parts.len() != 2 {
        return String::new();
    }
    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_str = parts[1].trim_end_matches('Z');
    let time_parts: Vec<&str> = time_str.split(':').collect();
    if date_parts.len() != 3 || time_parts.len() < 2 {
        return String::new();
    }

    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let hour: u32 = time_parts[0].parse().unwrap_or(0);
    let min: u32 = time_parts[1].parse().unwrap_or(0);
    let sec: u32 = time_parts[2]
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Simple days-since-epoch for comparison (not exact, but sufficient for relative time)
    let days_in_month = [0u32, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let created_days: u32 =
        year * 365 + year / 4 + (1..month).map(|m| days_in_month[m as usize]).sum::<u32>() + day;
    let created_secs =
        created_days as i64 * 86400 + hour as i64 * 3600 + min as i64 * 60 + sec as i64;

    // Get current UTC time via SystemTime (unix epoch based)
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Convert unix epoch to our days-since-epoch:
    // Unix epoch is 1970-01-01. We need to convert to the same scale.
    // Our epoch: year*365 + year/4 + month_days + day (approx days since year 0)
    // 1970-01-01 in our scale: 1970*365 + 1970/4 + 0 + 1 = 719243
    let epoch_offset: i64 = 1970 * 365 + 1970 / 4 + 1;
    let now_secs = now_unix + epoch_offset * 86400;

    let diff = now_secs - created_secs;
    if diff < 0 {
        return "just now".to_string();
    }
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

fn make_meta_icon(
    mtm: MainThreadMarker,
    png_bytes: &[u8],
    x: f64,
    y: f64,
    size: f64,
    tint: &NSColor,
) -> Option<Retained<NSImageView>> {
    let ns_data = NSData::with_bytes(png_bytes);
    let image: Option<Retained<NSImage>> =
        unsafe { msg_send_id![NSImage::alloc(), initWithData: &*ns_data] };
    image.map(|img| {
        unsafe {
            let _: () = msg_send![&img, setTemplate: Bool::YES];
        }
        let view = NSImageView::initWithFrame(
            NSImageView::alloc(mtm),
            CGRect::new(CGPoint::new(x, y), CGSize::new(size, size)),
        );
        view.setImage(Some(&img));
        unsafe {
            let _: () = msg_send![&view, setContentTintColor: tint];
        }
        view
    })
}

#[allow(clippy::too_many_arguments)]
fn make_dismiss_button(
    mtm: MainThreadMarker,
    png_bytes: &[u8],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    icon_size: f64,
    colors: &ToastColors,
) -> Option<Retained<NSView>> {
    let container = NSView::initWithFrame(
        NSView::alloc(mtm),
        CGRect::new(CGPoint::new(x, y), CGSize::new(w, h)),
    );
    container.setWantsLayer(true);
    if let Some(layer) = container.layer() {
        let bg_cg = nscolor_tuple(colors.badge_gray_bg).CGColor();
        layer.setBackgroundColor(Some(&bg_cg));
        layer.setCornerRadius(4.0);
    }

    let icon_x = (w - icon_size) / 2.0;
    let icon_y = (h - icon_size) / 2.0;
    let tint = nscolor_tuple(colors.text_muted);
    let icon_view = make_meta_icon(mtm, png_bytes, icon_x, icon_y, icon_size, &tint)?;
    container.addSubview(&icon_view);

    Some(container)
}

// --- Click handling via local event monitor ---

fn install_event_monitor() {
    EVENT_MONITOR_INSTALLED.get_or_init(|| {
        unsafe {
            let block = RcBlock::new(|event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
                let event_ref = event.as_ref();
                let Some(wrapper) = TOAST_PANEL.get() else {
                    return event.as_ptr();
                };
                let panel = &wrapper.0;

                // Check if click is within our panel
                let event_window_num: i64 = msg_send![event_ref, windowNumber];
                let panel_window_num: i64 = msg_send![panel, windowNumber];
                if event_window_num != panel_window_num {
                    return event.as_ptr();
                }

                // Get click location in effect_view coordinates
                let location = event_ref.locationInWindow();
                let local_x = location.x - PADDING;
                let local_y = location.y - PADDING;

                log::debug!(
                    "[native_toast] click at local_x={:.1}, local_y={:.1}",
                    local_x,
                    local_y
                );

                // Bottom-left dismiss area: 70x27 zone (two 28x22 buttons + gap)
                // X button: x=8..36, Trash button: x=42..70 (effect_view coords)
                if local_x < 70.0 && local_y < 27.0 {
                    if local_x < 38.0 {
                        log::debug!("[native_toast] dismiss_keep");
                        handle_dismiss_keep();
                    } else {
                        log::debug!("[native_toast] dismiss_delete");
                        handle_dismiss_delete();
                    }
                } else {
                    handle_card_click();
                }

                // Return null to consume the event
                std::ptr::null_mut()
            });

            let mask = NSEventMask::LeftMouseDown;
            let _monitor: Option<Retained<objc2_foundation::NSObject>> = msg_send_id![
                NSEvent::class(),
                addLocalMonitorForEventsMatchingMask: mask.0,
                handler: &*block
            ];

            // Keep alive for app lifetime
            std::mem::forget(_monitor);
            std::mem::forget(block);
        }
    });
}

// --- Click handlers ---

fn handle_card_click() {
    cancel_timer();

    let Some(state_mutex) = TOAST_STATE.get() else {
        return;
    };

    let (notification, has_next) = {
        let state = match state_mutex.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let current = state.queue.get(state.current_index).cloned();
        let has_next = state.current_index + 1 < state.queue.len();
        (current, has_next)
    };

    if let Some(n) = notification {
        // Delete notification from DB (unless force_focus)
        if !n.force_focus {
            if let Some(app_handle) = APP_HANDLE.get() {
                let db_path = config::db_path();
                if let Ok(conn) = agentoast_shared::db::open_reader(&db_path) {
                    let _ = agentoast_shared::db::delete_notification(&conn, n.id);
                    if let Ok(count) = agentoast_shared::db::get_unread_count(&conn) {
                        let _ = app_handle.emit("notifications:unread-count", count);
                        crate::watcher::update_tray_icon(app_handle, count);
                    }
                }
            }
        }

        // Focus terminal
        if !n.tmux_pane.is_empty() {
            if let Err(e) = terminal::focus_terminal(&n.tmux_pane, &n.terminal_bundle_id) {
                log::debug!("[native_toast] focus_terminal failed: {}", e);
            }
        }
    }

    if has_next {
        advance();
    } else {
        fade_out_and_hide();
    }
}

fn handle_dismiss_keep() {
    cancel_timer();
    let Some(state_mutex) = TOAST_STATE.get() else {
        return;
    };
    let has_next = {
        let state = match state_mutex.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        state.current_index + 1 < state.queue.len()
    };

    if has_next {
        advance();
    } else {
        fade_out_and_hide();
    }
}

fn handle_dismiss_delete() {
    cancel_timer();

    let Some(state_mutex) = TOAST_STATE.get() else {
        return;
    };

    let (notification, has_next) = {
        let state = match state_mutex.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let current = state.queue.get(state.current_index).cloned();
        let has_next = state.current_index + 1 < state.queue.len();
        (current, has_next)
    };

    if let Some(n) = notification {
        if !n.force_focus {
            if let Some(app_handle) = APP_HANDLE.get() {
                let db_path = config::db_path();
                if let Ok(conn) = agentoast_shared::db::open_reader(&db_path) {
                    let _ = agentoast_shared::db::delete_notification(&conn, n.id);
                    if let Ok(count) = agentoast_shared::db::get_unread_count(&conn) {
                        let _ = app_handle.emit("notifications:unread-count", count);
                        crate::watcher::update_tray_icon(app_handle, count);
                    }
                }
            }
        }
    }

    if has_next {
        advance();
    } else {
        fade_out_and_hide();
    }
}

fn advance() {
    let Some(state_mutex) = TOAST_STATE.get() else {
        return;
    };
    {
        let mut state = match state_mutex.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        state.current_index += 1;
    }
    update_and_show();
}

fn fade_out_and_hide() {
    let Some(wrapper) = TOAST_PANEL.get() else {
        return;
    };
    let panel = &wrapper.0;
    cancel_timer();

    let panel_ptr = Retained::as_ptr(panel) as usize;
    NSAnimationContext::runAnimationGroup(&RcBlock::new(
        move |context: std::ptr::NonNull<NSAnimationContext>| {
            let ctx = unsafe { context.as_ref() };
            ctx.setDuration(FADE_DURATION);
            let panel_ref: &NSPanel = unsafe { &*(panel_ptr as *const NSPanel) };
            let animator: Retained<NSPanel> = unsafe { msg_send_id![panel_ref, animator] };
            animator.setAlphaValue(0.0);
        },
    ));

    start_fade_timer();
}

fn position_at_top_right(mtm: MainThreadMarker, panel: &NSPanel, panel_height: f64) {
    let screen = match NSScreen::mainScreen(mtm) {
        Some(s) => s,
        None => return,
    };
    let screen_frame = screen.frame();
    let visible_frame = screen.visibleFrame();

    let menu_bar_height = (screen_frame.origin.y + screen_frame.size.height)
        - (visible_frame.origin.y + visible_frame.size.height);

    let margin = 16.0;
    let x = screen_frame.origin.x + screen_frame.size.width - PANEL_WIDTH - margin;
    let y =
        screen_frame.origin.y + screen_frame.size.height - menu_bar_height - panel_height - margin;

    panel.setFrame_display(
        CGRect::new(CGPoint::new(x, y), CGSize::new(PANEL_WIDTH, panel_height)),
        true,
    );
}

// --- Timer management ---

fn start_timer(duration_ms: u64) {
    cancel_timer();
    let interval = duration_ms as f64 / 1000.0;
    let block = RcBlock::new(move |_timer: std::ptr::NonNull<NSTimer>| {
        advance_or_hide();
    });
    let timer =
        unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(interval, false, &block) };
    if let Ok(mut t) = TOAST_TIMER.lock() {
        *t = Some(SendSyncWrapper(timer));
    }
}

fn cancel_timer() {
    if let Ok(mut t) = TOAST_TIMER.lock() {
        if let Some(wrapper) = t.take() {
            wrapper.0.invalidate();
        }
    }
}

fn start_fade_timer() {
    cancel_fade_timer();
    let fade_ms = (FADE_DURATION * 1000.0) as u64 + 50;
    let interval = fade_ms as f64 / 1000.0;
    let block = RcBlock::new(move |_timer: std::ptr::NonNull<NSTimer>| {
        hide();
    });
    let timer =
        unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(interval, false, &block) };
    if let Ok(mut t) = FADE_TIMER.lock() {
        *t = Some(SendSyncWrapper(timer));
    }
}

fn cancel_fade_timer() {
    if let Ok(mut t) = FADE_TIMER.lock() {
        if let Some(wrapper) = t.take() {
            wrapper.0.invalidate();
        }
    }
}

fn advance_or_hide() {
    let Some(state_mutex) = TOAST_STATE.get() else {
        return;
    };
    let has_next = {
        let state = match state_mutex.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        state.current_index + 1 < state.queue.len()
    };

    if has_next {
        advance();
    } else {
        fade_out_and_hide();
    }
}
