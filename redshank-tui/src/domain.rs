//! TUI domain types — AppState, AppEvent, UiCommand, ActivityState.

use serde::{Deserialize, Serialize};
use std::time::Instant;

// ── Application State ────────────────────────────────────────────────────────

/// Application state for the TUI.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Chat log entries.
    pub chat_log: Vec<ChatEntry>,
    /// Current user input buffer.
    pub input_buffer: String,
    /// Cursor position in the input buffer.
    pub input_cursor: usize,
    /// Chat scroll offset (lines from bottom).
    pub chat_scroll: u16,
    /// Activity indicator state.
    pub activity: ActivityState,
    /// Session list.
    pub sessions: Vec<SessionInfo>,
    /// Currently selected session index.
    pub selected_session: usize,
    /// Current model/provider display string.
    pub model_display: String,
    /// Current reasoning effort.
    pub reasoning_effort: ReasoningEffort,
    /// Wiki graph node summary (label, category).
    pub wiki_nodes: Vec<(String, String)>,
    /// Wiki graph edge pairs (from_idx, to_idx).
    pub wiki_edges: Vec<(usize, usize)>,
    /// Whether the agent is currently running.
    pub agent_running: bool,
    /// Should quit.
    pub should_quit: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            chat_log: Vec::new(),
            input_buffer: String::new(),
            input_cursor: 0,
            chat_scroll: 0,
            activity: ActivityState::Idle,
            sessions: Vec::new(),
            selected_session: 0,
            model_display: "none".into(),
            reasoning_effort: ReasoningEffort::Medium,
            wiki_nodes: Vec::new(),
            wiki_edges: Vec::new(),
            agent_running: false,
            should_quit: false,
        }
    }
}

/// A single chat log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatEntry {
    pub role: ChatRole,
    pub content: String,
}

/// Who authored a chat entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Session summary for sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub label: String,
    pub event_count: usize,
}

// ── Activity Indicator ───────────────────────────────────────────────────────

/// Activity indicator states — drives the spinner/status line.
#[derive(Debug, Clone)]
pub enum ActivityState {
    /// No activity.
    Idle,
    /// Agent is thinking (start time for elapsed display).
    Thinking(Instant),
    /// Agent is running a tool.
    Running {
        tool_name: String,
        started: Instant,
    },
    /// Agent is streaming output.
    Streaming {
        started: Instant,
        preview: String,
    },
}

impl ActivityState {
    /// Human-readable status string for the footer.
    pub fn status_text(&self) -> String {
        match self {
            Self::Idle => "Ready".into(),
            Self::Thinking(start) => {
                let elapsed = start.elapsed().as_secs();
                format!("Thinking… ({elapsed}s)")
            }
            Self::Running { tool_name, started } => {
                let elapsed = started.elapsed().as_secs();
                format!("Running {tool_name}… ({elapsed}s)")
            }
            Self::Streaming { started, preview } => {
                let elapsed = started.elapsed().as_secs();
                let truncated: String = preview.chars().take(40).collect();
                format!("Streaming ({elapsed}s): {truncated}")
            }
        }
    }

    /// Whether the indicator should animate (non-idle).
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Idle)
    }
}

// ── Reasoning Effort ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningEffort {
    Off,
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" => Some(Self::Off),
            "low" => Some(Self::Low),
            "medium" | "med" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

// ── Events ───────────────────────────────────────────────────────────────────

/// Events consumed by the TUI event loop.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// User typed input and pressed Enter.
    Input(String),
    /// Terminal key event.
    Key(crossterm::event::KeyEvent),
    /// Terminal tick (8fps animation).
    Tick,
    /// Agent produced a content delta (streaming text).
    ContentDelta(String),
    /// Agent started a tool call.
    ToolStart(String),
    /// Agent finished a tool call (name, result summary).
    ToolEnd(String, String),
    /// Agent completed its turn (final message).
    AgentComplete(String),
    /// Wiki graph changed (re-render canvas).
    WikiChanged,
    /// Quit requested.
    Quit,
}

// ── Slash Commands (CQRS command variants) ───────────────────────────────────

/// Parsed slash command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    /// `/model <name> [--save]` — switch model.
    Model { name: String, save: bool },
    /// `/model list` — list available models.
    ModelList,
    /// `/reasoning <level>` — set reasoning effort.
    Reasoning(ReasoningEffort),
    /// `/status` — show current status.
    Status,
    /// `/clear` — clear chat log.
    Clear,
    /// `/quit` — exit the TUI.
    Quit,
    /// `/help` — show help.
    Help,
}

/// Parse a slash command from input text.
pub fn parse_slash_command(input: &str) -> Option<SlashCommand> {
    let input = input.trim();
    if !input.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = input.split_whitespace().collect();
    let cmd = parts.first()?;

    match *cmd {
        "/model" => {
            if parts.len() < 2 {
                return None;
            }
            if parts[1] == "list" {
                return Some(SlashCommand::ModelList);
            }
            let save = parts.contains(&"--save");
            Some(SlashCommand::Model {
                name: parts[1].to_string(),
                save,
            })
        }
        "/reasoning" => {
            let level = parts.get(1).and_then(|s| ReasoningEffort::parse(s))?;
            Some(SlashCommand::Reasoning(level))
        }
        "/status" => Some(SlashCommand::Status),
        "/clear" => Some(SlashCommand::Clear),
        "/quit" | "/q" | "/exit" => Some(SlashCommand::Quit),
        "/help" | "/h" | "/?" => Some(SlashCommand::Help),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_transitions_idle_thinking_running_streaming_idle() {
        let mut state = ActivityState::Idle;
        assert!(!state.is_active());
        assert_eq!(state.status_text(), "Ready");

        state = ActivityState::Thinking(Instant::now());
        assert!(state.is_active());
        assert!(state.status_text().starts_with("Thinking"));

        state = ActivityState::Running {
            tool_name: "search".into(),
            started: Instant::now(),
        };
        assert!(state.is_active());
        assert!(state.status_text().contains("search"));

        state = ActivityState::Streaming {
            started: Instant::now(),
            preview: "The entity was found in…".into(),
        };
        assert!(state.is_active());
        assert!(state.status_text().contains("Streaming"));

        state = ActivityState::Idle;
        assert!(!state.is_active());
    }

    #[test]
    fn slash_command_parses_model_with_save() {
        let cmd = parse_slash_command("/model claude-opus-4-6 --save").unwrap();
        assert_eq!(
            cmd,
            SlashCommand::Model {
                name: "claude-opus-4-6".into(),
                save: true
            }
        );
    }

    #[test]
    fn slash_command_parses_model_list() {
        let cmd = parse_slash_command("/model list").unwrap();
        assert_eq!(cmd, SlashCommand::ModelList);
    }

    #[test]
    fn slash_command_parses_reasoning_level() {
        let cmd = parse_slash_command("/reasoning high").unwrap();
        assert_eq!(cmd, SlashCommand::Reasoning(ReasoningEffort::High));
    }

    #[test]
    fn slash_command_parses_quit_variants() {
        assert_eq!(parse_slash_command("/quit").unwrap(), SlashCommand::Quit);
        assert_eq!(parse_slash_command("/q").unwrap(), SlashCommand::Quit);
        assert_eq!(parse_slash_command("/exit").unwrap(), SlashCommand::Quit);
    }

    #[test]
    fn slash_command_parses_help_variants() {
        assert_eq!(parse_slash_command("/help").unwrap(), SlashCommand::Help);
        assert_eq!(parse_slash_command("/h").unwrap(), SlashCommand::Help);
        assert_eq!(parse_slash_command("/?").unwrap(), SlashCommand::Help);
    }

    #[test]
    fn slash_command_returns_none_for_non_slash_input() {
        assert!(parse_slash_command("hello world").is_none());
        assert!(parse_slash_command("").is_none());
    }

    #[test]
    fn reasoning_effort_roundtrips() {
        for level in [
            ReasoningEffort::Off,
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
        ] {
            assert_eq!(ReasoningEffort::parse(level.as_str()), Some(level));
        }
    }
}

