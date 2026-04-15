//! `RunInvestigation` command and handler.

use std::io::BufRead;
use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapters::tool_defs::tool_definitions;
use crate::application::queries::pol_analytics::{PolAnalyticsHandler, PolAnalyticsQuery};
use crate::application::queries::recall_entity_observations::{
    RecallEntityObservationsHandler, RecallEntityObservationsQuery,
};
use crate::application::queries::recall_observations::{
    RecallObservationsHandler, RecallObservationsQuery,
};
use crate::application::services::engine::{ExternalContext, RLMEngine};
use crate::domain::agent::{AgentConfig, AgentSession};
use crate::domain::auth::{AuthContext, StaticPolicy, can_run_agent};
use crate::domain::errors::DomainError;
use crate::domain::observation::EntityObservation;
use crate::domain::session::SessionId;
use crate::ports::model_provider::ModelProvider;
use crate::ports::observation_store::ObservationStore;
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
/// 5. Ingest observation sidecars into the temporal store.
/// 6. Persist `AgentCompleted`/`InvestigationFailed` event and update session.
/// 7. Record the idempotency key with the result.
pub struct RunInvestigationHandler<S> {
    session_store: S,
    policy: StaticPolicy,
}

impl<S: SessionStore + ObservationStore> RunInvestigationHandler<S> {
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

        // Seed context with prior observations from the recent event log.
        let recall = RecallObservationsHandler::new(&self.session_store);
        let recall_query = RecallObservationsQuery {
            session_id: cmd.session_id,
            since: Utc::now() - Duration::days(14),
            max_items: 24,
            auth: cmd.auth.clone(),
        };

        let mut initial_context = ExternalContext::default();
        initial_context.add_identity(format!("investigation objective: {}", cmd.objective));
        for line in recall.handle(recall_query).await? {
            initial_context.add_on_demand(line);
        }

        // Augment L2 context with the cross-entity PoL observation timeline.
        let entity_recall = RecallEntityObservationsHandler::new(&self.session_store);
        let entity_recall_query = RecallEntityObservationsQuery {
            since: Utc::now() - Duration::days(14),
            max_items: 48,
            auth: cmd.auth.clone(),
        };
        for line in entity_recall.handle(entity_recall_query).await? {
            initial_context.add_on_demand(line);
        }

        // Add PoL analytics summaries (state-change frequency, trends).
        let analytics = PolAnalyticsHandler::new(&self.session_store);
        let analytics_query = PolAnalyticsQuery {
            entity_id: None,
            since: Utc::now() - Duration::days(14),
            auth: cmd.auth.clone(),
        };
        for line in analytics.handle(analytics_query).await? {
            initial_context.add_on_demand(line);
        }

        let result = engine
            .solve_with_context(&cmd.objective, &tool_defs, initial_context)
            .await;

        // 5 — Import observation sidecars produced by fetchers during this run.
        let _ingested = self
            .ingest_observation_sidecars(&cmd.auth, &session.config.workspace)
            .await?;

        // 6 — Update session and persist outcome event.
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

        // 7 — Record idempotency key.
        let result_str = result.as_deref().unwrap_or("").to_owned();
        self.session_store
            .set_idempotency_key(&cmd.idempotency_key.0, &result_str)
            .await?;

        result
    }

    async fn ingest_observation_sidecars(
        &self,
        auth: &AuthContext,
        workspace: &Path,
    ) -> Result<usize, DomainError> {
        let sidecars = find_observation_sidecars(workspace)?;
        let mut ingested = 0_usize;

        for sidecar in sidecars {
            let observations = read_observations_from_sidecar(&sidecar)?;
            for observation in observations {
                self.session_store
                    .append_observation(auth, &observation)
                    .await?;
                ingested += 1;
            }
        }

        Ok(ingested)
    }
}

fn find_observation_sidecars(root: &Path) -> Result<Vec<PathBuf>, DomainError> {
    let mut results = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| DomainError::Other(format!("read_dir {}: {e}", dir.display())))?;

        for entry in entries {
            let entry = entry.map_err(|e| DomainError::Other(format!("read_dir entry: {e}")))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| DomainError::Other(format!("file_type {}: {e}", path.display())))?;

            if file_type.is_dir() {
                let name = entry.file_name();
                if matches!(name.to_str(), Some(".git" | "target")) {
                    continue;
                }
                stack.push(path);
                continue;
            }

            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.ends_with("_observations.ndjson") {
                results.push(path);
            }
        }
    }

    Ok(results)
}

fn read_observations_from_sidecar(path: &Path) -> Result<Vec<EntityObservation>, DomainError> {
    let file = std::fs::File::open(path)
        .map_err(|e| DomainError::Other(format!("open sidecar {}: {e}", path.display())))?;
    let reader = std::io::BufReader::new(file);

    let mut observations = Vec::new();
    for line_result in reader.lines() {
        let line = line_result
            .map_err(|e| DomainError::Other(format!("read sidecar {}: {e}", path.display())))?;
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(observation) = serde_json::from_str::<EntityObservation>(&line) {
            observations.push(observation);
        }
    }

    Ok(observations)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;

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

    #[test]
    fn find_observation_sidecars_discovers_recursive_matches() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();

        let sidecar = nested.join("blockchain_explorer_observations.ndjson");
        std::fs::write(&sidecar, "").unwrap();

        let found = find_observation_sidecars(root).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found.first(), Some(&sidecar));
    }

    #[test]
    fn read_observations_from_sidecar_skips_invalid_lines() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sidecar = temp_dir
            .path()
            .join("blockchain_explorer_observations.ndjson");
        let observation = EntityObservation::new(
            "ethereum:0xabc".to_owned(),
            "blockchain_explorer".to_owned(),
            Utc::now(),
            "abc12345".to_owned(),
            crate::domain::observation::ObservationDelta::New,
        );

        let mut file = std::fs::File::create(&sidecar).unwrap();
        serde_json::to_writer(&mut file, &observation).unwrap();
        writeln!(&mut file).unwrap();
        writeln!(&mut file, "this is not json").unwrap();

        let observations = read_observations_from_sidecar(&sidecar).unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(
            observations.first().map(|obs| obs.entity_id.as_str()),
            Some("ethereum:0xabc")
        );
    }
}
