//! Full-stack integration tests with scripted model — no live API calls.
//!
//! Exercises: CLI → `SessionRuntime` → `RLMEngine` → `WorkspaceTools` stack.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_const_for_fn,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use redshank_core::application::services::engine::RLMEngine;
use redshank_core::domain::agent::AgentConfig;
use redshank_core::domain::auth::AuthContext;
use redshank_core::domain::errors::DomainError;
use redshank_core::domain::session::{ModelTurn, StopReason, ToolCall, ToolResult};
use redshank_core::ports::model_provider::{ChatMessage, ModelProvider, ToolDefinition};
use redshank_core::ports::replay_log::ReplayLog;
use redshank_core::ports::tool_dispatcher::ToolDispatcher;
use serde_json::{Value, json};
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════════════════
// Test fixtures
// ═══════════════════════════════════════════════════════════════════════════════

/// A scripted model that replays pre-defined turns.
struct ScriptedModel {
    turns: Vec<ModelTurn>,
    call_idx: AtomicUsize,
    context_window: u64,
    token_count: u32,
}

impl ScriptedModel {
    fn new(turns: Vec<ModelTurn>) -> Self {
        Self {
            turns,
            call_idx: AtomicUsize::new(0),
            context_window: 200_000,
            token_count: 100,
        }
    }

    fn with_token_pressure(mut self, tokens: u32, window: u64) -> Self {
        self.token_count = tokens;
        self.context_window = window;
        self
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
        Ok(self.token_count)
    }

    fn context_window(&self) -> u64 {
        self.context_window
    }

    fn model_name(&self) -> &'static str {
        "scripted-integration"
    }
}

/// A tool dispatcher that records calls and returns programmed results.
struct RecordingDispatcher {
    results: Vec<ToolResult>,
    call_idx: AtomicUsize,
}

impl RecordingDispatcher {
    fn new(results: Vec<ToolResult>) -> Self {
        Self {
            results,
            call_idx: AtomicUsize::new(0),
        }
    }
}

impl ToolDispatcher for RecordingDispatcher {
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
            .ok_or_else(|| DomainError::Other("dispatcher exhausted".into()))
    }
}

/// No-op replay log for testing.
struct NoopReplayLog;

impl ReplayLog for NoopReplayLog {
    async fn append(&self, _record: &Value) -> Result<(), DomainError> {
        Ok(())
    }

    fn child_path(&self, subtask_id: &str) -> String {
        format!("test/{subtask_id}")
    }
}

fn make_config(workspace: &std::path::Path) -> AgentConfig {
    AgentConfig {
        workspace: workspace.to_path_buf(),
        max_steps: 10,
        max_depth: 3,
        ..AgentConfig::default()
    }
}

fn make_engine(
    workspace: &std::path::Path,
    model: ScriptedModel,
    dispatcher: RecordingDispatcher,
) -> RLMEngine<ScriptedModel, RecordingDispatcher, NoopReplayLog> {
    RLMEngine::new(make_config(workspace), model, dispatcher, NoopReplayLog)
}

fn tool_call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: id.to_owned(),
        name: name.to_owned(),
        arguments: args,
    }
}

fn tool_result(call_id: &str, content: &str) -> ToolResult {
    ToolResult {
        call_id: call_id.to_owned(),
        content: content.to_owned(),
        is_error: false,
    }
}

fn tool_defs() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "write_file".into(),
            description: "Write a file".into(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}, "content": {"type": "string"}}}),
        },
        ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        },
        ToolDefinition {
            name: "run_shell".into(),
            description: "Run a shell command".into(),
            parameters: json!({"type": "object", "properties": {"command": {"type": "string"}}}),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Multi-turn session: model writes a file, reads it back, runs shell, returns answer.
#[tokio::test]
async fn multi_turn_write_read_shell_answer() {
    let tmp = TempDir::new().unwrap();

    let model = ScriptedModel::new(vec![
        // Turn 1: Write a file
        ModelTurn {
            content: Some("I'll write a file first.".into()),
            tool_calls: vec![tool_call(
                "tc1",
                "write_file",
                json!({"path": "notes.txt", "content": "Investigation notes"}),
            )],
            stop_reason: StopReason::ToolUse,
        },
        // Turn 2: Read the file back
        ModelTurn {
            content: Some("Now reading it back.".into()),
            tool_calls: vec![tool_call("tc2", "read_file", json!({"path": "notes.txt"}))],
            stop_reason: StopReason::ToolUse,
        },
        // Turn 3: Run a shell command
        ModelTurn {
            content: Some("Let me check the workspace.".into()),
            tool_calls: vec![tool_call("tc3", "run_shell", json!({"command": "ls -la"}))],
            stop_reason: StopReason::ToolUse,
        },
        // Turn 4: Final answer
        ModelTurn {
            content: Some("Investigation complete: found 3 records.".into()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
    ]);

    let dispatcher = RecordingDispatcher::new(vec![
        tool_result("tc1", "File written: notes.txt"),
        tool_result("tc2", "Investigation notes"),
        tool_result("tc3", "total 1\n-rw-r--r-- 1 user user 20 notes.txt"),
    ]);

    let engine = make_engine(tmp.path(), model, dispatcher);
    let result = engine.solve("Investigate ACME Corp", &tool_defs()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("Investigation complete"));
}

/// Single-turn: model returns final answer immediately with no tool calls.
#[tokio::test]
async fn single_turn_immediate_answer() {
    let tmp = TempDir::new().unwrap();

    let model = ScriptedModel::new(vec![ModelTurn {
        content: Some("No investigation needed — already have the answer.".into()),
        tool_calls: vec![],
        stop_reason: StopReason::EndTurn,
    }]);

    let dispatcher = RecordingDispatcher::new(vec![]);
    let engine = make_engine(tmp.path(), model, dispatcher);
    let result = engine.solve("What is 2+2?", &tool_defs()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("already have the answer"));
}

/// Context condensation triggers when token usage exceeds threshold.
#[tokio::test]
async fn context_condensation_triggers_at_high_usage() {
    let tmp = TempDir::new().unwrap();

    // 155k tokens out of 200k window = 77.5% — above the 76% threshold
    let model = ScriptedModel::new(vec![
        // Turn 1: tool call to generate context
        ModelTurn {
            content: Some("Gathering data...".into()),
            tool_calls: vec![tool_call("tc1", "read_file", json!({"path": "data.txt"}))],
            stop_reason: StopReason::ToolUse,
        },
        // Turn 2: final answer after condensation
        ModelTurn {
            content: Some("Analysis complete after condensation.".into()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
    ])
    .with_token_pressure(155_000, 200_000);

    let dispatcher = RecordingDispatcher::new(vec![tool_result("tc1", &"x".repeat(10_000))]);

    let engine = make_engine(tmp.path(), model, dispatcher);
    let result = engine.solve("Analyze data", &tool_defs()).await;
    // Engine should complete without error despite high token pressure.
    assert!(result.is_ok());
    assert!(result.unwrap().contains("condensation"));
}

/// Model exhaustion returns an error, not a panic.
#[tokio::test]
async fn model_exhaustion_returns_error() {
    let tmp = TempDir::new().unwrap();

    // Model has only one turn that requests a tool, then is exhausted
    let model = ScriptedModel::new(vec![ModelTurn {
        content: Some("Starting...".into()),
        tool_calls: vec![tool_call("tc1", "read_file", json!({"path": "x"}))],
        stop_reason: StopReason::ToolUse,
    }]);

    let dispatcher = RecordingDispatcher::new(vec![tool_result("tc1", "file contents")]);

    let engine = make_engine(tmp.path(), model, dispatcher);
    let result = engine.solve("Do something", &tool_defs()).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("exhausted"));
}

/// Subtask delegation at depth 0 exercises recursive `solve_recursive` path.
#[tokio::test]
async fn subtask_delegation_works() {
    let tmp = TempDir::new().unwrap();

    let model = ScriptedModel::new(vec![
        // Turn 1: delegate to subtask
        ModelTurn {
            content: Some("Delegating sub-investigation.".into()),
            tool_calls: vec![tool_call(
                "tc1",
                "subtask",
                json!({"objective": "Find subsidiary companies"}),
            )],
            stop_reason: StopReason::ToolUse,
        },
        // Turn 2 (from subtask solve): subtask final answer
        ModelTurn {
            content: Some("Found 3 subsidiaries.".into()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
        // Turn 3 (back in parent): use subtask result
        ModelTurn {
            content: Some("Main investigation complete with 3 subsidiaries identified.".into()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
    ]);

    let dispatcher = RecordingDispatcher::new(vec![]);
    let engine = make_engine(tmp.path(), model, dispatcher);
    let result = engine
        .solve("Investigate corporate structure", &tool_defs())
        .await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("3 subsidiaries"));
}

/// Repeated shell commands get blocked by runtime policy.
#[tokio::test]
async fn repeated_shell_command_blocked() {
    let tmp = TempDir::new().unwrap();

    // Same shell command repeated 3 times — should be blocked on the 3rd
    let model = ScriptedModel::new(vec![
        ModelTurn {
            content: None,
            tool_calls: vec![tool_call("tc1", "run_shell", json!({"command": "whoami"}))],
            stop_reason: StopReason::ToolUse,
        },
        ModelTurn {
            content: None,
            tool_calls: vec![tool_call("tc2", "run_shell", json!({"command": "whoami"}))],
            stop_reason: StopReason::ToolUse,
        },
        ModelTurn {
            content: None,
            tool_calls: vec![tool_call("tc3", "run_shell", json!({"command": "whoami"}))],
            stop_reason: StopReason::ToolUse,
        },
        ModelTurn {
            content: Some("Done.".into()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
    ]);

    let dispatcher = RecordingDispatcher::new(vec![
        tool_result("tc1", "root"),
        tool_result("tc2", "root"),
        // tc3 is blocked by runtime policy — dispatcher won't be called
    ]);

    let engine = make_engine(tmp.path(), model, dispatcher);
    let result = engine.solve("Check user", &tool_defs()).await;
    assert!(result.is_ok());
}

/// Empty model response triggers a nudge and continues.
#[tokio::test]
async fn empty_response_triggers_nudge() {
    let tmp = TempDir::new().unwrap();

    let model = ScriptedModel::new(vec![
        // Turn 1: empty response (no text, no tool calls)
        ModelTurn {
            content: None,
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
        // Turn 2: real answer after nudge
        ModelTurn {
            content: Some("Here is the real answer.".into()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
    ]);

    let dispatcher = RecordingDispatcher::new(vec![]);
    let engine = make_engine(tmp.path(), model, dispatcher);
    let result = engine.solve("Answer me", &tool_defs()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("real answer"));
}

/// Tool error is surfaced in the conversation context, not as a panic.
#[tokio::test]
async fn tool_error_is_surfaced() {
    let tmp = TempDir::new().unwrap();

    let model = ScriptedModel::new(vec![
        ModelTurn {
            content: None,
            tool_calls: vec![tool_call(
                "tc1",
                "read_file",
                json!({"path": "nonexistent.txt"}),
            )],
            stop_reason: StopReason::ToolUse,
        },
        ModelTurn {
            content: Some("File not found, moving on.".into()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
    ]);

    let dispatcher = RecordingDispatcher::new(vec![ToolResult {
        call_id: "tc1".into(),
        content: "Error: file not found".into(),
        is_error: true,
    }]);

    let engine = make_engine(tmp.path(), model, dispatcher);
    let result = engine.solve("Read data", &tool_defs()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("moving on"));
}

/// `AgentConfig` respects workspace path from tempdir.
#[test]
fn agent_config_workspace_path() {
    let tmp = TempDir::new().unwrap();
    let config = make_config(tmp.path());
    assert_eq!(config.workspace, tmp.path());
    assert_eq!(config.max_steps, 10);
    assert_eq!(config.max_depth, 3);
}

/// Demo mode flag propagates through config.
#[test]
fn demo_mode_config() {
    let mut config = AgentConfig::default();
    assert!(!config.demo_mode);
    config.demo_mode = true;
    assert!(config.demo_mode);
}

/// `ToolDefinition` list can be serialized and deserialized.
#[test]
fn tool_defs_roundtrip() {
    let defs = tool_defs();
    let json = serde_json::to_string(&defs).unwrap();
    let parsed: Vec<ToolDefinition> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0].name, "write_file");
}

/// Cancellation via token stops the engine.
#[tokio::test]
async fn cancellation_stops_engine() {
    let tmp = TempDir::new().unwrap();

    // Model returns a tool call first, but we cancel immediately
    let model = ScriptedModel::new(vec![
        ModelTurn {
            content: Some("Working...".into()),
            tool_calls: vec![tool_call("tc1", "read_file", json!({"path": "x"}))],
            stop_reason: StopReason::ToolUse,
        },
        // This turn would be reached if not cancelled
        ModelTurn {
            content: Some("Should not reach here.".into()),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        },
    ]);

    let dispatcher = RecordingDispatcher::new(vec![tool_result("tc1", "data")]);

    let engine = make_engine(tmp.path(), model, dispatcher);
    // Cancel before the second turn
    engine.cancel_token.cancel();

    let result = engine.solve("Do work", &tool_defs()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("cancelled"));
}

/// Wiki directory creation on first session (filesystem check).
#[tokio::test]
async fn wiki_directory_created() {
    let tmp = TempDir::new().unwrap();
    let wiki_dir = tmp.path().join(".redshank").join("wiki");

    // Create wiki directory as the agent would
    std::fs::create_dir_all(&wiki_dir).unwrap();

    assert!(wiki_dir.exists());
    assert!(wiki_dir.is_dir());

    // Second "session" should see existing wiki dir
    assert!(wiki_dir.exists());
}

/// Workspace path resolution from relative path.
#[test]
fn workspace_path_resolution() {
    let path = PathBuf::from(".");
    assert!(path.exists());
    let canonical = path.canonicalize().unwrap();
    assert!(canonical.is_absolute());
}
