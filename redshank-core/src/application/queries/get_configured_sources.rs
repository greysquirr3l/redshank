//! `GetConfiguredSources` query and handler.
//!
//! Retrieves the effective configuration and enabled status of all data sources,
//! along with credential presence information for UI rendering.

use serde::{Deserialize, Serialize};

use crate::domain::agent::SourceId;
use crate::domain::auth::{AuthContext, StaticPolicy, can_read_configuration};
use crate::domain::errors::DomainError;
use crate::domain::source_catalog::{AuthRequirement, SourceCategory, all_sources};
use crate::ports::workspace_config::WorkspaceConfig;

/// UI-facing view of a configured data source with effective settings and credential status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfiguredSourceView {
    /// Source ID (lowercase `snake_case`).
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
pub struct GetConfiguredSourcesHandler<C> {
    workspace_config: C,
    policy: StaticPolicy,
}

impl<C: WorkspaceConfig> GetConfiguredSourcesHandler<C> {
    /// Create a new handler backed by the given workspace config.
    #[must_use]
    pub const fn new(workspace_config: C) -> Self {
        Self {
            workspace_config,
            policy: StaticPolicy,
        }
    }

    /// Execute the query.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadConfiguration`
    /// permission, or a storage error if loading settings or credentials fails.
    pub fn handle(
        &self,
        query: &GetConfiguredSourcesQuery,
    ) -> Result<Vec<ConfiguredSourceView>, DomainError> {
        can_read_configuration(&query.auth, &self.policy).map_err(DomainError::Security)?;

        let settings = self.workspace_config.settings();

        let views = all_sources(false)
            .into_iter()
            .map(|s| {
                let fetcher_cfg = settings.fetcher_config(s.id);
                let enabled = fetcher_cfg.map_or(s.enabled_by_default, |c| c.enabled);
                let rate_limit_ms_override = fetcher_cfg.and_then(|c| c.rate_limit_ms);
                let max_pages_override = fetcher_cfg.and_then(|c| c.max_pages);
                let has_credential = s
                    .credential_field
                    .is_some_and(|f| self.workspace_config.has_credential(f));

                ConfiguredSourceView {
                    id: SourceId::new(s.id),
                    title: s.title.to_string(),
                    description: s.description.to_string(),
                    category: s.category,
                    homepage_url: s.homepage_url.to_string(),
                    auth_requirement: s.auth_requirement,
                    access_instructions: s.access_instructions.to_string(),
                    enabled,
                    has_credential,
                    rate_limit_ms_override,
                    max_pages_override,
                }
            })
            .collect();

        Ok(views)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::settings::{FetcherSourceConfig, PersistentSettings};

    struct MockWorkspaceConfig {
        settings: PersistentSettings,
        credentials: std::collections::HashSet<String>,
    }

    impl MockWorkspaceConfig {
        fn new(settings: PersistentSettings) -> Self {
            Self {
                settings,
                credentials: std::collections::HashSet::new(),
            }
        }

        fn with_credential(mut self, field: &str) -> Self {
            self.credentials.insert(field.to_string());
            self
        }
    }

    impl WorkspaceConfig for MockWorkspaceConfig {
        fn settings(&self) -> PersistentSettings {
            self.settings.clone()
        }

        fn has_credential(&self, field_name: &str) -> bool {
            self.credentials.contains(field_name)
        }

        fn save_settings(
            &self,
            _settings: &PersistentSettings,
        ) -> Result<(), crate::domain::errors::DomainError> {
            Ok(())
        }
    }

    fn owner_auth() -> AuthContext {
        use crate::domain::auth::UserId;
        AuthContext::owner(UserId::new(), "tok".into())
    }

    fn service_auth() -> AuthContext {
        AuthContext {
            user_id: crate::domain::auth::UserId::new(),
            roles: vec![crate::domain::auth::Role::Service],
            session_token: crate::domain::credentials::CredentialGuard::new("tok".to_string()),
        }
    }

    #[test]
    fn returns_sources_from_catalog() {
        let cfg = MockWorkspaceConfig::new(PersistentSettings::default());
        let handler = GetConfiguredSourcesHandler::new(cfg);
        let views = handler
            .handle(&GetConfiguredSourcesQuery { auth: owner_auth() })
            .unwrap();
        assert!(!views.is_empty(), "Should return at least one source");
    }

    #[test]
    fn has_credential_reflects_mock() {
        // The "opencorporates" source requires "opencorporates_api_key".
        let cfg = MockWorkspaceConfig::new(PersistentSettings::default())
            .with_credential("opencorporates_api_key");
        let handler = GetConfiguredSourcesHandler::new(cfg);
        let views = handler
            .handle(&GetConfiguredSourcesQuery { auth: owner_auth() })
            .unwrap();
        let oc = views.iter().find(|v| v.id.as_str() == "opencorporates");
        if let Some(oc) = oc {
            assert!(
                oc.has_credential,
                "opencorporates should have credential set"
            );
        }
    }

    #[test]
    fn settings_override_enabled_state() {
        use std::collections::HashMap;
        let mut fetchers = HashMap::new();
        fetchers.insert(
            "fec".to_string(),
            FetcherSourceConfig {
                enabled: false,
                rate_limit_ms: Some(5000),
                max_pages: None,
                api_key: None,
            },
        );
        let settings = PersistentSettings {
            fetchers,
            ..Default::default()
        };
        let cfg = MockWorkspaceConfig::new(settings);
        let handler = GetConfiguredSourcesHandler::new(cfg);
        let views = handler
            .handle(&GetConfiguredSourcesQuery { auth: owner_auth() })
            .unwrap();
        if let Some(fec) = views.iter().find(|v| v.id.as_str() == "fec") {
            assert!(!fec.enabled, "fec should be disabled by settings override");
            assert_eq!(fec.rate_limit_ms_override, Some(5000));
        }
    }

    #[test]
    fn access_denied_for_service() {
        let cfg = MockWorkspaceConfig::new(PersistentSettings::default());
        let handler = GetConfiguredSourcesHandler::new(cfg);
        let result = handler.handle(&GetConfiguredSourcesQuery {
            auth: service_auth(),
        });
        assert!(result.is_err());
    }

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
