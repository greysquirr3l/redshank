//! Session, turn, tool call, and related value objects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique session identifier (newtype over UUID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    /// Generate a new random session ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Reason the agent stopped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StopReason {
    /// Normal completion — acceptance criteria met.
    Completed,
    /// Step budget exhausted.
    BudgetExhausted,
    /// User or system cancellation.
    Cancelled,
    /// Unrecoverable error.
    Error(String),
}

/// A single tool call made by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call ID from the provider.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// JSON-encoded arguments.
    pub arguments: serde_json::Value,
}

/// Result of executing a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Corresponding tool call ID.
    pub call_id: String,
    /// Whether the tool succeeded.
    pub success: bool,
    /// Output text.
    pub output: String,
}

/// A single model turn (request + response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTurn {
    /// Turn number (0-indexed).
    pub turn: u32,
    /// Model's text response.
    pub response_text: Option<String>,
    /// Tool calls requested by the model.
    pub tool_calls: Vec<ToolCall>,
    /// Results of tool execution.
    pub tool_results: Vec<ToolResult>,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

/// Summary of a condensed portion of conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSummary {
    /// Range of turns summarised.
    pub turn_range: (u32, u32),
    /// The summary text.
    pub summary: String,
    /// Timestamp of summarisation.
    pub timestamp: DateTime<Utc>,
}

/// A complete session record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session identifier.
    pub id: SessionId,
    /// Investigation goal / prompt.
    pub goal: String,
    /// Model turns.
    pub turns: Vec<ModelTurn>,
    /// Turn summaries from context condensation.
    pub summaries: Vec<TurnSummary>,
    /// How the session ended.
    pub stop_reason: Option<StopReason>,
    /// Owner user ID for access control.
    pub owner_user_id: String,
    /// Session creation time.
    pub created_at: DateTime<Utc>,
    /// Last updated time.
    pub updated_at: DateTime<Utc>,
}
