use crate::errors::api_error::{ApiError, ApiResult};

const CLASSIFICATION_RULES: &[(&str, &[&str])] = &[
    ("path_not_absolute", &["path must be absolute"]),
    (
        "invalid_path",
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
        ],
    ),
    (
        "invalid_input",
        &[
            "no paths provided",
            "no permission changes were provided",
            "no ownership changes were provided",
        ],
    ),
    (
        "root_forbidden",
        &["refusing to operate on filesystem root"],
    ),
    (
        "symlink_unsupported",
        &[
            "symlinks are not allowed in path",
            "symlinks are not allowed:",
            "permissions are not supported on symlinks",
            "ownership changes are not supported on symlinks",
        ],
    ),
    (
        "principal_not_found",
        &["user not found", "group not found"],
    ),
    ("group_unavailable", &["group information is unavailable"]),
    (
        "authentication_cancelled",
        &[
            "authentication was cancelled or denied",
            "request dismissed",
            "cancelled",
        ],
    ),
    (
        "elevated_required",
        &["requires elevated privileges", "pkexec is not installed"],
    ),
    (
        "helper_executable_not_found",
        &["failed to locate browsey executable"],
    ),
    (
        "helper_protocol_error",
        &["failed to serialize helper request", "invalid helper input"],
    ),
    ("helper_start_failed", &["failed to start pkexec"]),
    (
        "helper_io_error",
        &[
            "failed to send helper request",
            "failed reading helper input",
        ],
    ),
    ("helper_wait_failed", &["failed waiting for pkexec helper"]),
    (
        "permission_denied",
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
            "not authorized",
        ],
    ),
    ("read_only_filesystem", &["read-only file system"]),
    ("unsupported_platform", &["not supported on this platform"]),
    (
        "not_found",
        &["path does not exist", "no such file or directory"],
    ),
    (
        "metadata_read_failed",
        &[
            "failed to read metadata",
            "getnamedsecurityinfow failed",
            "getsecuritydescriptordacl failed",
            "getace failed",
            "createwellknownsid failed",
        ],
    ),
    ("ownership_update_failed", &["failed to change owner/group"]),
    (
        "permissions_update_failed",
        &[
            "failed to update permissions",
            "setentriesinaclw failed",
            "setnamedsecurityinfow failed",
        ],
    ),
    (
        "post_change_snapshot_failed",
        &[
            "failed to capture post-change permissions",
            "failed to capture post-change ownership",
        ],
    ),
    ("rollback_failed", &["rollback failed"]),
];

pub(super) fn classify_error_code(message: &str) -> &'static str {
    let normalized = message.to_ascii_lowercase();

    for &(code, patterns) in CLASSIFICATION_RULES {
        if patterns.iter().any(|pattern| normalized.contains(pattern)) {
            return code;
        }
    }

    "unknown_error"
}

pub(super) fn is_expected_batch_error_code(code: &str) -> bool {
    matches!(
        code,
        "symlink_unsupported" | "not_found" | "permission_denied" | "metadata_read_failed"
    )
}

pub(super) fn to_api_error(message: impl Into<String>) -> ApiError {
    let message = message.into();
    ApiError::new(classify_error_code(&message), message)
}

pub(super) fn map_api_result<T>(result: Result<T, String>) -> ApiResult<T> {
    result.map_err(to_api_error)
}
