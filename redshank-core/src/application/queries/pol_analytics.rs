//! `PolAnalytics` query and handler.
//!
//! Analyzes entity state-change frequency from the observation timeline,
//! providing compact summaries for L2 context injection. Example output:
//! "analytics entity=ethereum:0xabc changes=15 period=14d freq=1.1/day trend=accelerating".

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::auth::{AuthContext, StaticPolicy, can_read_session};
use crate::domain::errors::DomainError;
use crate::domain::observation::ObservationDelta;
use crate::ports::observation_store::ObservationStore;

/// Query for analyzing entity `PoL` activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolAnalyticsQuery {
    /// Entity ID to analyze (or empty to analyze all).
    pub entity_id: Option<String>,
    /// Inclusive UTC timestamp lower bound.
    pub since: DateTime<Utc>,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles [`PolAnalyticsQuery`].
pub struct PolAnalyticsHandler<'a, S> {
    store: &'a S,
    policy: StaticPolicy,
}

impl<'a, S: ObservationStore> PolAnalyticsHandler<'a, S> {
    /// Create a handler borrowing an observation store implementation.
    #[must_use]
    pub const fn new(store: &'a S) -> Self {
        Self {
            store,
            policy: StaticPolicy,
        }
    }

    /// Execute the analytics query.
    ///
    /// Returns compact `PoL` analytics lines, one per entity, ordered by change frequency.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadSession`
    /// permission, or storage errors from the underlying observation store.
    pub async fn handle(&self, query: PolAnalyticsQuery) -> Result<Vec<String>, DomainError> {
        can_read_session(&query.auth, &self.policy).map_err(DomainError::Security)?;

        let max_observations = 1000;
        let observations = self
            .store
            .list_recent_observations(&query.auth, query.since, max_observations)
            .await?;

        // Group by entity_id and count state changes (New, Changed, Removed).
        let mut entity_stats: std::collections::HashMap<String, EntityStats> =
            std::collections::HashMap::new();

        for obs in observations {
            if query
                .entity_id
                .as_deref()
                .is_some_and(|id| obs.entity_id != id)
            {
                continue;
            }            // Apply entity_id filter if specified in the query.
            if query
                .entity_id
                .as_deref()
                .is_some_and(|id| obs.entity_id != id)
            {
                continue;
            }            let is_change = matches!(
                obs.delta,
                ObservationDelta::New
                    | ObservationDelta::Changed { .. }
                    | ObservationDelta::Removed
            );
            if is_change {
                entity_stats
                    .entry(obs.entity_id.clone())
                    .or_insert_with(|| EntityStats {
                        entity_id: obs.entity_id,
                        change_count: 0,
                        first_ts: obs.observed_at,
                        last_ts: obs.observed_at,
                    })
                    .record_change(obs.observed_at);
            }
        }

        let now = Utc::now();
        let period_days = (now - query.since).num_days().max(1);

        let mut lines: Vec<String> = entity_stats
            .values()
            .map(|stats| format_analytics_line(stats, period_days, query.since))
            .collect();

        // Sort by change count, descending.
        lines.sort_by(|a, b| {
            let a_count = extract_change_count(a);
            let b_count = extract_change_count(b);
            b_count.cmp(&a_count)
        });

        Ok(lines)
    }
}

#[derive(Debug)]
struct EntityStats {
    entity_id: String,
    change_count: usize,
    first_ts: DateTime<Utc>,
    last_ts: DateTime<Utc>,
}

impl EntityStats {
    fn record_change(&mut self, ts: DateTime<Utc>) {
        self.change_count += 1;
        if ts > self.last_ts {
            self.last_ts = ts;
        }
        if ts < self.first_ts {
            self.first_ts = ts;
        }
    }
}

fn format_analytics_line(stats: &EntityStats, period_days: i64, since: DateTime<Utc>) -> String {
    let change_count_f64 = f64::from(u32::try_from(stats.change_count).unwrap_or(u32::MAX));
    let period_days_f64 = f64::from(i32::try_from(period_days.max(1)).unwrap_or(i32::MAX));
    let freq_per_day = change_count_f64 / period_days_f64;

    // Trend indicator: if most recent change falls in the last 25% of the window
    // (i.e., after the 75th-percentile bound), consider the entity accelerating.
    // Bound = since + (3/4 of the period), so activity after that is "accelerating".
    let recent_bound = since
        + chrono::Duration::days((period_days * 3 / 4).max(1))
            .max(chrono::Duration::seconds(1));
    let trend = if stats.last_ts > recent_bound {
        "accelerating"
    } else {
        "stable"
    };

    format!(
        "analytics entity={} changes={} period={}d freq={:.2}/day trend={}",
        stats.entity_id, stats.change_count, period_days, freq_per_day, trend
    )
}

fn extract_change_count(line: &str) -> usize {
    line.split("changes=")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|count_str| count_str.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn format_analytics_line_computes_frequency() {
        let stats = EntityStats {
            entity_id: "ethereum:0xabc".to_owned(),
            change_count: 14,
            first_ts: Utc::now() - chrono::Duration::days(7),
            last_ts: Utc::now(),
        };

        let line = format_analytics_line(&stats, 7, Utc::now() - chrono::Duration::days(7));
        assert!(line.contains("entity=ethereum:0xabc"));
        assert!(line.contains("changes=14"));
        assert!(line.contains("freq=2.00/day"));
    }

    #[test]
    fn format_analytics_line_trend_recent_activity() {
        let now = Utc::now();
        let start = now - chrono::Duration::days(14);

        // Last change very recent -> accelerating
        let mut stats = EntityStats {
            entity_id: "test:entity".to_owned(),
            change_count: 5,
            first_ts: start,
            last_ts: now - chrono::Duration::hours(1),
        };

        let line = format_analytics_line(&stats, 14, start);
        assert!(line.contains("trend=accelerating"));

        // Last change long ago -> stable
        stats.last_ts = start + chrono::Duration::days(2);
        let line = format_analytics_line(&stats, 14, start);
        assert!(line.contains("trend=stable"));
    }
}
