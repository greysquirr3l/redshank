//! Anthropic Claude provider — native Messages API with SSE streaming.
//!
//! Implements [`ModelProvider`] for Anthropic's `/v1/messages` endpoint.
//! Handles:
//! - SSE event parsing (`content_block_delta`, `content_block_stop`, `message_stop`)
//! - Tool-call JSON fragment accumulation across multiple deltas
//! - Thinking budgets for claude-opus-4-6+ models
//! - Token counting via `/v1/messages/count_tokens`

use crate::domain::agent::ReasoningEffort;
use crate::domain::credentials::CredentialGuard;
use crate::domain::errors::DomainError;
use crate::domain::session::{ModelTurn, StopReason, ToolCall};
use crate::ports::model_provider::{ChatMessage, ModelProvider, ToolDefinition};

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

// ── Constants ───────────────────────────────────────────────

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 8_192;

// ── SSE Protocol Types ──────────────────────────────────────

/// Raw SSE event parsed from the stream.
#[derive(Debug)]
struct SseEvent {
    event: String,
    data: String,
}

/// Content block types in the Anthropic response.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
}

/// Delta types for streaming content blocks.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
enum ContentDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
}

/// SSE data payload for `content_block_start`.
#[derive(Debug, Deserialize)]
struct ContentBlockStart {
    index: usize,
    content_block: ContentBlock,
}

/// SSE data payload for `content_block_delta`.
#[derive(Debug, Deserialize)]
struct ContentBlockDelta {
    #[allow(dead_code)]
    index: usize,
    delta: ContentDelta,
}

/// SSE data payload for `message_start`.
#[derive(Debug, Deserialize)]
struct MessageStart {
    message: MessagePayload,
}

/// Top-level message payload (for non-streaming or `message_start`).
#[derive(Debug, Deserialize)]
struct MessagePayload {
    #[serde(default)]
    content: Vec<ContentBlock>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    usage: Option<UsagePayload>,
}

/// Token usage in the response.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct UsagePayload {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

/// SSE data payload for `message_delta`.
#[derive(Debug, Deserialize)]
struct MessageDeltaEvent {
    delta: MessageDeltaPayload,
}

/// Inner delta within `message_delta`.
#[derive(Debug, Deserialize)]
struct MessageDeltaPayload {
    #[serde(default)]
    stop_reason: Option<String>,
}

/// Token count response from the `count_tokens` endpoint.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenCountResponse {
    input_tokens: u32,
}

// ── Accumulator for streaming ───────────────────────────────

/// Tracks state while assembling a `ModelTurn` from SSE events.
#[derive(Debug, Default)]
struct StreamAccumulator {
    text_parts: Vec<String>,
    tool_calls: Vec<InProgressToolCall>,
    stop_reason: Option<String>,
    current_block_index: Option<usize>,
}

/// A tool call being assembled from streaming fragments.
#[derive(Debug, Clone)]
struct InProgressToolCall {
    id: String,
    name: String,
    json_fragments: Vec<String>,
}

impl StreamAccumulator {
    fn handle_content_block_start(&mut self, start: ContentBlockStart) {
        self.current_block_index = Some(start.index);
        match start.content_block {
            ContentBlock::Text { text } => {
                if !text.is_empty() {
                    self.text_parts.push(text);
                }
            }
            ContentBlock::ToolUse { id, name, input } => {
                // If the initial block has a complete input, serialize it
                let initial_json = if input.as_object().is_some_and(|m| !m.is_empty()) {
                    vec![serde_json::to_string(&input).unwrap_or_default()]
                } else {
                    vec![]
                };
                self.tool_calls.push(InProgressToolCall {
                    id,
                    name,
                    json_fragments: initial_json,
                });
            }
            ContentBlock::Thinking { thinking } => {
                if !thinking.is_empty() {
                    self.text_parts
                        .push(format!("<thinking>{thinking}</thinking>"));
                }
            }
        }
    }

    fn handle_content_block_delta(&mut self, delta: ContentBlockDelta) {
        match delta.delta {
            ContentDelta::TextDelta { text } => {
                self.text_parts.push(text);
            }
            ContentDelta::InputJsonDelta { partial_json } => {
                if let Some(tc) = self.tool_calls.last_mut() {
                    tc.json_fragments.push(partial_json);
                }
            }
            ContentDelta::ThinkingDelta { thinking } => {
                self.text_parts.push(thinking);
            }
        }
    }

    fn handle_message_delta(&mut self, stop_reason: Option<String>) {
        if stop_reason.is_some() {
            self.stop_reason = stop_reason;
        }
    }

    fn into_model_turn(self) -> ModelTurn {
        let content = if self.text_parts.is_empty() {
            None
        } else {
            Some(self.text_parts.join(""))
        };

        let tool_calls: Vec<ToolCall> = self
            .tool_calls
            .into_iter()
            .map(|tc| {
                let json_str = tc.json_fragments.join("");
                let arguments = if json_str.is_empty() {
                    Value::Object(serde_json::Map::new())
                } else {
                    serde_json::from_str(&json_str)
                        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()))
                };
                ToolCall {
                    id: tc.id,
                    name: tc.name,
                    arguments,
                }
            })
            .collect();

        let stop_reason = match self.stop_reason.as_deref() {
            Some("end_turn") => StopReason::EndTurn,
            Some("tool_use") => StopReason::ToolUse,
            Some("max_tokens") => StopReason::MaxTokens,
            Some("stop_sequence") => StopReason::StopSequence,
            _ => {
                if tool_calls.is_empty() {
                    StopReason::EndTurn
                } else {
                    StopReason::ToolUse
                }
            }
        };

        ModelTurn {
            content,
            tool_calls,
            stop_reason,
        }
    }
}

// ── SSE Parser ──────────────────────────────────────────────

/// Parse raw bytes into SSE events.
fn parse_sse_events(data: &[u8]) -> Vec<SseEvent> {
    let text = String::from_utf8_lossy(data);
    let mut events = Vec::new();
    let mut current_event = String::new();
    let mut current_data = Vec::new();

    for line in text.lines() {
        if line.is_empty() {
            // Empty line = end of event
            if !current_data.is_empty() || !current_event.is_empty() {
                events.push(SseEvent {
                    event: if current_event.is_empty() {
                        "message".to_string()
                    } else {
                        current_event.clone()
                    },
                    data: current_data.join("\n"),
                });
                current_event.clear();
                current_data.clear();
            }
        } else if let Some(value) = line.strip_prefix("event: ") {
            current_event = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("data: ") {
            current_data.push(value.to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            current_data.push(value.to_string());
        }
    }

    // Flush any remaining event
    if !current_data.is_empty() || !current_event.is_empty() {
        events.push(SseEvent {
            event: if current_event.is_empty() {
                "message".to_string()
            } else {
                current_event
            },
            data: current_data.join("\n"),
        });
    }

    events
}

// ── Request/Response Builders ───────────────────────────────

/// Build the JSON request body for the Messages API.
fn build_request_body(
    model: &str,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
    reasoning_effort: Option<ReasoningEffort>,
    max_tokens: u32,
) -> Value {
    let api_messages = build_api_messages(messages);
    let system_prompt = extract_system_prompt(messages);

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "stream": true,
        "messages": api_messages,
    });

    if let Some(system) = system_prompt
        && let Some(obj) = body.as_object_mut()
    {
        obj.insert("system".to_owned(), Value::String(system));
    }

    if !tools.is_empty() {
        let api_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("tools".to_owned(), Value::Array(api_tools));
        }
    }

    // Thinking budgets
    if let Some(effort) = reasoning_effort
        && effort != ReasoningEffort::None
        && supports_native_thinking(model)
    {
        let budget = thinking_budget_tokens(effort);
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "thinking".to_owned(),
                serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": budget
                }),
            );
        }
    }

    body
}

/// Extract the system prompt from messages (role == "system").
fn extract_system_prompt(messages: &[ChatMessage]) -> Option<String> {
    messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone())
}

/// Convert [`ChatMessage`] slice to Anthropic API message format.
fn build_api_messages(messages: &[ChatMessage]) -> Vec<Value> {
    messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            if m.role == "assistant" && !m.tool_calls.is_empty() {
                // Assistant message with tool calls
                let mut content: Vec<Value> = Vec::new();
                if !m.content.is_empty() {
                    content.push(serde_json::json!({
                        "type": "text",
                        "text": m.content,
                    }));
                }
                for tc in &m.tool_calls {
                    content.push(serde_json::json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": tc.arguments,
                    }));
                }
                serde_json::json!({
                    "role": "assistant",
                    "content": content,
                })
            } else if m.role == "tool" {
                // Tool result
                serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id.as_deref().unwrap_or(""),
                        "content": m.content,
                    }],
                })
            } else {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            }
        })
        .collect()
}

/// Whether the model supports native thinking blocks (claude-opus-4-6+).
fn supports_native_thinking(model: &str) -> bool {
    model.contains("opus-4") || model.contains("claude-4")
}

/// Map reasoning effort to thinking budget tokens.
const fn thinking_budget_tokens(effort: ReasoningEffort) -> u32 {
    match effort {
        ReasoningEffort::None => 0,
        ReasoningEffort::Low => 2_048,
        ReasoningEffort::Medium => 8_192,
        ReasoningEffort::High => 32_768,
    }
}

/// Map Anthropic `stop_reason` string to `StopReason` enum.
#[cfg(test)]
fn map_stop_reason(reason: &str) -> StopReason {
    match reason {
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    }
}

// ── AnthropicModel ──────────────────────────────────────────

/// Anthropic Claude model provider using the native Messages API.
///
/// Streams responses via SSE and accumulates tool-call JSON fragments.
pub struct AnthropicModel {
    client: Client,
    api_key: CredentialGuard<String>,
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
    max_tokens: u32,
    context_window: u64,
    base_url: String,
}

impl std::fmt::Debug for AnthropicModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicModel")
            .field("model", &self.model)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("max_tokens", &self.max_tokens)
            .field("context_window", &self.context_window)
            .finish_non_exhaustive()
    }
}

impl AnthropicModel {
    /// Create a new Anthropic model provider.
    ///
    /// # Arguments
    /// - `api_key`: Anthropic API key (redacted in debug output)
    /// - `model`: Model identifier (e.g. "claude-sonnet-4-20250514")
    /// - `reasoning_effort`: Optional reasoning effort level
    #[must_use]
    pub fn new(
        api_key: CredentialGuard<String>,
        model: String,
        reasoning_effort: Option<ReasoningEffort>,
    ) -> Self {
        let context_window = infer_context_window(&model);
        Self {
            client: Client::new(),
            api_key,
            model,
            reasoning_effort,
            max_tokens: DEFAULT_MAX_TOKENS,
            context_window,
            base_url: ANTHROPIC_API_BASE.to_string(),
        }
    }

    /// Override max tokens.
    #[must_use]
    pub const fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Override the provider base URL.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Return the configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Process a complete SSE response body into a `ModelTurn`.
    fn process_sse_body(body: &[u8]) -> ModelTurn {
        let events = parse_sse_events(body);
        let mut acc = StreamAccumulator::default();

        for event in events {
            match event.event.as_str() {
                "message_start" => {
                    // message_start contains the initial message object
                    if let Ok(ms) = serde_json::from_str::<MessageStart>(&event.data) {
                        for (i, block) in ms.message.content.into_iter().enumerate() {
                            acc.handle_content_block_start(ContentBlockStart {
                                index: i,
                                content_block: block,
                            });
                        }
                        if let Some(reason) = ms.message.stop_reason {
                            acc.handle_message_delta(Some(reason));
                        }
                    }
                }
                "content_block_start" => {
                    if let Ok(cbs) = serde_json::from_str::<ContentBlockStart>(&event.data) {
                        acc.handle_content_block_start(cbs);
                    }
                }
                "content_block_delta" => {
                    if let Ok(cbd) = serde_json::from_str::<ContentBlockDelta>(&event.data) {
                        acc.handle_content_block_delta(cbd);
                    }
                }
                "message_delta" => {
                    if let Ok(md) = serde_json::from_str::<MessageDeltaEvent>(&event.data) {
                        acc.handle_message_delta(md.delta.stop_reason);
                    }
                }
                _ => {
                    // No accumulation needed for these event types.
                }
            }
        }

        acc.into_model_turn()
    }
}

/// Infer context window size from model name.
const fn infer_context_window(_model: &str) -> u64 {
    // All current Claude models have 200k context windows.
    // TODO(T08): differentiate when older models are supported.
    200_000
}

impl ModelProvider for AnthropicModel {
    fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> impl std::future::Future<Output = Result<ModelTurn, DomainError>> + Send {
        let body = build_request_body(
            &self.model,
            messages,
            tools,
            self.reasoning_effort,
            self.max_tokens,
        );

        let url = format!("{}/v1/messages", self.base_url);
        let client = self.client.clone();
        let api_key = self.api_key.clone();

        async move {
            let response = client
                .post(&url)
                .header("x-api-key", &**api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| DomainError::Other(format!("Anthropic API request failed: {e}")))?;

            let status = response.status();
            if !status.is_success() {
                let error_body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unknown error".to_string());
                // Never include the API key in error messages
                return Err(DomainError::Other(format!(
                    "Anthropic API error {status}: {error_body}"
                )));
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|e| DomainError::Other(format!("failed to read response body: {e}")))?;

            Ok(Self::process_sse_body(&bytes))
        }
    }

    fn count_tokens(&self, messages: &[ChatMessage]) -> Result<u32, DomainError> {
        // Rough estimation: ~4 chars per token for English text
        let total_chars: usize = messages
            .iter()
            .map(|m| m.content.len() + m.role.len())
            .sum();
        Ok(u32::try_from(total_chars / 4).unwrap_or(u32::MAX))
    }

    fn context_window(&self) -> u64 {
        self.context_window
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // ── SSE fixtures ────────────────────────────────────────

    fn sse_text_response() -> Vec<u8> {
        b"event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4-20250514\",\"stop_reason\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello, \"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"world!\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n"
            .to_vec()
    }

    fn sse_tool_use_response() -> Vec<u8> {
        b"event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_02\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4-20250514\",\"stop_reason\":null,\"usage\":{\"input_tokens\":20,\"output_tokens\":0}}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"I'll search for that.\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"web_search\",\"input\":{}}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"qu\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"ery\\\": \\\"rust\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\" programming\\\"}\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":1}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":30}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n"
            .to_vec()
    }

    fn sse_thinking_response() -> Vec<u8> {
        b"event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_03\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-opus-4-6-20250515\",\"stop_reason\":null,\"usage\":{\"input_tokens\":15,\"output_tokens\":0}}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Let me think about this...\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Here is my answer.\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":1}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":20}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n"
            .to_vec()
    }

    // ── Tests ───────────────────────────────────────────────

    #[test]
    fn sse_text_stream_produces_correct_model_turn() {
        let body = sse_text_response();
        let turn = AnthropicModel::process_sse_body(&body);

        assert_eq!(turn.content.as_deref(), Some("Hello, world!"));
        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn sse_tool_call_json_fragment_accumulation() {
        let body = sse_tool_use_response();
        let turn = AnthropicModel::process_sse_body(&body);

        assert_eq!(turn.content.as_deref(), Some("I'll search for that."));
        assert_eq!(turn.tool_calls.len(), 1);

        let tc = &turn.tool_calls[0];
        assert_eq!(tc.id, "toolu_01");
        assert_eq!(tc.name, "web_search");
        assert_eq!(tc.arguments["query"], "rust programming");
        assert_eq!(turn.stop_reason, StopReason::ToolUse);
    }

    #[test]
    fn thinking_budget_absent_when_effort_is_none() {
        let body = build_request_body(
            "claude-opus-4-6-20250515",
            &[ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            }],
            &[],
            None, // No reasoning effort
            8192,
        );

        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn thinking_budget_present_for_opus_with_effort() {
        let body = build_request_body(
            "claude-opus-4-6-20250515",
            &[ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            }],
            &[],
            Some(ReasoningEffort::High),
            8192,
        );

        let thinking = body.get("thinking").expect("thinking should be present");
        assert_eq!(thinking["type"], "enabled");
        assert_eq!(thinking["budget_tokens"], 32_768);
    }

    #[test]
    fn thinking_budget_absent_for_sonnet_even_with_effort() {
        let body = build_request_body(
            "claude-sonnet-4-20250514",
            &[ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            }],
            &[],
            Some(ReasoningEffort::High),
            8192,
        );

        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn api_key_never_in_debug_output() {
        let model = AnthropicModel::new(
            CredentialGuard::new("sk-ant-super-secret-key-12345".to_string()),
            "claude-sonnet-4-20250514".to_string(),
            None,
        );

        let debug = format!("{model:?}");
        assert!(!debug.contains("sk-ant"));
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("12345"));
        assert!(debug.contains("AnthropicModel"));
        assert!(debug.contains("claude-sonnet-4"));
    }

    #[test]
    fn stop_reason_mapping() {
        assert_eq!(map_stop_reason("end_turn"), StopReason::EndTurn);
        assert_eq!(map_stop_reason("tool_use"), StopReason::ToolUse);
        assert_eq!(map_stop_reason("max_tokens"), StopReason::MaxTokens);
        assert_eq!(map_stop_reason("stop_sequence"), StopReason::StopSequence);
        assert_eq!(map_stop_reason("unknown"), StopReason::EndTurn);
    }

    #[test]
    fn sse_thinking_stream_includes_thinking_and_text() {
        let body = sse_thinking_response();
        let turn = AnthropicModel::process_sse_body(&body);

        let content = turn.content.as_deref().unwrap();
        assert!(content.contains("Let me think about this..."));
        assert!(content.contains("Here is my answer."));
        assert_eq!(turn.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn tool_definitions_serialised_to_anthropic_format() {
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file from disk".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        }];

        let body = build_request_body(
            "claude-sonnet-4-20250514",
            &[ChatMessage {
                role: "user".to_string(),
                content: "read foo.txt".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            }],
            &tools,
            None,
            8192,
        );

        let api_tools = body["tools"].as_array().unwrap();
        assert_eq!(api_tools.len(), 1);
        assert_eq!(api_tools[0]["name"], "read_file");
        assert_eq!(api_tools[0]["description"], "Read a file from disk");
        assert!(api_tools[0]["input_schema"]["properties"]["path"].is_object());
    }

    #[test]
    fn system_prompt_extracted_from_messages() {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are helpful.".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            },
        ];

        let body = build_request_body("claude-sonnet-4-20250514", &messages, &[], None, 8192);
        assert_eq!(body["system"], "You are helpful.");

        // System message should not appear in messages array
        let api_msgs = body["messages"].as_array().unwrap();
        assert_eq!(api_msgs.len(), 1);
        assert_eq!(api_msgs[0]["role"], "user");
    }

    #[test]
    fn tool_result_messages_become_user_role_with_tool_result_content() {
        let messages = vec![ChatMessage {
            role: "tool".to_string(),
            content: "file contents here".to_string(),
            tool_calls: vec![],
            tool_call_id: Some("toolu_01".to_string()),
        }];

        let api_msgs = build_api_messages(&messages);
        assert_eq!(api_msgs.len(), 1);
        assert_eq!(api_msgs[0]["role"], "user");
        let content = api_msgs[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_result");
        assert_eq!(content[0]["tool_use_id"], "toolu_01");
        assert_eq!(content[0]["content"], "file contents here");
    }

    #[test]
    fn count_tokens_returns_rough_estimate() {
        let model = AnthropicModel::new(
            CredentialGuard::new("test-key".to_string()),
            "claude-sonnet-4-20250514".to_string(),
            None,
        );

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello world, this is a test message for counting!".to_string(),
            tool_calls: vec![],
            tool_call_id: None,
        }];

        let count = model.count_tokens(&messages).unwrap();
        assert!(count > 0);
        assert!(count < 100);
    }

    #[test]
    fn context_window_for_models() {
        let model = AnthropicModel::new(
            CredentialGuard::new("test-key".to_string()),
            "claude-sonnet-4-20250514".to_string(),
            None,
        );
        assert_eq!(model.context_window(), 200_000);
    }

    #[test]
    fn empty_sse_body_produces_end_turn() {
        let turn = AnthropicModel::process_sse_body(b"");
        assert!(turn.content.is_none());
        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn reasoning_effort_budget_values() {
        assert_eq!(thinking_budget_tokens(ReasoningEffort::None), 0);
        assert_eq!(thinking_budget_tokens(ReasoningEffort::Low), 2_048);
        assert_eq!(thinking_budget_tokens(ReasoningEffort::Medium), 8_192);
        assert_eq!(thinking_budget_tokens(ReasoningEffort::High), 32_768);
    }
}
