use crate::errors::{
    api_error::ApiResult,
    domain::{self, DomainError, ErrorCode},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryMetadataErrorCode {
    InvalidPath,
    NotFound,
    PermissionDenied,
    MetadataReadFailed,
    UnknownError,
}

impl ErrorCode for EntryMetadataErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::MetadataReadFailed => "metadata_read_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct EntryMetadataError {
    code: EntryMetadataErrorCode,
    message: String,
}

impl EntryMetadataError {
    pub(super) fn new(code: EntryMetadataErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for EntryMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EntryMetadataError {}

impl DomainError for EntryMetadataError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<crate::entry::EntryError> for EntryMetadataError {
    fn from(error: crate::entry::EntryError) -> Self {
        let code = match error.code_str() {
            "not_found" => EntryMetadataErrorCode::NotFound,
            "permission_denied" => EntryMetadataErrorCode::PermissionDenied,
            "metadata_read_failed" => EntryMetadataErrorCode::MetadataReadFailed,
            _ => EntryMetadataErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::fs_utils::FsUtilsError> for EntryMetadataError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => EntryMetadataErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::NotFound => EntryMetadataErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => {
                EntryMetadataErrorCode::PermissionDenied
            }
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem
            | crate::fs_utils::FsUtilsErrorCode::RootForbidden
            | crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported
            | crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => {
                EntryMetadataErrorCode::MetadataReadFailed
            }
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::metadata::MetadataError> for EntryMetadataError {
    fn from(error: crate::metadata::MetadataError) -> Self {
        let code = match error.code() {
            crate::metadata::MetadataErrorCode::MetadataReadFailed => {
                EntryMetadataErrorCode::MetadataReadFailed
            }
            crate::metadata::MetadataErrorCode::ArchiveReadFailed
            | crate::metadata::MetadataErrorCode::PdfiumLoadFailed
            | crate::metadata::MetadataErrorCode::UnsupportedArchiveVariant
            | crate::metadata::MetadataErrorCode::UnknownError => {
                EntryMetadataErrorCode::UnknownError
            }
        };
        Self::new(code, error.to_string())
    }
}

pub(super) type EntryMetadataResult<T> = Result<T, EntryMetadataError>;

pub(super) fn map_api_result<T>(result: EntryMetadataResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

#[cfg(test)]
mod tests {
    use super::EntryMetadataError;
    use crate::errors::domain::DomainError;
    use crate::metadata::{MetadataError, MetadataErrorCode};
    use std::io::ErrorKind;

    #[test]
    fn maps_entry_not_found_to_entry_metadata_not_found() {
        let entry_error = crate::entry::EntryError::from_io_error(
            "read metadata",
            std::io::Error::from(ErrorKind::NotFound),
        );
        let error = EntryMetadataError::from(entry_error);
        assert_eq!(error.code_str(), "not_found");
    }

    #[test]
    fn maps_entry_metadata_read_failed_to_entry_metadata_read_failed() {
        let entry_error = crate::entry::EntryError::from_io_error(
            "read metadata",
            std::io::Error::from(ErrorKind::Other),
        );
        let error = EntryMetadataError::from(entry_error);
        assert_eq!(error.code_str(), "metadata_read_failed");
    }

    #[test]
    fn maps_metadata_archive_read_failed_to_unknown() {
        let metadata_error =
            MetadataError::new(MetadataErrorCode::ArchiveReadFailed, "archive read failed");
        let error = EntryMetadataError::from(metadata_error);
        assert_eq!(error.code_str(), "unknown_error");
    }
}
