//! `SessionRuntime` — convenience façade over session CQRS handlers.
//!
//! Provides a single entry point for session CRUD operations, delegating to
//! the corresponding query/command handlers.

use crate::application::commands::delete_session::DeleteSessionHandler;
use crate::application::commands::run_investigation::IdempotencyKey;
use crate::application::queries::get_session::{GetSessionHandler, GetSessionQuery};
use crate::application::queries::list_sessions::ListSessionsHandler;
use crate::domain::agent::AgentSession;
use crate::domain::auth::{AuthContext, StaticPolicy, can_delete_session, can_read_session};
use crate::domain::errors::DomainError;
use crate::domain::session::SessionId;
use crate::ports::session_store::SessionStore;

/// Orchestration service for session lifecycle management.
///
/// Wraps the individual CQRS handlers behind a single struct so callers
/// (e.g. CLI or the TUI runtime) don't have to instantiate each handler
/// separately.
pub struct SessionRuntime<S> {
    store: S,
    policy: StaticPolicy,
}

impl<S: SessionStore> SessionRuntime<S> {
    /// Create a new runtime backed by the given session store.
    #[must_use]
    pub const fn new(store: S) -> Self {
        Self {
            store,
            policy: StaticPolicy,
        }
    }

    /// List all sessions visible to the caller.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadSession`
    /// permission, or a storage error if the query fails.
    pub async fn list_sessions(&self, auth: AuthContext) -> Result<Vec<AgentSession>, DomainError> {
        can_read_session(&auth, &self.policy).map_err(DomainError::Security)?;
        self.store.list(&auth).await
    }

    /// Get a specific session by ID.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadSession`
    /// permission, or a storage error if the query fails.
    pub async fn get_session(
        &self,
        auth: AuthContext,
        session_id: SessionId,
    ) -> Result<Option<AgentSession>, DomainError> {
        // Reuse the handler directly for full idempotency/auth logic.
        let query = GetSessionQuery { session_id, auth };
        // Inline the handler logic to avoid cloning the store.
        can_read_session(&query.auth, &self.policy).map_err(DomainError::Security)?;
        self.store.load(&query.auth, query.session_id).await
    }

    /// Delete a session by ID.
    ///
    /// Uses a fresh [`IdempotencyKey`] for each call.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `DeleteSession`
    /// permission, or a storage error if the delete fails.
    pub async fn delete_session(
        &self,
        auth: AuthContext,
        session_id: SessionId,
    ) -> Result<(), DomainError> {
        // Idempotency key is ephemeral here (no retry scenario in interactive CLI).
        let idem = IdempotencyKey::new();
        can_delete_session(&auth, &self.policy).map_err(DomainError::Security)?;
        self.store.delete(&auth, session_id).await?;
        self.store.set_idempotency_key(&idem.0, "deleted").await
    }
}

// ── Constructor helpers that delegate to CQRS handlers ──────

/// Build a [`GetSessionHandler`] backed by the given store.
///
/// Useful when callers need the full handler rather than the runtime façade.
pub const fn get_session_handler<S: SessionStore>(store: S) -> GetSessionHandler<S> {
    GetSessionHandler::new(store)
}

/// Build a [`ListSessionsHandler`] backed by the given store.
pub const fn list_sessions_handler<S: SessionStore>(store: S) -> ListSessionsHandler<S> {
    ListSessionsHandler::new(store)
}

/// Build a [`DeleteSessionHandler`] backed by the given store.
pub const fn delete_session_handler<S: SessionStore>(store: S) -> DeleteSessionHandler<S> {
    DeleteSessionHandler::new(store)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::auth::UserId;

    fn owner() -> AuthContext {
        AuthContext::owner(UserId::new(), "test-token".into())
    }

    #[test]
    fn session_runtime_module_compiles() {
        // Smoke test — verifies public API is accessible.
        // Full integration tests live in redshank-core/tests/integration.rs.
        let _ = owner();
    }
}
