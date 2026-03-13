//! `RunInvestigation` command — starts an agent investigation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::agent::AgentConfig;
use crate::domain::auth::AuthContext;
use crate::domain::session::SessionId;

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

impl std::fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Command to start an investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInvestigationCommand {
    /// Idempotency key to prevent duplicate execution.
    pub idempotency_key: IdempotencyKey,
    /// Session ID for the investigation.
    pub session_id: SessionId,
    /// Investigation objective / prompt.
    pub objective: String,
    /// Agent configuration.
    pub config: AgentConfig,
    /// Caller's auth context.
    pub auth: AuthContext,
}

// TODO(T15): Implement RunInvestigationHandler

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_investigation_command_roundtrip_serde() {
        let cmd = RunInvestigationCommand {
            idempotency_key: IdempotencyKey::new(),
            session_id: SessionId::new(),
            objective: "investigate entity X".to_string(),
            config: AgentConfig::default(),
            auth: AuthContext::system(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let restored: RunInvestigationCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.objective, "investigate entity X");
        assert_eq!(restored.idempotency_key, cmd.idempotency_key);
    }

    #[test]
    fn idempotency_key_display() {
        let key = IdempotencyKey::new();
        let display = format!("{key}");
        assert!(!display.is_empty());
    }
}
