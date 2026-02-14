use std::sync::Once;

use objc2::msg_send;
use objc2::rc::Retained;
use objc2_foundation::{NSObject, NSProcessInfo, NSString};

static INIT: Once = Once::new();

pub fn disable_app_nap() {
    INIT.call_once(|| unsafe {
        let process_info = NSProcessInfo::processInfo();
        let options: u64 = 0x00FFFFFF & !(1u64 << 20);
        let reason = NSString::from_str("DB file watcher for notifications");
        let token: Retained<NSObject> = msg_send![
            &process_info,
            beginActivityWithOptions: options,
            reason: &*reason
        ];
        std::mem::forget(token);
        log::info!("App Nap disabled");
    });
}
