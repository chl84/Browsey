use crate::errors::{
    api_error::{ApiError, ApiResult},
    domain::{self, classify_io_error, DomainError, ErrorCode, IoErrorHint},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::commands) enum PermissionsErrorCode {
    PathNotAbsolute,
    InvalidPath,
    InvalidInput,
    RootForbidden,
    SymlinkUnsupported,
    PrincipalNotFound,
    GroupUnavailable,
    AuthenticationCancelled,
    ElevatedRequired,
    HelperExecutableNotFound,
    HelperProtocolError,
    HelperStartFailed,
    HelperIoError,
    HelperWaitFailed,
    PermissionDenied,
    ReadOnlyFilesystem,
    UnsupportedPlatform,
    NotFound,
    MetadataReadFailed,
    OwnershipUpdateFailed,
    PermissionsUpdateFailed,
    PostChangeSnapshotFailed,
    RollbackFailed,
    UnknownError,
}

impl PermissionsErrorCode {
    pub(super) fn as_str(self) -> &'static str {
        self.as_code_str()
    }

    pub(super) fn from_code_str(code: &str) -> Option<Self> {
        Some(match code {
            "path_not_absolute" => Self::PathNotAbsolute,
            "invalid_path" => Self::InvalidPath,
            "invalid_input" => Self::InvalidInput,
            "root_forbidden" => Self::RootForbidden,
            "symlink_unsupported" => Self::SymlinkUnsupported,
            "principal_not_found" => Self::PrincipalNotFound,
            "group_unavailable" => Self::GroupUnavailable,
            "authentication_cancelled" => Self::AuthenticationCancelled,
            "elevated_required" => Self::ElevatedRequired,
            "helper_executable_not_found" => Self::HelperExecutableNotFound,
            "helper_protocol_error" => Self::HelperProtocolError,
            "helper_start_failed" => Self::HelperStartFailed,
            "helper_io_error" => Self::HelperIoError,
            "helper_wait_failed" => Self::HelperWaitFailed,
            "permission_denied" => Self::PermissionDenied,
            "read_only_filesystem" => Self::ReadOnlyFilesystem,
            "unsupported_platform" => Self::UnsupportedPlatform,
            "not_found" => Self::NotFound,
            "metadata_read_failed" => Self::MetadataReadFailed,
            "ownership_update_failed" => Self::OwnershipUpdateFailed,
            "permissions_update_failed" => Self::PermissionsUpdateFailed,
            "post_change_snapshot_failed" => Self::PostChangeSnapshotFailed,
            "rollback_failed" => Self::RollbackFailed,
            "unknown_error" => Self::UnknownError,
            _ => return None,
        })
    }
}

impl ErrorCode for PermissionsErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::PathNotAbsolute => "path_not_absolute",
            Self::InvalidPath => "invalid_path",
            Self::InvalidInput => "invalid_input",
            Self::RootForbidden => "root_forbidden",
            Self::SymlinkUnsupported => "symlink_unsupported",
            Self::PrincipalNotFound => "principal_not_found",
            Self::GroupUnavailable => "group_unavailable",
            Self::AuthenticationCancelled => "authentication_cancelled",
            Self::ElevatedRequired => "elevated_required",
            Self::HelperExecutableNotFound => "helper_executable_not_found",
            Self::HelperProtocolError => "helper_protocol_error",
            Self::HelperStartFailed => "helper_start_failed",
            Self::HelperIoError => "helper_io_error",
            Self::HelperWaitFailed => "helper_wait_failed",
            Self::PermissionDenied => "permission_denied",
            Self::ReadOnlyFilesystem => "read_only_filesystem",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::NotFound => "not_found",
            Self::MetadataReadFailed => "metadata_read_failed",
            Self::OwnershipUpdateFailed => "ownership_update_failed",
            Self::PermissionsUpdateFailed => "permissions_update_failed",
            Self::PostChangeSnapshotFailed => "post_change_snapshot_failed",
            Self::RollbackFailed => "rollback_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::commands) struct PermissionsError {
    code: PermissionsErrorCode,
    message: String,
}

impl PermissionsError {
    pub(super) fn new(code: PermissionsErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(PermissionsErrorCode::InvalidInput, message)
    }

    pub(super) fn path_not_absolute(raw_path: &str) -> Self {
        Self::new(
            PermissionsErrorCode::PathNotAbsolute,
            format!("Path must be absolute: {raw_path}"),
        )
    }

    pub(super) fn from_code_and_message(code: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        let code =
            PermissionsErrorCode::from_code_str(code).unwrap_or(PermissionsErrorCode::UnknownError);
        Self::new(code, message)
    }

    pub(super) fn from_io_error(
        fallback: PermissionsErrorCode,
        context: &str,
        error: std::io::Error,
    ) -> Self {
        let code = classify_io_error_code(&error, fallback);
        Self::new(code, format!("{context}: {error}"))
    }

    pub(super) fn code(&self) -> &'static str {
        self.code.as_str()
    }

    pub(super) fn code_enum(&self) -> PermissionsErrorCode {
        self.code
    }

    pub(super) fn message(&self) -> &str {
        &self.message
    }

    pub(super) fn to_api_error(&self) -> ApiError {
        <Self as DomainError>::to_api_error(self)
    }
}

impl fmt::Display for PermissionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PermissionsError {}

impl DomainError for PermissionsError {
    fn code_str(&self) -> &'static str {
        self.code()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<crate::fs_utils::FsUtilsError> for PermissionsError {
    fn from(error: crate::fs_utils::FsUtilsError) -> Self {
        let code = match error.code() {
            crate::fs_utils::FsUtilsErrorCode::InvalidPath => PermissionsErrorCode::InvalidPath,
            crate::fs_utils::FsUtilsErrorCode::NotFound => PermissionsErrorCode::NotFound,
            crate::fs_utils::FsUtilsErrorCode::PermissionDenied => {
                PermissionsErrorCode::PermissionDenied
            }
            crate::fs_utils::FsUtilsErrorCode::ReadOnlyFilesystem => {
                PermissionsErrorCode::ReadOnlyFilesystem
            }
            crate::fs_utils::FsUtilsErrorCode::RootForbidden => PermissionsErrorCode::RootForbidden,
            crate::fs_utils::FsUtilsErrorCode::SymlinkUnsupported => {
                PermissionsErrorCode::SymlinkUnsupported
            }
            crate::fs_utils::FsUtilsErrorCode::CanonicalizeFailed
            | crate::fs_utils::FsUtilsErrorCode::MetadataReadFailed => {
                PermissionsErrorCode::MetadataReadFailed
            }
        };
        Self::new(code, error.to_string())
    }
}

impl From<crate::undo::UndoError> for PermissionsError {
    fn from(error: crate::undo::UndoError) -> Self {
        let code = match error.code() {
            crate::undo::UndoErrorCode::InvalidInput => PermissionsErrorCode::InvalidInput,
            crate::undo::UndoErrorCode::NotFound => PermissionsErrorCode::NotFound,
            crate::undo::UndoErrorCode::PermissionDenied => PermissionsErrorCode::PermissionDenied,
            crate::undo::UndoErrorCode::ReadOnlyFilesystem => {
                PermissionsErrorCode::ReadOnlyFilesystem
            }
            crate::undo::UndoErrorCode::TargetExists => PermissionsErrorCode::RollbackFailed,
            crate::undo::UndoErrorCode::SymlinkUnsupported => {
                PermissionsErrorCode::SymlinkUnsupported
            }
            crate::undo::UndoErrorCode::LockFailed => PermissionsErrorCode::RollbackFailed,
            _ => PermissionsErrorCode::UnknownError,
        };
        Self::new(code, error.to_string())
    }
}

pub(in crate::commands) type PermissionsResult<T> = Result<T, PermissionsError>;

pub(super) fn is_expected_batch_error(error: &PermissionsError) -> bool {
    matches!(
        error.code_enum(),
        PermissionsErrorCode::SymlinkUnsupported
            | PermissionsErrorCode::NotFound
            | PermissionsErrorCode::PermissionDenied
            | PermissionsErrorCode::MetadataReadFailed
    )
}

pub(super) fn map_api_result<T>(result: PermissionsResult<T>) -> ApiResult<T> {
    domain::map_api_result(result)
}

fn classify_io_error_code(
    error: &std::io::Error,
    fallback: PermissionsErrorCode,
) -> PermissionsErrorCode {
    match classify_io_error(error) {
        IoErrorHint::NotFound => PermissionsErrorCode::NotFound,
        IoErrorHint::PermissionDenied => PermissionsErrorCode::PermissionDenied,
        IoErrorHint::ReadOnlyFilesystem => PermissionsErrorCode::ReadOnlyFilesystem,
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::{PermissionsError, PermissionsErrorCode};

    #[test]
    fn maps_typed_code_and_message_without_reclassification() {
        let error = PermissionsError::from_code_and_message(
            "helper_protocol_error",
            "Invalid helper response payload",
        );
        assert_eq!(error.code(), "helper_protocol_error");
        assert_eq!(error.message(), "Invalid helper response payload");
    }

    #[test]
    fn unknown_typed_code_falls_back_to_unknown_error() {
        let error = PermissionsError::from_code_and_message("not_real", "opaque");
        assert_eq!(error.code(), PermissionsErrorCode::UnknownError.as_str());
        assert_eq!(error.message(), "opaque");
    }
}
