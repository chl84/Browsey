use crate::errors::api_error::{ApiError, ApiResult};
use std::io::ErrorKind;

pub trait ErrorCode {
    fn as_code_str(self) -> &'static str;
}

pub trait DomainError: std::error::Error {
    fn code_str(&self) -> &'static str;
    fn message(&self) -> &str;

    fn to_api_error(&self) -> ApiError {
        ApiError::new(self.code_str(), self.message())
    }
}

pub fn map_api_result<T, E>(result: Result<T, E>) -> ApiResult<T>
where
    E: DomainError,
{
    result.map_err(|error| error.to_api_error())
}

pub fn classify_message_by_patterns<C: Copy>(
    message: &str,
    rules: &[(C, &[&str])],
    fallback: C,
) -> C {
    let normalized = message.to_ascii_lowercase();
    for &(code, patterns) in rules {
        if patterns.iter().any(|pattern| normalized.contains(pattern)) {
            return code;
        }
    }
    fallback
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoErrorHint {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidInput,
    ReadOnlyFilesystem,
    Other,
}

pub fn classify_io_error(error: &std::io::Error) -> IoErrorHint {
    match error.kind() {
        ErrorKind::NotFound => IoErrorHint::NotFound,
        ErrorKind::PermissionDenied => IoErrorHint::PermissionDenied,
        ErrorKind::AlreadyExists => IoErrorHint::AlreadyExists,
        ErrorKind::InvalidInput => IoErrorHint::InvalidInput,
        _ => {
            if is_read_only_filesystem_error(error.raw_os_error()) {
                IoErrorHint::ReadOnlyFilesystem
            } else {
                IoErrorHint::Other
            }
        }
    }
}

#[cfg(unix)]
fn is_read_only_filesystem_error(raw: Option<i32>) -> bool {
    raw == Some(libc::EROFS)
}

#[cfg(windows)]
fn is_read_only_filesystem_error(raw: Option<i32>) -> bool {
    // ERROR_WRITE_PROTECT
    raw == Some(19)
}

#[cfg(not(any(unix, windows)))]
fn is_read_only_filesystem_error(raw: Option<i32>) -> bool {
    let _ = raw;
    false
}
