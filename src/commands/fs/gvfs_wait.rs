#[cfg(not(target_os = "windows"))]
use std::{time::{Duration, Instant}};

#[cfg(not(target_os = "windows"))]
pub fn wait_for_mount(prefix: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if super::gvfs::has_mount_prefix(prefix) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}

#[cfg(target_os = "windows")]
pub fn wait_for_mount(_prefix: &str, _timeout: Duration) -> bool {
    false
}
