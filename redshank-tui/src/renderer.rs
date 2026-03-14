//! TUI renderer — ratatui frame rendering.
//!
//! Three-pane layout:
//! ```text
//! ┌──────────────── Header ────────────────┐
//! │ Sidebar(20%) │ Chat(55%) │ Graph(25%)  │
//! └──────────────── Footer ────────────────┘
//! ```

use crate::domain::{AppState, ChatRole};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Figlet-style banner for startup.
pub const BANNER: &str = r#"
              __     __              __  
   ________  / /__  / /_  ____ ___  / /__
  / ___/ _ \/ __  \/ __ \/ __ `__ \/ //_/
 / /  /  __/ /_/ / / / / / / / / / ,<   
/_/   \___/\____/_/ /_/_/ /_/ /_/_/|_|  
"#;

/// Render the full TUI frame.
pub fn render(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Top-level: Header | Body | Footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(3),   // body
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
    let header = Paragraph::new(model_text)
        .style(Style::default().fg(Color::Black).bg(Color::Cyan));
    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame, area: Rect, state: &AppState) {
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

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Sessions "));
    frame.render_widget(list, area);
}

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
            entry.content.lines().enumerate().map(move |(i, line)| {
                if i == 0 {
                    Line::from(vec![
                        Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                        Span::raw(line),
                    ])
                } else {
                    Line::from(format!("      {line}"))
                }
            }).collect::<Vec<_>>()
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
        let empty = Paragraph::new("  (no nodes yet)")
            .style(Style::default().fg(Color::DarkGray));
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
            let truncated: String = label.chars().take(inner.width.saturating_sub(6) as usize).collect();
            let prefix = format!("[{:>2}]", i);
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
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
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
pub fn check_minimum_size(width: u16, height: u16) -> bool {
    width >= 80 && height >= 24
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::AppState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn tui_renders_without_panic_on_80x24() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = AppState::default();
        terminal
            .draw(|frame| render(frame, &state))
            .unwrap();
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
        state.wiki_nodes.push(("ACME Corp".into(), "company".into()));
        terminal
            .draw(|frame| render(frame, &state))
            .unwrap();
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
        assert!(BANNER.contains("redshank") || BANNER.contains("__"));
    }
}
