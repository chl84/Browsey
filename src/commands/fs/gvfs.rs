//! Helpers for GVFS-backed mounts (e.g., MTP over GVFS).

#[cfg(not(target_os = "windows"))]
use crate::commands::fs::MountInfo;
#[cfg(not(target_os = "windows"))]
use crate::fs_utils::debug_log;
#[cfg(not(target_os = "windows"))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(not(target_os = "windows"))]
fn gvfs_root() -> Option<PathBuf> {
    dirs_next::runtime_dir().map(|p| p.join("gvfs"))
}

#[cfg(not(target_os = "windows"))]
fn display_name(path: &Path) -> Option<String> {
    // Use `gio info` to fetch the user-facing name. Falls back to directory name on failure.
    let output = Command::new("gio")
        .arg("info")
        .arg("--attributes=standard::display-name")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        debug_log(&format!(
            "gio info failed for {}: status {:?}",
            path.display(),
            output.status.code()
        ));
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        // Usually: "display name: Pixel 7" (localized key should still contain the attribute name)
        if let Some(rest) = trimmed.strip_prefix("display name:") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = trimmed.strip_prefix("standard::display-name:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
pub fn list_gvfs_mounts() -> Vec<MountInfo> {
    let mut mounts = Vec::new();
    let root = match gvfs_root() {
        Some(p) => p,
        None => return mounts,
    };

    if !root.exists() {
        return mounts;
    }

    let entries = match fs::read_dir(&root) {
        Ok(rd) => rd,
        Err(e) => {
            debug_log(&format!("Failed to read gvfs dir {}: {e}", root.display()));
            return mounts;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry
            .file_name()
            .to_string_lossy()
            .into_owned();

        let (fs, removable) = match name.split_once(':').map(|(p, _)| p) {
            Some("mtp") => ("mtp", true),
            Some("onedrive") => ("onedrive", true),
            _ => continue,
        };

        let label = display_name(&path).unwrap_or_else(|| name.clone());

        mounts.push(MountInfo {
            label: short_label(fs, &label, &name),
            path: path.to_string_lossy().into_owned(),
            fs: fs.to_string(),
            removable,
        });
    }

    mounts
}

#[cfg(target_os = "windows")]
pub fn list_gvfs_mounts() -> Vec<MountInfo> {
    Vec::new()
}

#[cfg(not(target_os = "windows"))]
fn short_label(fs: &str, display: &str, raw_name: &str) -> String {
    if fs == "onedrive" {
        // Privacy: avoid personal identifiers; prefer provider/host if available.
        let host = raw_name
            .split(',')
            .find_map(|part| part.strip_prefix("host="))
            .map(str::trim)
            .filter(|s| !s.is_empty());

        return match host {
            Some(h) => format!("OneDrive ({h})"),
            None => "OneDrive".to_string(),
        };
    }
    display.to_string()
}
