//! `GetSession` query and handler.

use serde::{Deserialize, Serialize};

use crate::domain::agent::AgentSession;
use crate::domain::auth::{AuthContext, StaticPolicy, can_read_session};
use crate::domain::errors::DomainError;
use crate::domain::session::SessionId;
use crate::ports::session_store::SessionStore;

/// Query to retrieve a session by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionQuery {
    /// Session ID to look up.
    pub session_id: SessionId,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`GetSessionQuery`].
///
/// Enforces `ReadSession` permission, then delegates to the session store.
pub struct GetSessionHandler<S> {
    session_store: S,
    policy: StaticPolicy,
}

impl<S: SessionStore> GetSessionHandler<S> {
    /// Create a new handler backed by the given session store.
    #[must_use]
    pub const fn new(session_store: S) -> Self {
        Self {
            session_store,
            policy: StaticPolicy,
        }
    }

    /// Execute the query.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadSession`
    /// permission, or a storage error if the lookup fails.
    pub async fn handle(
        &self,
        query: GetSessionQuery,
    ) -> Result<Option<AgentSession>, DomainError> {
        can_read_session(&query.auth, &self.policy).map_err(DomainError::Security)?;
        self.session_store.load(&query.auth, query.session_id).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn get_session_query_roundtrip_serde() {
        let query = GetSessionQuery {
            session_id: SessionId::new(),
            auth: AuthContext::system(),
        };
        let json = serde_json::to_string(&query).unwrap();
        let restored: GetSessionQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.session_id, query.session_id);
    }
}
