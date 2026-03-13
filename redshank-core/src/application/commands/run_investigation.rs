//! `RunInvestigation` command — starts an agent investigation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::agent::AgentConfig;

/// Newtype for idempotency keys (UUID v4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey(pub Uuid);

impl IdempotencyKey {
    /// Generate a new random idempotency key.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for IdempotencyKey {
    fn default() -> Self {
        Self::new()
    }
}

/// Command to start an investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInvestigationCommand {
    /// Idempotency key to prevent duplicate execution.
    pub idempotency_key: IdempotencyKey,
    /// Investigation goal / prompt.
    pub goal: String,
    /// Agent configuration.
    pub config: AgentConfig,
}

// TODO(T15): Implement RunInvestigationHandler
