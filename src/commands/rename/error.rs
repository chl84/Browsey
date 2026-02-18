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
            "invalid destination path",
        ],
    ),
    ("invalid_input", &["new name cannot be empty"]),
    (
        "root_forbidden",
        &["refusing to operate on filesystem root"],
    ),
    (
        "symlink_unsupported",
        &[
            "symlinks are not allowed in path",
            "symlinks are not allowed:",
        ],
    ),
    (
        "duplicate_source_path",
        &["duplicate source path in request"],
    ),
    (
        "duplicate_target_name",
        &["duplicate target name in request"],
    ),
    (
        "target_exists",
        &[
            "a file or directory with that name already exists",
            "target already exists",
        ],
    ),
    (
        "not_found",
        &["path does not exist", "no such file or directory"],
    ),
    (
        "permission_denied",
        &[
            "permission denied",
            "operation not permitted",
            "access is denied",
        ],
    ),
    ("rollback_failed", &["rollback also failed"]),
    ("rename_failed", &["failed to rename", "cannot rename root"]),
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

pub(super) fn to_api_error(message: impl Into<String>) -> ApiError {
    let message = message.into();
    ApiError::new(classify_error_code(&message), message)
}

pub(super) fn map_api_result<T>(result: Result<T, String>) -> ApiResult<T> {
    result.map_err(to_api_error)
}
