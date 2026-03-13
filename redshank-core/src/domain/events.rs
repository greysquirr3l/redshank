//! Domain events emitted by aggregate state transitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Typed domain event variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    /// A new investigation session was started.
    InvestigationStarted {
        /// Session ID.
        session_id: Uuid,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// A tool was executed during an investigation.
    ToolExecuted {
        /// Session ID.
        session_id: Uuid,
        /// Tool name.
        tool_name: String,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// A single agent turn was completed.
    TurnCompleted {
        /// Session ID.
        session_id: Uuid,
        /// Turn number.
        turn: u32,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// Context was condensed due to window limits.
    ContextCondensed {
        /// Session ID.
        session_id: Uuid,
        /// Tokens before condensation.
        tokens_before: u64,
        /// Tokens after condensation.
        tokens_after: u64,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// An investigation completed successfully.
    InvestigationCompleted {
        /// Session ID.
        session_id: Uuid,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// An investigation failed.
    InvestigationFailed {
        /// Session ID.
        session_id: Uuid,
        /// Error description.
        error: String,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
    /// A wiki entry was written.
    WikiEntryWritten {
        /// Session ID.
        session_id: Uuid,
        /// Entry title.
        title: String,
        /// Timestamp.
        timestamp: DateTime<Utc>,
    },
}
