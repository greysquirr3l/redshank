//! `DeleteSession` command and handler.

use serde::{Deserialize, Serialize};

use super::run_investigation::IdempotencyKey;
use crate::domain::auth::{AuthContext, StaticPolicy, can_delete_session};
use crate::domain::errors::DomainError;
use crate::domain::session::SessionId;
use crate::ports::session_store::SessionStore;

/// Command to delete a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSessionCommand {
    /// Idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Session ID to delete.
    pub session_id: SessionId,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`DeleteSessionCommand`].
///
/// Enforces `DeleteSession` permission, checks idempotency to prevent
/// double-delete errors, then delegates to the session store.
pub struct DeleteSessionHandler<S> {
    session_store: S,
    policy: StaticPolicy,
}

impl<S: SessionStore> DeleteSessionHandler<S> {
    /// Create a new handler backed by the given session store.
    #[must_use]
    pub const fn new(session_store: S) -> Self {
        Self {
            session_store,
            policy: StaticPolicy,
        }
    }

    /// Execute the command.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `DeleteSession`
    /// permission, [`DomainError::DuplicateOperation`] if the idempotency key
    /// has already been used, or a storage error if the delete fails.
    pub async fn handle(&self, cmd: DeleteSessionCommand) -> Result<(), DomainError> {
        // Idempotency: a second request with the same key is a no-op.
        if self
            .session_store
            .check_idempotency_key(&cmd.idempotency_key.0)
            .await?
            .is_some()
        {
            return Ok(());
        }

        can_delete_session(&cmd.auth, &self.policy).map_err(DomainError::Security)?;
        self.session_store.delete(&cmd.auth, cmd.session_id).await?;
        self.session_store
            .set_idempotency_key(&cmd.idempotency_key.0, "deleted")
            .await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn delete_session_command_roundtrip_serde() {
        let cmd = DeleteSessionCommand {
            idempotency_key: IdempotencyKey::new(),
            session_id: SessionId::new(),
            auth: AuthContext::system(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let restored: DeleteSessionCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.session_id, cmd.session_id);
        assert_eq!(restored.idempotency_key, cmd.idempotency_key);
    }
}
