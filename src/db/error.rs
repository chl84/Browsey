use crate::errors::domain::{classify_message_by_patterns, DomainError, ErrorCode};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbErrorCode {
    DataDirUnavailable,
    OpenFailed,
    SchemaInitFailed,
    ReadFailed,
    WriteFailed,
    TransactionFailed,
    SerializeFailed,
    ParseFailed,
    UnknownError,
}

impl ErrorCode for DbErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::DataDirUnavailable => "data_dir_unavailable",
            Self::OpenFailed => "open_failed",
            Self::SchemaInitFailed => "schema_init_failed",
            Self::ReadFailed => "read_failed",
            Self::WriteFailed => "write_failed",
            Self::TransactionFailed => "transaction_failed",
            Self::SerializeFailed => "serialize_failed",
            Self::ParseFailed => "parse_failed",
            Self::UnknownError => "unknown_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DbError {
    code: DbErrorCode,
    message: String,
}

impl DbError {
    pub fn new(code: DbErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn from_external_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = classify_message_by_patterns(
            &message,
            DB_CLASSIFICATION_RULES,
            DbErrorCode::UnknownError,
        );
        Self::new(code, message)
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DbError {}

impl DomainError for DbError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

pub type DbResult<T> = Result<T, DbError>;

const DB_CLASSIFICATION_RULES: &[(DbErrorCode, &[&str])] = &[
    (
        DbErrorCode::DataDirUnavailable,
        &[
            "could not resolve data directory",
            "failed to create data dir",
        ],
    ),
    (DbErrorCode::OpenFailed, &["failed to open db"]),
    (DbErrorCode::SchemaInitFailed, &["failed to init schema"]),
    (DbErrorCode::SerializeFailed, &["failed to serialize"]),
    (DbErrorCode::ParseFailed, &["failed to parse"]),
    (
        DbErrorCode::TransactionFailed,
        &["failed to start transaction", "failed to commit"],
    ),
    (
        DbErrorCode::WriteFailed,
        &[
            "failed to store",
            "failed to insert",
            "failed to delete",
            "failed to upsert",
        ],
    ),
    (
        DbErrorCode::ReadFailed,
        &["failed to read", "failed to query", "failed to prepare"],
    ),
];
