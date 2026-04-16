//! OpenAI-compatible provider — handles `OpenAI`, `OpenRouter`, `Cerebras`, `Ollama`.
//!
//! A single [`OpenAICompatibleModel`] struct implements [`ModelProvider`] for all
//! providers that speak the `OpenAI` Chat Completions API (`/v1/chat/completions`).
//!
//! Per-provider differences (base URL, auth headers, timeouts) are configured
//! through the [`for_provider`](OpenAICompatibleModel::for_provider) factory.

use crate::domain::agent::{ProviderKind, ReasoningEffort};
use crate::domain::credentials::CredentialGuard;
use crate::domain::errors::DomainError;
use crate::domain::session::{ModelTurn, StopReason, ToolCall};
use crate::ports::model_provider::{ChatMessage, ModelProvider, ToolDefinition};

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

// ── Constants ───────────────────────────────────────────────

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";
const OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";

/// Ollama can be slow on first inference (loading model).
const OLLAMA_TIMEOUT: Duration = Duration::from_secs(500);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

// ── SSE Protocol Types ──────────────────────────────────────

/// Raw SSE event.
#[derive(Debug)]
struct SseEvent {
    data: String,
}

/// A single SSE chunk from `OpenAI`'s streaming response.
#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Vec<ChunkChoice>,
}

/// A choice within a streaming chunk.
#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

/// Delta content within a streaming choice.
#[derive(Debug, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChunkToolCall>>,
}

/// Tool call delta in a streaming chunk.
#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChunkFunction>,
}

/// Function info within a tool call delta.
#[derive(Debug, Deserialize)]
struct ChunkFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

// ── Stream Accumulator ──────────────────────────────────────

/// Accumulates SSE chunks into a complete [`ModelTurn`].
#[derive(Debug, Default)]
struct StreamAccumulator {
    text_parts: Vec<String>,
    tool_calls: Vec<InProgressToolCall>,
    finish_reason: Option<String>,
}

/// A tool call being assembled from streaming fragments.
#[derive(Debug, Clone)]
struct InProgressToolCall {
    id: String,
    name: String,
    argument_fragments: Vec<String>,
}

impl StreamAccumulator {
    fn handle_chunk(&mut self, chunk: ChatChunk) {
        for choice in chunk.choices {
            if let Some(reason) = choice.finish_reason {
                self.finish_reason = Some(reason);
            }

            if let Some(text) = choice.delta.content {
                self.text_parts.push(text);
            }

            if let Some(tool_calls) = choice.delta.tool_calls {
                for tc_delta in tool_calls {
                    self.handle_tool_call_delta(tc_delta);
                }
            }
        }
    }

    fn handle_tool_call_delta(&mut self, delta: ChunkToolCall) {
        let idx = delta.index;

        // Grow the tool_calls vec if needed
        while self.tool_calls.len() <= idx {
            self.tool_calls.push(InProgressToolCall {
                id: String::new(),
                name: String::new(),
                argument_fragments: Vec::new(),
            });
        }

        let Some(tc) = self.tool_calls.get_mut(idx) else {
            return;
        };

        if let Some(id) = delta.id
            && !id.is_empty()
        {
            tc.id = id;
        }

        if let Some(func) = delta.function {
            if let Some(name) = func.name
                && !name.is_empty()
            {
                tc.name = name;
            }
            if let Some(args) = func.arguments {
                tc.argument_fragments.push(args);
            }
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
            .filter(|tc| !tc.name.is_empty())
            .map(|tc| {
                let json_str = tc.argument_fragments.join("");
                let arguments = if json_str.is_empty() {
                    Value::Object(serde_json::Map::new())
                } else {
                    match serde_json::from_str(&json_str) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!("failed to parse tool arguments from stream: {e}");
                            Value::Object(serde_json::Map::new())
                        }
                    }
                };
                ToolCall {
                    id: tc.id,
                    name: tc.name,
                    arguments,
                }
            })
            .collect();

        let stop_reason = match self.finish_reason.as_deref() {
            Some("stop") => StopReason::EndTurn,
            Some("tool_calls") => StopReason::ToolUse,
            Some("length") => StopReason::MaxTokens,
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

/// Parse raw bytes into SSE data events.
fn parse_sse_events(data: &[u8]) -> Vec<SseEvent> {
    let text = String::from_utf8_lossy(data);
    let mut events = Vec::new();
    let mut current_data = Vec::new();

    for line in text.lines() {
        if line.is_empty() {
            if !current_data.is_empty() {
                let data = current_data.join("\n");
                if data != "[DONE]" {
                    events.push(SseEvent { data });
                }
                current_data.clear();
            }
        } else if let Some(value) = line.strip_prefix("data: ") {
            current_data.push(value.to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            current_data.push(value.to_string());
        }
    }

    // Flush remaining
    if !current_data.is_empty() {
        let data = current_data.join("\n");
        if data != "[DONE]" {
            events.push(SseEvent { data });
        }
    }

    events
}

// ── Request Builders ────────────────────────────────────────

/// Build the JSON request body for the Chat Completions API.
fn build_request_body(
    model: &str,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
    reasoning_effort: Option<ReasoningEffort>,
) -> Value {
    let api_messages: Vec<Value> = messages
        .iter()
        .map(|m| {
            if m.role == "assistant" && !m.tool_calls.is_empty() {
                let tool_calls: Vec<Value> = m
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        {
                            let arguments = match serde_json::to_string(&tc.arguments) {
                                Ok(s) => s,
                                Err(e) => {
                                    tracing::warn!("failed to serialize tool arguments: {e}");
                                    String::new()
                                }
                            };
                            serde_json::json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": arguments,
                                }
                            })
                        }
                    })
                    .collect();
                serde_json::json!({
                    "role": "assistant",
                    "content": if m.content.is_empty() { Value::Null } else { Value::String(m.content.clone()) },
                    "tool_calls": tool_calls,
                })
            } else if m.role == "tool" {
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": m.tool_call_id.as_deref().unwrap_or(""),
                    "content": m.content,
                })
            } else {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            }
        })
        .collect();

    let mut body = serde_json::json!({
        "model": model,
        "stream": true,
        "messages": api_messages,
    });

    if !tools.is_empty() {
        let api_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("tools".to_owned(), Value::Array(api_tools));
        }
    }

    // OpenAI o-series models support reasoning_effort in the request body
    if let Some(effort) = reasoning_effort
        && is_o_series(model)
        && effort != ReasoningEffort::None
    {
        let effort_str = Value::String(match effort {
            ReasoningEffort::Low => "low".to_string(),
            ReasoningEffort::Medium => "medium".to_string(),
            ReasoningEffort::High => "high".to_string(),
            ReasoningEffort::None => unreachable!(),
        });
        if let Some(obj) = body.as_object_mut() {
            obj.insert("reasoning_effort".to_owned(), effort_str);
        }
    }

    body
}

/// Whether the model is an `OpenAI` o-series reasoning model.
fn is_o_series(model: &str) -> bool {
    model.starts_with("o1") || model.starts_with("o3") || model.starts_with("o4")
}

// ── OpenAICompatibleModel ───────────────────────────────────

/// OpenAI-compatible model provider for `OpenAI`, `OpenRouter`, Cerebras, and Ollama.
pub struct OpenAICompatibleModel {
    client: Client,
    base_url: String,
    api_key: CredentialGuard<String>,
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
    extra_headers: HashMap<String, String>,
    context_window: u64,
}

impl std::fmt::Debug for OpenAICompatibleModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAICompatibleModel")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("context_window", &self.context_window)
            .finish_non_exhaustive()
    }
}

impl OpenAICompatibleModel {
    /// Create a provider for a specific [`ProviderKind`].
    #[must_use]
    pub fn for_provider(
        kind: ProviderKind,
        api_key: CredentialGuard<String>,
        model: String,
        reasoning_effort: Option<ReasoningEffort>,
    ) -> Self {
        let model = if kind == ProviderKind::OpenAiCompatible {
            normalize_ollama_model_name(&model)
        } else {
            model
        };

        let (base_url, extra_headers, timeout, context_window) = match kind {
            ProviderKind::OpenAI => (
                OPENAI_BASE_URL.to_string(),
                HashMap::new(),
                DEFAULT_TIMEOUT,
                128_000,
            ),
            ProviderKind::OpenRouter => {
                let mut headers = HashMap::new();
                headers.insert(
                    "HTTP-Referer".to_string(),
                    "https://redshank.dev".to_string(),
                );
                headers.insert("X-Title".to_string(), "Redshank".to_string());
                (
                    OPENROUTER_BASE_URL.to_string(),
                    headers,
                    DEFAULT_TIMEOUT,
                    128_000,
                )
            }
            ProviderKind::Cerebras => (
                CEREBRAS_BASE_URL.to_string(),
                HashMap::new(),
                DEFAULT_TIMEOUT,
                128_000,
            ),
            ProviderKind::OpenAiCompatible => (
                OLLAMA_BASE_URL.to_string(),
                HashMap::new(),
                OLLAMA_TIMEOUT,
                32_000,
            ),
            // Anthropic uses its own provider (T06)
            ProviderKind::Anthropic => (
                "https://api.anthropic.com/v1".to_string(),
                HashMap::new(),
                DEFAULT_TIMEOUT,
                200_000,
            ),
        };

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url,
            api_key,
            model,
            reasoning_effort,
            extra_headers,
            context_window,
        }
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

    /// Process a complete SSE response body into a [`ModelTurn`].
    fn process_sse_body(body: &[u8]) -> ModelTurn {
        let events = parse_sse_events(body);
        let mut acc = StreamAccumulator::default();

        for event in events {
            if let Ok(chunk) = serde_json::from_str::<ChatChunk>(&event.data) {
                acc.handle_chunk(chunk);
            }
        }

        acc.into_model_turn()
    }
}

fn normalize_ollama_model_name(model: &str) -> String {
    model.strip_prefix("ollama/").unwrap_or(model).to_string()
}

impl ModelProvider for OpenAICompatibleModel {
    fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> impl std::future::Future<Output = Result<ModelTurn, DomainError>> + Send {
        let body = build_request_body(&self.model, messages, tools, self.reasoning_effort);
        let url = format!("{}/chat/completions", self.base_url);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let extra_headers = self.extra_headers.clone();
        let is_ollama = self.base_url == OLLAMA_BASE_URL;
        let reasoning_effort = self.reasoning_effort;
        let model_name = self.model.clone();

        async move {
            match send_chat_completion_request(&client, &url, &api_key, &extra_headers, &body).await
            {
                Ok(bytes) => Ok(Self::process_sse_body(&bytes)),
                Err((status, error_body))
                    if is_ollama
                        && body.get("tools").is_some()
                        && status == reqwest::StatusCode::BAD_REQUEST
                        && error_body.contains("does not support tools") =>
                {
                    let fallback_body =
                        build_request_body(&model_name, messages, &[], reasoning_effort);
                    let bytes = send_chat_completion_request(
                        &client,
                        &url,
                        &api_key,
                        &extra_headers,
                        &fallback_body,
                    )
                    .await
                    .map_err(|(status, error_body)| {
                        DomainError::Other(format!("API error {status}: {error_body}"))
                    })?;

                    Ok(Self::process_sse_body(&bytes))
                }
                Err((status, error_body)) => Err(DomainError::Other(format!(
                    "API error {status}: {error_body}"
                ))),
            }
        }
    }

    fn count_tokens(&self, messages: &[ChatMessage]) -> Result<u32, DomainError> {
        // Rough estimation: ~4 chars per token
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

async fn send_chat_completion_request(
    client: &Client,
    url: &str,
    api_key: &CredentialGuard<String>,
    extra_headers: &HashMap<String, String>,
    body: &Value,
) -> Result<Vec<u8>, (reqwest::StatusCode, String)> {
    let mut request = client.post(url).header("Content-Type", "application/json");

    if !api_key.expose().trim().is_empty() {
        request = request.header("Authorization", format!("Bearer {}", api_key.expose()));
    }

    for (key, value) in extra_headers {
        request = request.header(key.as_str(), value.as_str());
    }

    let response = request.json(body).send().await.map_err(|e| {
        (
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            format!("API request failed: {e}"),
        )
    })?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err((status, error_body));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|e| {
            (
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read response body: {e}"),
            )
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // ── SSE fixtures ────────────────────────────────────────

    fn sse_text_response() -> Vec<u8> {
        b"data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\", world!\"},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\
\n\
data: [DONE]\n\
\n"
            .to_vec()
    }

    fn sse_tool_call_response() -> Vec<u8> {
        b"data: {\"id\":\"chatcmpl-2\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":null,\"tool_calls\":[{\"index\":0,\"id\":\"call_abc\",\"type\":\"function\",\"function\":{\"name\":\"web_search\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-2\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"qu\"}}]},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-2\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"ery\\\": \\\"rust\"}}]},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-2\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\" lang\\\"}\"}}]},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-2\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\
\n\
data: [DONE]\n\
\n"
            .to_vec()
    }

    fn sse_multi_tool_response() -> Vec<u8> {
        b"data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":null,\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"path\\\": \\\"a.txt\\\"}\"}}]},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_2\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"function\":{\"arguments\":\"{\\\"path\\\": \\\"b.txt\\\"}\"}}]},\"finish_reason\":null}]}\n\
\n\
data: {\"id\":\"chatcmpl-3\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\
\n\
data: [DONE]\n\
\n"
            .to_vec()
    }

    // ── Tests ───────────────────────────────────────────────

    #[test]
    fn openrouter_includes_referer_and_title_headers() {
        let model = OpenAICompatibleModel::for_provider(
            ProviderKind::OpenRouter,
            CredentialGuard::new("test-key".to_string()),
            "openai/gpt-4o".to_string(),
            None,
        );

        assert_eq!(
            model.extra_headers.get("HTTP-Referer").map(String::as_str),
            Some("https://redshank.dev")
        );
        assert_eq!(
            model.extra_headers.get("X-Title").map(String::as_str),
            Some("Redshank")
        );
    }

    #[test]
    fn ollama_defaults_to_localhost() {
        let model = OpenAICompatibleModel::for_provider(
            ProviderKind::OpenAiCompatible,
            CredentialGuard::new(String::new()),
            "llama3".to_string(),
            None,
        );

        assert_eq!(model.base_url, "http://localhost:11434/v1");
    }

    #[test]
    fn ollama_model_prefix_is_stripped() {
        let model = OpenAICompatibleModel::for_provider(
            ProviderKind::OpenAiCompatible,
            CredentialGuard::new(String::new()),
            "ollama/gemma3:27b".to_string(),
            None,
        );

        assert_eq!(model.model_name(), "gemma3:27b");
    }

    #[test]
    fn reasoning_effort_absent_for_non_o_series() {
        let body = build_request_body(
            "gpt-4o",
            &[ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            }],
            &[],
            Some(ReasoningEffort::High),
        );

        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn reasoning_effort_present_for_o_series() {
        let body = build_request_body(
            "o3-mini",
            &[ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            }],
            &[],
            Some(ReasoningEffort::High),
        );

        assert_eq!(body["reasoning_effort"], "high");
    }

    #[test]
    fn sse_text_stream_produces_correct_model_turn() {
        let body = sse_text_response();
        let turn = OpenAICompatibleModel::process_sse_body(&body);

        assert_eq!(turn.content.as_deref(), Some("Hello, world!"));
        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn sse_tool_call_json_fragment_accumulation() {
        let body = sse_tool_call_response();
        let turn = OpenAICompatibleModel::process_sse_body(&body);

        assert!(turn.content.is_none());
        assert_eq!(turn.tool_calls.len(), 1);

        let tc = &turn.tool_calls[0];
        assert_eq!(tc.id, "call_abc");
        assert_eq!(tc.name, "web_search");
        assert_eq!(tc.arguments["query"], "rust lang");
        assert_eq!(turn.stop_reason, StopReason::ToolUse);
    }

    #[test]
    fn sse_multiple_tool_calls_accumulated_by_index() {
        let body = sse_multi_tool_response();
        let turn = OpenAICompatibleModel::process_sse_body(&body);

        assert_eq!(turn.tool_calls.len(), 2);

        assert_eq!(turn.tool_calls[0].id, "call_1");
        assert_eq!(turn.tool_calls[0].name, "read_file");
        assert_eq!(turn.tool_calls[0].arguments["path"], "a.txt");

        assert_eq!(turn.tool_calls[1].id, "call_2");
        assert_eq!(turn.tool_calls[1].name, "read_file");
        assert_eq!(turn.tool_calls[1].arguments["path"], "b.txt");

        assert_eq!(turn.stop_reason, StopReason::ToolUse);
    }

    #[test]
    fn api_key_never_in_debug_output() {
        let model = OpenAICompatibleModel::for_provider(
            ProviderKind::OpenAI,
            CredentialGuard::new("sk-super-secret-key".to_string()),
            "gpt-4o".to_string(),
            None,
        );

        let debug = format!("{model:?}");
        assert!(!debug.contains("sk-super"));
        assert!(!debug.contains("secret"));
        assert!(debug.contains("OpenAICompatibleModel"));
    }

    #[test]
    fn tool_definitions_serialised_to_openai_format() {
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        }];

        let body = build_request_body(
            "gpt-4o",
            &[ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_calls: vec![],
                tool_call_id: None,
            }],
            &tools,
            None,
        );

        let api_tools = body["tools"].as_array().unwrap();
        assert_eq!(api_tools.len(), 1);
        assert_eq!(api_tools[0]["type"], "function");
        assert_eq!(api_tools[0]["function"]["name"], "read_file");
    }

    #[test]
    fn empty_sse_body_produces_end_turn() {
        let turn = OpenAICompatibleModel::process_sse_body(b"");
        assert_eq!(turn.content, None);
        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn system_messages_preserved_in_openai_format() {
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

        let body = build_request_body("gpt-4o", &messages, &[], None);
        let api_msgs = body["messages"].as_array().unwrap();
        assert_eq!(api_msgs.len(), 2);
        assert_eq!(api_msgs[0]["role"], "system");
        assert_eq!(api_msgs[1]["role"], "user");
    }

    #[test]
    fn done_marker_ignored_in_sse_parsing() {
        let events = parse_sse_events(b"data: {\"test\":true}\n\ndata: [DONE]\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"test\":true}");
    }
}
