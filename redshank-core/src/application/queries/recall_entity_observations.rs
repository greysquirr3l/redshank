//! `RecallEntityObservations` query and handler.
//!
//! Queries the [`ObservationStore`] for recent cross-entity `PoL` records and
//! formats them into compact context lines for L2 prompt injection.  Unlike
//! [`super::recall_observations`], which recalls domain events from the event
//! log, this handler returns the actual temporal observation timeline — i.e.
//! what external sources said about tracked entities and when they changed.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::auth::{AuthContext, StaticPolicy, can_read_session};
use crate::domain::errors::DomainError;
use crate::domain::observation::{EntityObservation, ObservationDelta};
use crate::ports::observation_store::ObservationStore;

/// Query for recalling recent entity observations as `PoL` context lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallEntityObservationsQuery {
    /// Inclusive UTC timestamp lower bound.
    pub since: DateTime<Utc>,
    /// Maximum number of lines returned.
    pub max_items: usize,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles [`RecallEntityObservationsQuery`].
pub struct RecallEntityObservationsHandler<'a, S> {
    store: &'a S,
    policy: StaticPolicy,
}

impl<'a, S: ObservationStore> RecallEntityObservationsHandler<'a, S> {
    /// Create a handler borrowing an observation store implementation.
    #[must_use]
    pub const fn new(store: &'a S) -> Self {
        Self {
            store,
            policy: StaticPolicy,
        }
    }

    /// Execute the recall query.
    ///
    /// Returns compact `PoL` lines ordered newest-first, capped to
    /// `query.max_items`.  Lines are suitable for direct injection into L2
    /// `add_on_demand` context slots.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadSession`
    /// permission, or storage errors from the underlying observation store.
    pub async fn handle(
        &self,
        query: RecallEntityObservationsQuery,
    ) -> Result<Vec<String>, DomainError> {
        can_read_session(&query.auth, &self.policy).map_err(DomainError::Security)?;
        if query.max_items == 0 {
            return Ok(Vec::new());
        }

        let observations = self
            .store
            .list_recent_observations(&query.auth, query.since, query.max_items)
            .await?;

        Ok(observations.iter().map(format_pol_line).collect())
    }
}

/// Format a single `EntityObservation` as a compact `PoL` context line.
fn format_pol_line(obs: &EntityObservation) -> String {
    let ts = obs.observed_at.to_rfc3339();
    let delta = match &obs.delta {
        ObservationDelta::New => "new".to_owned(),
        ObservationDelta::Unchanged => "unchanged".to_owned(),
        ObservationDelta::Changed { previous_hash } => {
            format!("changed prev={previous_hash}")
        }
        ObservationDelta::Removed => "removed".to_owned(),
    };
    format!(
        "{ts} pol entity={} source={} delta={delta} hash={}",
        obs.entity_id, obs.source_id, obs.payload_hash
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::domain::observation::ObservationDelta;

    #[test]
    fn format_pol_line_new() {
        let obs = EntityObservation::new(
            "ethereum:0xabc".to_owned(),
            "blockchain_explorer".to_owned(),
            Utc::now(),
            "deadbeef".to_owned(),
            ObservationDelta::New,
        );
        let line = format_pol_line(&obs);
        assert!(line.contains("pol entity=ethereum:0xabc"));
        assert!(line.contains("source=blockchain_explorer"));
        assert!(line.contains("delta=new"));
        assert!(line.contains("hash=deadbeef"));
    }

    #[test]
    fn format_pol_line_changed() {
        let obs = EntityObservation::new(
            "ethereum:0xdef".to_owned(),
            "blockchain_explorer".to_owned(),
            Utc::now(),
            "cafebabe".to_owned(),
            ObservationDelta::Changed {
                previous_hash: "deadbeef".to_owned(),
            },
        );
        let line = format_pol_line(&obs);
        assert!(line.contains("delta=changed prev=deadbeef"));
        assert!(line.contains("hash=cafebabe"));
    }

    #[test]
    fn format_pol_line_unchanged() {
        let obs = EntityObservation::new(
            "btc:bc1q".to_owned(),
            "blockchain_explorer".to_owned(),
            Utc::now(),
            "aabb1234".to_owned(),
            ObservationDelta::Unchanged,
        );
        let line = format_pol_line(&obs);
        assert!(line.contains("delta=unchanged"));
    }

    #[test]
    fn format_pol_line_removed() {
        let obs = EntityObservation::new(
            "ethereum:0x999".to_owned(),
            "blockchain_explorer".to_owned(),
            Utc::now(),
            "00000000".to_owned(),
            ObservationDelta::Removed,
        );
        let line = format_pol_line(&obs);
        assert!(line.contains("delta=removed"));
    }
}
