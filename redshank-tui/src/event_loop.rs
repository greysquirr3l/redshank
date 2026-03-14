//! TUI event loop — ties together rendering, input, and agent events.

use crate::domain::{parse_slash_command, AppEvent, AppState, ChatEntry, ChatRole, SlashCommand};
use crate::renderer;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;

/// Tick interval for animation (8 fps ≈ 125ms).
const TICK_MS: u64 = 125;

/// Run the TUI event loop.
///
/// `rx` receives agent-side events (content deltas, tool starts/ends, etc.).
/// Returns when the user quits.
pub async fn run(rx: &mut mpsc::Receiver<AppEvent>) -> std::io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut state = AppState::default();

    // Show startup banner as system message
    state.chat_log.push(ChatEntry {
        role: ChatRole::System,
        content: renderer::BANNER.trim().to_owned(),
    });

    let result = event_loop_inner(&mut terminal, &mut state, rx).await;

    // Clean up terminal state
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn event_loop_inner(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state: &mut AppState,
    rx: &mut mpsc::Receiver<AppEvent>,
) -> std::io::Result<()> {
    loop {
        // Render
        terminal.draw(|frame| renderer::render(frame, state))?;

        if state.should_quit {
            break;
        }

        // Poll: check crossterm events first (non-blocking), then mpsc, then tick
        tokio::select! {
            // Channel events from agent
            Some(ev) = rx.recv() => {
                handle_app_event(state, ev);
            }
            // Terminal input (poll with timeout to not block)
            _ = tokio::time::sleep(Duration::from_millis(TICK_MS)) => {
                // Check crossterm events without blocking
                while event::poll(Duration::ZERO)? {
                    if let Event::Key(key) = event::read()? {
                        handle_key(state, key);
                    }
                }
                // Tick for animation
                handle_app_event(state, AppEvent::Tick);
            }
        }
    }
    Ok(())
}

fn handle_app_event(state: &mut AppState, ev: AppEvent) {
    match ev {
        AppEvent::Input(text) => {
            state.chat_log.push(ChatEntry {
                role: ChatRole::User,
                content: text,
            });
        }
        AppEvent::ContentDelta(delta) => {
            // Append to last assistant entry or create one
            if let Some(last) = state.chat_log.last_mut()
                && matches!(last.role, ChatRole::Assistant)
            {
                last.content.push_str(&delta);
                return;
            }
            state.chat_log.push(ChatEntry {
                role: ChatRole::Assistant,
                content: delta,
            });
        }
        AppEvent::ToolStart(name) => {
            use crate::domain::ActivityState;
            state.activity = ActivityState::Running {
                tool_name: name.clone(),
                started: std::time::Instant::now(),
            };
            state.chat_log.push(ChatEntry {
                role: ChatRole::Tool,
                content: format!("▶ {name}"),
            });
        }
        AppEvent::ToolEnd(name, summary) => {
            use crate::domain::ActivityState;
            state.activity = ActivityState::Idle;
            state.chat_log.push(ChatEntry {
                role: ChatRole::Tool,
                content: format!("✓ {name}: {summary}"),
            });
        }
        AppEvent::AgentComplete(summary) => {
            use crate::domain::ActivityState;
            state.activity = ActivityState::Idle;
            state.agent_running = false;
            if !summary.is_empty() {
                state.chat_log.push(ChatEntry {
                    role: ChatRole::System,
                    content: summary,
                });
            }
        }
        AppEvent::WikiChanged => {
            // The wiki state is updated externally; we just redraw on next tick.
        }
        AppEvent::Tick => {
            // Triggers a redraw (animation frame).
        }
        AppEvent::Key(_) => {
            // Handled separately by handle_key.
        }
        AppEvent::Quit => {
            state.should_quit = true;
        }
    }
}

fn handle_key(state: &mut AppState, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_quit = true;
        }
        KeyCode::Enter => {
            let input = state.input_buffer.trim().to_owned();
            if input.is_empty() {
                return;
            }
            state.input_buffer.clear();
            state.input_cursor = 0;

            // Check for slash commands
            if let Some(cmd) = parse_slash_command(&input) {
                handle_slash_command(state, cmd);
            } else {
                // Regular input — emit to chat log
                state.chat_log.push(ChatEntry {
                    role: ChatRole::User,
                    content: input,
                });
                state.agent_running = true;
                state.activity = crate::domain::ActivityState::Thinking(std::time::Instant::now());
            }
        }
        KeyCode::Char(c) => {
            state.input_buffer.insert(state.input_cursor, c);
            state.input_cursor += 1;
        }
        KeyCode::Backspace => {
            if state.input_cursor > 0 {
                state.input_cursor -= 1;
                state.input_buffer.remove(state.input_cursor);
            }
        }
        KeyCode::Left => {
            state.input_cursor = state.input_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            if state.input_cursor < state.input_buffer.len() {
                state.input_cursor += 1;
            }
        }
        KeyCode::Up => {
            state.chat_scroll = state.chat_scroll.saturating_add(1);
        }
        KeyCode::Down => {
            state.chat_scroll = state.chat_scroll.saturating_sub(1);
        }
        _ => {}
    }
}

fn handle_slash_command(state: &mut AppState, cmd: SlashCommand) {
    match cmd {
        SlashCommand::Quit => {
            state.should_quit = true;
        }
        SlashCommand::Clear => {
            state.chat_log.clear();
            state.chat_scroll = 0;
        }
        SlashCommand::Help => {
            state.chat_log.push(ChatEntry {
                role: ChatRole::System,
                content: [
                    "Commands:",
                    "  /model <name> [--save]  Switch model",
                    "  /model list             List available models",
                    "  /reasoning <level>      Set reasoning (off|low|medium|high)",
                    "  /status                 Show current session info",
                    "  /clear                  Clear chat log",
                    "  /quit                   Exit",
                    "  /help                   Show this help",
                ]
                .join("\n"),
            });
        }
        SlashCommand::Status => {
            let session_info = if state.sessions.is_empty() {
                "No active session".to_owned()
            } else {
                let s = &state.sessions[state.selected_session];
                format!("Session: {} ({} events)", s.label, s.event_count)
            };
            let status = format!(
                "Model: {}\nReasoning: {}\nActivity: {}\n{}",
                state.model_display,
                state.reasoning_effort.as_str(),
                state.activity.status_text(),
                session_info,
            );
            state.chat_log.push(ChatEntry {
                role: ChatRole::System,
                content: status,
            });
        }
        SlashCommand::Model { name, save } => {
            state.model_display = name.clone();
            let msg = if save {
                format!("Model set to {name} (saved)")
            } else {
                format!("Model set to {name}")
            };
            state.chat_log.push(ChatEntry {
                role: ChatRole::System,
                content: msg,
            });
        }
        SlashCommand::ModelList => {
            state.chat_log.push(ChatEntry {
                role: ChatRole::System,
                content: "Available models: (not yet implemented)".into(),
            });
        }
        SlashCommand::Reasoning(level) => {
            state.reasoning_effort = level;
            state.chat_log.push(ChatEntry {
                role: ChatRole::System,
                content: format!("Reasoning set to {}", state.reasoning_effort.as_str()),
            });
        }
    }
}

/// Headless (non-TUI) mode — processes events without terminal rendering.
/// Useful for --no-tui / CI.
pub async fn run_headless(rx: &mut mpsc::Receiver<AppEvent>) {
    let mut state = AppState::default();
    while let Some(ev) = rx.recv().await {
        handle_app_event(&mut state, ev);
        if state.should_quit {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::*;

    #[test]
    fn handle_content_delta_appends() {
        let mut state = AppState::default();
        handle_app_event(&mut state, AppEvent::ContentDelta("Hello".into()));
        handle_app_event(&mut state, AppEvent::ContentDelta(" world".into()));
        assert_eq!(state.chat_log.len(), 1);
        assert_eq!(state.chat_log[0].content, "Hello world");
    }

    #[test]
    fn handle_tool_start_sets_activity() {
        let mut state = AppState::default();
        handle_app_event(&mut state, AppEvent::ToolStart("web_search".into()));
        assert!(state.activity.is_active());
        assert!(state.activity.status_text().contains("web_search"));
    }

    #[test]
    fn handle_tool_end_resets_idle() {
        let mut state = AppState::default();
        handle_app_event(&mut state, AppEvent::ToolStart("web_search".into()));
        handle_app_event(
            &mut state,
            AppEvent::ToolEnd("web_search".into(), "3 results".into()),
        );
        assert!(!state.activity.is_active());
    }

    #[test]
    fn handle_agent_complete() {
        let mut state = AppState::default();
        state.agent_running = true;
        handle_app_event(
            &mut state,
            AppEvent::AgentComplete("Done investigating.".into()),
        );
        assert!(!state.agent_running);
        assert_eq!(state.chat_log.last().unwrap().content, "Done investigating.");
    }

    #[test]
    fn handle_quit_event() {
        let mut state = AppState::default();
        handle_app_event(&mut state, AppEvent::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn slash_quit_sets_should_quit() {
        let mut state = AppState::default();
        handle_slash_command(&mut state, SlashCommand::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn slash_clear_empties_log() {
        let mut state = AppState::default();
        state.chat_log.push(ChatEntry {
            role: ChatRole::User,
            content: "hello".into(),
        });
        handle_slash_command(&mut state, SlashCommand::Clear);
        assert!(state.chat_log.is_empty());
    }

    #[test]
    fn slash_help_adds_system_message() {
        let mut state = AppState::default();
        handle_slash_command(&mut state, SlashCommand::Help);
        assert_eq!(state.chat_log.len(), 1);
        assert!(state.chat_log[0].content.contains("/model"));
    }

    #[test]
    fn slash_model_updates_display() {
        let mut state = AppState::default();
        handle_slash_command(
            &mut state,
            SlashCommand::Model {
                name: "claude-opus-4-6".into(),
                save: true,
            },
        );
        assert_eq!(state.model_display, "claude-opus-4-6");
        assert!(state.chat_log[0].content.contains("saved"));
    }

    #[test]
    fn slash_reasoning_updates_effort() {
        let mut state = AppState::default();
        handle_slash_command(
            &mut state,
            SlashCommand::Reasoning(ReasoningEffort::High),
        );
        assert_eq!(state.reasoning_effort, ReasoningEffort::High);
    }

    #[test]
    fn channel_delivers_content_deltas_in_order() {
        let mut state = AppState::default();
        let deltas = vec!["one", "two", "three"];
        for d in &deltas {
            handle_app_event(&mut state, AppEvent::ContentDelta((*d).to_string()));
        }
        assert_eq!(state.chat_log.len(), 1);
        assert_eq!(state.chat_log[0].content, "onetwothree");
    }

    #[tokio::test]
    async fn headless_mode_exits_on_quit() {
        let (tx, mut rx) = mpsc::channel(16);
        tx.send(AppEvent::ContentDelta("hello".into()))
            .await
            .unwrap();
        tx.send(AppEvent::Quit).await.unwrap();
        drop(tx);
        run_headless(&mut rx).await;
        // If we got here, it exited cleanly.
    }
}
