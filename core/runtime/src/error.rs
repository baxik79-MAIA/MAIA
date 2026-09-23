use maia_domain::DomainError;
use maia_store::StoreError;

/// Runtime failures are deliberately small and typed.  CAS races are a normal
/// application outcome; adapter-specific details remain behind `StoreError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    Conflict,
    InvalidInput(DomainError),
    Store(StoreError),
}

impl From<DomainError> for RuntimeError {
    fn from(value: DomainError) -> Self {
        Self::InvalidInput(value)
    }
}

impl From<StoreError> for RuntimeError {
    fn from(value: StoreError) -> Self {
        match value {
            StoreError::ExternalConflict | StoreError::Duplicate => Self::Conflict,
            StoreError::InvalidInput(error) => Self::InvalidInput(error),
            other => Self::Store(other),
        }
    }
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
