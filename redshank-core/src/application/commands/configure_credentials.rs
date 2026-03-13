//! `ConfigureCredentials` command.

use serde::{Deserialize, Serialize};

use super::run_investigation::IdempotencyKey;
use crate::domain::auth::AuthContext;
use crate::domain::credentials::CredentialBundle;

/// Command to update credential configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureCredentialsCommand {
    /// Idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Updated credential bundle.
    pub credentials: CredentialBundle,
    /// Caller's auth context.
    pub auth: AuthContext,
}

// TODO(T03): Implement ConfigureCredentialsHandler
