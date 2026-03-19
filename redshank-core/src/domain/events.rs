//! Domain events emitted by aggregate state transitions.
//!
//! All variants carry `session_id` and `timestamp`. Events are immutable
//! value types — no mutation methods.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::agent::AgentConfig;
use super::session::SessionId;
use super::wiki::WikiCategory;

/// Typed domain event variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    /// A new session was created.
    SessionCreated {
        /// Session ID.
        session_id: SessionId,
        /// The configuration used.
        config: AgentConfig,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// An agent investigation was started.
    AgentStarted {
        /// Session ID.
        session_id: SessionId,
        /// Investigation objective.
        objective: String,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// A tool was called during an investigation.
    ToolCalled {
        /// Session ID.
        session_id: SessionId,
        /// Tool name.
        tool_name: String,
        /// Summary of arguments (for logging, not full args).
        args_summary: String,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// An investigation completed successfully.
    AgentCompleted {
        /// Session ID.
        session_id: SessionId,
        /// Summary of findings.
        result_summary: String,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// An investigation failed.
    InvestigationFailed {
        /// Session ID.
        session_id: SessionId,
        /// Error description.
        error: String,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// A wiki entry was written.
    WikiEntryWritten {
        /// Session ID.
        session_id: SessionId,
        /// File path of the entry.
        entry_path: PathBuf,
        /// Category of the entry.
        category: WikiCategory,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
}

impl DomainEvent {
    /// Get the session ID from any event variant.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        match self {
            Self::SessionCreated { session_id, .. }
            | Self::AgentStarted { session_id, .. }
            | Self::ToolCalled { session_id, .. }
            | Self::AgentCompleted { session_id, .. }
            | Self::InvestigationFailed { session_id, .. }
            | Self::WikiEntryWritten { session_id, .. } => *session_id,
        }
    }

    /// Get the timestamp from any event variant.
    #[must_use]
    pub const fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::SessionCreated { timestamp, .. }
            | Self::AgentStarted { timestamp, .. }
            | Self::ToolCalled { timestamp, .. }
            | Self::AgentCompleted { timestamp, .. }
            | Self::InvestigationFailed { timestamp, .. }
            | Self::WikiEntryWritten { timestamp, .. } => *timestamp,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::agent::AgentConfig;

    #[test]
    fn all_variants_carry_session_id_and_timestamp() {
        let sid = SessionId::new();
        let ts = Utc::now();

        let events: Vec<DomainEvent> = vec![
            DomainEvent::SessionCreated {
                session_id: sid,
                config: AgentConfig::default(),
                timestamp: ts,
            },
            DomainEvent::AgentStarted {
                session_id: sid,
                objective: "test".to_string(),
                timestamp: ts,
            },
            DomainEvent::ToolCalled {
                session_id: sid,
                tool_name: "web_search".to_string(),
                args_summary: "q=test".to_string(),
                timestamp: ts,
            },
            DomainEvent::AgentCompleted {
                session_id: sid,
                result_summary: "done".to_string(),
                timestamp: ts,
            },
            DomainEvent::InvestigationFailed {
                session_id: sid,
                error: "timeout".to_string(),
                timestamp: ts,
            },
            DomainEvent::WikiEntryWritten {
                session_id: sid,
                entry_path: PathBuf::from("wiki/test.md"),
                category: WikiCategory::Corporate,
                timestamp: ts,
            },
        ];

        for event in &events {
            assert_eq!(event.session_id(), sid);
            assert_eq!(event.timestamp(), ts);
        }
    }

    #[test]
    fn domain_event_roundtrip_serde() {
        let event = DomainEvent::AgentStarted {
            session_id: SessionId::new(),
            objective: "investigate".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(restored, DomainEvent::AgentStarted { .. }));
    }
}
