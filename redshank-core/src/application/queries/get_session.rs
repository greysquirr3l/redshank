//! `GetSession` query.

use serde::{Deserialize, Serialize};

use crate::domain::auth::AuthContext;
use crate::domain::session::SessionId;

/// Query to retrieve a session by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionQuery {
    /// Session ID to look up.
    pub session_id: SessionId,
    /// Caller's auth context.
    pub auth: AuthContext,
}

// TODO(T17): Implement GetSessionHandler
