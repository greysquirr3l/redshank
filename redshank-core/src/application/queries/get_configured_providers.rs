//! `GetConfiguredProviders` query and handler.
//!
//! Retrieves the effective configuration of all model providers,
//! with resolved endpoint URLs, default models, and credential status.

use serde::{Deserialize, Serialize};

use crate::domain::agent::ProviderKind;
use crate::domain::auth::{AuthContext, StaticPolicy, can_read_configuration};
use crate::domain::errors::DomainError;
use crate::domain::settings::{ProviderDeploymentKind, ProviderProtocolKind};
use crate::ports::session_store::SessionStore;

/// UI-facing view of a configured model provider with endpoint routing and credential status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfiguredProviderView {
    /// Provider kind (e.g., Anthropic, OpenAI).
    pub provider_kind: ProviderKind,
    /// Display name for the provider.
    pub display_name: String,
    /// Whether this provider is currently enabled.
    pub enabled: bool,
    /// Protocol family for the endpoint.
    pub protocol: ProviderProtocolKind,
    /// Whether the endpoint is local or hosted.
    pub deployment: ProviderDeploymentKind,
    /// Default model to use for this provider, resolved from settings or provider defaults.
    pub default_model: String,
    /// Base URL override for the provider (if local or custom endpoint).
    pub base_url: Option<String>,
    /// Whether a credential is currently configured for this provider.
    pub has_credential: bool,
    /// Credential field name, if explicitly set in settings.
    pub credential_field_name: Option<String>,
}

/// Query to retrieve all configured providers with effective settings.
///
/// Joins provider metadata, endpoint configuration, and credential presence
/// information to produce UI-ready view models without exposing secret values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfiguredProvidersQuery {
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`GetConfiguredProvidersQuery`].
///
/// Enforces `ReadConfiguration` permission, then joins:
/// - Provider kind defaults (protocol, deployment, credential field)
/// - User settings (enabled state, endpoint config, default models)
/// - Credential bundle (credential presence, not values)
///
/// Returns a list of UI-ready provider views.
#[allow(dead_code)]
pub struct GetConfiguredProvidersHandler<S> {
    session_store: S,
    policy: StaticPolicy,
}

impl<S: SessionStore> GetConfiguredProvidersHandler<S> {
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
        query: GetConfiguredProvidersQuery,
    ) -> Result<Vec<ConfiguredProviderView>, DomainError> {
        can_read_configuration(&query.auth, &self.policy).map_err(DomainError::Security)?;

        // TODO(T44): Load settings and credentials from session store / workspace.
        // For now, return a placeholder to make test fail.
        Ok(Vec::new())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn get_configured_providers_query_serde() {
        let query = GetConfiguredProvidersQuery {
            auth: AuthContext::system(),
        };
        let json = serde_json::to_string(&query).unwrap();
        let restored: GetConfiguredProvidersQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.auth, query.auth);
    }

    #[test]
    fn configured_provider_view_shows_resolved_settings() {
        let view = ConfiguredProviderView {
            provider_kind: ProviderKind::Anthropic,
            display_name: "Anthropic Claude".to_string(),
            enabled: true,
            protocol: ProviderProtocolKind::Native,
            deployment: ProviderDeploymentKind::Hosted,
            default_model: "claude-3-5-sonnet-20241022".to_string(),
            base_url: None,
            has_credential: true,
            credential_field_name: None,
        };

        assert!(view.enabled, "Provider should be enabled");
        assert_eq!(
            view.provider_kind,
            ProviderKind::Anthropic,
            "Provider kind should match"
        );
        assert_eq!(
            view.protocol,
            ProviderProtocolKind::Native,
            "Protocol should be Native"
        );
    }

    #[test]
    fn configured_provider_view_local_endpoint_has_base_url() {
        let view = ConfiguredProviderView {
            provider_kind: ProviderKind::OpenAiCompatible,
            display_name: "Local Ollama".to_string(),
            enabled: true,
            protocol: ProviderProtocolKind::OpenAiCompatible,
            deployment: ProviderDeploymentKind::Local,
            default_model: "llama3.2".to_string(),
            base_url: Some("http://localhost:11434/v1".to_string()),
            has_credential: false,
            credential_field_name: None,
        };

        assert!(
            view.deployment == ProviderDeploymentKind::Local,
            "Deployment should be Local"
        );
        assert_eq!(
            view.base_url,
            Some("http://localhost:11434/v1".to_string()),
            "Local endpoint should have base URL"
        );
        assert!(
            !view.has_credential,
            "Local endpoint should not require credential"
        );
    }

    #[test]
    fn configured_provider_view_credential_status_never_exposes_secret() {
        let view = ConfiguredProviderView {
            provider_kind: ProviderKind::OpenAI,
            display_name: "OpenAI".to_string(),
            enabled: true,
            protocol: ProviderProtocolKind::OpenAiCompatible,
            deployment: ProviderDeploymentKind::Hosted,
            default_model: "gpt-4o".to_string(),
            base_url: None,
            has_credential: true,
            credential_field_name: None,
        };

        // Verify that has_credential is a boolean; never exposes the actual API key.
        assert!(
            view.has_credential,
            "Credential should be marked as present"
        );
    }
}
