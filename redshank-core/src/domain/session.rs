//! Session, turn, tool call, and related value objects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique session identifier (newtype over UUID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    /// Generate a new random session ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StopReason {
    /// Normal end of turn.
    EndTurn,
    /// Model wants to use a tool.
    ToolUse,
    /// Hit token limit.
    MaxTokens,
    /// Hit a stop sequence.
    StopSequence,
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
    /// Output content.
    pub content: String,
    /// Whether the tool execution errored.
    pub is_error: bool,
}

/// A single model turn (response from the model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTurn {
    /// Text content from the model (if any).
    pub content: Option<String>,
    /// Tool calls requested by the model.
    pub tool_calls: Vec<ToolCall>,
    /// Why the model stopped.
    pub stop_reason: StopReason,
}

/// Summary of a turn, used for context condensation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSummary {
    /// Turn number (0-indexed).
    pub turn: u32,
    /// Summary text.
    pub summary: String,
    /// Names of tools used in this turn.
    pub tool_names: Vec<String>,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn session_id_display() {
        let sid = SessionId::new();
        let display = format!("{sid}");
        assert!(!display.is_empty());
        assert_eq!(display, sid.0.to_string());
    }

    #[test]
    fn tool_call_roundtrip_serde() {
        let tc = ToolCall {
            id: "call_123".to_string(),
            name: "web_search".to_string(),
            arguments: serde_json::json!({"query": "test"}),
        };
        let json = serde_json::to_string(&tc).unwrap();
        let restored: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "web_search");
    }

    #[test]
    fn model_turn_roundtrip_serde() {
        let turn = ModelTurn {
            content: Some("I found something".to_string()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        };
        let json = serde_json::to_string(&turn).unwrap();
        let restored: ModelTurn = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn stop_reason_variants_roundtrip() {
        for reason in [
            StopReason::EndTurn,
            StopReason::ToolUse,
            StopReason::MaxTokens,
            StopReason::StopSequence,
        ] {
            let json = serde_json::to_string(&reason).unwrap();
            let restored: StopReason = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, reason);
        }
    }
}
