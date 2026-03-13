//! `SessionStore` port — session persistence.

use crate::domain::agent::AgentSession;
use crate::domain::auth::AuthContext;
use crate::domain::errors::DomainError;
use crate::domain::events::DomainEvent;
use crate::domain::session::SessionId;
use uuid::Uuid;

/// Port trait for session persistence. All methods require `AuthContext`.
///
/// Uses RPITIT — not dyn-compatible. Use generics (`T: SessionStore`).
pub trait SessionStore: Send + Sync {
    /// Save or update an agent session.
    fn save(
        &self,
        auth: &AuthContext,
        session: &AgentSession,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;

    /// Load a session by ID.
    fn load(
        &self,
        auth: &AuthContext,
        id: SessionId,
    ) -> impl std::future::Future<Output = Result<Option<AgentSession>, DomainError>> + Send;

    /// List all sessions visible to the caller.
    fn list(
        &self,
        auth: &AuthContext,
    ) -> impl std::future::Future<Output = Result<Vec<AgentSession>, DomainError>> + Send;

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
