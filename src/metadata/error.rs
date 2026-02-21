use crate::errors::domain::{classify_message_by_patterns, DomainError, ErrorCode};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataErrorCode {
    MetadataReadFailed,
    ArchiveReadFailed,
    PdfiumLoadFailed,
    UnsupportedArchiveVariant,
    UnknownError,
}

impl ErrorCode for MetadataErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::MetadataReadFailed => "metadata_read_failed",
            Self::ArchiveReadFailed => "archive_read_failed",
            Self::PdfiumLoadFailed => "pdfium_load_failed",
            Self::UnsupportedArchiveVariant => "unsupported_archive_variant",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetadataError {
    code: MetadataErrorCode,
    message: String,
}

impl MetadataError {
    pub fn new(code: MetadataErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = classify_message_by_patterns(
            &message,
            METADATA_CLASSIFICATION_RULES,
            MetadataErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MetadataError {}

impl DomainError for MetadataError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<MetadataError> for String {
    fn from(error: MetadataError) -> Self {
        error.to_string()
    }
}

pub type MetadataResult<T> = Result<T, MetadataError>;

const METADATA_CLASSIFICATION_RULES: &[(MetadataErrorCode, &[&str])] = &[
    (
        MetadataErrorCode::MetadataReadFailed,
        &["failed to read metadata"],
    ),
    (
        MetadataErrorCode::ArchiveReadFailed,
        &[
            "failed to open archive",
            "failed to open zip",
            "failed to read zip",
            "failed to read zip entry",
            "failed to iterate tar entries",
            "failed to read tar entry",
            "failed to read tar entry size",
            "failed to read zstd stream",
        ],
    ),
    (MetadataErrorCode::PdfiumLoadFailed, &["pdfium load failed"]),
    (
        MetadataErrorCode::UnsupportedArchiveVariant,
        &["unsupported tar variant"],
    ),
];
