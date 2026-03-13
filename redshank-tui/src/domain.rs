//! TUI domain types — AppState, AppEvent, UiCommand.

use serde::{Deserialize, Serialize};

/// Application state for the TUI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    /// Whether the agent is currently running.
    pub agent_running: bool,
    /// Current status message.
    pub status: String,
}

/// Events consumed by the TUI event loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    /// User typed input.
    Input(String),
    /// Agent produced output.
    AgentOutput(String),
    /// Quit requested.
    Quit,
}
