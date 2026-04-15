//! Temporal knowledge-graph value types.
//!
//! These pure domain types represent time-bounded relationships between
//! entities and can be reused by adapters without introducing I/O concerns.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Inclusive/exclusive validity interval for a temporal fact.
///
/// `valid_from` is inclusive and `valid_to` is exclusive when present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalInterval {
    /// UTC timestamp when the fact becomes valid.
    pub valid_from: DateTime<Utc>,
    /// UTC timestamp when the fact ceases to be valid.
    pub valid_to: Option<DateTime<Utc>>,
}

impl TemporalInterval {
    /// Returns true when `ts` falls within this interval.
    #[must_use]
    pub fn contains(&self, ts: DateTime<Utc>) -> bool {
        if ts < self.valid_from {
            return false;
        }
        self.valid_to.is_none_or(|end| ts < end)
    }
}

/// A time-bounded triple describing a relationship between entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalTriple {
    /// Subject entity identifier or canonical label.
    pub subject: String,
    /// Predicate connecting the subject and object.
    pub predicate: String,
    /// Object entity identifier or canonical label.
    pub object: String,
    /// Validity interval for the relationship.
    pub interval: TemporalInterval,
    /// Source descriptor ID for provenance.
    pub source_id: String,
    /// Optional evidence payload reference.
    pub evidence_ref: Option<String>,
}

impl TemporalTriple {
    /// Returns true if this triple is active at `ts`.
    #[must_use]
    pub fn is_active_at(&self, ts: DateTime<Utc>) -> bool {
        self.interval.contains(ts)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn interval_contains_open_ended_range() {
        let start = Utc::now();
        let interval = TemporalInterval {
            valid_from: start,
            valid_to: None,
        };

        assert!(interval.contains(start));
        assert!(interval.contains(start + Duration::hours(1)));
        assert!(!interval.contains(start - Duration::seconds(1)));
    }

    #[test]
    fn interval_uses_exclusive_upper_bound() {
        let start = Utc::now();
        let end = start + Duration::hours(2);
        let interval = TemporalInterval {
            valid_from: start,
            valid_to: Some(end),
        };

        assert!(interval.contains(start + Duration::hours(1)));
        assert!(!interval.contains(end));
    }

    #[test]
    fn temporal_triple_roundtrip_and_activity() {
        let start = Utc::now();
        let triple = TemporalTriple {
            subject: "acme-holdings".to_owned(),
            predicate: "controls".to_owned(),
            object: "port-asset-77".to_owned(),
            interval: TemporalInterval {
                valid_from: start,
                valid_to: None,
            },
            source_id: "opencorporates".to_owned(),
            evidence_ref: Some("doc:oc:123".to_owned()),
        };

        let json = serde_json::to_string(&triple).unwrap();
        let restored: TemporalTriple = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.subject, "acme-holdings");
        assert!(restored.is_active_at(start + Duration::minutes(30)));
    }
}
