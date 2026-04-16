//! Context-window tracking and conversation condensation.
//!
//! When token usage exceeds 75 % of the model's context window, old tool
//! results are replaced with `[earlier tool output condensed]` placeholders
//! and, optionally, a judge model is asked to summarise the investigation so
//! far into a single condensation turn.

use crate::domain::errors::DomainError;
use crate::domain::session::TurnSummary;
use crate::ports::model_provider::{ChatMessage, ModelProvider, ToolDefinition};

/// Fraction of the context window at which condensation triggers.
const CONDENSATION_THRESHOLD: f64 = 0.75;

/// Placeholder text that replaces condensed tool outputs.
const CONDENSED_PLACEHOLDER: &str = "[earlier tool output condensed]";

/// Number of recent tool-result messages to keep intact.
const KEEP_RECENT_TURNS: usize = 4;

/// Well-known model context window sizes (tokens).
///
/// Models not listed here fall back to [`DEFAULT_CONTEXT_WINDOW`].
#[must_use]
pub fn model_context_window(model: &str) -> u64 {
    let lower = model.to_ascii_lowercase();
    if lower.contains("claude") {
        return 200_000;
    }
    if lower.starts_with("gpt-4.1") || lower.starts_with("gpt-4o") {
        return 128_000;
    }
    if lower.starts_with("o4-mini") || lower.starts_with("o3") {
        return 128_000;
    }
    if lower.contains("cerebras") || lower.contains("llama") {
        return 128_000;
    }
    // Ollama default / fallback
    DEFAULT_CONTEXT_WINDOW
}

/// Default context window for unknown models.
pub const DEFAULT_CONTEXT_WINDOW: u64 = 128_000;

/// Tracks context usage and decides when condensation is needed.
#[derive(Debug, Clone)]
pub struct ContextTracker {
    /// Model context window size in tokens.
    window_size: u64,
    /// Tokens used by the most recent model call.
    used_tokens: u64,
    /// Accumulated turn summaries for the current session.
    turn_summaries: Vec<TurnSummary>,
}

impl ContextTracker {
    /// Create a tracker for a model with the given context window.
    #[must_use]
    pub const fn new(window_size: u64) -> Self {
        Self {
            window_size,
            used_tokens: 0,
            turn_summaries: Vec::new(),
        }
    }

    /// Create a tracker from a model name (looks up the context window).
    #[must_use]
    pub fn for_model(model_name: &str) -> Self {
        Self::new(model_context_window(model_name))
    }

    /// Update the recorded token usage.
    pub const fn set_used_tokens(&mut self, tokens: u64) {
        self.used_tokens = tokens;
    }

    /// Whether context usage exceeds the condensation threshold.
    #[must_use]
    // Token counts are well below 2^52; precision loss from u64→f64 is acceptable for threshold comparison.
    #[allow(clippy::cast_precision_loss)]
    pub fn should_condense(&self) -> bool {
        self.window_size > 0
            && (self.used_tokens as f64) > CONDENSATION_THRESHOLD * (self.window_size as f64)
    }

    /// Current token usage.
    #[must_use]
    pub const fn used_tokens(&self) -> u64 {
        self.used_tokens
    }

    /// Context window size.
    #[must_use]
    pub const fn window_size(&self) -> u64 {
        self.window_size
    }

    /// Append a turn summary.
    pub fn push_summary(&mut self, summary: TurnSummary) {
        self.turn_summaries.push(summary);
    }

    /// All accumulated turn summaries.
    #[must_use]
    pub fn summaries(&self) -> &[TurnSummary] {
        &self.turn_summaries
    }
}

/// Condense old tool-result messages in-place.
///
/// Finds all messages with `role == "tool"` and replaces all but the most
/// recent `KEEP_RECENT_TURNS` turns with a short placeholder. Returns the number
/// of messages condensed.
pub fn condense_tool_outputs(messages: &mut [ChatMessage]) -> usize {
    let tool_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == "tool")
        .map(|(i, _)| i)
        .collect();

    if tool_indices.len() <= KEEP_RECENT_TURNS {
        return 0;
    }

    let end = tool_indices.len() - KEEP_RECENT_TURNS;
    let to_condense = tool_indices.get(..end).unwrap_or_default();
    let mut condensed = 0;
    for &idx in to_condense {
        if let Some(msg) = messages.get_mut(idx)
            && msg.content != CONDENSED_PLACEHOLDER
        {
            msg.content.clear();
            msg.content.push_str(CONDENSED_PLACEHOLDER);
            condensed += 1;
        }
    }
    condensed
}

/// Ask a judge model to produce a condensation summary, then rebuild the
/// conversation with a single `<condensation>` system turn replacing the
/// condensed messages.
///
/// Returns the new, shorter conversation.
///
/// The original user objective (first user message) is always preserved.
///
/// # Errors
///
/// Returns `Err` if the judge model fails to produce a completion.
pub async fn condense_with_judge<M: ModelProvider>(
    messages: &[ChatMessage],
    judge: &M,
) -> Result<Vec<ChatMessage>, DomainError> {
    // Find the first user message (the objective).
    let first_user_idx = messages.iter().position(|m| m.role == "user").unwrap_or(0);

    // Build the summary request.
    let summary_prompt = ChatMessage {
        role: "user".to_owned(),
        content: "Summarise the investigation so far in 400 words. \
                  Focus on: (1) what we set out to find, (2) what we discovered, \
                  (3) open questions remaining."
            .to_owned(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    };

    let summary_turn = judge
        .complete(&[summary_prompt], &[] as &[ToolDefinition])
        .await?;

    let summary_text = summary_turn
        .content
        .unwrap_or_else(|| "Investigation summary unavailable.".to_owned());

    // Rebuild: keep the original objective + a condensation turn + recent messages.
    let mut result = Vec::new();

    // Preserve the objective.
    if let Some(first_msg) = messages.get(first_user_idx) {
        result.push(first_msg.clone());
    }

    // Insert condensation turn.
    result.push(ChatMessage {
        role: "system".to_owned(),
        content: format!("<condensation>\n{summary_text}\n</condensation>"),
        tool_calls: Vec::new(),
        tool_call_id: None,
    });

    // Keep the last KEEP_RECENT_TURNS * 2 messages (assistant + tool pairs).
    let keep_count = KEEP_RECENT_TURNS * 2;
    let start = if messages.len() > keep_count {
        messages.len() - keep_count
    } else {
        first_user_idx + 1
    };
    for msg in messages.get(start..).unwrap_or_default() {
        result.push(msg.clone());
    }

    Ok(result)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_owned(),
            content: content.to_owned(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    #[test]
    fn should_condense_false_at_74_percent() {
        let mut tracker = ContextTracker::new(100_000);
        tracker.set_used_tokens(74_000);
        assert!(!tracker.should_condense());
    }

    #[test]
    fn should_condense_true_at_76_percent() {
        let mut tracker = ContextTracker::new(100_000);
        tracker.set_used_tokens(76_000);
        assert!(tracker.should_condense());
    }

    #[test]
    fn should_condense_exact_threshold() {
        let mut tracker = ContextTracker::new(100_000);
        // Exactly 75% — not over, so should not condense.
        tracker.set_used_tokens(75_000);
        assert!(!tracker.should_condense());
    }

    #[test]
    fn should_condense_zero_window() {
        let mut tracker = ContextTracker::new(0);
        tracker.set_used_tokens(99_999);
        assert!(!tracker.should_condense());
    }

    #[test]
    fn condense_tool_outputs_preserves_recent() {
        let mut msgs = vec![
            msg("user", "investigate X"),
            msg("assistant", "calling tool 1"),
            msg("tool", "result 1"),
            msg("assistant", "calling tool 2"),
            msg("tool", "result 2"),
            msg("assistant", "calling tool 3"),
            msg("tool", "result 3"),
            msg("assistant", "calling tool 4"),
            msg("tool", "result 4"),
            msg("assistant", "calling tool 5"),
            msg("tool", "result 5"),
            msg("assistant", "calling tool 6"),
            msg("tool", "result 6"),
        ];
        let condensed = condense_tool_outputs(&mut msgs);
        assert_eq!(condensed, 2);
        // First 2 tool results condensed.
        assert_eq!(msgs[2].content, CONDENSED_PLACEHOLDER);
        assert_eq!(msgs[4].content, CONDENSED_PLACEHOLDER);
        // Last 4 kept.
        assert_eq!(msgs[6].content, "result 3");
        assert_eq!(msgs[8].content, "result 4");
        assert_eq!(msgs[10].content, "result 5");
        assert_eq!(msgs[12].content, "result 6");
    }

    #[test]
    fn condense_tool_outputs_noop_when_few() {
        let mut msgs = vec![
            msg("user", "investigate X"),
            msg("assistant", "calling tool 1"),
            msg("tool", "result 1"),
            msg("assistant", "calling tool 2"),
            msg("tool", "result 2"),
        ];
        let condensed = condense_tool_outputs(&mut msgs);
        assert_eq!(condensed, 0);
    }

    #[test]
    fn condense_idempotent() {
        let mut msgs = vec![
            msg("user", "go"),
            msg("tool", "r1"),
            msg("tool", "r2"),
            msg("tool", "r3"),
            msg("tool", "r4"),
            msg("tool", "r5"),
            msg("tool", "r6"),
        ];
        let first = condense_tool_outputs(&mut msgs);
        let second = condense_tool_outputs(&mut msgs);
        assert_eq!(first, 2);
        assert_eq!(second, 0); // Already condensed.
    }

    #[test]
    fn turn_summary_accumulation() {
        let mut tracker = ContextTracker::for_model("claude-sonnet-4-5");
        assert_eq!(tracker.window_size(), 200_000);

        tracker.push_summary(TurnSummary {
            turn: 0,
            summary: "Found initial leads".into(),
            tool_names: vec!["web_search".into()],
            timestamp: Utc::now(),
        });
        tracker.push_summary(TurnSummary {
            turn: 1,
            summary: "Fetched SEC filings".into(),
            tool_names: vec!["fetch_url".into()],
            timestamp: Utc::now(),
        });
        assert_eq!(tracker.summaries().len(), 2);
    }

    #[test]
    fn model_context_windows() {
        assert_eq!(model_context_window("claude-sonnet-4-5"), 200_000);
        assert_eq!(model_context_window("claude-haiku-4-5"), 200_000);
        assert_eq!(model_context_window("gpt-4.1"), 128_000);
        assert_eq!(model_context_window("gpt-4o"), 128_000);
        assert_eq!(model_context_window("o4-mini"), 128_000);
        assert_eq!(
            model_context_window("unknown-model"),
            DEFAULT_CONTEXT_WINDOW
        );
    }
}
