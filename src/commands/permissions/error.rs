use crate::errors::{
    api_error::{ApiError, ApiResult},
    domain::{
        self, classify_io_error, classify_io_hint_from_message, classify_message_by_patterns,
        DomainError, ErrorCode, IoErrorHint,
    },
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

    pub(super) fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        Self::new(classify_external_message(&message), message)
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

impl From<String> for PermissionsError {
    fn from(message: String) -> Self {
        Self::from_external_message(message)
    }
}

impl From<&str> for PermissionsError {
    fn from(message: &str) -> Self {
        Self::from_external_message(message)
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

fn classify_external_message(message: &str) -> PermissionsErrorCode {
    if let Some(hint) = classify_io_hint_from_message(message) {
        let io_code = match hint {
            IoErrorHint::NotFound => Some(PermissionsErrorCode::NotFound),
            IoErrorHint::PermissionDenied => Some(PermissionsErrorCode::PermissionDenied),
            IoErrorHint::ReadOnlyFilesystem => Some(PermissionsErrorCode::ReadOnlyFilesystem),
            _ => None,
        };
        if let Some(code) = io_code {
            return code;
        }
    }
    classify_message_by_patterns(
        message,
        EXTERNAL_CLASSIFICATION_RULES,
        PermissionsErrorCode::UnknownError,
    )
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

const EXTERNAL_CLASSIFICATION_RULES: &[(PermissionsErrorCode, &[&str])] = &[
    (
        PermissionsErrorCode::PathNotAbsolute,
        &["path must be absolute"],
    ),
    (
        PermissionsErrorCode::InvalidPath,
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
        ],
    ),
    (
        PermissionsErrorCode::InvalidInput,
        &[
            "no paths provided",
            "no permission changes were provided",
            "no ownership changes were provided",
        ],
    ),
    (
        PermissionsErrorCode::RootForbidden,
        &["refusing to operate on filesystem root"],
    ),
    (
        PermissionsErrorCode::SymlinkUnsupported,
        &[
            "symlinks are not allowed in path",
            "symlinks are not allowed:",
            "permissions are not supported on symlinks",
            "ownership changes are not supported on symlinks",
        ],
    ),
    (
        PermissionsErrorCode::PrincipalNotFound,
        &["user not found", "group not found"],
    ),
    (
        PermissionsErrorCode::GroupUnavailable,
        &["group information is unavailable"],
    ),
    (
        PermissionsErrorCode::AuthenticationCancelled,
        &[
            "authentication was cancelled or denied",
            "request dismissed",
            "cancelled",
        ],
    ),
    (
        PermissionsErrorCode::ElevatedRequired,
        &["requires elevated privileges", "pkexec is not installed"],
    ),
    (
        PermissionsErrorCode::HelperExecutableNotFound,
        &["failed to locate browsey executable"],
    ),
    (
        PermissionsErrorCode::HelperProtocolError,
        &["failed to serialize helper request", "invalid helper input"],
    ),
    (
        PermissionsErrorCode::HelperStartFailed,
        &["failed to start pkexec"],
    ),
    (
        PermissionsErrorCode::HelperIoError,
        &[
            "failed to send helper request",
            "failed reading helper input",
        ],
    ),
    (
        PermissionsErrorCode::HelperWaitFailed,
        &["failed waiting for pkexec helper"],
    ),
    (
        PermissionsErrorCode::PermissionDenied,
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
            "not authorized",
        ],
    ),
    (
        PermissionsErrorCode::ReadOnlyFilesystem,
        &["read-only file system"],
    ),
    (
        PermissionsErrorCode::UnsupportedPlatform,
        &["not supported on this platform"],
    ),
    (
        PermissionsErrorCode::NotFound,
        &["path does not exist", "no such file or directory"],
    ),
    (
        PermissionsErrorCode::MetadataReadFailed,
        &[
            "failed to read metadata",
            "getnamedsecurityinfow failed",
            "getsecuritydescriptordacl failed",
            "getace failed",
            "createwellknownsid failed",
        ],
    ),
    (
        PermissionsErrorCode::OwnershipUpdateFailed,
        &["failed to change owner/group"],
    ),
    (
        PermissionsErrorCode::PermissionsUpdateFailed,
        &[
            "failed to update permissions",
            "setentriesinaclw failed",
            "setnamedsecurityinfow failed",
        ],
    ),
    (
        PermissionsErrorCode::PostChangeSnapshotFailed,
        &[
            "failed to capture post-change permissions",
            "failed to capture post-change ownership",
        ],
    ),
    (PermissionsErrorCode::RollbackFailed, &["rollback failed"]),
];
