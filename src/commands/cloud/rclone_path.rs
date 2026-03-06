use super::{providers::rclone::RcloneCloudProvider, rclone_cli::RcloneCli};
use crate::binary_resolver::{resolve_binary_checked, resolve_explicit_binary_path_checked};
use std::{ffi::OsString, path::PathBuf};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RclonePathErrorCode {
    BinaryMissing,
    InvalidBinaryPath,
    DbOpenFailed,
    DbReadFailed,
}

#[derive(Debug, Clone)]
pub(crate) struct RclonePathError {
    code: RclonePathErrorCode,
    message: String,
}

impl RclonePathError {
    pub(crate) fn new(code: RclonePathErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn code(&self) -> RclonePathErrorCode {
        self.code
    }

    #[allow(dead_code)]
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedRcloneBinary {
    configured_path: Option<String>,
    resolved_binary_path: OsString,
}

impl ResolvedRcloneBinary {
    pub(crate) fn configured_path(&self) -> Option<&str> {
        self.configured_path.as_deref()
    }

    pub(crate) fn resolved_binary_path(&self) -> &OsString {
        &self.resolved_binary_path
    }
}

impl std::fmt::Display for RclonePathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RclonePathError {}

fn map_db_open_error(error: crate::db::DbError) -> RclonePathError {
    RclonePathError::new(RclonePathErrorCode::DbOpenFailed, error.to_string())
}

fn map_db_read_error(error: crate::db::DbError) -> RclonePathError {
    RclonePathError::new(RclonePathErrorCode::DbReadFailed, error.to_string())
}

pub(crate) fn configured_rclone_cli() -> Result<RcloneCli, RclonePathError> {
    Ok(RcloneCli::with_binary(
        resolve_configured_rclone_binary()?.resolved_binary_path,
    ))
}

pub(crate) fn configured_rclone_provider() -> Result<RcloneCloudProvider, RclonePathError> {
    Ok(RcloneCloudProvider::from_cli(configured_rclone_cli()?))
}

pub(crate) fn resolve_configured_rclone_binary() -> Result<ResolvedRcloneBinary, RclonePathError> {
    let configured_path = load_rclone_path_setting()?;

    #[cfg(test)]
    if let Some(override_result) = test_rclone_resolution_override_take() {
        return override_result.map(|resolved_binary_path| ResolvedRcloneBinary {
            configured_path: configured_path.clone(),
            resolved_binary_path,
        });
    }

    let Some(trimmed_owned) = configured_path.clone() else {
        return resolve_binary_checked("rclone")
            .map(|path| ResolvedRcloneBinary {
                configured_path,
                resolved_binary_path: path.into_os_string(),
            })
            .map_err(|_| {
                RclonePathError::new(
                    RclonePathErrorCode::BinaryMissing,
                    "Unable to auto-detect rclone; install it or set Rclone path in Settings.",
                )
            });
    };
    let trimmed = trimmed_owned.as_str();

    let explicit = PathBuf::from(trimmed);
    if trimmed == "rclone" {
        return resolve_binary_checked("rclone")
            .map(|path| ResolvedRcloneBinary {
                configured_path,
                resolved_binary_path: path.into_os_string(),
            })
            .map_err(|_| {
                RclonePathError::new(
                    RclonePathErrorCode::InvalidBinaryPath,
                    "Configured Rclone path could not be resolved; leave it empty to auto-detect or provide a valid executable path.",
                )
            });
    }
    resolve_explicit_binary_path_checked(&explicit)
        .map(|path| ResolvedRcloneBinary {
            configured_path,
            resolved_binary_path: path.into_os_string(),
        })
        .map_err(|_| {
            RclonePathError::new(
                RclonePathErrorCode::InvalidBinaryPath,
                format!("Configured Rclone path is invalid or not executable: {trimmed}"),
            )
        })
}

pub(crate) fn load_rclone_path_setting() -> Result<Option<String>, RclonePathError> {
    #[cfg(test)]
    {
        let guard = match test_rclone_path_override().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(value) = guard.as_ref() {
            let trimmed = value.trim();
            return Ok((!trimmed.is_empty()).then(|| trimmed.to_string()));
        }
    }
    let conn = crate::db::open().map_err(map_db_open_error)?;
    let value = crate::db::get_setting_string(&conn, "rclonePath").map_err(map_db_read_error)?;
    Ok(value.and_then(|raw| {
        let trimmed = raw.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    }))
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

#[cfg(test)]
type TestResolutionOverride = Result<OsString, RclonePathError>;

#[cfg(test)]
fn test_rclone_resolution_override() -> &'static Mutex<Option<TestResolutionOverride>> {
    static OVERRIDE: OnceLock<Mutex<Option<TestResolutionOverride>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn test_rclone_resolution_override_take() -> Option<TestResolutionOverride> {
    let guard = match test_rclone_resolution_override().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.clone()
}

#[cfg(test)]
pub(crate) fn set_rclone_resolution_override_for_tests(value: Option<TestResolutionOverride>) {
    let mut guard = match test_rclone_resolution_override().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = value;
}
