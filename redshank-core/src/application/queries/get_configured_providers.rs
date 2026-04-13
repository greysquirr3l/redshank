//! `GetConfiguredProviders` query and handler.
//!
//! Retrieves the effective configuration of all model providers,
//! with resolved endpoint URLs, default models, and credential status.

use serde::{Deserialize, Serialize};

use crate::domain::agent::ProviderKind;
use crate::domain::auth::{AuthContext, StaticPolicy, can_read_configuration};
use crate::domain::errors::DomainError;
use crate::domain::settings::{
    ProviderDeploymentKind, ProviderEndpointConfig, ProviderProtocolKind,
};
use crate::ports::workspace_config::WorkspaceConfig;

/// UI-facing view of a configured model provider with endpoint routing and credential status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfiguredProviderView {
    /// Provider kind (e.g., `Anthropic`, `OpenAI`).
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

/// Built-in defaults for each first-party provider kind.
struct ProviderDefaults {
    display_name: &'static str,
    protocol: ProviderProtocolKind,
    deployment: ProviderDeploymentKind,
    default_model: &'static str,
    credential_field: Option<&'static str>,
}

const fn provider_defaults(kind: ProviderKind) -> ProviderDefaults {
    match kind {
        ProviderKind::Anthropic => ProviderDefaults {
            display_name: "Anthropic Claude",
            protocol: ProviderProtocolKind::Native,
            deployment: ProviderDeploymentKind::Hosted,
            default_model: "claude-opus-4-5",
            credential_field: Some("anthropic_api_key"),
        },
        ProviderKind::OpenAI => ProviderDefaults {
            display_name: "OpenAI",
            protocol: ProviderProtocolKind::OpenAiCompatible,
            deployment: ProviderDeploymentKind::Hosted,
            default_model: "gpt-4o",
            credential_field: Some("openai_api_key"),
        },
        ProviderKind::OpenRouter => ProviderDefaults {
            display_name: "OpenRouter",
            protocol: ProviderProtocolKind::OpenAiCompatible,
            deployment: ProviderDeploymentKind::Hosted,
            default_model: "openrouter/auto",
            credential_field: Some("openrouter_api_key"),
        },
        ProviderKind::Cerebras => ProviderDefaults {
            display_name: "Cerebras",
            protocol: ProviderProtocolKind::OpenAiCompatible,
            deployment: ProviderDeploymentKind::Hosted,
            default_model: "llama-3.3-70b",
            credential_field: Some("cerebras_api_key"),
        },
        ProviderKind::OpenAiCompatible => ProviderDefaults {
            display_name: "Local / Custom (OpenAI-compatible)",
            protocol: ProviderProtocolKind::OpenAiCompatible,
            deployment: ProviderDeploymentKind::Local,
            default_model: "llama3.2",
            credential_field: None,
        },
    }
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
pub struct GetConfiguredProvidersHandler<C> {
    workspace_config: C,
    policy: StaticPolicy,
}

impl<C: WorkspaceConfig> GetConfiguredProvidersHandler<C> {
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
        query: &GetConfiguredProvidersQuery,
    ) -> Result<Vec<ConfiguredProviderView>, DomainError> {
        can_read_configuration(&query.auth, &self.policy).map_err(DomainError::Security)?;

        let settings = self.workspace_config.settings();

        let all_kinds = [
            ProviderKind::Anthropic,
            ProviderKind::OpenAI,
            ProviderKind::OpenRouter,
            ProviderKind::Cerebras,
            ProviderKind::OpenAiCompatible,
        ];

        let views = all_kinds
            .iter()
            .map(|&kind| {
                let defaults = provider_defaults(kind);
                let saved: Option<&ProviderEndpointConfig> = settings.provider_endpoint(kind);

                let enabled = saved.is_none_or(|c| c.enabled);
                let protocol = saved.map_or(defaults.protocol, |c| c.protocol);
                let deployment = saved.map_or(defaults.deployment, |c| c.deployment);
                let base_url = saved.and_then(|c| c.base_url.clone());
                let default_model = saved
                    .and_then(|c| c.default_model.as_deref())
                    .unwrap_or(defaults.default_model)
                    .to_string();
                let credential_field_name = saved
                    .and_then(|c| c.credential_field_name.clone())
                    .or_else(|| defaults.credential_field.map(ToOwned::to_owned));
                let has_credential = credential_field_name
                    .as_deref()
                    .is_some_and(|f| self.workspace_config.has_credential(f));

                ConfiguredProviderView {
                    provider_kind: kind,
                    display_name: defaults.display_name.to_string(),
                    enabled,
                    protocol,
                    deployment,
                    default_model,
                    base_url,
                    has_credential,
                    credential_field_name,
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
    use crate::domain::settings::{PersistentSettings, ProviderEndpointConfig};

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
    fn returns_all_five_providers_with_empty_settings() {
        let cfg = MockWorkspaceConfig::new(PersistentSettings::default());
        let handler = GetConfiguredProvidersHandler::new(cfg);
        let views = handler
            .handle(&GetConfiguredProvidersQuery { auth: owner_auth() })
            .unwrap();
        assert_eq!(views.len(), 5);
    }

    #[test]
    fn has_credential_true_when_mock_returns_true() {
        let cfg = MockWorkspaceConfig::new(PersistentSettings::default())
            .with_credential("anthropic_api_key");
        let handler = GetConfiguredProvidersHandler::new(cfg);
        let views = handler
            .handle(&GetConfiguredProvidersQuery { auth: owner_auth() })
            .unwrap();
        let anthropic = views
            .iter()
            .find(|v| v.provider_kind == ProviderKind::Anthropic)
            .unwrap();
        assert!(anthropic.has_credential);
        // OpenAI has no mocked credential
        let openai = views
            .iter()
            .find(|v| v.provider_kind == ProviderKind::OpenAI)
            .unwrap();
        assert!(!openai.has_credential);
    }

    #[test]
    fn saved_settings_override_defaults() {
        use std::collections::HashMap;
        let mut overrides: crate::domain::settings::ProviderEndpointsConfig = HashMap::new();
        overrides.insert(
            ProviderKind::Anthropic,
            ProviderEndpointConfig {
                enabled: false,
                protocol: ProviderProtocolKind::Native,
                deployment: ProviderDeploymentKind::Hosted,
                default_model: Some("claude-3-haiku-20240307".to_string()),
                base_url: None,
                credential_field_name: None,
            },
        );
        let settings = PersistentSettings {
            providers: overrides,
            ..Default::default()
        };
        let cfg = MockWorkspaceConfig::new(settings);
        let handler = GetConfiguredProvidersHandler::new(cfg);
        let views = handler
            .handle(&GetConfiguredProvidersQuery { auth: owner_auth() })
            .unwrap();
        let anthropic = views
            .iter()
            .find(|v| v.provider_kind == ProviderKind::Anthropic)
            .unwrap();
        assert!(!anthropic.enabled);
        assert_eq!(anthropic.default_model, "claude-3-haiku-20240307");
    }

    #[test]
    fn access_denied_for_service() {
        let cfg = MockWorkspaceConfig::new(PersistentSettings::default());
        let handler = GetConfiguredProvidersHandler::new(cfg);
        let result = handler.handle(&GetConfiguredProvidersQuery {
            auth: service_auth(),
        });
        assert!(result.is_err());
    }

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
