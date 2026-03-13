//! Agent configuration value object and `AgentSession` aggregate root.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for a single agent invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Primary model identifier (e.g. `"claude-sonnet-4-20250514"`).
    pub model: String,
    /// Optional cheap judge model for acceptance-criteria evaluation.
    pub judge_model: Option<String>,
    /// Maximum number of tool-calling turns before the agent stops.
    pub max_steps: u32,
    /// Maximum recursion depth for subtask delegation.
    pub max_depth: u32,
    /// Reasoning effort level (0.0–1.0). Provider-specific mapping.
    pub reasoning_effort: f64,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            judge_model: None,
            max_steps: 200,
            max_depth: 3,
            reasoning_effort: 0.8,
        }
    }
}

/// Aggregate root representing a running agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    /// Unique session identifier.
    pub id: Uuid,
    /// Configuration for this session.
    pub config: AgentConfig,
    /// Current depth in the recursion tree (0 = root).
    pub depth: u32,
    /// Parent session ID if this is a subtask.
    pub parent_id: Option<Uuid>,
    /// Pending domain events not yet persisted.
    #[serde(skip)]
    pub pending_events: Vec<super::events::DomainEvent>,
}

impl AgentSession {
    /// Create a new root-level agent session.
    pub fn new(config: AgentConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config,
            depth: 0,
            parent_id: None,
            pending_events: Vec::new(),
        }
    }

    /// Create a child session for subtask delegation.
    pub fn child(&self, config: AgentConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config,
            depth: self.depth + 1,
            parent_id: Some(self.id),
            pending_events: Vec::new(),
        }
    }

    /// Take and drain all pending domain events.
    pub fn take_events(&mut self) -> Vec<super::events::DomainEvent> {
        std::mem::take(&mut self.pending_events)
    }
}
