//! `ListSessions` query.

use serde::{Deserialize, Serialize};

/// Query to list all sessions visible to the authenticated user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsQuery {
    /// Optional filter by owner user ID.
    pub owner_filter: Option<String>,
}

// TODO(T17): Implement ListSessionsHandler
