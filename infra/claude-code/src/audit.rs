//! Local audit persistence for Claude Code consultations.
//!
//! The raw CLI envelope is preserved verbatim for audit. Audit write failure
//! fails the invocation: an unaudited consultation is not a successful one.
//! Scrubbed credential *names* are recorded; their values never are.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditRecord {
    pub schema_version: &'static str,
    pub correlation_id: String,
    pub consultation_id: String,
    pub participant_id: String,
    pub executable: String,
    pub cli_version: String,
    pub auth_mode: String,
    pub working_directory: String,
    pub args: Vec<String>,
    /// Names only. Values are never recorded.
    pub scrubbed_environment_variables: Vec<String>,
    pub denied_tools: Vec<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub cancelled: bool,
    /// Raw stdout envelope, preserved for audit.
    pub raw_stdout: String,
    pub raw_stderr: String,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    WriteFailed,
    SerializeFailed,
}

pub trait AuditSink: Send + Sync {
    fn persist(&self, record: &AuditRecord) -> Result<(), AuditError>;
}

/// Writes one JSON file per consultation, named by correlation ID.
pub struct FileAuditSink {
    directory: PathBuf,
}

impl FileAuditSink {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
        }
    }
}

impl AuditSink for FileAuditSink {
    fn persist(&self, record: &AuditRecord) -> Result<(), AuditError> {
        std::fs::create_dir_all(&self.directory).map_err(|_| AuditError::WriteFailed)?;
        let body = serde_json::to_string_pretty(record).map_err(|_| AuditError::SerializeFailed)?;
        let path = self
            .directory
            .join(format!("{}.json", sanitize(&record.correlation_id)));
        std::fs::write(path, body).map_err(|_| AuditError::WriteFailed)
    }
}

/// Correlation IDs are adapter-generated, but never build a path from an
/// unsanitized string.
fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlation_id_cannot_escape_the_audit_directory() {
        assert_eq!(sanitize("../../etc/passwd"), "______etc_passwd");
        assert_eq!(sanitize("maia-cc-123_45"), "maia-cc-123_45");
    }
}
