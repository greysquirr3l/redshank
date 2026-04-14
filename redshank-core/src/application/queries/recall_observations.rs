//! `RecallObservations` query and handler.
//!
//! Recalls recent domain events for a session and formats them into compact
//! context lines for L1/L2 prompt injection.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::auth::{AuthContext, StaticPolicy, can_read_session};
use crate::domain::errors::DomainError;
use crate::domain::events::DomainEvent;
use crate::domain::session::SessionId;
use crate::ports::session_store::SessionStore;

/// Query for recalling recent observations from the event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallObservationsQuery {
    /// Session ID to inspect.
    pub session_id: SessionId,
    /// Inclusive UTC timestamp lower bound.
    pub since: DateTime<Utc>,
    /// Maximum number of lines returned.
    pub max_items: usize,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles [`RecallObservationsQuery`].
pub struct RecallObservationsHandler<'a, S> {
    session_store: &'a S,
    policy: StaticPolicy,
}

impl<'a, S: SessionStore> RecallObservationsHandler<'a, S> {
    /// Create a handler borrowing a session store implementation.
    #[must_use]
    pub const fn new(session_store: &'a S) -> Self {
        Self {
            session_store,
            policy: StaticPolicy,
        }
    }

    /// Execute the recall query.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadSession`
    /// permission, or storage errors from the underlying session store.
    pub async fn handle(&self, query: RecallObservationsQuery) -> Result<Vec<String>, DomainError> {
        can_read_session(&query.auth, &self.policy).map_err(DomainError::Security)?;
        if query.max_items == 0 {
            return Ok(Vec::new());
        }

        let events = self
            .session_store
            .list_events(&query.auth, query.session_id)
            .await?;

        let mut lines: Vec<String> = events
            .into_iter()
            .filter(|event| event.timestamp() >= query.since)
            .filter_map(|event| format_event_line(&event))
            .collect();

        if lines.len() > query.max_items {
            let keep_from = lines.len() - query.max_items;
            lines = lines.split_off(keep_from);
        }

        Ok(lines)
    }
}

fn format_event_line(event: &DomainEvent) -> Option<String> {
    let ts = event.timestamp().to_rfc3339();
    match event {
        DomainEvent::ToolCalled {
            tool_name,
            args_summary,
            ..
        } => Some(format!("{ts} tool={tool_name} args={args_summary}")),
        DomainEvent::WikiEntryWritten {
            entry_path,
            category,
            ..
        } => Some(format!(
            "{ts} wiki={} category={category:?}",
            entry_path.display()
        )),
        DomainEvent::AgentCompleted { result_summary, .. } => {
            Some(format!("{ts} completed={result_summary}"))
        }
        DomainEvent::InvestigationFailed { error, .. } => Some(format!("{ts} failed={error}")),
        DomainEvent::AgentStarted { objective, .. } => Some(format!("{ts} objective={objective}")),
        DomainEvent::SessionCreated { .. } => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn recall_observations_query_roundtrip_serde() {
        let query = RecallObservationsQuery {
            session_id: SessionId::new(),
            since: Utc::now(),
            max_items: 8,
            auth: AuthContext::system(),
        };

        let json = serde_json::to_string(&query).unwrap();
        let restored: RecallObservationsQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.session_id, query.session_id);
        assert_eq!(restored.max_items, 8);
    }

    #[test]
    fn format_event_line_maps_relevant_variants() {
        let sid = SessionId::new();
        let ts = Utc::now();

        let tool = DomainEvent::ToolCalled {
            session_id: sid,
            tool_name: "web_search".to_owned(),
            args_summary: "q=acme".to_owned(),
            timestamp: ts,
        };
        let line = format_event_line(&tool).unwrap();
        assert!(line.contains("tool=web_search"));

        let created = DomainEvent::SessionCreated {
            session_id: sid,
            config: crate::domain::agent::AgentConfig::default(),
            timestamp: ts,
        };
        assert!(format_event_line(&created).is_none());
    }
}
