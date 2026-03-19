//! `RunInvestigation` command and handler.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapters::tool_defs::tool_definitions;
use crate::application::services::engine::RLMEngine;
use crate::domain::agent::{AgentConfig, AgentSession};
use crate::domain::auth::{AuthContext, StaticPolicy, can_run_agent};
use crate::domain::errors::DomainError;
use crate::domain::session::SessionId;
use crate::ports::model_provider::ModelProvider;
use crate::ports::replay_log::ReplayLog;
use crate::ports::session_store::SessionStore;
use crate::ports::tool_dispatcher::ToolDispatcher;

/// Newtype for idempotency keys (UUID v4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey(pub Uuid);

impl IdempotencyKey {
    /// Generate a new random idempotency key.
    #[must_use]
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

/// Handles the [`RunInvestigationCommand`].
///
/// Steps:
/// 1. Check idempotency key — return cached result on duplicate.
/// 2. Enforce `RunAgent` permission via the security policy.
/// 3. Create and persist an [`AgentSession`].
/// 4. Build an [`RLMEngine`] and call `solve()`.
/// 5. Persist `AgentCompleted`/`InvestigationFailed` event and update session.
/// 6. Record the idempotency key with the result.
pub struct RunInvestigationHandler<S> {
    session_store: S,
    policy: StaticPolicy,
}

impl<S: SessionStore> RunInvestigationHandler<S> {
    /// Create a new handler with the given session store.
    #[must_use]
    pub const fn new(session_store: S) -> Self {
        Self {
            session_store,
            policy: StaticPolicy,
        }
    }

    /// Execute the investigation command.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::DuplicateOperation`] if the idempotency key has
    /// already been seen, [`DomainError::Security`] if the caller lacks
    /// `RunAgent` permission, or any engine / persistence error that occurs
    /// during the run.
    pub async fn handle<M, D, R>(
        &self,
        cmd: RunInvestigationCommand,
        model: M,
        tools: D,
        replay_log: R,
    ) -> Result<String, DomainError>
    where
        M: ModelProvider,
        D: ToolDispatcher,
        R: ReplayLog,
    {
        // 1 — Idempotency: return cached result if key was seen before.
        if let Some(cached) = self
            .session_store
            .check_idempotency_key(&cmd.idempotency_key.0)
            .await?
        {
            return Ok(cached);
        }

        // 2 — Auth check.
        can_run_agent(&cmd.auth, &self.policy).map_err(DomainError::Security)?;

        // 3 — Create and persist session.
        let mut session = AgentSession::create(cmd.config.clone());
        session.start(cmd.objective.clone());
        self.session_store.save(&cmd.auth, &session).await?;
        for event in session.drain_events() {
            self.session_store
                .append_event(&cmd.auth, session.session_id, event)
                .await?;
        }

        // 4 — Build engine and solve.
        let tool_defs = tool_definitions(cmd.config.recursive);
        let engine = RLMEngine::new(cmd.config, model, tools, replay_log);
        let result = engine.solve(&cmd.objective, &tool_defs).await;

        // 5 — Update session and persist outcome event.
        match &result {
            Ok(answer) => session.complete(answer.clone()),
            Err(err) => session.fail(err.to_string()),
        }
        self.session_store.save(&cmd.auth, &session).await?;
        for event in session.drain_events() {
            self.session_store
                .append_event(&cmd.auth, session.session_id, event)
                .await?;
        }

        // 6 — Record idempotency key.
        let result_str = result.as_deref().unwrap_or("").to_owned();
        self.session_store
            .set_idempotency_key(&cmd.idempotency_key.0, &result_str)
            .await?;

        result
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
