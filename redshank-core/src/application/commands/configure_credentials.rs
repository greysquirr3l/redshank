//! `ConfigureCredentials` command and handler.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::run_investigation::IdempotencyKey;
use crate::adapters::persistence::credential_store::FileCredentialStore;
use crate::domain::auth::{AuthContext, StaticPolicy, can_configure_credentials};
use crate::domain::credentials::CredentialBundle;
use crate::domain::errors::DomainError;

/// Command to update credential configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureCredentialsCommand {
    /// Idempotency key.
    ///
    /// Credential writes are inherently idempotent (applying the same bundle
    /// twice leaves the system in the same state), so the handler logs but
    /// does not short-circuit on duplicate keys.
    pub idempotency_key: IdempotencyKey,
    /// Updated credential bundle.
    pub credentials: CredentialBundle,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`ConfigureCredentialsCommand`].
///
/// Enforces `ConfigureCredentials` permission, then persists the bundle to
/// the workspace credential store at `<workspace>/.redshank/credentials.json`
/// with `0o600` permissions.
pub struct ConfigureCredentialsHandler {
    policy: StaticPolicy,
}

impl ConfigureCredentialsHandler {
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
    /// Returns [`DomainError::Security`] if the caller lacks
    /// `ConfigureCredentials` permission, or a storage error if the file
    /// cannot be written.
    pub fn handle(
        &self,
        cmd: &ConfigureCredentialsCommand,
        workspace: &Path,
    ) -> Result<(), DomainError> {
        can_configure_credentials(&cmd.auth, &self.policy).map_err(DomainError::Security)?;
        let store = FileCredentialStore::workspace(workspace);
        store.save(&cmd.credentials)
    }
}

impl Default for ConfigureCredentialsHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::auth::{Role, UserId};
    use crate::domain::credentials::CredentialGuard;

    fn owner_auth() -> AuthContext {
        AuthContext::owner(UserId::new(), "tok".into())
    }

    fn reader_auth() -> AuthContext {
        AuthContext {
            user_id: UserId::new(),
            roles: vec![Role::Reader],
            session_token: CredentialGuard::new("tok".into()),
        }
    }

    #[test]
    fn handler_saves_bundle_as_owner() {
        let dir = tempfile::tempdir().unwrap();
        let handler = ConfigureCredentialsHandler::new();
        let cmd = ConfigureCredentialsCommand {
            idempotency_key: IdempotencyKey::new(),
            credentials: CredentialBundle {
                anthropic_api_key: Some(CredentialGuard::new("sk-test".into())),
                ..Default::default()
            },
            auth: owner_auth(),
        };
        handler.handle(&cmd, dir.path()).unwrap();
        let store = FileCredentialStore::workspace(dir.path());
        let loaded = store.load();
        assert_eq!(
            loaded.anthropic_api_key.as_ref().unwrap().expose(),
            "sk-test"
        );
    }

    #[test]
    fn handler_denies_reader() {
        let dir = tempfile::tempdir().unwrap();
        let handler = ConfigureCredentialsHandler::new();
        let cmd = ConfigureCredentialsCommand {
            idempotency_key: IdempotencyKey::new(),
            credentials: CredentialBundle::default(),
            auth: reader_auth(),
        };
        let result = handler.handle(&cmd, dir.path());
        assert!(result.is_err());
    }
}
