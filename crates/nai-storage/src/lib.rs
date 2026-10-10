//! Single-owner local persistence. Owner administration is trusted composition
//! code; authenticated actor IDs must never be taken from model claims.
mod budget;
pub use budget::DispatchClaim;
mod decision;
pub use decision::{
    Criticality, DecisionAck, DecisionEntryView, DecisionKind, GENESIS_HASH,
    TELEMETRY_BUFFER_CAPACITY,
};
mod knowledge;
pub use knowledge::{ImportChunk, ImportedDocument, SnapshotPublish, VectorManifest};
mod migration;
mod paths;
mod publication;
mod snapshot;
mod store;

use nai_contracts::v1::ErrorCode;
pub use nai_security::Operation;
pub use paths::DataPaths;
use std::fmt;
pub use store::{ConfigKind, PendingApproval, Store, candidate_hash};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    pub code: ErrorCode,
    pub reason: &'static str,
    /// First divergent sequence number for integrity checks that can locate it.
    pub position: Option<i64>,
}
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "storage: {} ({})", self.code.as_str_name(), self.reason)
    }
}
impl std::error::Error for StorageError {}
impl From<rusqlite::Error> for StorageError {
    /// Classifies by the portable SQLite result code, for the same reason the
    /// `std::io::Error` conversion below classifies by kind: a report has to tell
    /// a contended database from a corrupt or unopenable one without a debug
    /// build. The codes are SQLite's, not the host's, so the vocabulary stays
    /// portable across platforms.
    fn from(sqlite_error: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode as Code;
        let reason = match &sqlite_error {
            rusqlite::Error::SqliteFailure(failure, _) => match failure.code {
                Code::DatabaseBusy | Code::DatabaseLocked => "sqlite busy",
                Code::OutOfMemory => "sqlite out of memory",
                Code::DiskFull => "sqlite storage full",
                Code::SystemIoFailure => "sqlite io failure",
                Code::DatabaseCorrupt => "sqlite database corrupt",
                Code::CannotOpen => "sqlite database unopenable",
                Code::NotADatabase => "sqlite file is not a database",
                Code::ReadOnly | Code::PermissionDenied => "sqlite write refused",
                Code::OperationInterrupted => "sqlite interrupted",
                Code::SchemaChanged => "sqlite schema changed",
                Code::ConstraintViolation => "sqlite constraint violated",
                Code::TypeMismatch => "sqlite type mismatch",
                _ => "sqlite operation failed",
            },
            _ => "sqlite operation failed",
        };
        error(ErrorCode::Internal, reason)
    }
}
impl From<std::io::Error> for StorageError {
    /// Keeps a portable, stable reason. The OS message is localised platform
    /// text, so only the kind is carried: a report must be able to tell a
    /// missing path from a denied one without a debug build.
    fn from(io_error: std::io::Error) -> Self {
        let reason = match io_error.kind() {
            std::io::ErrorKind::NotFound => "filesystem path missing",
            std::io::ErrorKind::PermissionDenied => "filesystem access denied",
            std::io::ErrorKind::AlreadyExists => "filesystem target exists",
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
                "filesystem input unusable"
            }
            std::io::ErrorKind::UnexpectedEof => "filesystem input truncated",
            std::io::ErrorKind::WriteZero => "filesystem write incomplete",
            _ => "filesystem operation failed",
        };
        error(ErrorCode::Internal, reason)
    }
}
impl From<nai_security::SecurityError> for StorageError {
    fn from(e: nai_security::SecurityError) -> Self {
        error(e.code, "operation stopped")
    }
}
pub type Result<T> = std::result::Result<T, StorageError>;
fn error(code: ErrorCode, reason: &'static str) -> StorageError {
    StorageError {
        code,
        reason,
        position: None,
    }
}
fn error_at(code: ErrorCode, reason: &'static str, position: i64) -> StorageError {
    StorageError {
        code,
        reason,
        position: Some(position),
    }
}
fn hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn sql_int(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| error(ErrorCode::InvalidArgument, "integer exceeds SQLite range"))
}
fn utc_ms() -> Result<i64> {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| error(ErrorCode::Internal, "invalid UTC clock"))?
            .as_millis(),
    )
    .map_err(|_| error(ErrorCode::Internal, "UTC clock exceeds range"))
}
fn id(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        Err(error(
            ErrorCode::InvalidArgument,
            "invalid scoped identifier",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
