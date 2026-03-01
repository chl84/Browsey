use super::{providers::rclone::RcloneCloudProvider, rclone_cli::RcloneCli};
use crate::binary_resolver::{resolve_binary, resolve_explicit_binary_path};
use std::{ffi::OsString, path::PathBuf};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

pub(crate) fn configured_rclone_cli() -> Result<RcloneCli, String> {
    Ok(RcloneCli::with_binary(configured_rclone_binary()?))
}

pub(crate) fn configured_rclone_provider() -> Result<RcloneCloudProvider, String> {
    Ok(RcloneCloudProvider::from_cli(configured_rclone_cli()?))
}

fn configured_rclone_binary() -> Result<OsString, String> {
    let configured = load_rclone_path_setting()?;
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return resolve_binary("rclone")
            .map(|path| path.into_os_string())
            .ok_or_else(|| {
                "Unable to auto-detect rclone; install it or set Rclone path in Settings."
                    .to_string()
            });
    }

    let explicit = PathBuf::from(trimmed);
    if trimmed == "rclone" {
        return resolve_binary("rclone")
            .map(|path| path.into_os_string())
            .ok_or_else(|| "Configured Rclone path could not be resolved; leave it empty to auto-detect or provide a valid executable path.".to_string());
    }
    if let Some(path) = resolve_explicit_binary_path(&explicit) {
        return Ok(path.into_os_string());
    }

    Err(format!(
        "Configured Rclone path is invalid or not executable: {trimmed}"
    ))
}

fn load_rclone_path_setting() -> Result<String, String> {
    #[cfg(test)]
    {
        let guard = match test_rclone_path_override().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(value) = guard.as_ref() {
            return Ok(value.clone());
        }
    }
    let conn =
        crate::db::open().map_err(|error| format!("Failed to open settings database: {error}"))?;
    let value = crate::db::get_setting_string(&conn, "rclonePath")
        .map_err(|error| format!("Failed to read Rclone path setting: {error}"))?;
    Ok(value.unwrap_or_default())
}

#[cfg(test)]
fn test_rclone_path_override() -> &'static Mutex<Option<String>> {
    static OVERRIDE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) fn set_rclone_path_override_for_tests(value: Option<&str>) {
    let mut guard = match test_rclone_path_override().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = value.map(ToOwned::to_owned);
}
