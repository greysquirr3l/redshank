//! Agent configuration, provider kinds, and `AgentSession` aggregate root.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use super::events::DomainEvent;
use super::session::{SessionId, TurnSummary};

/// Unique data source identifier (newtype over string).
///
/// Source IDs are lowercase snake_case, e.g., "fec", "opencorporates", "ofac".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(String);

impl SourceId {
    /// Create a new source ID from a string.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Get the source ID as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// LLM provider kind, inferred from model name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    /// Anthropic Claude models.
    Anthropic,
    /// `OpenAI` GPT models.
    OpenAI,
    /// `OpenRouter` multi-provider gateway.
    OpenRouter,
    /// Cerebras fast inference.
    Cerebras,
    /// Local Ollama server or OpenAI-compatible endpoint.
    OpenAiCompatible,
}

impl ProviderKind {
    /// Infer provider kind from a model name string.
    #[must_use]
    pub fn from_model_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        if lower.starts_with("claude") {
            Some(Self::Anthropic)
        } else if lower.starts_with("gpt")
            || lower.starts_with("o1")
            || lower.starts_with("o3")
            || lower.starts_with("o4")
        {
            Some(Self::OpenAI)
        } else if lower.starts_with("ollama/") {
            Some(Self::OpenAiCompatible)
        } else if lower.starts_with("cerebras/") || lower.starts_with("llama") {
            Some(Self::Cerebras)
        } else if lower.starts_with("openrouter/") || lower.contains('/') {
            Some(Self::OpenRouter)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod provider_kind_tests {
    use super::*;

    #[test]
    fn infer_provider_kind_from_model_name() {
        assert_eq!(ProviderKind::from_model_name("claude-opus"), Some(ProviderKind::Anthropic));
        assert_eq!(ProviderKind::from_model_name("gpt-4"), Some(ProviderKind::OpenAI));
        assert_eq!(ProviderKind::from_model_name("ollama/llama2"), Some(ProviderKind::OpenAiCompatible));
    }
}

/// Reasoning effort level for model requests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReasoningEffort {
    /// No extended reasoning.
    None,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort (default).
    #[default]
    Medium,
    /// High reasoning effort.
    High,
}

/// Serde helper for Duration (seconds).
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.as_secs().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

/// Configuration value object for a single agent invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Working directory for the investigation.
    pub workspace: PathBuf,
    /// LLM provider kind.
    pub provider: ProviderKind,
    /// Model identifier (e.g. `"claude-sonnet-4-20250514"`).
    pub model: String,
    /// Reasoning effort level.
    pub reasoning_effort: ReasoningEffort,
    /// Maximum recursion depth for subtask delegation.
    pub max_depth: u8,
    /// Maximum number of tool-calling turns.
    pub max_steps: u32,
    /// Maximum characters in a tool observation before truncation.
    pub max_observation_chars: usize,
    /// Timeout for shell commands.
    #[serde(with = "duration_serde")]
    pub command_timeout: Duration,
    /// Maximum characters when reading a file.
    pub max_file_chars: usize,
    /// Whether recursion (subtask delegation) is enabled.
    pub recursive: bool,
    /// Whether to run acceptance criteria via judge model.
    pub acceptance_criteria: bool,
    /// Whether demo mode is active (no real API calls).
    pub demo_mode: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            workspace: PathBuf::from("."),
            provider: ProviderKind::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            reasoning_effort: ReasoningEffort::Medium,
            max_depth: 3,
            max_steps: 200,
            max_observation_chars: 16_000,
            command_timeout: Duration::from_secs(120),
            max_file_chars: 32_000,
            recursive: true,
            acceptance_criteria: true,
            demo_mode: false,
        }
    }
}

/// Status of an agent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Session created but not yet started.
    Idle,
    /// Agent is actively running.
    Running,
    /// Investigation completed successfully.
    Completed,
    /// Investigation failed.
    Failed,
}

/// Aggregate root representing an agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    /// Session identifier.
    pub session_id: SessionId,
    /// Configuration for this session.
    pub config: AgentConfig,
    /// Current session status.
    pub status: SessionStatus,
    /// Turn summaries accumulated during the run.
    pub turns: Vec<TurnSummary>,
    /// Pending domain events not yet persisted.
    #[serde(skip)]
    pub pending_events: Vec<DomainEvent>,
}

impl AgentSession {
    /// Create a new session, emitting a `SessionCreated` event.
    #[must_use]
    pub fn create(config: AgentConfig) -> Self {
        let session_id = SessionId::new();
        let mut session = Self {
            session_id,
            config: config.clone(),
            status: SessionStatus::Idle,
            turns: Vec::new(),
            pending_events: Vec::new(),
        };
        session.pending_events.push(DomainEvent::SessionCreated {
            session_id,
            config,
            timestamp: Utc::now(),
        });
        session
    }

    /// Start the investigation with an objective, emitting `AgentStarted`.
    pub fn start(&mut self, objective: String) {
        self.status = SessionStatus::Running;
        self.pending_events.push(DomainEvent::AgentStarted {
            session_id: self.session_id,
            objective,
            timestamp: Utc::now(),
        });
    }

    /// Mark the investigation as completed, emitting `AgentCompleted`.
    pub fn complete(&mut self, result_summary: String) {
        self.status = SessionStatus::Completed;
        self.pending_events.push(DomainEvent::AgentCompleted {
            session_id: self.session_id,
            result_summary,
            timestamp: Utc::now(),
        });
    }

    /// Mark the investigation as failed.
    pub fn fail(&mut self, error: String) {
        self.status = SessionStatus::Failed;
        self.pending_events.push(DomainEvent::InvestigationFailed {
            session_id: self.session_id,
            error,
            timestamp: Utc::now(),
        });
    }

    /// Add a turn summary, emitting `ToolCalled` for each tool used.
    pub fn add_turn(&mut self, summary: TurnSummary, tool_names: &[String]) {
        for name in tool_names {
            self.pending_events.push(DomainEvent::ToolCalled {
                session_id: self.session_id,
                tool_name: name.clone(),
                args_summary: String::new(),
                timestamp: Utc::now(),
            });
        }
        self.turns.push(summary);
    }

    /// Take and drain all pending domain events.
    pub fn drain_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let cfg = AgentConfig::default();
        assert_eq!(cfg.provider, ProviderKind::Anthropic);
        assert_eq!(cfg.model, "claude-sonnet-4-20250514");
        assert_eq!(cfg.reasoning_effort, ReasoningEffort::Medium);
        assert_eq!(cfg.max_depth, 3);
        assert_eq!(cfg.max_steps, 200);
        assert!(cfg.recursive);
        assert!(cfg.acceptance_criteria);
        assert!(!cfg.demo_mode);
    }

    #[test]
    fn provider_kind_from_model_name() {
        assert_eq!(
            ProviderKind::from_model_name("claude-sonnet-4-20250514"),
            Some(ProviderKind::Anthropic)
        );
        assert_eq!(
            ProviderKind::from_model_name("claude-opus-4-20250514"),
            Some(ProviderKind::Anthropic)
        );
        assert_eq!(
            ProviderKind::from_model_name("gpt-4o"),
            Some(ProviderKind::OpenAI)
        );
        assert_eq!(
            ProviderKind::from_model_name("o1-preview"),
            Some(ProviderKind::OpenAI)
        );
        assert_eq!(
            ProviderKind::from_model_name("o3-mini"),
            Some(ProviderKind::OpenAI)
        );
        assert_eq!(
            ProviderKind::from_model_name("openrouter/anthropic/claude"),
            Some(ProviderKind::OpenRouter)
        );
        assert_eq!(
            ProviderKind::from_model_name("ollama/mistral"),
            Some(ProviderKind::OpenAiCompatible)
        );
        assert_eq!(ProviderKind::from_model_name("unknown-model"), None);
    }

    #[test]
    fn agent_config_roundtrip_serde() {
        let cfg = AgentConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.model, cfg.model);
        assert_eq!(restored.max_steps, cfg.max_steps);
        assert_eq!(restored.provider, cfg.provider);
    }

    #[test]
    fn session_create_emits_session_created() {
        let session = AgentSession::create(AgentConfig::default());
        assert_eq!(session.status, SessionStatus::Idle);
        assert_eq!(session.pending_events.len(), 1);
        assert!(matches!(
            &session.pending_events[0],
            DomainEvent::SessionCreated { .. }
        ));
    }

    #[test]
    fn session_drain_events_clears_pending() {
        let mut session = AgentSession::create(AgentConfig::default());
        assert_eq!(session.pending_events.len(), 1);
        let events = session.drain_events();
        assert_eq!(events.len(), 1);
        assert!(session.pending_events.is_empty());
    }

    #[test]
    fn session_start_emits_agent_started() {
        let mut session = AgentSession::create(AgentConfig::default());
        session.start("Investigate entity X".to_string());
        assert_eq!(session.status, SessionStatus::Running);
        assert!(matches!(
            &session.pending_events[1],
            DomainEvent::AgentStarted { .. }
        ));
    }

    #[test]
    fn session_complete_emits_agent_completed() {
        let mut session = AgentSession::create(AgentConfig::default());
        session.start("test".to_string());
        session.complete("found connections".to_string());
        assert_eq!(session.status, SessionStatus::Completed);
        assert!(matches!(
            &session.pending_events[2],
            DomainEvent::AgentCompleted { .. }
        ));
    }

    #[test]
    fn session_add_turn_emits_tool_called() {
        let mut session = AgentSession::create(AgentConfig::default());
        let summary = TurnSummary {
            turn: 0,
            summary: "searched for entity".to_string(),
            tool_names: vec!["web_search".to_string(), "fetch_url".to_string()],
            timestamp: Utc::now(),
        };
        session.add_turn(
            summary,
            &["web_search".to_string(), "fetch_url".to_string()],
        );
        // 1 SessionCreated + 2 ToolCalled
        assert_eq!(session.pending_events.len(), 3);
        assert!(
            matches!(&session.pending_events[1], DomainEvent::ToolCalled { tool_name, .. } if tool_name == "web_search")
        );
        assert!(
            matches!(&session.pending_events[2], DomainEvent::ToolCalled { tool_name, .. } if tool_name == "fetch_url")
        );
    }
}
