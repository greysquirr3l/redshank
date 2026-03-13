//! `DeleteSession` command.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::run_investigation::IdempotencyKey;

/// Command to delete a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSessionCommand {
    /// Idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Session UUID to delete.
    pub session_id: Uuid,
}

// TODO(T17): Implement DeleteSessionHandler
