//! `DeleteSession` command.

use serde::{Deserialize, Serialize};

use super::run_investigation::IdempotencyKey;
use crate::domain::auth::AuthContext;
use crate::domain::session::SessionId;

/// Command to delete a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSessionCommand {
    /// Idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Session ID to delete.
    pub session_id: SessionId,
    /// Caller's auth context.
    pub auth: AuthContext,
}

// TODO(T17): Implement DeleteSessionHandler
