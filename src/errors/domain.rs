use crate::errors::api_error::{ApiError, ApiResult};
use std::io::ErrorKind;

pub trait ErrorCode {
    #[allow(clippy::wrong_self_convention)]
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
    let from_kind = match error.kind() {
        ErrorKind::NotFound => IoErrorHint::NotFound,
        ErrorKind::PermissionDenied => IoErrorHint::PermissionDenied,
        ErrorKind::AlreadyExists => IoErrorHint::AlreadyExists,
        ErrorKind::InvalidInput => IoErrorHint::InvalidInput,
        _ => IoErrorHint::Other,
    };
    if from_kind != IoErrorHint::Other {
        return from_kind;
    }
    error
        .raw_os_error()
        .map(classify_raw_os_error)
        .unwrap_or(IoErrorHint::Other)
}

pub fn classify_raw_os_error(raw: i32) -> IoErrorHint {
    #[cfg(windows)]
    {
        return match raw {
            5 => IoErrorHint::PermissionDenied,     // ERROR_ACCESS_DENIED
            2 | 3 => IoErrorHint::NotFound,         // ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND
            80 | 183 => IoErrorHint::AlreadyExists, // ERROR_FILE_EXISTS | ERROR_ALREADY_EXISTS
            19 => IoErrorHint::ReadOnlyFilesystem,  // ERROR_WRITE_PROTECT
            87 => IoErrorHint::InvalidInput,        // ERROR_INVALID_PARAMETER
            _ => IoErrorHint::Other,
        };
    }

    #[cfg(unix)]
    {
        return match raw {
            1 | 13 => IoErrorHint::PermissionDenied, // EPERM | EACCES
            2 => IoErrorHint::NotFound,              // ENOENT
            17 => IoErrorHint::AlreadyExists,        // EEXIST
            22 => IoErrorHint::InvalidInput,         // EINVAL
            30 => IoErrorHint::ReadOnlyFilesystem,   // EROFS
            _ => IoErrorHint::Other,
        };
    }

    #[allow(unreachable_code)]
    IoErrorHint::Other
}

pub fn classify_io_hint_from_message(message: &str) -> Option<IoErrorHint> {
    extract_raw_os_error(message).map(classify_raw_os_error)
}

fn extract_raw_os_error(message: &str) -> Option<i32> {
    let needle = "os error ";
    let start = message.to_ascii_lowercase().rfind(needle)? + needle.len();
    let tail = &message[start..];
    let mut end = 0usize;
    for (idx, ch) in tail.char_indices() {
        let is_num = ch.is_ascii_digit() || (idx == 0 && ch == '-');
        if is_num {
            end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    tail[..end].parse::<i32>().ok()
}
