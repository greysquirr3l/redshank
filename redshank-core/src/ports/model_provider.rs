//! `ModelProvider` port — LLM completion and token counting.

use crate::domain::session::{ModelTurn, ToolCall};
use serde::{Deserialize, Serialize};

/// A message in the conversation history sent to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role: "system", "user", "assistant", or "tool".
    pub role: String,
    /// Text content.
    pub content: String,
    /// Tool calls (only for assistant messages).
    pub tool_calls: Vec<ToolCall>,
    /// Tool call ID (only for tool-result messages).
    pub tool_call_id: Option<String>,
}

/// Tool definition sent to the provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for the tool parameters.
    pub parameters: serde_json::Value,
}

/// Response from a model completion call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// The model turn generated.
    pub turn: ModelTurn,
    /// Total tokens used (prompt + completion).
    pub total_tokens: u64,
    /// Prompt tokens used.
    pub prompt_tokens: u64,
    /// Completion tokens used.
    pub completion_tokens: u64,
}

/// Port trait for LLM model providers.
pub trait ModelProvider: Send + Sync {
    /// Send a completion request and return the model's response.
    fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> impl std::future::Future<Output = Result<CompletionResponse, crate::domain::errors::DomainError>> + Send;

    /// Count tokens in the given messages without making an API call.
    fn count_tokens(
        &self,
        messages: &[ChatMessage],
    ) -> Result<u64, crate::domain::errors::DomainError>;

    /// The model's context window size in tokens.
    fn context_window(&self) -> u64;

    /// Human-readable model name.
    fn model_name(&self) -> &str;
}
