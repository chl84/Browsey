use std::path::{Path, PathBuf};

#[cfg(not(target_os = "windows"))]
use std::process::Command;

use crate::clipboard::CopyProgressPayload;
use crate::runtime_lifecycle;

pub fn estimate_total_size(entries: &[PathBuf], evt: &str, app: &tauri::AppHandle) -> u64 {
    let mut total: u64 = 0;
    for p in entries {
        if runtime_lifecycle::is_shutting_down(app) {
            break;
        }
        if let Ok(meta) = std::fs::metadata(p) {
            total = total.saturating_add(meta.len());
            continue;
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(size) = gio_size(p) {
                total = total.saturating_add(size);
            }
        }
    }
    if total > 0 {
        let _ = runtime_lifecycle::emit_if_running(
            app,
            evt,
            CopyProgressPayload {
                bytes: 0,
                total,
                finished: false,
            },
        );
    }
    total
}

#[cfg(not(target_os = "windows"))]
fn gio_size(path: &Path) -> Result<u64, String> {
    let output = Command::new("gio")
        .arg("info")
        .arg("--attributes=standard::size")
        .arg(path)
        .output()
        .map_err(|e| format!("gio info failed: {e}"))?;
    if !output.status.success() {
        return Err("gio info failed".into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(':').map(|s| s.trim()).collect();
        if parts.len() == 2 && parts[0] == "standard::size" {
            if let Ok(n) = parts[1].parse::<u64>() {
                return Ok(n);
            }
        }
    }
    Err("size not found".into())
}

#[cfg(target_os = "windows")]
fn gio_size(_path: &Path) -> Result<u64, String> {
    Err("gio size unsupported on Windows".into())
}
