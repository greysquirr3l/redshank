//! `SessionStore` port — session persistence.

use crate::domain::auth::AuthContext;
use crate::domain::errors::DomainError;
use crate::domain::events::DomainEvent;
use crate::domain::session::{Session, SessionId};
use uuid::Uuid;

/// Port trait for session persistence. All methods require `AuthContext`.
pub trait SessionStore: Send + Sync {
    /// Save a new session.
    fn save(
        &self,
        auth: &AuthContext,
        session: &Session,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;

    /// Load a session by ID.
    fn load(
        &self,
        auth: &AuthContext,
        id: SessionId,
    ) -> impl std::future::Future<Output = Result<Option<Session>, DomainError>> + Send;

    /// List all sessions visible to the caller.
    fn list(
        &self,
        auth: &AuthContext,
    ) -> impl std::future::Future<Output = Result<Vec<Session>, DomainError>> + Send;

    /// Delete a session by ID.
    fn delete(
        &self,
        auth: &AuthContext,
        id: SessionId,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;

    /// Append a domain event to the session's event log.
    fn append_event(
        &self,
        auth: &AuthContext,
        session_id: SessionId,
        event: DomainEvent,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;

    /// Check if an idempotency key has been used.
    fn check_idempotency_key(
        &self,
        key: &Uuid,
    ) -> impl std::future::Future<Output = Result<Option<String>, DomainError>> + Send;

    /// Record an idempotency key with its result.
    fn set_idempotency_key(
        &self,
        key: &Uuid,
        result: &str,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;
}
