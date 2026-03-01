use super::{providers::rclone::RcloneCloudProvider, rclone_cli::RcloneCli};
use crate::binary_resolver::{resolve_binary_checked, resolve_explicit_binary_path_checked};
use std::{ffi::OsString, path::PathBuf};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RclonePathErrorCode {
    InvalidConfig,
    DbOpenFailed,
    DbReadFailed,
}

#[derive(Debug, Clone)]
pub(crate) struct RclonePathError {
    code: RclonePathErrorCode,
    message: String,
}

impl RclonePathError {
    fn new(code: RclonePathErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn code(&self) -> RclonePathErrorCode {
        self.code
    }
}

impl std::fmt::Display for RclonePathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RclonePathError {}

pub(crate) fn configured_rclone_cli() -> Result<RcloneCli, RclonePathError> {
    Ok(RcloneCli::with_binary(configured_rclone_binary()?))
}

pub(crate) fn configured_rclone_provider() -> Result<RcloneCloudProvider, RclonePathError> {
    Ok(RcloneCloudProvider::from_cli(configured_rclone_cli()?))
}

fn configured_rclone_binary() -> Result<OsString, RclonePathError> {
    let configured = load_rclone_path_setting()?;
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return resolve_binary_checked("rclone")
            .map(|path| path.into_os_string())
            .map_err(|_| {
                RclonePathError::new(
                    RclonePathErrorCode::InvalidConfig,
                    "Unable to auto-detect rclone; install it or set Rclone path in Settings.",
                )
            });
    }

    let explicit = PathBuf::from(trimmed);
    if trimmed == "rclone" {
        return resolve_binary_checked("rclone")
            .map(|path| path.into_os_string())
            .map_err(|_| {
                RclonePathError::new(
                    RclonePathErrorCode::InvalidConfig,
                    "Configured Rclone path could not be resolved; leave it empty to auto-detect or provide a valid executable path.",
                )
            });
    }
    resolve_explicit_binary_path_checked(&explicit)
        .map(|path| path.into_os_string())
        .map_err(|_| {
            RclonePathError::new(
                RclonePathErrorCode::InvalidConfig,
                format!("Configured Rclone path is invalid or not executable: {trimmed}"),
            )
        })
}

fn load_rclone_path_setting() -> Result<String, RclonePathError> {
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
    let conn = crate::db::open().map_err(|error| {
        RclonePathError::new(
            RclonePathErrorCode::DbOpenFailed,
            format!("Failed to open settings database: {error}"),
        )
    })?;
    let value = crate::db::get_setting_string(&conn, "rclonePath").map_err(|error| {
        RclonePathError::new(
            RclonePathErrorCode::DbReadFailed,
            format!("Failed to read Rclone path setting: {error}"),
        )
    })?;
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
