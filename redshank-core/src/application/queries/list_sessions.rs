//! `ListSessions` query and handler.

use serde::{Deserialize, Serialize};

use crate::domain::agent::AgentSession;
use crate::domain::auth::{AuthContext, StaticPolicy, can_read_session};
use crate::domain::errors::DomainError;
use crate::ports::session_store::SessionStore;

/// Query to list all sessions visible to the authenticated user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsQuery {
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`ListSessionsQuery`].
///
/// Enforces `ReadSession` permission, then delegates to the session store.
pub struct ListSessionsHandler<S> {
    session_store: S,
    policy: StaticPolicy,
}

impl<S: SessionStore> ListSessionsHandler<S> {
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
    pub async fn handle(&self, query: ListSessionsQuery) -> Result<Vec<AgentSession>, DomainError> {
        can_read_session(&query.auth, &self.policy).map_err(DomainError::Security)?;
        self.session_store.list(&query.auth).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn list_sessions_query_roundtrip_serde() {
        let query = ListSessionsQuery {
            auth: AuthContext::system(),
        };
        let json = serde_json::to_string(&query).unwrap();
        let restored: ListSessionsQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.auth.user_id, query.auth.user_id,);
    }
}
