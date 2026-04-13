//! `WorkspaceConfig` port — settings and credential-presence access.

use crate::domain::errors::DomainError;
use crate::domain::settings::PersistentSettings;

/// Port for reading and writing workspace configuration.
///
/// Abstracts over the on-disk settings file and credential store so that
/// application-layer handlers remain I/O-free in tests.
pub trait WorkspaceConfig: Send + Sync {
    /// Load the current persistent settings.
    fn settings(&self) -> PersistentSettings;

    /// Return `true` if the named credential field is set (non-empty).
    ///
    /// Field names match the JSON keys in `credentials.json`
    /// (e.g., `"anthropic_api_key"`).  Returns `false` for unknown fields.
    fn has_credential(&self, field_name: &str) -> bool;

    /// Persist the given settings, replacing the current stored settings.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the backing store cannot be written.
    fn save_settings(&self, settings: &PersistentSettings) -> Result<(), DomainError>;
}
