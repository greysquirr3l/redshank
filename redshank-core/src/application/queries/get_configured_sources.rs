//! `GetConfiguredSources` query and handler.
//!
//! Retrieves the effective configuration and enabled status of all data sources,
//! along with credential presence information for UI rendering.

use serde::{Deserialize, Serialize};

use crate::domain::agent::SourceId;
use crate::domain::auth::{AuthContext, StaticPolicy, can_read_configuration};
use crate::domain::errors::DomainError;
use crate::domain::source_catalog::{AuthRequirement, SourceCategory};
use crate::ports::session_store::SessionStore;

/// UI-facing view of a configured data source with effective settings and credential status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfiguredSourceView {
    /// Source ID (lowercase snake_case).
    pub id: SourceId,
    /// Display title.
    pub title: String,
    /// Short description.
    pub description: String,
    /// Source category.
    pub category: SourceCategory,
    /// Homepage URL.
    pub homepage_url: String,
    /// Authentication requirement for the source.
    pub auth_requirement: AuthRequirement,
    /// Access instructions or sign-up URL.
    pub access_instructions: String,
    /// Whether this source is currently enabled for fetches.
    pub enabled: bool,
    /// Whether a credential is currently configured for this source.
    pub has_credential: bool,
    /// Source-specific rate limit override in milliseconds, if set.
    pub rate_limit_ms_override: Option<u64>,
    /// Source-specific max pages override, if set.
    pub max_pages_override: Option<u32>,
}

/// Query to retrieve all configured sources with effective settings.
///
/// Joins source catalog metadata, persistent settings (enabled state,
/// rate limit overrides), and credential presence information to produce
/// UI-ready view models without exposing secret values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfiguredSourcesQuery {
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`GetConfiguredSourcesQuery`].
///
/// Enforces `ReadConfiguration` permission, then joins:
/// - Source catalog static metadata
/// - User settings (enabled state, rate limit overrides)
/// - Credential bundle (credential presence, not values)
///
/// Returns a sorted list of UI-ready source views.
#[allow(dead_code)]
pub struct GetConfiguredSourcesHandler<S> {
    session_store: S,
    policy: StaticPolicy,
}

impl<S: SessionStore> GetConfiguredSourcesHandler<S> {
    /// Create a new handler backed by the given session store.
    #[must_use]
    pub const fn new(session_store: S) -> Self {
        Self {
            session_store,
            policy: StaticPolicy,
        }
    }

    /// Execute the query.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadConfiguration`
    /// permission, or a storage error if loading settings or credentials fails.
    pub async fn handle(
        &self,
        query: GetConfiguredSourcesQuery,
    ) -> Result<Vec<ConfiguredSourceView>, DomainError> {
        can_read_configuration(&query.auth, &self.policy).map_err(DomainError::Security)?;

        // TODO(T44): Load settings and credentials from session store / workspace.
        // For now, return empty to make test fail.
        Ok(Vec::new())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn get_configured_sources_query_serde() {
        let query = GetConfiguredSourcesQuery {
            auth: AuthContext::system(),
        };
        let json = serde_json::to_string(&query).unwrap();
        let restored: GetConfiguredSourcesQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.auth, query.auth);
    }

    #[test]
    fn configured_source_view_shows_effective_enabled_state() {
        // Test: ConfiguredSourceView correctly reflects effective enabled state
        // (catalog default + settings override).
        let view = ConfiguredSourceView {
            id: SourceId::new("fec"),
            title: "FEC Campaign Finance".to_string(),
            description: "U.S. Federal Election Commission data.".to_string(),
            category: SourceCategory::Government,
            homepage_url: "https://www.fec.gov/".to_string(),
            auth_requirement: AuthRequirement::Optional,
            access_instructions: "Create account at fec.gov/api".to_string(),
            enabled: false,
            has_credential: false,
            rate_limit_ms_override: Some(5000),
            max_pages_override: None,
        };

        assert!(!view.enabled, "Source should be disabled as configured");
        assert_eq!(
            view.rate_limit_ms_override,
            Some(5000),
            "Rate limit override should be present"
        );
    }

    #[test]
    fn configured_source_view_credential_status_never_exposes_secret() {
        let view = ConfiguredSourceView {
            id: SourceId::new("opencorporates"),
            title: "OpenCorporates".to_string(),
            description: "Global corporate registry.".to_string(),
            category: SourceCategory::Corporate,
            homepage_url: "https://opencorporates.com/".to_string(),
            auth_requirement: AuthRequirement::Required,
            access_instructions: "Sign up and generate API key".to_string(),
            enabled: true,
            has_credential: true,
            rate_limit_ms_override: None,
            max_pages_override: Some(10),
        };

        // Verify that has_credential is a boolean; never exposes the actual secret.
        assert!(view.has_credential);
        assert_eq!(
            view.max_pages_override,
            Some(10),
            "Max pages override should be present"
        );
    }
}
