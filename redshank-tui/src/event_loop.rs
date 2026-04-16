//! TUI event loop — ties together rendering, input, and agent events.

use crate::domain::{
    AppEvent, AppState, ChatEntry, ChatRole, ReasoningEffort, SlashCommand, UiCommand,
    parse_slash_command,
};
use crate::renderer;
use crossterm::{
    ExecutableCommand,
    event::{self, Event},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::stdout;
use std::time::Duration;
use tokio::sync::mpsc;

/// Tick interval for animation (8 fps ≈ 125ms).
const TICK_MS: u64 = 125;

/// Run the TUI event loop.
///
/// `rx` receives agent-side events (content deltas, tool starts/ends, etc.).
/// Returns when the user quits.
///
/// # Errors
///
/// Returns `Err` if terminal setup or I/O operations fail.
pub async fn run(
    rx: &mut mpsc::Receiver<AppEvent>,
    model_display: String,
    reasoning_effort: ReasoningEffort,
    command_tx: Option<mpsc::UnboundedSender<UiCommand>>,
) -> std::io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut state = AppState {
        model_display,
        reasoning_effort,
        ..AppState::default()
    };

    // Show startup banner as system message
    state.chat_log.push(ChatEntry {
        role: ChatRole::System,
        content: renderer::BANNER.trim().to_owned(),
    });

    let result = event_loop_inner(&mut terminal, &mut state, rx, command_tx).await;

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
    command_tx: Option<mpsc::UnboundedSender<UiCommand>>,
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
            () = tokio::time::sleep(Duration::from_millis(TICK_MS)) => {
                while event::poll(Duration::ZERO)? {
                    if let Event::Key(key) = event::read()?
                        && let Some(cmd) = handle_key_with_command(state, key)
                    {
                        dispatch_ui_command(command_tx.as_ref(), cmd);
                    }
                }
                // Tick for animation
                handle_app_event(state, AppEvent::Tick);
            }
        }
    }
    Ok(())
}

fn dispatch_ui_command(command_tx: Option<&mpsc::UnboundedSender<UiCommand>>, cmd: UiCommand) {
    if let Some(tx) = command_tx {
        let _ = tx.send(cmd);
    }
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
        AppEvent::WikiChanged | AppEvent::Tick => {
            // Triggers a redraw on next animation frame.
        }
        AppEvent::FetcherHealthChanged(health) => {
            state.fetcher_health = health;
        }
        AppEvent::Key(key) => {
            handle_key(state, key);
        }
        AppEvent::Quit => {
            state.should_quit = true;
        }
    }
}

fn handle_key(state: &mut AppState, key: crossterm::event::KeyEvent) {
    let _ = handle_key_with_command(state, key);
}

fn handle_key_with_command(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> Option<UiCommand> {
    match state.active_screen {
        crate::domain::ActiveScreen::Chat => handle_chat_key_with_command(state, key),
        crate::domain::ActiveScreen::Workbench => handle_workbench_key_with_command(state, key),
    }
}

fn handle_chat_key_with_command(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> Option<UiCommand> {
    use crossterm::event::{KeyCode, KeyModifiers};

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_quit = true;
            None
        }
        KeyCode::Enter => {
            let input = state.input_buffer.trim().to_owned();
            if input.is_empty() {
                return None;
            }
            state.input_buffer.clear();
            state.input_cursor = 0;

            // Check for slash commands
            if let Some(cmd) = parse_slash_command(&input) {
                handle_slash_command_with_command(state, cmd)
            } else {
                // Regular input — emit to chat log
                state.chat_log.push(ChatEntry {
                    role: ChatRole::User,
                    content: input.clone(),
                });
                state.agent_running = true;
                state.activity = crate::domain::ActivityState::Thinking(std::time::Instant::now());
                Some(UiCommand::SubmitObjective(input))
            }
        }
        KeyCode::Char(c) => {
            state.input_buffer.insert(state.input_cursor, c);
            state.input_cursor += 1;
            None
        }
        KeyCode::Backspace => {
            if state.input_cursor > 0 {
                state.input_cursor -= 1;
                state.input_buffer.remove(state.input_cursor);
            }
            None
        }
        KeyCode::Left => {
            state.input_cursor = state.input_cursor.saturating_sub(1);
            None
        }
        KeyCode::Right => {
            if state.input_cursor < state.input_buffer.len() {
                state.input_cursor += 1;
            }
            None
        }
        KeyCode::Up => {
            state.chat_scroll = state.chat_scroll.saturating_add(1);
            None
        }
        KeyCode::Down => {
            state.chat_scroll = state.chat_scroll.saturating_sub(1);
            None
        }
        _ => None,
    }
}

const fn handle_workbench_key_with_command(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> Option<UiCommand> {
    use crate::domain::{ActiveScreen, WorkbenchTab};
    use crossterm::event::{KeyCode, KeyModifiers};

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.active_screen = ActiveScreen::Chat;
            Some(UiCommand::CloseWorkbench)
        }
        KeyCode::Tab => {
            state.workbench_tab = match state.workbench_tab {
                WorkbenchTab::Providers => WorkbenchTab::Sources,
                WorkbenchTab::Sources => WorkbenchTab::Providers,
            };
            None
        }
        KeyCode::Up => {
            match state.workbench_tab {
                WorkbenchTab::Providers => {
                    state.workbench_provider_idx = state.workbench_provider_idx.saturating_sub(1);
                }
                WorkbenchTab::Sources => {
                    state.workbench_source_idx = state.workbench_source_idx.saturating_sub(1);
                }
            }
            None
        }
        KeyCode::Down => {
            match state.workbench_tab {
                WorkbenchTab::Providers => {
                    state.workbench_provider_idx = state.workbench_provider_idx.saturating_add(1);
                }
                WorkbenchTab::Sources => {
                    state.workbench_source_idx = state.workbench_source_idx.saturating_add(1);
                }
            }
            None
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_quit = true;
            None
        }
        _ => None,
    }
}

fn handle_slash_command_with_command(state: &mut AppState, cmd: SlashCommand) -> Option<UiCommand> {
    match cmd {
        SlashCommand::Quit => {
            state.should_quit = true;
            None
        }
        SlashCommand::Clear => {
            state.chat_log.clear();
            state.chat_scroll = 0;
            None
        }
        SlashCommand::Help => {
            state.chat_log.push(ChatEntry {
                role: ChatRole::System,
                content: [
                    "Commands:",
                    "  /model <name> [--save]  Switch model",
                    "  /model [list]           List available models",
                    "  /reasoning <level>      Set reasoning (off|low|medium|high)",
                    "  /status                 Show current session info",
                    "  /config                 Open configuration workbench",
                    "  /clear                  Clear chat log",
                    "  /quit                   Exit",
                    "  /help                   Show this help",
                ]
                .join("\n"),
            });
            None
        }
        SlashCommand::Config => {
            state.active_screen = crate::domain::ActiveScreen::Workbench;
            Some(UiCommand::OpenWorkbench)
        }
        SlashCommand::Status => {
            let session_info = if state.sessions.is_empty() {
                "No active session".to_owned()
            } else {
                state.sessions.get(state.selected_session).map_or_else(
                    || "No active session".to_owned(),
                    |s| format!("Session: {} ({} events)", s.label, s.event_count),
                )
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
            None
        }
        SlashCommand::Model { name, save } => {
            state.model_display.clone_from(&name);
            let msg = if save {
                format!("Model set to {name} (saved)")
            } else {
                format!("Model set to {name}")
            };
            state.chat_log.push(ChatEntry {
                role: ChatRole::System,
                content: msg,
            });
            Some(UiCommand::SetModel { name, save })
        }
        SlashCommand::ModelList => {
            state.activity = crate::domain::ActivityState::Thinking(std::time::Instant::now());
            Some(UiCommand::ListModels)
        }
        SlashCommand::Reasoning(level) => {
            state.reasoning_effort = level;
            state.chat_log.push(ChatEntry {
                role: ChatRole::System,
                content: format!("Reasoning set to {}", state.reasoning_effort.as_str()),
            });
            Some(UiCommand::SetReasoning(level))
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::*;
    use crossterm::event::KeyCode;

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
        let mut state = AppState {
            agent_running: true,
            ..Default::default()
        };
        handle_app_event(
            &mut state,
            AppEvent::AgentComplete("Done investigating.".into()),
        );
        assert!(!state.agent_running);
        assert_eq!(
            state.chat_log.last().unwrap().content,
            "Done investigating."
        );
    }

    #[test]
    fn handle_quit_event() {
        let mut state = AppState::default();
        handle_app_event(&mut state, AppEvent::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn handle_key_event_updates_input_buffer() {
        let mut state = AppState::default();
        handle_app_event(
            &mut state,
            AppEvent::Key(crossterm::event::KeyEvent::from(KeyCode::Char('x'))),
        );
        assert_eq!(state.input_buffer, "x");
        assert_eq!(state.input_cursor, 1);
    }

    #[test]
    fn handle_enter_key_event_submits_input() {
        let mut state = AppState {
            input_buffer: "hello".into(),
            input_cursor: 5,
            ..Default::default()
        };

        handle_app_event(
            &mut state,
            AppEvent::Key(crossterm::event::KeyEvent::from(KeyCode::Enter)),
        );

        assert!(state.input_buffer.is_empty());
        assert_eq!(state.input_cursor, 0);
        assert_eq!(state.chat_log.len(), 1);
        assert_eq!(state.chat_log[0].role, ChatRole::User);
        assert_eq!(state.chat_log[0].content, "hello");
        assert!(state.agent_running);
    }

    #[test]
    fn handle_enter_returns_submit_command() {
        let mut state = AppState {
            input_buffer: "investigate acme".into(),
            input_cursor: 16,
            ..Default::default()
        };

        let command =
            handle_key_with_command(&mut state, crossterm::event::KeyEvent::from(KeyCode::Enter));

        assert_eq!(
            command,
            Some(UiCommand::SubmitObjective("investigate acme".into()))
        );
    }

    #[test]
    fn slash_quit_sets_should_quit() {
        let mut state = AppState::default();
        let _ = handle_slash_command_with_command(&mut state, SlashCommand::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn slash_clear_empties_log() {
        let mut state = AppState::default();
        state.chat_log.push(ChatEntry {
            role: ChatRole::User,
            content: "hello".into(),
        });
        let _ = handle_slash_command_with_command(&mut state, SlashCommand::Clear);
        assert!(state.chat_log.is_empty());
    }

    #[test]
    fn slash_help_adds_system_message() {
        let mut state = AppState::default();
        let _ = handle_slash_command_with_command(&mut state, SlashCommand::Help);
        assert_eq!(state.chat_log.len(), 1);
        assert!(state.chat_log[0].content.contains("/model"));
    }

    #[test]
    fn slash_model_updates_display() {
        let mut state = AppState::default();
        let _ = handle_slash_command_with_command(
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
    fn slash_model_returns_set_model_command() {
        let mut state = AppState::default();
        let command = handle_slash_command_with_command(
            &mut state,
            SlashCommand::Model {
                name: "ollama/gemma3:27b".into(),
                save: false,
            },
        );

        assert_eq!(
            command,
            Some(UiCommand::SetModel {
                name: "ollama/gemma3:27b".into(),
                save: false,
            })
        );
    }

    #[test]
    fn slash_model_list_returns_command() {
        let mut state = AppState::default();
        let command = handle_slash_command_with_command(&mut state, SlashCommand::ModelList);

        assert_eq!(command, Some(UiCommand::ListModels));
        assert!(state.activity.is_active());
    }

    #[test]
    fn slash_reasoning_updates_effort() {
        let mut state = AppState::default();
        let _ = handle_slash_command_with_command(
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

    #[test]
    fn slash_config_opens_workbench() {
        let mut state = AppState::default();
        assert_eq!(state.active_screen, crate::domain::ActiveScreen::Chat);

        let command = handle_slash_command_with_command(&mut state, SlashCommand::Config);

        assert_eq!(state.active_screen, crate::domain::ActiveScreen::Workbench);
        assert_eq!(command, Some(UiCommand::OpenWorkbench));
    }

    #[test]
    fn workbench_tab_starts_on_providers() {
        let state = AppState::default();
        assert_eq!(state.workbench_tab, crate::domain::WorkbenchTab::Providers);
    }

    #[test]
    fn workbench_provider_selection_initializes_to_zero() {
        let state = AppState::default();
        assert_eq!(state.workbench_provider_idx, 0);
    }

    #[test]
    fn workbench_source_selection_initializes_to_zero() {
        let state = AppState::default();
        assert_eq!(state.workbench_source_idx, 0);
    }

    #[test]
    fn workbench_esc_returns_to_chat() {
        let mut state = AppState {
            active_screen: crate::domain::ActiveScreen::Workbench,
            ..Default::default()
        };

        let key = crossterm::event::KeyEvent::from(KeyCode::Esc);
        let cmd = handle_key_with_command(&mut state, key);

        assert_eq!(state.active_screen, crate::domain::ActiveScreen::Chat);
        assert_eq!(cmd, Some(UiCommand::CloseWorkbench));
    }

    #[test]
    fn workbench_tab_key_cycles_tabs() {
        let mut state = AppState {
            active_screen: crate::domain::ActiveScreen::Workbench,
            ..Default::default()
        };
        assert_eq!(state.workbench_tab, crate::domain::WorkbenchTab::Providers);

        // Tab switches to Sources
        let key = crossterm::event::KeyEvent::from(KeyCode::Tab);
        let _ = handle_key_with_command(&mut state, key);
        assert_eq!(state.workbench_tab, crate::domain::WorkbenchTab::Sources);

        // Tab again wraps back to Providers
        let key = crossterm::event::KeyEvent::from(KeyCode::Tab);
        let _ = handle_key_with_command(&mut state, key);
        assert_eq!(state.workbench_tab, crate::domain::WorkbenchTab::Providers);
    }

    #[test]
    fn workbench_up_key_decrements_selection() {
        let mut state = AppState {
            active_screen: crate::domain::ActiveScreen::Workbench,
            workbench_provider_idx: 2,
            ..Default::default()
        };

        let key = crossterm::event::KeyEvent::from(KeyCode::Up);
        let _ = handle_key_with_command(&mut state, key);

        assert_eq!(state.workbench_provider_idx, 1);
    }

    #[test]
    fn workbench_up_key_saturates_at_zero() {
        let mut state = AppState {
            active_screen: crate::domain::ActiveScreen::Workbench,
            workbench_provider_idx: 0,
            ..Default::default()
        };

        let key = crossterm::event::KeyEvent::from(KeyCode::Up);
        let _ = handle_key_with_command(&mut state, key);

        assert_eq!(state.workbench_provider_idx, 0);
    }

    #[test]
    fn workbench_down_key_increments_selection() {
        let mut state = AppState {
            active_screen: crate::domain::ActiveScreen::Workbench,
            workbench_provider_idx: 1,
            ..Default::default()
        };

        let key = crossterm::event::KeyEvent::from(KeyCode::Down);
        let _ = handle_key_with_command(&mut state, key);

        assert_eq!(state.workbench_provider_idx, 2);
    }

    #[test]
    fn workbench_source_up_down_navigate_source_idx() {
        let mut state = AppState {
            active_screen: crate::domain::ActiveScreen::Workbench,
            workbench_tab: crate::domain::WorkbenchTab::Sources,
            workbench_source_idx: 3,
            ..Default::default()
        };

        let key = crossterm::event::KeyEvent::from(KeyCode::Up);
        let _ = handle_key_with_command(&mut state, key);
        assert_eq!(state.workbench_source_idx, 2);

        let key = crossterm::event::KeyEvent::from(KeyCode::Down);
        let _ = handle_key_with_command(&mut state, key);
        assert_eq!(state.workbench_source_idx, 3);
    }
}
