//! `UpdateProviderConfiguration` command and handler.
//!
//! Updates provider configuration: enabled state, default model, endpoint URL,
//! and credential field name.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::run_investigation::IdempotencyKey;
use crate::domain::agent::ProviderKind;
use crate::domain::auth::{AuthContext, StaticPolicy, can_configure_providers};
use crate::domain::errors::DomainError;

/// Command to update provider configuration (enabled state, default model, endpoint URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProviderConfigurationCommand {
    /// Idempotency key to prevent duplicate updates.
    pub idempotency_key: IdempotencyKey,
    /// Provider kind to update.
    pub provider_kind: ProviderKind,
    /// Whether to enable or disable this provider.
    pub enabled: bool,
    /// Override default model for this provider.
    pub default_model: Option<String>,
    /// Override base URL for the provider (for local or custom endpoints).
    pub base_url: Option<String>,
    /// Override credential field name (for named credentials, e.g., `"github_token"`).
    pub credential_field_name: Option<String>,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`UpdateProviderConfigurationCommand`].
///
/// Enforces `ConfigureProviders` permission, then updates the provider's
/// endpoint configuration in the persistent settings.
pub struct UpdateProviderConfigurationHandler {
    policy: StaticPolicy,
}

impl UpdateProviderConfigurationHandler {
    /// Create a new handler with the static security policy.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            policy: StaticPolicy,
        }
    }

    /// Execute the command.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ConfigureProviders`
    /// permission, or a storage error if the settings file cannot be written.
    pub fn handle(
        &self,
        cmd: &UpdateProviderConfigurationCommand,
        workspace: &Path,
    ) -> Result<(), DomainError> {
        can_configure_providers(&cmd.auth, &self.policy).map_err(DomainError::Security)?;

        let store = crate::adapters::persistence::settings_store::SettingsStore::new(workspace);
        let mut settings = store.load();
        let endpoint = settings
            .providers
            .entry(cmd.provider_kind)
            .or_insert_with(|| {
                crate::domain::settings::ProviderEndpointConfig::new(
                    crate::domain::settings::ProviderProtocolKind::Native,
                    crate::domain::settings::ProviderDeploymentKind::Hosted,
                )
            });
        endpoint.enabled = cmd.enabled;
        endpoint.default_model.clone_from(&cmd.default_model);
        endpoint.base_url.clone_from(&cmd.base_url);
        endpoint
            .credential_field_name
            .clone_from(&cmd.credential_field_name);
        store.save(&settings)
    }
}

impl Default for UpdateProviderConfigurationHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::agent::ProviderKind;
    use crate::domain::auth::UserId;

    fn owner_auth() -> AuthContext {
        AuthContext::owner(UserId::new(), "tok".into())
    }

    #[test]
    fn update_provider_configuration_command_serde() {
        let cmd = UpdateProviderConfigurationCommand {
            idempotency_key: IdempotencyKey::new(),
            provider_kind: ProviderKind::Anthropic,
            enabled: true,
            default_model: Some("claude-3-5-sonnet-20241022".to_string()),
            base_url: None,
            credential_field_name: None,
            auth: owner_auth(),
        };

        let json = serde_json::to_string(&cmd).unwrap();
        let restored: UpdateProviderConfigurationCommand = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.provider_kind, ProviderKind::Anthropic);
        assert!(restored.enabled);
        assert_eq!(
            restored.default_model,
            Some("claude-3-5-sonnet-20241022".to_string())
        );
    }

    #[test]
    fn update_provider_configuration_with_local_endpoint() {
        let cmd = UpdateProviderConfigurationCommand {
            idempotency_key: IdempotencyKey::new(),
            provider_kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            default_model: Some("llama3.2".to_string()),
            base_url: Some("http://localhost:11434/v1".to_string()),
            credential_field_name: None,
            auth: owner_auth(),
        };

        assert_eq!(cmd.provider_kind, ProviderKind::OpenAiCompatible);
        assert_eq!(cmd.base_url, Some("http://localhost:11434/v1".to_string()));
    }

    #[test]
    fn update_provider_configuration_with_named_credential() {
        let cmd = UpdateProviderConfigurationCommand {
            idempotency_key: IdempotencyKey::new(),
            provider_kind: ProviderKind::OpenAI,
            enabled: true,
            default_model: Some("gpt-4o".to_string()),
            base_url: None,
            credential_field_name: Some("github_token".to_string()),
            auth: owner_auth(),
        };

        assert_eq!(
            cmd.credential_field_name,
            Some("github_token".to_string()),
            "Named credential should be preserved"
        );
    }
}
