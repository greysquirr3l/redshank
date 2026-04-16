//! File-based settings store: `.redshank/settings.json`.

use crate::domain::errors::DomainError;
use crate::domain::settings::PersistentSettings;
use std::path::{Path, PathBuf};

/// File-backed persistent settings store.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    /// Path to the `settings.json` file.
    settings_path: PathBuf,
}

impl SettingsStore {
    /// Create a settings store for the given workspace directory.
    ///
    /// Settings file: `<workspace>/.redshank/settings.json`.
    #[must_use]
    pub fn new(workspace: &Path) -> Self {
        let settings_path = workspace.join(".redshank").join("settings.json");
        Self { settings_path }
    }

    /// Path to the settings file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.settings_path
    }

    /// Load settings from disk. Returns default settings if the file is missing or invalid.
    #[must_use]
    pub fn load(&self) -> PersistentSettings {
        let Ok(contents) = std::fs::read_to_string(&self.settings_path) else {
            return PersistentSettings::default();
        };
        match serde_json::from_str(&contents) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(
                    "failed to parse settings from {}: {e}; using defaults",
                    self.settings_path.display()
                );
                PersistentSettings::default()
            }
        }
    }

    /// Save settings to disk as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the directory cannot be created or the file cannot be written.
    pub fn save(&self, settings: &PersistentSettings) -> Result<(), DomainError> {
        if let Some(parent) = self.settings_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DomainError::Other(format!(
                    "failed to create settings directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| DomainError::Other(format!("failed to serialize settings: {e}")))?;

        std::fs::write(&self.settings_path, &json).map_err(|e| {
            DomainError::Other(format!(
                "failed to write settings to {}: {e}",
                self.settings_path.display()
            ))
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::domain::agent::ReasoningEffort;

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        let settings = store.load();
        assert_eq!(settings, PersistentSettings::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());

        let settings = PersistentSettings {
            default_model: Some("gpt-4o".into()),
            default_reasoning_effort: Some(ReasoningEffort::High),
            default_model_anthropic: Some("claude-sonnet-4-20250514".into()),
            default_model_ollama: Some("ollama/llama3".into()),
            ..Default::default()
        };

        store.save(&settings).unwrap();
        let loaded = store.load();
        assert_eq!(loaded, settings);
    }

    #[test]
    fn unknown_keys_in_file_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".redshank");
        std::fs::create_dir_all(&path).unwrap();
        let file = path.join("settings.json");
        std::fs::write(
            &file,
            r#"{"default_model":"gpt-4o","totally_unknown":"value","another":42}"#,
        )
        .unwrap();

        let store = SettingsStore::new(dir.path());
        let settings = store.load();
        assert_eq!(settings.default_model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn save_creates_directory_if_needed() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        // .redshank/ doesn't exist yet
        assert!(!store.path().exists());

        store.save(&PersistentSettings::default()).unwrap();
        assert!(store.path().exists());
    }
}
