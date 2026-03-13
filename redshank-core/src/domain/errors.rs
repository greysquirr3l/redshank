//! Domain error hierarchy.

use super::auth::SecurityError;
use thiserror::Error;

/// Top-level domain error type.
#[derive(Debug, Error)]
pub enum DomainError {
    /// A security policy check failed.
    #[error(transparent)]
    Security(#[from] SecurityError),

    /// A referenced entity was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// A business rule was violated.
    #[error("validation error: {0}")]
    Validation(String),

    /// Duplicate idempotency key — operation already executed.
    #[error("duplicate operation: idempotency key {0} already exists")]
    DuplicateOperation(String),

    /// Generic domain error.
    #[error("{0}")]
    Other(String),
}
