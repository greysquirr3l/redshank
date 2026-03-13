//! `ListSessions` query.

use serde::{Deserialize, Serialize};

use crate::domain::auth::AuthContext;

/// Query to list all sessions visible to the authenticated user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsQuery {
    /// Caller's auth context.
    pub auth: AuthContext,
}

// TODO(T17): Implement ListSessionsHandler
