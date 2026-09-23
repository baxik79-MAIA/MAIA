use maia_domain::DomainError;

/// Stable persistence-facing errors.  Adapter-specific errors must be mapped
/// into this enum; `rusqlite::Error` and filesystem details never cross the
/// `maia-store` boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    ExternalConflict,
    PersistenceError,
    MigrationError,
    Corruption,
    AuthorityUnavailable,
    AuthorityLost,
    StorageBusy,
    NotFound,
    Duplicate,
    InvalidInput(DomainError),
}

pub type StoreResult<T> = Result<T, StoreError>;

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExternalConflict => f.write_str("external conflict"),
            Self::PersistenceError => f.write_str("persistence error"),
            Self::MigrationError => f.write_str("migration error"),
            Self::Corruption => f.write_str("persistent data corruption"),
            Self::AuthorityUnavailable => f.write_str("writable authority unavailable"),
            Self::AuthorityLost => f.write_str("writable authority lost"),
            Self::StorageBusy => f.write_str("storage busy"),
            Self::NotFound => f.write_str("record not found"),
            Self::Duplicate => f.write_str("record already exists"),
            Self::InvalidInput(error) => write!(f, "invalid input: {error}"),
        }
    }
}

impl std::error::Error for StoreError {}
