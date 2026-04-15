//! `ObservationStore` port — temporal observation persistence.

use chrono::{DateTime, Utc};

use crate::domain::auth::AuthContext;
use crate::domain::errors::DomainError;
use crate::domain::observation::EntityObservation;

/// Port trait for observation persistence and recall.
///
/// Uses RPITIT — not dyn-compatible. Use generics (`T: ObservationStore`).
pub trait ObservationStore: Send + Sync {
    /// Persist a single observation record.
    fn append_observation(
        &self,
        auth: &AuthContext,
        observation: &EntityObservation,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;

    /// Return the most recent observation for an entity/source pair.
    fn latest_observation(
        &self,
        auth: &AuthContext,
        entity_id: &str,
        source_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<EntityObservation>, DomainError>> + Send;

    /// List observations for an entity since a timestamp, newest first.
    fn list_observations_since(
        &self,
        auth: &AuthContext,
        entity_id: &str,
        since: DateTime<Utc>,
        max_items: usize,
    ) -> impl std::future::Future<Output = Result<Vec<EntityObservation>, DomainError>> + Send;

    /// List all observations across all entities since a timestamp, newest first.
    ///
    /// Used for L2 context injection — returns a cross-entity `PoL` timeline.
    fn list_recent_observations(
        &self,
        auth: &AuthContext,
        since: DateTime<Utc>,
        max_items: usize,
    ) -> impl std::future::Future<Output = Result<Vec<EntityObservation>, DomainError>> + Send;
}
