//! Entity observation types used for pattern-of-life tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Delta classification against the previous observation for an entity/source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationDelta {
    /// First observation for this entity/source pair.
    New,
    /// Same payload hash as the immediately previous observation.
    Unchanged,
    /// Payload hash differs from the previous observation.
    Changed {
        /// Previous payload hash.
        previous_hash: String,
    },
    /// Observation is no longer present in source data.
    Removed,
}

/// A normalized entity observation persisted for temporal analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityObservation {
    /// Unique observation identifier.
    pub id: Uuid,
    /// Canonical entity identifier.
    pub entity_id: String,
    /// Source descriptor identifier.
    pub source_id: String,
    /// Observation timestamp in UTC.
    pub observed_at: DateTime<Utc>,
    /// Content hash of the normalized observation payload.
    pub payload_hash: String,
    /// Delta classification against prior observation state.
    pub delta: ObservationDelta,
}

impl EntityObservation {
    /// Build a new observation with a random identifier.
    #[must_use]
    pub fn new(
        entity_id: String,
        source_id: String,
        observed_at: DateTime<Utc>,
        payload_hash: String,
        delta: ObservationDelta,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            entity_id,
            source_id,
            observed_at,
            payload_hash,
            delta,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn observation_roundtrip_serde() {
        let obs = EntityObservation::new(
            "ethereum:0xabc".to_owned(),
            "blockchain_explorer".to_owned(),
            Utc::now(),
            "deadbeef".to_owned(),
            ObservationDelta::New,
        );

        let json = serde_json::to_string(&obs).unwrap();
        let restored: EntityObservation = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.entity_id, "ethereum:0xabc");
        assert_eq!(restored.source_id, "blockchain_explorer");
        assert!(matches!(restored.delta, ObservationDelta::New));
    }
}
