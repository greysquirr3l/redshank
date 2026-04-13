//! TUI renderer — ratatui frame rendering.
//!
//! Three-pane layout:
//! ```text
//! ┌──────────────── Header ────────────────┐
//! │ Sidebar(20%) │ Chat(55%) │ Graph(25%)  │
//! └──────────────── Footer ────────────────┘
//! ```

use crate::domain::{ActiveScreen, AppState, ChatRole, WorkbenchTab};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use redshank_core::domain::{
    agent::ProviderKind,
    source_catalog::{AuthRequirement, all_sources},
};

/// Figlet-style banner for startup.
pub const BANNER: &str = r"
┏━┓┏━╸╺┳┓┏━┓╻ ╻┏━┓┏┓╻╻┏ 
┣┳┛┣╸  ┃┃┗━┓┣━┫┣━┫┃┗┫┣┻┓
╹┗╸┗━╸╺┻┛┗━┛╹ ╹╹ ╹╹ ╹╹ ╹
";

/// Render the full TUI frame.
#[allow(clippy::indexing_slicing)]
pub fn render(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Top-level: Header | Body | Footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(3),    // body
            Constraint::Length(1), // footer
        ])
        .split(size);

    render_header(frame, outer[0], state);
    render_body(frame, outer[1], state);
    render_footer(frame, outer[2], state);
}

fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let model_text = format!(
        " redshank │ model: {} │ reasoning: {} ",
        state.model_display,
        state.reasoning_effort.as_str()
    );
    let header =
        Paragraph::new(model_text).style(Style::default().fg(Color::Black).bg(Color::Cyan));
    frame.render_widget(header, area);
}

#[allow(clippy::indexing_slicing)]
fn render_body(frame: &mut Frame, area: Rect, state: &AppState) {
    match state.active_screen {
        ActiveScreen::Chat => render_chat_layout(frame, area, state),
        ActiveScreen::Workbench => render_workbench(frame, area, state),
    }
}

#[allow(clippy::indexing_slicing)]
fn render_chat_layout(frame: &mut Frame, area: Rect, state: &AppState) {
    // Three panes: Sidebar(20%) | Chat(55%) | Graph(25%)
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(55),
            Constraint::Percentage(25),
        ])
        .split(area);

    render_sidebar(frame, panes[0], state);
    render_chat(frame, panes[1], state);
    render_graph(frame, panes[2], state);
}

fn render_sidebar(frame: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state
        .sessions
        .iter()
        .enumerate()
        .map(|(i, session)| {
            let style = if i == state.selected_session {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let label = format!(" {} ({})", session.label, session.event_count);
            ListItem::new(label).style(style)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" Sessions "));
    frame.render_widget(list, area);
}

#[allow(clippy::indexing_slicing)]
fn render_chat(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(" Chat ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner area: chat log above, input below
    let chat_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(inner);

    // Render chat log
    let lines: Vec<Line> = state
        .chat_log
        .iter()
        .flat_map(|entry| {
            let (prefix, color) = match entry.role {
                ChatRole::User => ("you> ", Color::Green),
                ChatRole::Assistant => ("bot> ", Color::Cyan),
                ChatRole::System => ("sys> ", Color::Yellow),
                ChatRole::Tool => ("tool> ", Color::Magenta),
            };
            entry
                .content
                .lines()
                .enumerate()
                .map(move |(i, line)| {
                    if i == 0 {
                        Line::from(vec![
                            Span::styled(
                                prefix,
                                Style::default().fg(color).add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(line),
                        ])
                    } else {
                        Line::from(format!("      {line}"))
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let chat_paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.chat_scroll, 0));
    frame.render_widget(chat_paragraph, chat_layout[0]);

    // Render input box
    let input_text = format!("│ {}", state.input_buffer);
    let input = Paragraph::new(input_text)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::TOP).title(" Input "));
    frame.render_widget(input, chat_layout[1]);
}

fn render_graph(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().borders(Borders::ALL).title(" Wiki Graph ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.wiki_nodes.is_empty() {
        let empty = Paragraph::new("  (no nodes yet)").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    // Simple character-cell node rendering
    let lines: Vec<Line> = state
        .wiki_nodes
        .iter()
        .enumerate()
        .map(|(i, (label, category))| {
            let color = category_color(category);
            let truncated: String = label
                .chars()
                .take(inner.width.saturating_sub(6) as usize)
                .collect();
            let prefix = format!("[{i:>2}]");
            Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled(truncated, Style::default().fg(color)),
            ])
        })
        .collect();

    let graph_paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(graph_paragraph, inner);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &AppState) {
    let status = state.activity.status_text();
    let nodes = state.wiki_nodes.len();
    let edges = state.wiki_edges.len();
    let footer_text = format!(" {status} │ nodes: {nodes} │ edges: {edges} │ /help for commands ");
    let footer =
        Paragraph::new(footer_text).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(footer, area);
}

fn category_color(category: &str) -> Color {
    match category {
        "person" => Color::Green,
        "organization" | "company" => Color::Blue,
        "government" => Color::Yellow,
        "financial" => Color::Cyan,
        "legal" => Color::Red,
        "property" => Color::Magenta,
        _ => Color::White,
    }
}

/// Check that the layout renders without panic on a minimal terminal size.
#[must_use]
pub const fn check_minimum_size(width: u16, height: u16) -> bool {
    width >= 80 && height >= 24
}

/// Render the configuration workbench screen.
#[allow(clippy::indexing_slicing)]
fn render_workbench(frame: &mut Frame, area: Rect, state: &AppState) {
    // Vertical split: tab bar (1 line) + content
    let vstack = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    render_workbench_tabs(frame, vstack[0], state);
    render_workbench_content(frame, vstack[1], state);
}

fn render_workbench_tabs(frame: &mut Frame, area: Rect, state: &AppState) {
    let (providers_style, sources_style) = match state.workbench_tab {
        WorkbenchTab::Providers => (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::DarkGray),
        ),
        WorkbenchTab::Sources => (
            Style::default().fg(Color::DarkGray),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    };
    let tab_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(" Providers ", providers_style),
        Span::raw(" │ "),
        Span::styled(" Data Sources ", sources_style),
        Span::raw("  "),
        Span::styled(
            " Tab: switch  ↑↓: select  Esc: close ",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(tab_line), area);
}

#[allow(clippy::indexing_slicing)]
fn render_workbench_content(frame: &mut Frame, area: Rect, state: &AppState) {
    // Two panes: list (30%) | detail (70%)
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    match state.workbench_tab {
        WorkbenchTab::Providers => {
            render_provider_list(frame, panes[0], state);
            render_provider_detail(frame, panes[1], state);
        }
        WorkbenchTab::Sources => {
            render_source_list(frame, panes[0], state);
            render_source_detail(frame, panes[1], state);
        }
    }
}

const ALL_PROVIDERS: &[ProviderKind] = &[
    ProviderKind::Anthropic,
    ProviderKind::OpenAI,
    ProviderKind::OpenRouter,
    ProviderKind::Cerebras,
    ProviderKind::OpenAiCompatible,
];

fn provider_display_name(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "Anthropic",
        ProviderKind::OpenAI => "OpenAI",
        ProviderKind::OpenRouter => "OpenRouter",
        ProviderKind::Cerebras => "Cerebras",
        ProviderKind::OpenAiCompatible => "OpenAI-Compatible",
    }
}

fn provider_protocol(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "Anthropic Messages API",
        ProviderKind::OpenAI => "OpenAI Chat Completions",
        ProviderKind::OpenRouter => "OpenAI Chat Completions",
        ProviderKind::Cerebras => "OpenAI Chat Completions",
        ProviderKind::OpenAiCompatible => "OpenAI Chat Completions",
    }
}

fn provider_default_endpoint(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "https://api.anthropic.com",
        ProviderKind::OpenAI => "https://api.openai.com",
        ProviderKind::OpenRouter => "https://openrouter.ai/api",
        ProviderKind::Cerebras => "https://api.cerebras.ai",
        ProviderKind::OpenAiCompatible => "http://localhost:11434 (configurable)",
    }
}

fn provider_credential_field(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "anthropic_api_key",
        ProviderKind::OpenAI => "openai_api_key",
        ProviderKind::OpenRouter => "openrouter_api_key",
        ProviderKind::Cerebras => "cerebras_api_key",
        ProviderKind::OpenAiCompatible => "(none required)",
    }
}

fn render_provider_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = ALL_PROVIDERS
        .iter()
        .enumerate()
        .map(|(i, &kind)| {
            let label = provider_display_name(kind);
            let style = if i == state.workbench_provider_idx {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Span::styled(format!(" {label}"), style))
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" Providers "));
    frame.render_widget(list, area);
}

fn render_provider_detail(frame: &mut Frame, area: Rect, state: &AppState) {
    let idx = state
        .workbench_provider_idx
        .min(ALL_PROVIDERS.len().saturating_sub(1));
    let kind = ALL_PROVIDERS[idx];

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" Name:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                provider_display_name(kind),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Protocol:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(provider_protocol(kind)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Endpoint:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(provider_default_endpoint(kind)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Credential: ", Style::default().fg(Color::DarkGray)),
            Span::raw(provider_credential_field(kind)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Secret:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[set via credentials.json — never entered here]",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let para = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Provider Detail "),
    );
    frame.render_widget(para, area);
}

fn render_source_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let sources = all_sources(false);
    let items: Vec<ListItem> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let enabled_marker = if s.enabled_by_default { "●" } else { "○" };
            let style = if i == state.workbench_source_idx {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if s.enabled_by_default {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            ListItem::new(Span::styled(
                format!(" {enabled_marker} {}", s.title),
                style,
            ))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Data Sources "),
    );
    frame.render_widget(list, area);
}

fn render_source_detail(frame: &mut Frame, area: Rect, state: &AppState) {
    let sources = all_sources(false);
    if sources.is_empty() {
        let empty = Paragraph::new("  (no sources)").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Source Detail "),
        );
        frame.render_widget(empty, area);
        return;
    }
    let idx = state
        .workbench_source_idx
        .min(sources.len().saturating_sub(1));
    let s = sources[idx];

    let auth_text = match s.auth_requirement {
        AuthRequirement::None => "Public — no credentials needed",
        AuthRequirement::Optional => "Optional API key — works without, higher rate limits with",
        AuthRequirement::Required => "API key required",
    };
    let enabled_label = if s.enabled_by_default { "Yes" } else { "No" };
    let credential_label = s.credential_field.unwrap_or("(none)");

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" Title:        ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                s.title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Category:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:?}", s.category)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Enabled:      ", Style::default().fg(Color::DarkGray)),
            Span::raw(enabled_label),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Auth:         ", Style::default().fg(Color::DarkGray)),
            Span::raw(auth_text),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Credential:   ", Style::default().fg(Color::DarkGray)),
            Span::raw(credential_label),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Homepage:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(s.homepage_url),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Description:  ",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    // Wrap description lines manually
    for desc_line in s.description.lines() {
        lines.push(Line::from(vec![Span::raw(format!("   {desc_line}"))]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Access:       ",
        Style::default().fg(Color::DarkGray),
    )]));
    for access_line in s.access_instructions.lines() {
        lines.push(Line::from(vec![Span::raw(format!("   {access_line}"))]));
    }

    let para = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Source Detail "),
    );
    frame.render_widget(para, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::AppState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn tui_renders_without_panic_on_80x24() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = AppState::default();
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }

    #[test]
    fn tui_renders_with_chat_entries() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::default();
        state.chat_log.push(crate::domain::ChatEntry {
            role: ChatRole::User,
            content: "Investigate ACME Corp".into(),
        });
        state.chat_log.push(crate::domain::ChatEntry {
            role: ChatRole::Assistant,
            content: "Starting investigation…".into(),
        });
        state
            .wiki_nodes
            .push(("ACME Corp".into(), "company".into()));
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }

    #[test]
    fn check_minimum_size_rejects_small_terminals() {
        assert!(!check_minimum_size(79, 24));
        assert!(!check_minimum_size(80, 23));
        assert!(check_minimum_size(80, 24));
        assert!(check_minimum_size(200, 60));
    }

    #[test]
    fn banner_is_non_empty() {
        assert!(!BANNER.is_empty());
        assert!(BANNER.contains("┏━┓") || BANNER.contains("redshank"));
    }

    #[test]
    fn tui_renders_workbench_without_panic_on_80x24() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::default();
        state.active_screen = crate::domain::ActiveScreen::Workbench;
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }

    #[test]
    fn tui_renders_chat_screen_by_default() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = AppState::default();
        assert_eq!(state.active_screen, crate::domain::ActiveScreen::Chat);
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }

    #[test]
    fn tui_renders_workbench_with_providers_tab() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::default();
        state.active_screen = crate::domain::ActiveScreen::Workbench;
        state.workbench_tab = crate::domain::WorkbenchTab::Providers;
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }

    #[test]
    fn tui_renders_workbench_with_sources_tab() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::default();
        state.active_screen = crate::domain::ActiveScreen::Workbench;
        state.workbench_tab = crate::domain::WorkbenchTab::Sources;
        terminal.draw(|frame| render(frame, &state)).unwrap();
    }
}
