//! `WorkspaceConfigStore` — combined settings + credential adapter.
//!
//! Implements [`WorkspaceConfig`] by composing [`SettingsStore`] and
//! [`FileCredentialStore`] for on-disk access.

use std::path::Path;

use crate::adapters::persistence::{
    credential_store::FileCredentialStore, settings_store::SettingsStore,
};
use crate::domain::errors::DomainError;
use crate::domain::settings::PersistentSettings;
use crate::ports::workspace_config::WorkspaceConfig;

/// Combined workspace config adapter for production use.
///
/// Reads `<workspace>/.redshank/settings.json` and
/// `<workspace>/.redshank/credentials.json`.
#[derive(Debug, Clone)]
pub struct WorkspaceConfigStore {
    settings: SettingsStore,
    credentials: FileCredentialStore,
}

impl WorkspaceConfigStore {
    /// Create a store rooted at `workspace`.
    #[must_use]
    pub fn new(workspace: &Path) -> Self {
        Self {
            settings: SettingsStore::new(workspace),
            credentials: FileCredentialStore::workspace(workspace),
        }
    }
}

impl WorkspaceConfig for WorkspaceConfigStore {
    fn settings(&self) -> PersistentSettings {
        self.settings.load()
    }

    fn has_credential(&self, field_name: &str) -> bool {
        self.credentials.load().has_field(field_name)
    }

    fn save_settings(&self, settings: &PersistentSettings) -> Result<(), DomainError> {
        self.settings.save(settings)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn load_settings_returns_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorkspaceConfigStore::new(dir.path());
        let settings = store.settings();
        assert_eq!(settings, PersistentSettings::default());
    }

    #[test]
    fn has_credential_returns_false_when_no_cred_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorkspaceConfigStore::new(dir.path());
        assert!(!store.has_credential("anthropic_api_key"));
    }

    #[test]
    fn save_and_reload_settings_roundtrips() {
        use crate::domain::agent::ReasoningEffort;

        let dir = tempfile::tempdir().unwrap();
        let store = WorkspaceConfigStore::new(dir.path());

        let settings = PersistentSettings {
            default_reasoning_effort: Some(ReasoningEffort::High),
            ..Default::default()
        };
        store.save_settings(&settings).unwrap();

        let reloaded = store.settings();
        assert_eq!(
            reloaded.default_reasoning_effort,
            Some(ReasoningEffort::High)
        );
    }
}
