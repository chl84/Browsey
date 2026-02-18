use crate::errors::api_error::ApiError;

const SET_HIDDEN_CLASSIFICATION_RULES: &[(&str, &[&str])] = &[
    ("path_not_absolute", &["path must be absolute"]),
    (
        "invalid_path",
        &[
            "parent directory components are not allowed",
            "invalid path component (nul byte)",
            "path contains nul byte",
            "unsupported path prefix",
            "invalid file name",
            "cannot derive visible name",
            "missing parent",
        ],
    ),
    ("invalid_input", &["no paths provided"]),
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
    ("target_exists", &["target already exists"]),
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
    (
        "hidden_update_failed",
        &[
            "setfileattributes failed",
            "getfileattributes failed",
            "failed to rename",
        ],
    ),
];

fn classify_set_hidden_error_code(message: &str) -> &'static str {
    let normalized = message.to_ascii_lowercase();
    for &(code, patterns) in SET_HIDDEN_CLASSIFICATION_RULES {
        if patterns.iter().any(|pattern| normalized.contains(pattern)) {
            return code;
        }
    }
    "unknown_error"
}

pub(super) fn is_expected_set_hidden_error_code(code: &str) -> bool {
    matches!(
        code,
        "symlink_unsupported" | "not_found" | "permission_denied" | "target_exists"
    )
}

pub(super) fn to_set_hidden_api_error(message: impl Into<String>) -> ApiError {
    let message = message.into();
    ApiError::new(classify_set_hidden_error_code(&message), message)
}
