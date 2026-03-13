//! `ReplayLog` port — JSONL delta-encoded LLM call logging.

use crate::domain::errors::DomainError;
use serde_json::Value;

/// Port trait for replay logging.
///
/// Uses `impl Future` (RPITIT) for async methods. Not dyn-compatible —
/// use generics (`T: ReplayLog`) rather than `dyn ReplayLog`.
pub trait ReplayLog: Send + Sync {
    /// Append a log record (full snapshot or delta).
    fn append(
        &self,
        record: &Value,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;

    /// Create a child logger for a subtask, returning the subtask path.
    fn child_path(&self, subtask_id: &str) -> String;
}
