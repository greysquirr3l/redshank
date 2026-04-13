//! `UpdateSourceConfiguration` command and handler.
//!
//! Updates the enabled/disabled state and optional overrides (rate limit, max pages)
//! for a single data source.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::run_investigation::IdempotencyKey;
use crate::domain::agent::SourceId;
use crate::domain::auth::{AuthContext, StaticPolicy, can_configure_sources};
use crate::domain::errors::DomainError;

/// Command to update source configuration (enabled state, rate limits, page limits).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSourceConfigurationCommand {
    /// Idempotency key to prevent duplicate updates.
    pub idempotency_key: IdempotencyKey,
    /// Source ID to update.
    pub source_id: SourceId,
    /// Whether to enable or disable this source.
    pub enabled: bool,
    /// Optional rate limit override in milliseconds.
    pub rate_limit_ms_override: Option<u64>,
    /// Optional max pages override.
    pub max_pages_override: Option<u32>,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`UpdateSourceConfigurationCommand`].
///
/// Enforces `ConfigureSources` permission, then updates the source's
/// enabled state and optional overrides in the persistent settings.
pub struct UpdateSourceConfigurationHandler {
    policy: StaticPolicy,
}

impl UpdateSourceConfigurationHandler {
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
    /// Returns [`DomainError::Security`] if the caller lacks `ConfigureSources`
    /// permission, or a storage error if the settings file cannot be written.
    pub fn handle(
        &self,
        cmd: &UpdateSourceConfigurationCommand,
        workspace: &Path,
    ) -> Result<(), DomainError> {
        can_configure_sources(&cmd.auth, &self.policy).map_err(DomainError::Security)?;

        let store = crate::adapters::persistence::settings_store::SettingsStore::new(workspace);
        let mut settings = store.load();
        let fetcher_cfg = settings
            .fetchers
            .entry(cmd.source_id.as_str().to_string())
            .or_default();
        fetcher_cfg.enabled = cmd.enabled;
        fetcher_cfg.rate_limit_ms = cmd.rate_limit_ms_override;
        fetcher_cfg.max_pages = cmd.max_pages_override;
        store.save(&settings)
    }
}

impl Default for UpdateSourceConfigurationHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::agent::SourceId;
    use crate::domain::auth::UserId;

    fn owner_auth() -> AuthContext {
        AuthContext::owner(UserId::new(), "tok".into())
    }

    #[test]
    fn update_source_configuration_command_serde() {
        let cmd = UpdateSourceConfigurationCommand {
            idempotency_key: IdempotencyKey::new(),
            source_id: SourceId::new("fec"),
            enabled: false,
            rate_limit_ms_override: Some(10000),
            max_pages_override: Some(5),
            auth: owner_auth(),
        };

        let json = serde_json::to_string(&cmd).unwrap();
        let restored: UpdateSourceConfigurationCommand = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.source_id, cmd.source_id);
        assert_eq!(restored.enabled, false);
        assert_eq!(restored.rate_limit_ms_override, Some(10000));
    }

    #[test]
    fn update_source_configuration_preserves_intent() {
        let cmd = UpdateSourceConfigurationCommand {
            idempotency_key: IdempotencyKey::new(),
            source_id: SourceId::new("opencorporates"),
            enabled: true,
            rate_limit_ms_override: None,
            max_pages_override: None,
            auth: owner_auth(),
        };

        assert_eq!(cmd.source_id, SourceId::new("opencorporates"));
        assert!(cmd.enabled, "Source should be enabled");
    }
}
