//! `RLMEngine` — recursive tool-calling agent loop.
//!
//! Mirrors `agent/engine.py` from the `OpenPlanter` Python implementation.
//! The engine runs a model → tool-dispatch → model loop, supporting:
//!
//! - Step budget (`max_steps`)
//! - Depth-controlled recursion (`max_depth`, `subtask` tool)
//! - Runtime policy enforcement (shell repetition blocking)
//! - Context condensation (triggered by [`ContextTracker`])
//! - Cancellation via [`CancellationToken`]
//! - Replay logging via [`ReplayLog`]

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::application::services::condensation::{ContextTracker, condense_tool_outputs};
use crate::domain::agent::AgentConfig;
use crate::domain::errors::DomainError;
use crate::domain::session::{StopReason, ToolCall, ToolResult};
use crate::ports::model_provider::{ChatMessage, ModelProvider, ToolDefinition};
use crate::ports::replay_log::ReplayLog;
use crate::ports::tool_dispatcher::ToolDispatcher;

#[cfg(feature = "runtime")]
use tokio_util::sync::CancellationToken;

/// Cross-turn observations accumulated during the solve loop.
#[derive(Debug, Clone, Default)]
pub struct ExternalContext {
    identity: Vec<String>,
    essential: Vec<String>,
    on_demand: Vec<String>,
    search: Vec<String>,
}

impl ExternalContext {
    /// Append an observation note.
    pub fn add(&mut self, text: String) {
        self.add_essential(text);
    }

    /// Append stable high-priority context.
    pub fn add_identity(&mut self, text: String) {
        self.identity.push(text);
    }

    /// Append recent context that should be prioritised in prompts.
    pub fn add_essential(&mut self, text: String) {
        self.essential.push(text);
    }

    /// Append medium-priority context retrieved on demand.
    pub fn add_on_demand(&mut self, text: String) {
        self.on_demand.push(text);
    }

    /// Append low-priority search context.
    pub fn add_search(&mut self, text: String) {
        self.search.push(text);
    }

    /// Summary of recent observations for prompt injection.
    #[must_use]
    pub fn summary(&self, max_items: usize, max_chars: usize) -> String {
        if max_items == 0 || max_chars == 0 {
            return "(empty)".to_owned();
        }

        let empty = self.identity.is_empty()
            && self.essential.is_empty()
            && self.on_demand.is_empty()
            && self.search.is_empty();
        if empty {
            return "(empty)".to_owned();
        }

        let identity_items = (max_items / 6).max(1);
        let essential_items = (max_items / 2).max(1);
        let on_demand_items = (max_items / 3).max(1);
        let search_items = (max_items / 4).max(1);

        let identity_budget = (max_chars / 10).max(32);
        let essential_budget = (max_chars * 45 / 100).max(64);
        let on_demand_budget = (max_chars * 30 / 100).max(48);
        let search_budget = (max_chars * 15 / 100).max(32);

        let mut sections = Vec::new();
        if let Some(section) = render_layer(
            "L0:IDENTITY",
            &self.identity,
            identity_items,
            identity_budget,
        ) {
            sections.push(section);
        }
        if let Some(section) = render_layer(
            "L1:ESSENTIAL",
            &self.essential,
            essential_items,
            essential_budget,
        ) {
            sections.push(section);
        }
        if let Some(section) = render_layer(
            "L2:ON_DEMAND",
            &self.on_demand,
            on_demand_items,
            on_demand_budget,
        ) {
            sections.push(section);
        }
        if let Some(section) = render_layer("L3:SEARCH", &self.search, search_items, search_budget)
        {
            sections.push(section);
        }

        let joined = sections.join("\n\n");
        if joined.len() <= max_chars {
            joined
        } else {
            let safe_end = joined.floor_char_boundary(max_chars);
            format!(
                "{}\n...[truncated external context]...",
                &joined[..safe_end]
            )
        }
    }
}

fn render_layer(
    label: &str,
    items: &[String],
    max_items: usize,
    max_chars: usize,
) -> Option<String> {
    if items.is_empty() || max_items == 0 || max_chars == 0 {
        return None;
    }

    let start = items.len().saturating_sub(max_items);
    let recent = items.get(start..).unwrap_or_default();
    let joined = recent.join("\n");
    let compressed = compress_for_context(&joined, max_chars);
    Some(format!("[{label}]\n{compressed}"))
}

fn compress_for_context(input: &str, max_chars: usize) -> String {
    if input.len() <= max_chars {
        return input.to_owned();
    }

    let compact = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() <= max_chars {
        return compact;
    }

    let marker = " [truncated]";
    if marker.len() >= max_chars {
        return marker.trim().to_owned();
    }

    let safe_end = compact.floor_char_boundary(max_chars - marker.len());
    format!("{}{}", &compact[..safe_end], marker)
}

/// Maximum number of times an identical `run_shell` command can be repeated
/// at the same depth before the runtime policy blocks it.
const MAX_SHELL_REPEATS: u32 = 2;

/// Recursive language-model engine.
///
/// Generic over `M` (model provider), `D` (tool dispatcher), and `R` (replay log)
/// because all three port traits use RPITIT and are not dyn-compatible.
pub struct RLMEngine<M, D, R> {
    /// Agent configuration.
    pub config: AgentConfig,
    /// Primary model provider.
    pub model: M,
    /// Tool dispatcher.
    pub tools: D,
    /// Replay logger.
    pub replay_log: R,
    /// Cancellation token.
    #[cfg(feature = "runtime")]
    pub cancel_token: CancellationToken,
    /// Context tracker for condensation decisions.
    pub context_tracker: ContextTracker,
    /// Shell command repetition counter: `(depth, command) → count`.
    shell_command_counts: Mutex<HashMap<(u32, String), u32>>,
}

impl<M: ModelProvider, D: ToolDispatcher, R: ReplayLog> RLMEngine<M, D, R> {
    /// Create a new engine.
    pub fn new(config: AgentConfig, model: M, tools: D, replay_log: R) -> Self {
        let context_tracker = ContextTracker::for_model(&config.model);
        Self {
            config,
            model,
            tools,
            replay_log,
            #[cfg(feature = "runtime")]
            cancel_token: CancellationToken::new(),
            context_tracker,
            shell_command_counts: Mutex::new(HashMap::new()),
        }
    }

    /// Entry point: solve an objective.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the model provider or tool dispatcher fails.
    pub async fn solve(
        &self,
        objective: &str,
        tools: &[ToolDefinition],
    ) -> Result<String, DomainError> {
        self.solve_with_context(objective, tools, ExternalContext::default())
            .await
    }

    /// Entry point with caller-provided context layers.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the model provider or tool dispatcher fails.
    pub async fn solve_with_context(
        &self,
        objective: &str,
        tools: &[ToolDefinition],
        mut context: ExternalContext,
    ) -> Result<String, DomainError> {
        self.solve_recursive(objective, 0, tools, &mut context)
            .await
    }

    /// Recursive solve loop with depth control.
    fn solve_recursive<'a>(
        &'a self,
        objective: &'a str,
        depth: u32,
        tools: &'a [ToolDefinition],
        context: &'a mut ExternalContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, DomainError>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut messages: Vec<ChatMessage> = vec![ChatMessage {
                role: "user".to_owned(),
                content: format!(
                    r#"{{"objective":"{}","depth":{},"max_depth":{},"workspace":"{}","external_context":"{}"}}"#,
                    objective,
                    depth,
                    self.config.max_depth,
                    self.config.workspace.display(),
                    context.summary(12, 8000),
                ),
                tool_calls: Vec::new(),
                tool_call_id: None,
            }];

            for step in 1..=self.config.max_steps {
                // Check cancellation.
                #[cfg(feature = "runtime")]
                if self.cancel_token.is_cancelled() {
                    return Ok("Task cancelled.".to_owned());
                }

                // Call the model.
                let turn = self.model.complete(&messages, tools).await?;

                // Update token tracking.
                let input_tokens = self.model.count_tokens(&messages).unwrap_or(0);
                self.context_tracker
                    .clone()
                    .set_used_tokens(u64::from(input_tokens));

                // Condense if needed.
                if self.context_tracker.should_condense() {
                    condense_tool_outputs(&mut messages);
                }

                // Append assistant message.
                messages.push(ChatMessage {
                    role: "assistant".to_owned(),
                    content: turn.content.clone().unwrap_or_default(),
                    tool_calls: turn.tool_calls.clone(),
                    tool_call_id: None,
                });

                // No tool calls → final answer or empty nudge.
                if turn.tool_calls.is_empty() {
                    if let Some(text) = &turn.content
                        && !text.is_empty()
                    {
                        return Ok(text.clone());
                    }
                    // Empty response — nudge.
                    messages.push(ChatMessage {
                        role: "tool".to_owned(),
                        content: "No tool calls and no text in response. \
                              Please use a tool or provide a final answer."
                            .to_owned(),
                        tool_calls: Vec::new(),
                        tool_call_id: Some("empty".to_owned()),
                    });
                    continue;
                }

                // Dispatch tool calls; returns Some(answer) when done.
                if let Some(answer) = self
                    .process_tool_calls(&mut messages, &turn, depth, tools, context, step)
                    .await?
                {
                    return Ok(answer);
                }
            }

            // Step budget exhausted.
            Ok(format!(
                "Step budget exhausted at depth {depth} for objective: {objective}\n\
             Try a more specific task, higher step budget, or deeper recursion."
            ))
        }) // end Box::pin
    }

    /// Dispatch the tool calls from one model turn, pushing results into `messages`.
    ///
    /// Returns `Ok(Some(answer))` when a final answer is determined, `Ok(None)` to continue.
    ///
    /// # Errors
    ///
    /// Propagates [`DomainError`] from the model provider on the `EndTurn` path exclusively;
    /// tool dispatch errors are converted to error [`ToolResult`] entries instead.
    async fn process_tool_calls<'a>(
        &'a self,
        messages: &mut Vec<ChatMessage>,
        turn: &crate::domain::session::ModelTurn,
        depth: u32,
        tools: &'a [ToolDefinition],
        context: &'a mut ExternalContext,
        step: u32,
    ) -> Result<Option<String>, DomainError> {
        for tc in &turn.tool_calls {
            // Runtime policy: block repeated shell commands.
            if let Some(blocked_msg) = self.runtime_policy_check(&tc.name, &tc.arguments, depth) {
                messages.push(ChatMessage {
                    role: "tool".to_owned(),
                    content: blocked_msg,
                    tool_calls: Vec::new(),
                    tool_call_id: Some(tc.id.clone()),
                });
                continue;
            }

            // Subtask handling.
            if tc.name == "subtask" {
                let result = self.handle_subtask(tc, depth, tools, context).await;
                if !result.is_error {
                    context.add(format!("[depth {depth} subtask] {}", &result.content));
                }
                messages.push(ChatMessage {
                    role: "tool".to_owned(),
                    content: result.content,
                    tool_calls: Vec::new(),
                    tool_call_id: Some(tc.id.clone()),
                });
                continue;
            }

            // Regular tool dispatch.
            let auth = crate::domain::auth::AuthContext::system();
            let raw = self
                .tools
                .dispatch(&auth, &tc.name, tc.arguments.clone())
                .await
                .unwrap_or_else(|e| ToolResult {
                    call_id: tc.id.clone(),
                    content: format!("Tool error: {e}"),
                    is_error: true,
                });
            let observation = clip_observation(&raw.content, self.config.max_observation_chars);
            context.add(format!(
                "[depth {depth} step {step}] {}",
                &observation[..observation.len().min(200)]
            ));
            messages.push(ChatMessage {
                role: "tool".to_owned(),
                content: observation,
                tool_calls: Vec::new(),
                tool_call_id: Some(tc.id.clone()),
            });
        }

        // Final answer check: EndTurn with non-empty text.
        if turn.stop_reason == StopReason::EndTurn
            && let Some(text) = &turn.content
            && !text.is_empty()
        {
            return Ok(Some(text.clone()));
        }
        Ok(None)
    }

    /// Handle a subtask tool call by recursing.
    async fn handle_subtask(
        &self,
        tc: &ToolCall,
        depth: u32,
        tools: &[ToolDefinition],
        context: &mut ExternalContext,
    ) -> ToolResult {
        if depth >= u32::from(self.config.max_depth) {
            return ToolResult {
                call_id: tc.id.clone(),
                content: format!(
                    "Maximum recursion depth ({}) reached. Cannot create subtask.",
                    self.config.max_depth
                ),
                is_error: true,
            };
        }

        let sub_objective = tc
            .arguments
            .get("objective")
            .and_then(|v| v.as_str())
            .unwrap_or("(no objective)")
            .to_owned();

        match self
            .solve_recursive(&sub_objective, depth + 1, tools, context)
            .await
        {
            Ok(result) => ToolResult {
                call_id: tc.id.clone(),
                content: result,
                is_error: false,
            },
            Err(e) => ToolResult {
                call_id: tc.id.clone(),
                content: format!("Subtask error: {e}"),
                is_error: true,
            },
        }
    }

    /// Runtime policy: block identical `run_shell` commands repeated more than twice.
    fn runtime_policy_check(&self, name: &str, args: &Value, depth: u32) -> Option<String> {
        if name != "run_shell" {
            return None;
        }
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_owned();
        if command.is_empty() {
            return None;
        }
        let key = (depth, command);
        let mut counts = self
            .shell_command_counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = counts.entry(key).or_insert(0);
        *count += 1;
        let over_limit = *count > MAX_SHELL_REPEATS;
        drop(counts);
        if over_limit {
            Some(
                "Blocked by runtime policy: identical run_shell command repeated more than twice \
                 at the same depth. Change strategy instead of retrying the same command."
                    .to_owned(),
            )
        } else {
            None
        }
    }
}

/// Truncate an observation to at most `max_chars` bytes.
///
/// Uses [`str::floor_char_boundary`] to avoid splitting multi-byte
/// UTF-8 codepoints.
fn clip_observation(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_owned()
    } else {
        let safe_end = text.floor_char_boundary(max_chars);
        format!(
            "{}\n...[truncated {} chars]...",
            &text[..safe_end],
            text.len() - safe_end
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::auth::AuthContext;
    use crate::domain::session::{ModelTurn, StopReason};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ── Scripted model ──────────────────────────────────────────────────────

    /// A scripted model that returns pre-programmed turns in sequence.
    struct ScriptedModel {
        turns: Vec<ModelTurn>,
        call_idx: AtomicUsize,
        context_window: u64,
    }

    impl ScriptedModel {
        fn new(turns: Vec<ModelTurn>) -> Self {
            Self {
                turns,
                call_idx: AtomicUsize::new(0),
                context_window: 200_000,
            }
        }
    }

    impl ModelProvider for ScriptedModel {
        async fn complete(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
        ) -> Result<ModelTurn, DomainError> {
            let idx = self.call_idx.fetch_add(1, Ordering::Relaxed);
            self.turns
                .get(idx)
                .cloned()
                .ok_or_else(|| DomainError::Other("scripted model exhausted".into()))
        }

        fn count_tokens(&self, _messages: &[ChatMessage]) -> Result<u32, DomainError> {
            Ok(100)
        }

        fn context_window(&self) -> u64 {
            self.context_window
        }

        fn model_name(&self) -> &'static str {
            "scripted-test"
        }
    }

    // ── Scripted tool dispatcher ────────────────────────────────────────────

    /// A tool dispatcher that returns pre-programmed results.
    struct ScriptedDispatcher {
        results: Vec<ToolResult>,
        call_idx: AtomicUsize,
    }

    impl ScriptedDispatcher {
        fn new(results: Vec<ToolResult>) -> Self {
            Self {
                results,
                call_idx: AtomicUsize::new(0),
            }
        }
    }

    impl ToolDispatcher for ScriptedDispatcher {
        async fn dispatch(
            &self,
            _auth: &AuthContext,
            _tool_name: &str,
            _arguments: Value,
        ) -> Result<ToolResult, DomainError> {
            let idx = self.call_idx.fetch_add(1, Ordering::Relaxed);
            self.results
                .get(idx)
                .cloned()
                .ok_or_else(|| DomainError::Other("scripted dispatcher exhausted".into()))
        }
    }

    // ── No-op replay log ────────────────────────────────────────────────────

    struct NoopReplayLog;

    impl ReplayLog for NoopReplayLog {
        async fn append(&self, _record: &Value) -> Result<(), DomainError> {
            Ok(())
        }

        fn child_path(&self, subtask_id: &str) -> String {
            format!("root/{subtask_id}")
        }
    }

    // ── Helper ──────────────────────────────────────────────────────────────

    fn make_engine(
        model: ScriptedModel,
        dispatcher: ScriptedDispatcher,
    ) -> RLMEngine<ScriptedModel, ScriptedDispatcher, NoopReplayLog> {
        RLMEngine::new(AgentConfig::default(), model, dispatcher, NoopReplayLog)
    }

    // ── Tests ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn final_answer_on_end_turn() {
        let model = ScriptedModel::new(vec![ModelTurn {
            content: Some("The answer is 42.".into()),
            tool_calls: Vec::new(),
            stop_reason: StopReason::EndTurn,
        }]);
        let engine = make_engine(model, ScriptedDispatcher::new(vec![]));

        let result = engine.solve("What is the answer?", &[]).await.unwrap();
        assert_eq!(result, "The answer is 42.");
    }

    #[tokio::test]
    async fn tool_call_then_final_answer() {
        let model = ScriptedModel::new(vec![
            // Step 1: model requests a tool call.
            ModelTurn {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "tc1".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path": "test.txt"}),
                }],
                stop_reason: StopReason::ToolUse,
            },
            // Step 2: model gives final answer.
            ModelTurn {
                content: Some("File contains hello.".into()),
                tool_calls: Vec::new(),
                stop_reason: StopReason::EndTurn,
            },
        ]);
        let dispatcher = ScriptedDispatcher::new(vec![ToolResult {
            call_id: "tc1".into(),
            content: "hello".into(),
            is_error: false,
        }]);
        let engine = make_engine(model, dispatcher);

        let result = engine.solve("Read the file", &[]).await.unwrap();
        assert_eq!(result, "File contains hello.");
    }

    #[tokio::test]
    async fn depth_limit_returns_error() {
        let model = ScriptedModel::new(vec![
            // Model requests a subtask (depth will be 0 → tries depth 1).
            ModelTurn {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "tc1".into(),
                    name: "subtask".into(),
                    arguments: serde_json::json!({"objective": "sub-goal"}),
                }],
                stop_reason: StopReason::ToolUse,
            },
            // After getting subtask result, model gives final answer.
            ModelTurn {
                content: Some("Done with subtask.".into()),
                tool_calls: Vec::new(),
                stop_reason: StopReason::EndTurn,
            },
        ]);
        let config = AgentConfig {
            max_depth: 0, // No recursion allowed.
            ..Default::default()
        };
        let engine = RLMEngine::new(
            config,
            model,
            ScriptedDispatcher::new(vec![]),
            NoopReplayLog,
        );

        let result = engine.solve("Go deep", &[]).await.unwrap();
        assert!(result.contains("Done with subtask"));
    }

    #[tokio::test]
    async fn step_budget_exhaustion() {
        // Model always requests tool calls, never gives final answer.
        let turns: Vec<ModelTurn> = (0..5)
            .map(|i| ModelTurn {
                content: None,
                tool_calls: vec![ToolCall {
                    id: format!("tc{i}"),
                    name: "web_search".into(),
                    arguments: serde_json::json!({"query": "test"}),
                }],
                stop_reason: StopReason::ToolUse,
            })
            .collect();
        let results: Vec<ToolResult> = (0..5)
            .map(|i| ToolResult {
                call_id: format!("tc{i}"),
                content: "search result".into(),
                is_error: false,
            })
            .collect();

        let config = AgentConfig {
            max_steps: 3,
            ..Default::default()
        };
        let engine = RLMEngine::new(
            config,
            ScriptedModel::new(turns),
            ScriptedDispatcher::new(results),
            NoopReplayLog,
        );

        let result = engine.solve("Search forever", &[]).await.unwrap();
        assert!(result.contains("Step budget exhausted"));
    }

    #[tokio::test]
    async fn identical_shell_blocked_after_two() {
        let turns: Vec<ModelTurn> = (0..4)
            .map(|i| ModelTurn {
                content: None,
                tool_calls: vec![ToolCall {
                    id: format!("tc{i}"),
                    name: "run_shell".into(),
                    arguments: serde_json::json!({"command": "echo hello"}),
                }],
                stop_reason: StopReason::ToolUse,
            })
            .chain(std::iter::once(ModelTurn {
                content: Some("Done.".into()),
                tool_calls: Vec::new(),
                stop_reason: StopReason::EndTurn,
            }))
            .collect();

        // Only 2 dispatches needed (third and fourth are blocked by policy).
        let results: Vec<ToolResult> = (0..2)
            .map(|i| ToolResult {
                call_id: format!("tc{i}"),
                content: "hello\n".into(),
                is_error: false,
            })
            .collect();

        let engine = make_engine(ScriptedModel::new(turns), ScriptedDispatcher::new(results));
        let result = engine.solve("Run shell", &[]).await.unwrap();
        assert_eq!(result, "Done.");
    }

    #[cfg(feature = "runtime")]
    #[tokio::test]
    async fn cancel_stops_engine() {
        // Model would loop forever, but we cancel immediately.
        let turns: Vec<ModelTurn> = (0..100)
            .map(|i| ModelTurn {
                content: None,
                tool_calls: vec![ToolCall {
                    id: format!("tc{i}"),
                    name: "web_search".into(),
                    arguments: serde_json::json!({"query": "loop"}),
                }],
                stop_reason: StopReason::ToolUse,
            })
            .collect();
        let results: Vec<ToolResult> = (0..100)
            .map(|i| ToolResult {
                call_id: format!("tc{i}"),
                content: "result".into(),
                is_error: false,
            })
            .collect();

        let engine = make_engine(ScriptedModel::new(turns), ScriptedDispatcher::new(results));
        engine.cancel_token.cancel();

        let result = engine.solve("loop", &[]).await.unwrap();
        assert_eq!(result, "Task cancelled.");
    }

    #[test]
    fn clip_observation_truncates() {
        let long = "a".repeat(200);
        let clipped = clip_observation(&long, 100);
        assert!(clipped.contains("truncated"));
        assert!(clipped.len() < 200);
    }

    #[test]
    fn clip_observation_short_passes_through() {
        let short = "hello";
        assert_eq!(clip_observation(short, 100), "hello");
    }

    #[test]
    fn external_context_summary() {
        let mut ctx = ExternalContext::default();
        assert_eq!(ctx.summary(12, 8000), "(empty)");

        ctx.add("first".into());
        ctx.add("second".into());
        let s = ctx.summary(12, 8000);
        assert!(s.contains("first"));
        assert!(s.contains("second"));
    }

    #[test]
    fn external_context_summary_layers() {
        let mut ctx = ExternalContext::default();
        ctx.add_identity("investigation profile: maritime sanctions".into());
        ctx.add_essential("target shell company appears in two registries".into());
        ctx.add_on_demand("historical ownership chain recovered".into());
        ctx.add_search("open-source mention in local media feed".into());

        let s = ctx.summary(12, 2000);
        assert!(s.contains("L0:IDENTITY"));
        assert!(s.contains("L1:ESSENTIAL"));
        assert!(s.contains("L2:ON_DEMAND"));
        assert!(s.contains("L3:SEARCH"));
    }

    #[test]
    fn external_context_summary_truncates() {
        let mut ctx = ExternalContext::default();
        ctx.add("a".repeat(100));
        let s = ctx.summary(12, 50);
        assert!(s.contains("truncated"));
    }

    #[test]
    fn external_context_summary_compacts_whitespace() {
        let mut ctx = ExternalContext::default();
        ctx.add("first\n\n\nline\n\n second".into());
        let s = ctx.summary(4, 20);
        assert!(s.contains("first line second") || s.contains("truncated"));
    }

    #[test]
    fn runtime_policy_allows_first_two() {
        let engine = make_engine(ScriptedModel::new(vec![]), ScriptedDispatcher::new(vec![]));
        let args = serde_json::json!({"command": "ls -la"});

        assert!(engine.runtime_policy_check("run_shell", &args, 0).is_none());
        assert!(engine.runtime_policy_check("run_shell", &args, 0).is_none());
        assert!(engine.runtime_policy_check("run_shell", &args, 0).is_some());
    }

    #[test]
    fn runtime_policy_different_depth_resets() {
        let engine = make_engine(ScriptedModel::new(vec![]), ScriptedDispatcher::new(vec![]));
        let args = serde_json::json!({"command": "ls -la"});

        assert!(engine.runtime_policy_check("run_shell", &args, 0).is_none());
        assert!(engine.runtime_policy_check("run_shell", &args, 0).is_none());
        // Third at depth 0 → blocked.
        assert!(engine.runtime_policy_check("run_shell", &args, 0).is_some());
        // Different depth → allowed again.
        assert!(engine.runtime_policy_check("run_shell", &args, 1).is_none());
    }

    #[test]
    fn runtime_policy_ignores_non_shell() {
        let engine = make_engine(ScriptedModel::new(vec![]), ScriptedDispatcher::new(vec![]));
        let args = serde_json::json!({"path": "/tmp"});

        for _ in 0..10 {
            assert!(engine.runtime_policy_check("read_file", &args, 0).is_none());
        }
    }
}
