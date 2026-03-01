use crate::errors::domain::{DomainError, ErrorCode};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryResolverErrorCode {
    InvalidBinaryName,
    ExplicitPathInvalid,
    NotFound,
    NotExecutable,
    MetadataReadFailed,
    CanonicalizeFailed,
}

impl ErrorCode for BinaryResolverErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::InvalidBinaryName => "invalid_binary_name",
            Self::ExplicitPathInvalid => "explicit_path_invalid",
            Self::NotFound => "not_found",
            Self::NotExecutable => "not_executable",
            Self::MetadataReadFailed => "metadata_read_failed",
            Self::CanonicalizeFailed => "canonicalize_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BinaryResolverError {
    code: BinaryResolverErrorCode,
    message: String,
}

impl BinaryResolverError {
    pub fn new(code: BinaryResolverErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_binary_name(name: &str) -> Self {
        Self::new(
            BinaryResolverErrorCode::InvalidBinaryName,
            format!("Invalid binary name: {name}"),
        )
    }

    pub fn explicit_path_invalid(path: &Path) -> Self {
        Self::new(
            BinaryResolverErrorCode::ExplicitPathInvalid,
            format!("Expected an explicit binary path: {}", path.display()),
        )
    }

    pub fn not_found(target: &str) -> Self {
        Self::new(
            BinaryResolverErrorCode::NotFound,
            format!("Binary not found: {target}"),
        )
    }

    pub fn not_executable(path: &Path) -> Self {
        Self::new(
            BinaryResolverErrorCode::NotExecutable,
            format!("Binary is not executable: {}", path.display()),
        )
    }

    pub fn metadata_read_failed(path: &Path, error: std::io::Error) -> Self {
        Self::new(
            BinaryResolverErrorCode::MetadataReadFailed,
            format!(
                "Failed to read binary metadata for {}: {error}",
                path.display()
            ),
        )
    }

    pub fn canonicalize_failed(path: &Path, error: std::io::Error) -> Self {
        Self::new(
            BinaryResolverErrorCode::CanonicalizeFailed,
            format!("Failed to resolve binary path {}: {error}", path.display()),
        )
    }
}

impl fmt::Display for BinaryResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BinaryResolverError {}

impl DomainError for BinaryResolverError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub type BinaryResolverResult<T> = Result<T, BinaryResolverError>;
