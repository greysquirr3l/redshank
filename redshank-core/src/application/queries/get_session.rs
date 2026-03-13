//! `GetSession` query.

use serde::{Deserialize, Serialize};
use crate::domain::session::SessionId;

/// Query to retrieve a session by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionQuery {
    /// Session ID to look up.
    pub session_id: SessionId,
}

// TODO(T17): Implement GetSessionHandler
